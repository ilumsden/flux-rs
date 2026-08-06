use std::ffi::{CStr, CString, c_char};
use std::ops::{Deref, DerefMut};

use flux_sys::core::{
    flux_job_cancel, flux_job_kill, flux_job_kvs_guest_key, flux_job_kvs_key,
    flux_job_kvs_namespace, flux_job_raise, flux_job_result, flux_job_set_urgency, flux_job_wait,
};

use crate::SignalCode;
use crate::error::{FluxError, Result, check_ptr};
use crate::flux_ptr_management::FromFluxPtrNoArgs;
use crate::future::FluxFuture;
use crate::handle::FluxHandle;
use crate::job::jobid::JobId;
use crate::job::status::JobStatus;
use crate::job::{JobResult, JobUrgency};
use crate::kvs::KvsDir;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobEventSeverity(u8);

impl JobEventSeverity {
    pub const FATAL: Self = Self(0);
    pub const MIN_SEVERITY: Self = Self(7);

    pub fn new(val: u8) -> Option<Self> {
        if val > 7 { None } else { Some(Self(val)) }
    }

    pub fn as_raw(&self) -> u8 {
        self.0
    }
}

impl Deref for JobEventSeverity {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for JobEventSeverity {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct Job<'a> {
    handle: &'a FluxHandle,
    id: JobId,
}

impl<'a> Job<'a> {
    pub fn new(handle: &'a FluxHandle, id: JobId) -> Result<Self> {
        Ok(Self { handle, id })
    }

    /// Build a Job from a FluxHandle reference and a FluxFuture from submit_async.
    ///
    /// This function is an alias to `Job::try_from`, provided by the implementation of `TryFrom<(&FluxHandle, FluxFuture)>`.
    pub fn try_from_handle_and_future(handle: &'a FluxHandle, future: FluxFuture) -> Result<Self> {
        Self::try_from((handle, future))
    }

    pub fn get_id(&self) -> &JobId {
        &self.id
    }

    pub fn wait(&self) -> Result<JobStatus> {
        let future_ptr = unsafe { flux_job_wait(self.handle.h.as_mut_ptr(), *self.id) };
        check_ptr(future_ptr)?;
        Ok(JobStatus::from(unsafe {
            FluxFuture::from_ptr(future_ptr)?
        }))
    }

    pub fn result(&self) -> Result<JobResult> {
        let future_ptr = unsafe { flux_job_result(self.handle.h.as_mut_ptr(), *self.id, 0) };
        check_ptr(future_ptr)?;
        Ok(JobResult::from(unsafe {
            FluxFuture::from_ptr(future_ptr)?
        }))
    }

    // TODO implement something related to watch_eventlog, list_id

    pub fn raise(
        &self,
        severity: Option<JobEventSeverity>,
        message: Option<&str>,
        exception_type: Option<&str>,
    ) -> Result<FluxFuture<'static>> {
        let sev = severity.unwrap_or(JobEventSeverity::FATAL);
        let c_msg = message.map(CString::new).transpose()?;
        let c_exc_type = CString::new(exception_type.unwrap_or("cancel"))?;
        let future_ptr = unsafe {
            flux_job_raise(
                self.handle.h.as_mut_ptr(),
                *self.id,
                c_exc_type.as_ptr(),
                sev.as_raw() as _,
                c_msg.map(|cm| cm.as_ptr()).unwrap_or(std::ptr::null()),
            )
        };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn cancel(&self, reason: Option<&str>) -> Result<FluxFuture<'static>> {
        let c_reason = reason.map(CString::new).transpose()?;
        let future_ptr = unsafe {
            flux_job_cancel(
                self.handle.h.as_mut_ptr(),
                *self.id,
                c_reason
                    .map(|cstr| cstr.as_ptr())
                    .unwrap_or(std::ptr::null()),
            )
        };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn kill(&self, signal: SignalCode) -> Result<FluxFuture<'static>> {
        let future_ptr =
            unsafe { flux_job_kill(self.handle.h.as_mut_ptr(), *self.id, signal.as_raw()) };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn set_urgency(&mut self, urgency: JobUrgency) -> Result<FluxFuture<'static>> {
        let future_ptr = unsafe {
            flux_job_set_urgency(self.handle.h.as_mut_ptr(), *self.id, urgency.as_u8() as _)
        };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn get_kvs_dir(&self) -> Result<KvsDir> {
        let key = self.get_kvs_key("")?;
        KvsDir::new(self.handle, Some(key.as_str()), None)
    }

    pub fn get_kvs_key(&self, key: &str) -> Result<String> {
        self.get_kvs_key_with_max_size(key, 1024 * 8)
    }

    pub fn get_kvs_key_with_max_size(&self, key: &str, max_key_size: usize) -> Result<String> {
        let c_key = CString::new(key)?;
        let key_ptr = if key.is_empty() {
            std::ptr::null()
        } else {
            c_key.as_ptr()
        };
        let mut bufsize: usize = 1024;
        loop {
            let mut buf = vec![0u8; bufsize];
            let rc = unsafe {
                flux_job_kvs_key(
                    buf.as_mut_ptr() as *mut c_char,
                    bufsize as _,
                    *self.id,
                    key_ptr,
                )
            };
            if rc >= 0 {
                let c_str = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
                return Ok(c_str.to_string_lossy().into_owned());
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EOVERFLOW) {
                bufsize *= 2;
                if bufsize > max_key_size {
                    return Err(FluxError::Logic(format!(
                        "KVS Key encoding exceeded {max_key_size} bytes",
                    )));
                }
                continue;
            }
            return Err(FluxError::System(err));
        }
    }

    pub fn get_kvs_guest_dir(&self) -> Result<KvsDir> {
        let key = self.get_kvs_guest_key("")?;
        KvsDir::new(self.handle, Some(key.as_str()), None)
    }

    pub fn get_kvs_guest_key(&self, key: &str) -> Result<String> {
        self.get_kvs_guest_key_with_max_size(key, 1024 * 8)
    }

    pub fn get_kvs_guest_key_with_max_size(
        &self,
        key: &str,
        max_key_size: usize,
    ) -> Result<String> {
        let c_key = CString::new(key)?;
        let key_ptr = if key.is_empty() {
            std::ptr::null()
        } else {
            c_key.as_ptr()
        };
        let mut bufsize: usize = 1024;
        loop {
            let mut buf = vec![0u8; bufsize];
            let rc = unsafe {
                flux_job_kvs_guest_key(
                    buf.as_mut_ptr() as *mut c_char,
                    bufsize as _,
                    *self.id,
                    key_ptr,
                )
            };
            if rc >= 0 {
                let c_str = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
                return Ok(c_str.to_string_lossy().into_owned());
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EOVERFLOW) {
                bufsize *= 2;
                if bufsize > max_key_size {
                    return Err(FluxError::Logic(format!(
                        "KVS Guest Key encoding exceeded {max_key_size} bytes",
                    )));
                }
                continue;
            }
            return Err(FluxError::System(err));
        }
    }

    pub fn get_kvs_namespace(&self) -> Result<String> {
        self.get_kvs_namespace_with_max_size(6 * 1024)
    }

    pub fn get_kvs_namespace_with_max_size(&self, max_ns_size: usize) -> Result<String> {
        let mut bufsize: usize = 128;
        loop {
            let mut buf = vec![0u8; bufsize];
            let rc = unsafe {
                flux_job_kvs_namespace(buf.as_mut_ptr() as *mut c_char, bufsize as _, *self.id)
            };
            if rc >= 0 {
                let c_str = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
                return Ok(c_str.to_string_lossy().into_owned());
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EOVERFLOW) {
                bufsize *= 2;
                if bufsize > max_ns_size {
                    return Err(FluxError::Logic(format!(
                        "KVS namespace encoding exceeded {max_ns_size} bytes",
                    )));
                }
                continue;
            }
            return Err(FluxError::System(err));
        }
    }
}

impl<'a> TryFrom<(&'a FluxHandle, FluxFuture<'_>)> for Job<'a> {
    type Error = FluxError;

    fn try_from(value: (&'a FluxHandle, FluxFuture)) -> Result<Self> {
        let job_id = JobId::try_from(value.1)?;
        Self::new(value.0, job_id)
    }
}
