use std::collections::HashMap;
use std::ffi::CStr;
use std::fmt::Display;
use std::ops::Deref;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::duration::FluxDuration;
use crate::error::Result;
use crate::hostlist::Hostlist;
use crate::job::jobid::JobId;
use crate::job::result::JobResultCode;
use crate::job::state::{JobState, JobStateFormat};
use crate::job::urgency::JobUrgency;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobExceptionInfo {
    occured: bool,
    severity: Option<i32>,
    #[serde(rename = "type")]
    execption_type: Option<String>,
    note: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobAnnotationsInfo {
    #[serde(flatten)]
    pub annotations: HashMap<String, Value>,
}

impl JobAnnotationsInfo {
    pub fn get_sched<'a>(&'a self) -> Option<&'a Map<String, Value>> {
        self.annotations
            .get("sched")
            .map(|v| v.as_object())
            .flatten()
    }

    pub fn get_user<'a>(&'a self) -> Option<&'a Map<String, Value>> {
        self.annotations
            .get("user")
            .map(|v| v.as_object())
            .flatten()
    }
}

impl Deref for JobAnnotationsInfo {
    type Target = HashMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.annotations
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobDependencyList {
    pub dependencies: Vec<String>,
}

impl JobDependencyList {
    pub fn new<I>(items: I) -> Self
    where
        I: IntoIterator,
        I::Item: ToString,
    {
        let today = Local::now().date_naive();
        let result: Vec<String> = items
            .into_iter()
            .map(|i| i.to_string())
            .map(|dep| {
                if let Some(timestamp_str) = dep.strip_prefix("begin-time=") {
                    if let Ok(ts_float) = timestamp_str.parse::<f64>() {
                        let secs = ts_float.trunc() as i64;
                        let nsecs = (ts_float.fract() * 1_000_000_000.0) as u32;
                        if let chrono::LocalResult::Single(dt) = Local.timestamp_opt(secs, nsecs) {
                            let mut formatted_dep = if dt.date_naive() == today {
                                format!("begin@{}", dt.format("%H:%M"))
                            } else {
                                format!("begin@{}", dt.format("%b%d-%H:%M"))
                            };
                            if dt.second() > 0 {
                                formatted_dep.push_str(&format!(":{}", dt.format("%S")));
                            }
                            return formatted_dep;
                        }
                    }
                }
                dep
            })
            .collect();
        Self {
            dependencies: result,
        }
    }
}

impl Deref for JobDependencyList {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.dependencies
    }
}

impl Display for JobDependencyList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.dependencies.join(","))
    }
}

#[derive(Serialize, Deserialize)]
pub struct JobInfo {
    pub id: JobId,
    pub userid: u32,
    pub urgency: JobUrgency,
    pub priority: u64,
    pub t_submit: f64,
    pub t_depend: f64,
    pub t_run: f64,
    pub t_cleanup: f64,
    pub t_inactive: f64,
    pub state: JobState,
    pub name: String,
    pub cwd: PathBuf,
    pub queue: String,
    pub project: String,
    pub bank: String,
    pub ntasks: u64,
    pub ncores: u64,
    pub ranks: u64,
    pub nodelist: Hostlist,
    pub duration: f64,
    pub expiration: f64,
    pub success: bool,
    pub result: JobResultCode,
    pub waitstatus: Option<i32>,
    pub exception: JobExceptionInfo,
    pub annotations: JobAnnotationsInfo,
    pub dependencies: JobDependencyList,
}

impl JobInfo {
    pub fn get_instance_info(&self) {
        unimplemented!()
    }

    pub fn get_runtime(&self) -> f64 {
        unimplemented!()
    }

    pub fn get_remaining_time(&self) -> f64 {
        unimplemented!()
    }

    pub fn get_uri(&self) {
        unimplemented!()
    }

    pub fn get_state(&self) -> Result<String> {
        self.state.encode(JobStateFormat::UpperCaseLong)
    }

    pub fn get_state_single(&self) -> Result<String> {
        self.state.encode(JobStateFormat::UpperCaseShort)
    }

    pub fn get_state_emoji(&self) -> Result<String> {
        self.state.encode(JobStateFormat::Emoji)
    }

    pub fn get_result(&self) -> Result<String> {
        self.result.encode(JobStateFormat::UpperCaseLong)
    }

    pub fn get_result_abbrev(&self) -> Result<String> {
        self.result.encode(JobStateFormat::UpperCaseShort)
    }

    pub fn get_result_emoji(&self) -> Result<String> {
        self.result.encode(JobStateFormat::Emoji)
    }

    pub fn get_username(&self) -> String {
        let pw_ptr = unsafe { libc::getpwuid(self.userid) };
        if pw_ptr.is_null() {
            return self.userid.to_string();
        }
        unsafe {
            let c_str = CStr::from_ptr((*pw_ptr).pw_name);
            c_str.to_string_lossy().into_owned()
        }
    }

    fn encode_status(&self, fmt: JobStateFormat) -> Result<String> {
        if self.state.intersects(JobState::PENDING | JobState::RUNNING) {
            self.state.encode(fmt)
        } else {
            self.result.encode(fmt)
        }
    }

    pub fn get_status(&self) -> Result<String> {
        self.encode_status(JobStateFormat::UpperCaseLong)
    }

    pub fn get_status_abbrev(&self) -> Result<String> {
        self.encode_status(JobStateFormat::UpperCaseShort)
    }

    pub fn get_status_emoji(&self) -> Result<String> {
        self.encode_status(JobStateFormat::Emoji)
    }

    pub fn get_return_code(&self) -> Option<i32> {
        match self.waitstatus {
            None => {
                if self.result.contains(JobResultCode::CANCELED) {
                    Some(-128)
                } else if self.result.contains(JobResultCode::FAILED) {
                    Some(1)
                } else {
                    None
                }
            }
            Some(status) => {
                if libc::WIFSIGNALED(status) {
                    Some(-(libc::WTERMSIG(status)))
                } else if libc::WIFEXITED(status) {
                    Some(libc::WEXITSTATUS(status))
                } else {
                    None
                }
            }
        }
    }

    pub fn get_contextual_info(&self) -> String {
        if self.state.contains(JobState::PRIORITY) {
            "priority-wait".to_string()
        } else if self.state.contains(JobState::DEPEND) {
            format!("depends:{}", self.dependencies)
        } else if self.state.contains(JobState::SCHED) {
            let mut ctx_info_str = if self.urgency == JobUrgency::HOLD {
                "held".to_string()
            } else if self.priority == 0 {
                "priority-hold".to_string()
            } else {
                String::new()
            };
            if let Some(sched_map) = self.annotations.get_sched() {
                if let Some(t_estimate) = sched_map.get("t_estimate") {
                    if let Some(t_estimate_float) = t_estimate.as_f64() {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64();
                        let eta = t_estimate_float - now;
                        let eta = if eta < 0.0 {
                            "now".to_string()
                        } else {
                            FluxDuration::Secs(eta).to_string()
                        };
                        ctx_info_str = format!("eta:{}", eta);
                    }
                }
            }
            ctx_info_str
        } else {
            self.nodelist.to_string()
        }
    }

    pub fn get_contextual_time(&self) -> f64 {
        if self
            .state
            .intersects(JobState::PRIORITY | JobState::DEPEND | JobState::SCHED)
        {
            self.duration
        } else {
            self.get_runtime()
        }
    }

    pub fn get_inactive_reason(&self) -> String {
        if !self.state.contains(JobState::INACTIVE) {
            String::new()
        } else if self.result.contains(JobResultCode::CANCELED) {
            let mut canceled_str = "Canceled".to_string();
            if self.exception.occured {
                if let Some(exception_type) = self.exception.execption_type.as_ref() {
                    if exception_type == "cancel" {
                        if let Some(exception_note) = self.exception.note.as_ref() {
                            canceled_str = format!("Canceled: {}", exception_note);
                        }
                    }
                }
            }
            canceled_str
        } else if self.result.contains(JobResultCode::FAILED) {
            let mut failed_str = format!(
                "Exit {}",
                match self.get_return_code() {
                    Some(c) => c.to_string(),
                    None => "UNKNOWN".to_string(),
                }
            );
            if self.exception.occured {
                if let Some(exception_type) = self.exception.execption_type.as_ref() {
                    if exception_type != "exec" {
                        if let Some(severity) = self.exception.severity {
                            if severity == 0 {
                                let note_str = self
                                    .exception
                                    .note
                                    .as_ref()
                                    .map(|n| format!(" note={}", n))
                                    .unwrap_or_default();
                                failed_str =
                                    format!("Exception: type={}{}", exception_type, note_str);
                            }
                        }
                    }
                }
            } else if let Some(returncode) = self.get_return_code() {
                if returncode > 128 {
                    let signum = returncode - 128;
                    let c_sig_ptr = unsafe { libc::strsignal(signum) };
                    if c_sig_ptr.is_null() {
                        failed_str = format!("Signaled {}", signum);
                    } else {
                        failed_str =
                            unsafe { CStr::from_ptr(c_sig_ptr).to_string_lossy().to_string() };
                    }
                } else if returncode == 126 {
                    failed_str = "Command invoked cannot execute".to_string();
                } else if returncode == 127 {
                    failed_str = "Command not found".to_string();
                } else if returncode == 128 {
                    failed_str = "Invalid argument to exit".to_string();
                }
            }
            failed_str
        } else if self.result.contains(JobResultCode::TIMEOUT) {
            "Timeout".to_string()
        } else {
            format!(
                "Exit {}",
                match self.get_return_code() {
                    Some(c) => c.to_string(),
                    None => "UNKNOWN".to_string(),
                }
            )
        }
    }
}
