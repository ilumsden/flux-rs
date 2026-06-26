use std::ffi::{CStr, CString, c_char, c_void};
use std::fmt::Display;

use bitflags::bitflags;
use flux_sys::core::{
    flux_job_result_get, flux_job_result_t, flux_job_result_t_FLUX_JOB_RESULT_CANCELED, flux_job_result_t_FLUX_JOB_RESULT_COMPLETED, flux_job_result_t_FLUX_JOB_RESULT_FAILED, flux_job_result_t_FLUX_JOB_RESULT_TIMEOUT, flux_job_resulttostr, flux_job_strtoresult,
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
        let encoded_ptr = unsafe { flux_job_resulttostr(self.bits(), c_fmt.as_ptr()) };
        check_ptr(encoded_ptr as *mut i8)?;
        let mut owned_str = unsafe { CStr::from_ptr(encoded_ptr).to_str()?.to_owned() };
        unsafe { libc::free(encoded_ptr as *mut c_void) };
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

impl Display for JobResultCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let job_result_str = match self.encode(JobStateFormat::UpperCaseLong) {
            Ok(s) => s,
            Err(e) => format!("UNKNOWN (failed to convert C string to Rust string: {})", e),
        };
        write!(f, "{}", job_result_str)
    }
}

pub struct JobResult {
    future: FluxFuture,
}

impl JobResult {
    pub fn get_info(&self) -> Result<JobInfo> {
        if self.future.c_future.is_null() {
            return Err(FluxError::Logic("Cannot get info from a JobResult is the future is NULL".to_string()));
        }
        let mut json_str: *const c_char = std::ptr::null();
        let rc = unsafe {
            flux_job_result_get(self.future.c_future, &mut json_str as *mut *const c_char)
        };
        check_rc(rc)?;
        let rust_json_str = unsafe { CStr::from_ptr(json_str).to_str()? };
        let unpacked_info: JobInfo = serde_json::from_str(rust_json_str)?;
        Ok(())
    }
}

impl From<FluxFuture> for JobResult {
    fn from(value: FluxFuture) -> Self {
        Self {
            future: value
        }
    }
}

impl_async_future_wrapper!(
    #[from_sync(JobResult)]
    pub struct AsyncJobResult {
        #[from_sync(future)]
        future: AsyncFluxFuture,
    }
);
