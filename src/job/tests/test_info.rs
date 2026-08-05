use std::collections::HashMap;

use serde_json::json;

use crate::hostlist::Hostlist;
use crate::job::info::{JobAnnotationsInfo, JobDependencyList, JobExceptionInfo, JobInfo};
use crate::job::result::JobResultCode;
use crate::job::state::JobState;
use crate::job::urgency::JobUrgency;

// =========================================================================
// JobExceptionInfo
// =========================================================================

#[test]
fn exception_info_default_values() {
    let exc = JobExceptionInfo::default();
    assert!(!exc.occured);
    assert!(exc.severity.is_none());
    assert!(exc.execption_type.is_none());
    assert!(exc.note.is_none());
}

#[test]
fn exception_info_serde_round_trip() {
    let exc = JobExceptionInfo {
        occured: true,
        severity: Some(2),
        execption_type: Some("cancel".to_string()),
        note: Some("user requested cancel".to_string()),
    };
    let json = serde_json::to_string(&exc).unwrap();
    let decoded: JobExceptionInfo = serde_json::from_str(&json).unwrap();
    assert!(decoded.occured);
    assert_eq!(decoded.severity, Some(2));
    assert_eq!(decoded.execption_type.as_deref(), Some("cancel"));
    assert_eq!(decoded.note.as_deref(), Some("user requested cancel"));
}

// =========================================================================
// JobAnnotationsInfo
// =========================================================================

#[test]
fn annotations_info_get_sched_and_user() {
    let mut map = HashMap::new();
    map.insert("sched".to_string(), json!({"t_estimate": 1000.0}));
    map.insert("user".to_string(), json!({"uri": "local:///tmp/flux"}));
    let ann = JobAnnotationsInfo { annotations: map };

    let sched = ann.get_sched().unwrap();
    assert_eq!(sched.get("t_estimate").unwrap(), &json!(1000.0));

    let user = ann.get_user().unwrap();
    assert_eq!(user.get("uri").unwrap(), &json!("local:///tmp/flux"));
}

#[test]
fn annotations_info_deref_exposes_hashmap() {
    let mut map = HashMap::new();
    map.insert("custom".to_string(), json!("val"));
    let ann = JobAnnotationsInfo { annotations: map };
    assert_eq!(ann.get("custom").unwrap(), &json!("val"));
}

// =========================================================================
// JobDependencyList
// =========================================================================

#[test]
fn dependency_list_new_preserves_regular_dependencies() {
    let deps = JobDependencyList::new(vec!["afterok:123", "afterany:456"]);
    assert_eq!(deps.dependencies, vec!["afterok:123", "afterany:456"]);
}

#[test]
fn dependency_list_display_joins_with_commas() {
    let deps = JobDependencyList::new(vec!["dep1", "dep2"]);
    assert_eq!(deps.to_string(), "dep1,dep2");
}

#[test]
fn dependency_list_deref_exposes_vec() {
    let deps = JobDependencyList::new(vec!["a", "b"]);
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0], "a");
}

// =========================================================================
// JobInfo::get_runtime
// =========================================================================

#[test]
fn get_runtime_completed_job() {
    let mut info = JobInfo::default();
    info.t_run = 100.0;
    info.t_cleanup = 150.0;
    assert_eq!(info.get_runtime(), 50.0);
}

#[test]
fn get_runtime_not_started_returns_zero() {
    let info = JobInfo::default();
    assert_eq!(info.get_runtime(), 0.0);
}

#[test]
fn get_runtime_currently_running_returns_positive() {
    let mut info = JobInfo::default();
    info.t_run = 1.0; // Ran near Unix epoch start
    assert!(info.get_runtime() > 0.0);
}

// =========================================================================
// JobInfo::get_remaining_time
// =========================================================================

#[test]
fn get_remaining_time_non_running_returns_zero() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::INACTIVE);
    assert_eq!(info.get_remaining_time().unwrap(), 0.0);
}

#[test]
fn get_remaining_time_expired_running_returns_zero() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::RUN);
    info.expiration = 1.0; // Expired near epoch start
    assert_eq!(info.get_remaining_time().unwrap(), 0.0);
}

// =========================================================================
// JobInfo::get_uri
// =========================================================================

#[test]
fn get_uri_from_annotations() {
    let mut info = JobInfo::default();
    info.annotations.annotations.insert(
        "user".to_string(),
        json!({"uri": "local:///run/flux/local"}),
    );
    let uri = info.get_uri().unwrap();
    assert_eq!(uri.base.scheme, "local");
}

#[test]
fn get_uri_missing_annotation_returns_none() {
    let info = JobInfo::default();
    assert!(info.get_uri().is_none());
}

// =========================================================================
// JobInfo::get_username
// =========================================================================

#[test]
fn get_username_none_userid_returns_none() {
    let info = JobInfo::default();
    assert!(info.get_username().is_none());
}

#[test]
fn get_username_valid_userid_returns_non_empty_string() {
    let mut info = JobInfo::default();
    let uid = unsafe { libc::getuid() };
    info.userid = Some(uid);
    let username = info.get_username().unwrap();
    assert!(!username.is_empty());
}

// =========================================================================
// JobInfo::get_return_code
// =========================================================================

#[test]
fn get_return_code_from_exited_waitstatus() {
    let mut info = JobInfo::default();
    // Construct a waitstatus for normal exit with code 42
    // In POSIX: (exit_code << 8) & 0xff00
    let waitstatus = (42 << 8) & 0xff00;
    info.waitstatus = Some(waitstatus);
    assert_eq!(info.get_return_code(), Some(42));
}

#[test]
fn get_return_code_from_signaled_waitstatus() {
    let mut info = JobInfo::default();
    // Construct a waitstatus for signal termination with sig 9 (SIGKILL)
    let waitstatus = 9 & 0x7f;
    info.waitstatus = Some(waitstatus);
    assert_eq!(info.get_return_code(), Some(-9));
}

#[test]
fn get_return_code_from_canceled_result() {
    let mut info = JobInfo::default();
    info.result = Some(JobResultCode::CANCELED);
    assert_eq!(info.get_return_code(), Some(-128));
}

#[test]
fn get_return_code_from_failed_result() {
    let mut info = JobInfo::default();
    info.result = Some(JobResultCode::FAILED);
    assert_eq!(info.get_return_code(), Some(1));
}

// =========================================================================
// JobInfo::get_contextual_info
// =========================================================================

#[test]
fn get_contextual_info_priority_wait() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::PRIORITY);
    assert_eq!(info.get_contextual_info(), "priority-wait");
}

#[test]
fn get_contextual_info_depends() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::DEPEND);
    info.dependencies = JobDependencyList::new(vec!["afterok:123"]);
    assert_eq!(info.get_contextual_info(), "depends:afterok:123");
}

#[test]
fn get_contextual_info_sched_hold() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::SCHED);
    info.urgency = Some(JobUrgency::HOLD);
    assert_eq!(info.get_contextual_info(), "held");
}

#[test]
fn get_contextual_info_nodelist_fallback() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::RUN);
    info.nodelist = Hostlist::try_from_iter(["node0", "node1"]).ok();
    assert_eq!(
        info.get_contextual_info(),
        info.nodelist.as_ref().unwrap().to_string()
    );
}

// =========================================================================
// JobInfo::get_inactive_reason
// =========================================================================

#[test]
fn get_inactive_reason_not_inactive_returns_empty() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::RUN);
    assert_eq!(info.get_inactive_reason(), "");
}

#[test]
fn get_inactive_reason_canceled_with_note() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::INACTIVE);
    info.result = Some(JobResultCode::CANCELED);
    info.exception.occured = true;
    info.exception.execption_type = Some("cancel".to_string());
    info.exception.note = Some("user canceled".to_string());
    assert_eq!(info.get_inactive_reason(), "Canceled: user canceled");
}

#[test]
fn get_inactive_reason_timeout() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::INACTIVE);
    info.result = Some(JobResultCode::TIMEOUT);
    assert_eq!(info.get_inactive_reason(), "Timeout");
}

#[test]
fn get_inactive_reason_command_not_found() {
    let mut info = JobInfo::default();
    info.state = Some(JobState::INACTIVE);
    info.result = Some(JobResultCode::FAILED);
    // Exit code 127 -> (127 << 8) & 0xff00
    info.waitstatus = Some((127 << 8) & 0xff00);
    assert_eq!(info.get_inactive_reason(), "Command not found");
}
