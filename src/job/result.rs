use std::ffi::{CStr, CString, c_char};
use std::fmt::Display;
use std::ops::{Deref, DerefMut};

use bitflags::bitflags;
use flux_sys::core::{
    flux_job_result_get, flux_job_result_t, flux_job_result_t_FLUX_JOB_RESULT_CANCELED,
    flux_job_result_t_FLUX_JOB_RESULT_COMPLETED, flux_job_result_t_FLUX_JOB_RESULT_FAILED,
    flux_job_result_t_FLUX_JOB_RESULT_TIMEOUT, flux_job_resulttostr, flux_job_strtoresult,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{FluxError, Result, check_ptr, check_rc};
use crate::future::FluxFuture;
use crate::job::{JobId, JobInfo, JobStateFormat};
use crate::utils::impl_async_future_wrapper;

bitflags! {
    #[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct JobResultCode: u32 {
        const NONE = 0;
        const COMPLETED = flux_job_result_t_FLUX_JOB_RESULT_COMPLETED;
        const FAILED = flux_job_result_t_FLUX_JOB_RESULT_FAILED;
        const CANCELED = flux_job_result_t_FLUX_JOB_RESULT_CANCELED;
        const TIMEOUT = flux_job_result_t_FLUX_JOB_RESULT_TIMEOUT;
    }
}

impl JobResultCode {
    pub fn encode(&self, fmt: JobStateFormat) -> Result<String> {
        let c_fmt = fmt.as_c_str();
        // Note: do not free this string since flux_job_resulttostr just returns a string literal
        let encoded_ptr = unsafe { flux_job_resulttostr(self.bits(), c_fmt.as_ptr()) };
        check_ptr(encoded_ptr as *mut i8)?;
        let mut owned_str = unsafe { CStr::from_ptr(encoded_ptr).to_str()?.to_owned() };
        if matches!(fmt, JobStateFormat::Emoji) {
            owned_str = match owned_str.as_str() {
                "CD" => String::from("\u{1F600}"), // grinning face
                "F" => String::from("\u{1F4A9}"),  // pile of poo
                "CA" => String::from("\u{1F4A5}"), // collision
                "TO" => String::from("\u{231B}"),  // hourglass done
                _ => owned_str,
            };
        }
        Ok(owned_str)
    }

    pub fn decode(enocded_result: &str) -> Result<Self> {
        let c_encoded_result = CString::new(enocded_result)?;
        let mut result: flux_job_result_t = 0;
        let rc = unsafe {
            flux_job_strtoresult(
                c_encoded_result.as_ptr(),
                &mut result as *mut flux_job_result_t,
            )
        };
        check_rc(rc)?;
        Ok(Self::from_bits_retain(result))
    }
}

impl From<flux_job_result_t> for JobResultCode {
    fn from(value: flux_job_result_t) -> Self {
        JobResultCode::from_bits_truncate(value)
    }
}

impl Display for JobResultCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let job_result_str = match self.encode(JobStateFormat::UpperCaseLong) {
            Ok(s) => s,
            Err(e) => format!("UNKNOWN (failed to convert C string to Rust string: {e})"),
        };
        write!(f, "{}", job_result_str)
    }
}

pub struct JobResult {
    future: FluxFuture<'static>,
}

impl JobResult {
    pub fn get_info_map(&self) -> Result<Map<String, Value>> {
        let mut json_str: *const c_char = std::ptr::null();
        let rc = unsafe {
            flux_job_result_get(
                self.future.c_future.as_mut_ptr(),
                &mut json_str as *mut *const c_char,
            )
        };
        check_rc(rc)?;
        let rust_json_str = unsafe { CStr::from_ptr(json_str).to_str()? };
        let unpacked_info: Map<String, Value> = serde_json::from_str(rust_json_str)?;
        Ok(unpacked_info)
    }

    pub fn get_info_json(&self) -> Result<Value> {
        Ok(Value::Object(self.get_info_map()?))
    }

    pub fn get_info(&self) -> Result<JobInfo> {
        let unpacked_info = self.get_info_map()?;
        let mut jobinfo = JobInfo::default();
        jobinfo.id = JobId::from(
            unpacked_info
                .get("id")
                .ok_or(FluxError::Logic(
                    "Job id not found in value of flux_job_result_get".to_string(),
                ))?
                .as_u64()
                .ok_or(FluxError::Logic(
                    "Cannot convert job ID from C into a flux_jobid_t".to_string(),
                ))?,
        );
        jobinfo.result = Some(JobResultCode::from(
            unpacked_info
                .get("result")
                .ok_or(FluxError::Logic(
                    "Result code not found in value of flux_job_result_get".to_string(),
                ))?
                .as_u64()
                .ok_or(FluxError::Logic(
                    "Cannot convert job result code from C into a flux_job_result_t".to_string(),
                ))? as flux_job_result_t,
        ));
        jobinfo.t_submit = unpacked_info
            .get("t_submit")
            .ok_or(FluxError::Logic(
                "Submit time not found in value of flux_job_result_get".to_string(),
            ))?
            .as_f64()
            .ok_or(FluxError::Logic(
                "Cannot convert submit time to floating-point value".to_string(),
            ))?;
        jobinfo.t_cleanup = unpacked_info
            .get("t_cleanup")
            .ok_or(FluxError::Logic(
                "Cleanup time not found in value of flux_job_result_get".to_string(),
            ))?
            .as_f64()
            .ok_or(FluxError::Logic(
                "Cannot convert cleanup time to floating-point value".to_string(),
            ))?;
        jobinfo.exception.occured = unpacked_info
            .get("exception_occured")
            .ok_or(FluxError::Logic(
                "The 'exception_occured' key not found in value of flux_job_result_get".to_string(),
            ))?
            .as_bool()
            .ok_or(FluxError::Logic(
                "Cannot convert 'exception_occured' to a boolean".to_string(),
            ))?;
        if let Some(waitstatus) = unpacked_info.get("waitstatus") {
            jobinfo.waitstatus = Some(waitstatus.as_i64().ok_or(FluxError::Logic(
                "Cannot convert waitstatus to integer".to_string(),
            ))? as i32);
        }
        if let Some(severity) = unpacked_info.get("exception_severity")
            && jobinfo.exception.occured
        {
            jobinfo.exception.severity = Some(severity.as_i64().ok_or(FluxError::Logic(
                "Cannot convert exception severity to integer".to_string(),
            ))? as i32);
        }
        if let Some(exception_type) = unpacked_info.get("exception_type")
            && jobinfo.exception.occured
        {
            jobinfo.exception.execption_type = Some(
                exception_type
                    .as_str()
                    .ok_or(FluxError::Logic(
                        "Cannot convert exception type to string".to_string(),
                    ))?
                    .to_owned(),
            );
        }
        if let Some(note) = unpacked_info.get("exception_note")
            && jobinfo.exception.occured
        {
            jobinfo.exception.note = Some(
                note.as_str()
                    .ok_or(FluxError::Logic(
                        "Cannot convert exception note to string".to_string(),
                    ))?
                    .to_owned(),
            );
        }
        Ok(jobinfo)
    }
}

impl From<FluxFuture<'static>> for JobResult {
    fn from(value: FluxFuture<'static>) -> Self {
        Self { future: value }
    }
}

impl Deref for JobResult {
    type Target = FluxFuture<'static>;

    fn deref(&self) -> &Self::Target {
        &self.future
    }
}

impl DerefMut for JobResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.future
    }
}

impl_async_future_wrapper!(
    #[from_sync(JobResult)]
    pub struct AsyncJobResult {
        #[from_sync(future)]
        future: AsyncFluxFuture,
    }
);
