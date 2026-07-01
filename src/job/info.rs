use std::cell::RefCell;
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
use crate::uri::JobUri;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobExceptionInfo {
    #[serde(default)]
    pub occured: bool,
    #[serde(default)]
    pub severity: Option<i32>,
    #[serde(rename = "type", default)]
    pub execption_type: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

impl Default for JobExceptionInfo {
    fn default() -> Self {
        Self {
            occured: false,
            severity: None,
            execption_type: None,
            note: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct JobAnnotationsInfo {
    #[serde(flatten, default)]
    pub annotations: HashMap<String, Value>,
}

impl JobAnnotationsInfo {
    pub fn get_sched(&self) -> Option<&Map<String, Value>> {
        self.annotations
            .get("sched")
            .map(|v| v.as_object())
            .flatten()
    }

    pub fn get_user(&self) -> Option<&Map<String, Value>> {
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

#[derive(Serialize, Deserialize, Default)]
#[serde(transparent, default)]
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
    #[serde(default)]
    pub t_depend: f64,
    #[serde(default)]
    pub t_run: f64,
    #[serde(default)]
    pub t_cleanup: f64,
    #[serde(default)]
    pub t_inactive: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub expiration: f64,
    #[serde(default)]
    pub t_submit: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urgency: Option<JobUrgency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<JobState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ntasks: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ncores: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranks: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodelist: Option<Hostlist>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JobResultCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waitstatus: Option<i32>,
    pub exception: JobExceptionInfo,
    pub annotations: JobAnnotationsInfo,
    pub dependencies: JobDependencyList,
    #[serde(skip_serializing)]
    cached_uri: RefCell<Option<JobUri>>,
}

impl JobInfo {
    pub fn get_instance_info(&self) {
        unimplemented!("Not yet implemented (awaiting implementation of resource_list)")
    }

    pub fn get_runtime(&self) -> f64 {
        if self.t_cleanup > 0.0 && self.t_run > 0.0 {
            self.t_cleanup - self.t_run
        } else if self.t_run > 0.0 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64()
                - self.t_run
        } else {
            0.0
        }
    }

    pub fn get_remaining_time(&self) -> Result<f64> {
        let status = self.get_status()?;
        if status.is_none() || status.unwrap() != "RUN" {
            return Ok(0.0);
        }
        let tleft = self.expiration
            - SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
        if tleft < 0.0 {
            Ok(0.0)
        } else {
            Ok(tleft)
        }
    }

    pub fn get_uri(&self) -> Option<JobUri> {
        let mut cache = self.cached_uri.borrow_mut();
        if cache.is_none() {
            let resolved = {
                let mut resolved_ret = None;
                if let Some(user_value) = self.annotations.get("user") {
                    if let Some(user_map) = user_value.as_object() {
                        if let Some(uri_value) = user_map.get("uri") {
                            if let Some(uri_str) = uri_value.as_str() {
                                resolved_ret = JobUri::new(uri_str, None).ok();
                            }
                        }
                    }
                }
                resolved_ret
            }?;
            *cache = Some(resolved);
        }
        cache.as_ref().cloned()
    }

    pub fn get_state(&self) -> Result<Option<String>> {
        self.state
            .map(|s| s.encode(JobStateFormat::UpperCaseLong))
            .transpose()
    }

    pub fn get_state_single(&self) -> Result<Option<String>> {
        self.state
            .map(|s| s.encode(JobStateFormat::UpperCaseShort))
            .transpose()
    }

    pub fn get_state_emoji(&self) -> Result<Option<String>> {
        self.state
            .map(|s| s.encode(JobStateFormat::Emoji))
            .transpose()
    }

    pub fn get_result(&self) -> Result<Option<String>> {
        self.result
            .map(|s| s.encode(JobStateFormat::UpperCaseLong))
            .transpose()
    }

    pub fn get_result_abbrev(&self) -> Result<Option<String>> {
        self.result
            .map(|s| s.encode(JobStateFormat::UpperCaseShort))
            .transpose()
    }

    pub fn get_result_emoji(&self) -> Result<Option<String>> {
        self.result
            .map(|s| s.encode(JobStateFormat::Emoji))
            .transpose()
    }

    pub fn get_username(&self) -> Option<String> {
        if self.userid.is_none() {
            return None;
        }
        let userid = self.userid.unwrap();
        let pw_ptr = unsafe { libc::getpwuid(userid) };
        if pw_ptr.is_null() {
            return Some(userid.to_string());
        }
        unsafe {
            let c_str = CStr::from_ptr((*pw_ptr).pw_name);
            Some(c_str.to_string_lossy().into_owned())
        }
    }

    fn encode_status(&self, fmt: JobStateFormat) -> Result<Option<String>> {
        let mut encoded = None;
        if let Some(state) = self.state {
            if state.intersects(JobState::PENDING | JobState::RUNNING) {
                encoded = Some(state.encode(fmt)?);
            }
        }
        if encoded.is_none() {
            if let Some(result) = self.result {
                encoded = Some(result.encode(fmt)?);
            }
        }
        Ok(encoded)
    }

    pub fn get_status(&self) -> Result<Option<String>> {
        self.encode_status(JobStateFormat::UpperCaseLong)
    }

    pub fn get_status_abbrev(&self) -> Result<Option<String>> {
        self.encode_status(JobStateFormat::UpperCaseShort)
    }

    pub fn get_status_emoji(&self) -> Result<Option<String>> {
        self.encode_status(JobStateFormat::Emoji)
    }

    pub fn get_return_code(&self) -> Option<i32> {
        match self.waitstatus {
            None => {
                if let Some(result) = self.result {
                    if result.contains(JobResultCode::CANCELED) {
                        Some(-128)
                    } else if result.contains(JobResultCode::FAILED) {
                        Some(1)
                    } else {
                        None
                    }
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
        if let Some(state) = self.state {
            if state.contains(JobState::PRIORITY) {
                "priority-wait".to_string()
            } else if state.contains(JobState::DEPEND) {
                format!("depends:{}", self.dependencies)
            } else if state.contains(JobState::SCHED) {
                let mut ctx_info_str =
                    if self.urgency.is_some() && self.urgency.unwrap() == JobUrgency::HOLD {
                        "held".to_string()
                    } else if self.priority.is_some() && self.priority.unwrap() == 0 {
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
                self.nodelist
                    .as_ref()
                    .map(|hl| hl.to_string())
                    .unwrap_or(String::new())
            }
        } else {
            self.nodelist
                .as_ref()
                .map(|hl| hl.to_string())
                .unwrap_or(String::new())
        }
    }

    pub fn get_contextual_time(&self) -> f64 {
        if self.state.is_some()
            && self
                .state
                .unwrap()
                .intersects(JobState::PRIORITY | JobState::DEPEND | JobState::SCHED)
        {
            self.duration
        } else {
            self.get_runtime()
        }
    }

    pub fn get_inactive_reason(&self) -> String {
        if self.state.is_none() || !self.state.unwrap().contains(JobState::INACTIVE) {
            return String::new();
        }
        if let Some(result) = self.result {
            if result.contains(JobResultCode::CANCELED) {
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
            } else if result.contains(JobResultCode::FAILED) {
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
            } else if result.contains(JobResultCode::TIMEOUT) {
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

impl Default for JobInfo {
    fn default() -> Self {
        Self {
            id: JobId::default(),
            t_depend: 0.0,
            t_run: 0.0,
            t_cleanup: 0.0,
            t_inactive: 0.0,
            duration: 0.0,
            expiration: 0.0,
            t_submit: 0.0,
            userid: None,
            urgency: None,
            priority: None,
            state: None,
            name: None,
            cwd: None,
            queue: None,
            project: None,
            bank: None,
            ntasks: None,
            ncores: None,
            ranks: None,
            nodelist: None,
            success: None,
            result: None,
            waitstatus: None,
            exception: JobExceptionInfo::default(),
            annotations: JobAnnotationsInfo::default(),
            dependencies: JobDependencyList::default(),
            cached_uri: RefCell::new(None),
        }
    }
}
