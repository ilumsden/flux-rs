use std::ffi::CString;

use flux_sys::core::{
    flux_job_submit, job_submit_flags_FLUX_JOB_DEBUG, job_submit_flags_FLUX_JOB_NOVALIDATE,
    job_submit_flags_FLUX_JOB_PRE_SIGNED, job_submit_flags_FLUX_JOB_WAITABLE,
};

use bitflags::bitflags;

use crate::error::{check_ptr, FluxError, Result};
use crate::future::FluxFuture;
use crate::handle::FluxHandle;
use crate::job::job::Job;
use crate::job::jobspec::Jobspec;
use crate::job::urgency::JobUrgency;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct JobSubmitFlags: u32 {
        const WAITABLE = job_submit_flags_FLUX_JOB_WAITABLE;
        const DEBUG = job_submit_flags_FLUX_JOB_DEBUG;
        const PRE_SIGNED = job_submit_flags_FLUX_JOB_PRE_SIGNED;
        const NOVALIDATE = job_submit_flags_FLUX_JOB_NOVALIDATE;
    }
}

pub fn submit_async(
    handle: &FluxHandle,
    jobspec: &Jobspec,
    urgency: Option<JobUrgency>,
    flags: Option<JobSubmitFlags>,
) -> Result<FluxFuture> {
    if handle.h.is_null() {
        return Err(FluxError::Logic(
            "Cannot submit a job when the Flux handle is NULL".to_string(),
        ));
    }
    let serialized_jobspec = serde_json::to_string(jobspec)?;
    let c_serialized_jobspec = CString::new(serialized_jobspec)?;
    let c_urgency: i32 = urgency.unwrap_or(JobUrgency::DEFAULT).into();
    let c_flags: i32 = flags.map(|f| f.bits() as i32).unwrap_or(0);
    let future_ptr =
        unsafe { flux_job_submit(handle.h, c_serialized_jobspec.as_ptr(), c_urgency, c_flags) };
    check_ptr(future_ptr)?;
    Ok(FluxFuture::from(future_ptr))
}

pub fn submit<'h, 'j>(
    handle: &'h FluxHandle,
    jobspec: &'j Jobspec,
    urgency: Option<JobUrgency>,
    flags: Option<JobSubmitFlags>,
) -> Result<Job<'h>> {
    let submit_future = submit_async(handle, jobspec, urgency, flags)?;
    Job::try_from((handle, submit_future))
}
