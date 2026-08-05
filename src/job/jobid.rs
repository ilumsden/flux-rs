use std::ffi::{c_char, CStr, CString};
use std::fmt::Display;
use std::ops::{Deref, DerefMut};

use flux_sys::core::{
    flux_job_id_encode, flux_job_id_parse, flux_job_submit_get_id, flux_jobid_t, FLUX_JOBID_ANY,
};
use serde::{Deserialize, Serialize};

use crate::error::{check_rc, FluxError, Result};
use crate::future::FluxFuture;

pub enum JobIdEncodingType {
    Dec,
    Hex,
    Kvs,
    Dothex,
    Words,
    F58,
    F58plain,
    Emoji,
}

impl JobIdEncodingType {
    pub const fn as_str(&self) -> &str {
        match self {
            JobIdEncodingType::Dec => "dec",
            JobIdEncodingType::Hex => "hex",
            JobIdEncodingType::Kvs => "kvs",
            JobIdEncodingType::Dothex => "dothex",
            JobIdEncodingType::Words => "words",
            JobIdEncodingType::F58 => "f58",
            JobIdEncodingType::F58plain => "f58plain",
            JobIdEncodingType::Emoji => "emoji",
        }
    }

    pub const fn as_c_str(&self) -> &CStr {
        match self {
            JobIdEncodingType::Dec => c"dec",
            JobIdEncodingType::Hex => c"hex",
            JobIdEncodingType::Kvs => c"kvs",
            JobIdEncodingType::Dothex => c"dothex",
            JobIdEncodingType::Words => c"words",
            JobIdEncodingType::F58 => c"f58",
            JobIdEncodingType::F58plain => c"f58plain",
            JobIdEncodingType::Emoji => c"emoji",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobId(pub(crate) flux_jobid_t);

impl JobId {
    pub fn parse(str_repr: &str) -> Result<Self> {
        let mut raw_jobid: flux_jobid_t = 0;
        let c_str_repr = CString::new(str_repr)?;
        let rc =
            unsafe { flux_job_id_parse(c_str_repr.as_ptr(), &mut raw_jobid as *mut flux_jobid_t) };
        check_rc(rc)?;
        Ok(Self(raw_jobid))
    }

    pub fn encode(&self, encoding_type: JobIdEncodingType) -> Result<String> {
        self.encode_with_max_size(encoding_type, 6 * 1024)
    }

    pub fn encode_with_max_size(
        &self,
        encoding_type: JobIdEncodingType,
        max_id_size: usize,
    ) -> Result<String> {
        // Get the C-String for 'type'
        let c_type = encoding_type.as_c_str();

        // Set the initial buffer size
        let mut buf_size = 128;

        if buf_size > max_id_size {
            buf_size = max_id_size;
        }

        // Repeatedly call flux_job_id_encode until we have a value of buf_size that is
        // large enough to actually store the encoded job ID
        loop {
            // Create the buffer to send to C
            let mut buf = vec![0u8; buf_size];
            // Call flux_job_id_encode with the current buffer size
            let rc = unsafe {
                flux_job_id_encode(
                    self.0,
                    c_type.as_ptr(),
                    buf.as_mut_ptr() as *mut c_char,
                    buf_size,
                )
            };
            // If flux_job_id_encode succeeded, we had a big enough buffer size.
            // In this case, we convert the buffer to a CStr to cut off everything after
            // the NUL byte. Then, we call CStr::to_string_lossy to safely handle emojis
            if rc == 0 {
                let c_str = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
                return Ok(c_str.to_string_lossy().into_owned());
            }
            // If flux_job_id_encode failed, we need to decide if we should try again and error out.
            // If errno is EOVERFLOW, that means buf_size was too small, so we double it and try again.
            // Otherwise, an actual error occured, so we error out.
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EOVERFLOW) {
                buf_size *= 2;
                // To avoid infinite loops, we limit buf_size to 'max_id_size' bytes.
                // If we exceed that value, we error out.
                if buf_size > max_id_size {
                    return Err(FluxError::Logic(format!(
                        "Job ID encoding exceeded {} bytes",
                        max_id_size
                    )));
                }
                continue;
            }
            return Err(FluxError::System(err));
        }
    }

    pub fn dec(&self) -> Result<String> {
        self.encode(JobIdEncodingType::Dec)
    }

    pub fn f58(&self) -> Result<String> {
        self.encode(JobIdEncodingType::F58)
    }

    pub fn f58plain(&self) -> Result<String> {
        self.encode(JobIdEncodingType::F58plain)
    }

    pub fn hex(&self) -> Result<String> {
        self.encode(JobIdEncodingType::Hex)
    }

    pub fn dothex(&self) -> Result<String> {
        self.encode(JobIdEncodingType::Dothex)
    }

    pub fn words(&self) -> Result<String> {
        self.encode(JobIdEncodingType::Words)
    }

    pub fn emoji(&self) -> Result<String> {
        self.encode(JobIdEncodingType::Emoji)
    }

    pub fn kvs(&self) -> Result<String> {
        self.encode(JobIdEncodingType::Kvs)
    }
}

impl Deref for JobId {
    type Target = flux_jobid_t;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for JobId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str_repr = match self.f58() {
            Ok(s) => s,
            Err(e) => format!("{} (failed to convert to f58: {})", self.0, e),
        };
        write!(f, "{}", str_repr)
    }
}

impl From<flux_jobid_t> for JobId {
    fn from(value: flux_jobid_t) -> Self {
        Self(value)
    }
}

impl TryFrom<FluxFuture<'_>> for JobId {
    type Error = FluxError;

    fn try_from(value: FluxFuture) -> Result<Self> {
        let mut c_jobid: flux_jobid_t = 0;
        let rc = unsafe {
            flux_job_submit_get_id(
                value.c_future.as_mut_ptr(),
                &mut c_jobid as *mut flux_jobid_t,
            )
        };
        check_rc(rc)?;
        Ok(Self::from(c_jobid))
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self(FLUX_JOBID_ANY as flux_jobid_t)
    }
}
