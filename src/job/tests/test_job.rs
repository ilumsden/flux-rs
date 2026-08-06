use crate::SignalCode;
use crate::job::job::{Job, JobEventSeverity};
use crate::job::jobid::JobId;
use crate::job::jobspec::JobspecV1;
use crate::job::submit::{JobSubmitFlags, submit};
use crate::job::urgency::JobUrgency;
use crate::tests::common::with_handle;

// =========================================================================
// JobEventSeverity
// =========================================================================

#[test]
fn severity_constants() {
    assert_eq!(JobEventSeverity::FATAL.as_raw(), 0);
    assert_eq!(JobEventSeverity::MIN_SEVERITY.as_raw(), 7);
}

#[test]
fn severity_new_valid_range() {
    for val in 0..=7 {
        let sev = JobEventSeverity::new(val).unwrap();
        assert_eq!(sev.as_raw(), val);
    }
}

#[test]
fn severity_new_invalid_range_returns_none() {
    assert!(JobEventSeverity::new(8).is_none());
    assert!(JobEventSeverity::new(255).is_none());
}

#[test]
fn severity_deref_and_deref_mut() {
    let mut sev = JobEventSeverity::FATAL;
    assert_eq!(*sev, 0);
    *sev = 5;
    assert_eq!(sev.as_raw(), 5);
}

// =========================================================================
// Job::new & get_id
// =========================================================================

#[test]
fn job_new_and_get_id() {
    with_handle(|h| {
        let id = JobId::from(12345);
        let job = Job::new(h, id).unwrap();
        assert_eq!(job.get_id(), &JobId::from(12345));
    });
}

// =========================================================================
// RPC Methods (Future Creation)
// =========================================================================

#[test]
fn wait_and_result_return_handles() {
    with_handle(|h| {
        let job = Job::new(h, JobId::from(12345)).unwrap();
        assert!(job.wait().is_ok());
        assert!(job.result().is_ok());
    });
}

#[test]
fn raise_cancel_kill_urgency_return_futures() {
    with_handle(|h| {
        let mut job = Job::new(h, JobId::from(12345)).unwrap();
        assert!(job.raise(None, None, None).is_ok());
        assert!(job.cancel(Some("test cancel")).is_ok());
        assert!(job.kill(SignalCode::new(9).unwrap()).is_ok());
        assert!(job.set_urgency(JobUrgency::DEFAULT).is_ok());
    });
}

// =========================================================================
// KVS Operations (Live Submitted Job)
// =========================================================================

#[test]
fn kvs_key_operations_on_submitted_job() {
    with_handle(|h| {
        let js = JobspecV1::per_resource(["true"])
            .ncores(1)
            .build_jobspec()
            .unwrap();
        let job = submit(h, &js, None, Some(JobSubmitFlags::WAITABLE)).unwrap();

        let kvs_key = job.get_kvs_key("").unwrap();
        assert!(!kvs_key.is_empty());

        let guest_key = job.get_kvs_guest_key("").unwrap();
        assert!(!guest_key.is_empty());

        let ns = job.get_kvs_namespace().unwrap();
        assert!(!ns.is_empty());
    });
}
