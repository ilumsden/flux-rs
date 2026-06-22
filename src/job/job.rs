use flux_sys::core::flux_job_wait;

use crate::error::{check_ptr, FluxError, Result};
use crate::future::FluxFuture;
use crate::handle::FluxHandle;
use crate::job::jobid::JobId;
use crate::job::status::JobStatus;

pub struct Job<'a> {
    handle: &'a FluxHandle,
    id: JobId,
}

impl<'a> Job<'a> {
    pub fn new(handle: &'a FluxHandle, id: JobId) -> Result<Self> {
        if handle.h.is_null() {
            return Err(FluxError::Logic("Cannot create a Job object from a NULL Flux handle. Either provide a valid handle, or invoke functions from 'flux-sys' directly".to_string()));
        }
        Ok(Self { handle, id })
    }

    pub fn wait(&self) -> Result<JobStatus> {
        if self.handle.h.is_null() {
            return Err(FluxError::Logic(
                "Cannot wait on a job with a NULL Flux handle".to_string(),
            ));
        }
        let future_ptr = unsafe { flux_job_wait(self.handle.h, *self.id) };
        check_ptr(future_ptr)?;
        Ok(JobStatus::from(FluxFuture::from(future_ptr)))
    }
}

impl<'a> TryFrom<(&'a FluxHandle, FluxFuture)> for Job<'a> {
    type Error = FluxError;

    fn try_from(value: (&'a FluxHandle, FluxFuture)) -> Result<Self> {
        let job_id = JobId::try_from(value.1)?;
        Self::new(value.0, job_id)
    }
}
