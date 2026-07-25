use std::ffi::{c_char, CStr};
use std::ops::{Deref, DerefMut};

use flux_sys::core::{flux_job_wait_get_id, flux_job_wait_get_status, flux_jobid_t};

use crate::error::{check_rc, FluxError, Result};
use crate::future::FluxFuture;
use crate::job::jobid::JobId;
use crate::utils::impl_async_future_wrapper;

pub struct JobStatus {
    future: FluxFuture<'static>,
    id: Option<JobId>,
    success: Option<bool>,
    errstr: Option<String>,
}

impl JobStatus {
    fn update_with_status(&mut self) -> Result<()> {
        if self.id.is_some() {
            return Ok(());
        }
        let mut errstr_ptr: *const c_char = std::ptr::null();
        let mut success = false;
        let mut raw_jobid: flux_jobid_t = 0;
        let mut rc = unsafe {
            flux_job_wait_get_status(
                self.future.c_future.as_mut_ptr(),
                &mut success as *mut bool,
                &mut errstr_ptr as *mut *const c_char,
            )
        };
        check_rc(rc)?;
        rc = unsafe {
            flux_job_wait_get_id(
                self.future.c_future.as_mut_ptr(),
                &mut raw_jobid as *mut flux_jobid_t,
            )
        };
        check_rc(rc)?;
        self.id = Some(JobId::from(raw_jobid));
        self.success = Some(success);
        self.errstr = Some(unsafe { CStr::from_ptr(errstr_ptr).to_str()?.to_string() });
        Ok(())
    }

    pub fn get_id(&mut self) -> Result<JobId> {
        self.update_with_status()?;
        self.id.ok_or(FluxError::Logic(
            "Job ID is None after fetching it from Flux".to_string(),
        ))
    }

    pub fn get_success(&mut self) -> Result<bool> {
        self.update_with_status()?;
        self.success.ok_or(FluxError::Logic(
            "Success state is None after fetching it from Flux".to_string(),
        ))
    }

    pub fn get_errstr(&mut self) -> Result<&str> {
        self.update_with_status()?;
        self.errstr
            .as_ref()
            .map(|es| es.as_str())
            .ok_or(FluxError::Logic(
                "Success state is None after fetching it from Flux".to_string(),
            ))
    }
}

impl Deref for JobStatus {
    type Target = FluxFuture<'static>;

    fn deref(&self) -> &Self::Target {
        &self.future
    }
}

impl DerefMut for JobStatus {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.future
    }
}

impl From<FluxFuture<'static>> for JobStatus {
    fn from(value: FluxFuture<'static>) -> Self {
        Self {
            future: value,
            id: None,
            success: None,
            errstr: None,
        }
    }
}

impl_async_future_wrapper!(
    #[from_sync(JobStatus)]
    pub struct AsyncJobStatus {
        #[from_sync(future)]
        future: AsyncFluxFuture,
        #[from_sync(id)]
        id: Option<JobId>,
        #[from_sync(success)]
        success: Option<bool>,
        #[from_sync(errstr)]
        #[to_sync_action(Clone)]
        errstr: Option<String>,
    }
);
