use std::io;
use thiserror::Error;

use crate::flux_log_error;
use crate::handle::FluxHandle;

/// The main error type for the flux crate.
#[derive(Error, Debug)]
pub enum FluxError {
    /// Represents an error returned by the underlying C API via errno.
    /// `std::io::Error` is the standard way to represent OS/errno values in Rust.
    #[error("Flux system error: {0}")]
    System(#[from] io::Error),

    /// Error when a Rust string contains a null byte, making it invalid for C.
    #[error("String contains null byte: {0}")]
    NulError(#[from] std::ffi::NulError),

    /// Error when interpreting null bytes in a Rust string.
    #[error("String's use of null bytes cannot be handled: {0}")]
    NulInterpretError(#[from] std::ffi::FromBytesWithNulError),

    /// Error when parsing a C string using CStr::from_bytes_until_nul
    #[error("Cannot build a string from a C string by looking for a NUL byte: {0}")]
    FromBytesUntilNulError(#[from] std::ffi::FromBytesUntilNulError),

    /// Error when a C string is not valid UTF-8.
    #[error("Invalid UTF-8 from C API: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    /// Error when converting string duration timestamps to floats
    #[error("Duration parsing error: {0}")]
    DurationParseError(#[from] std::num::ParseFloatError),

    /// Error when parsing a URL (e.g., a Flux URI)
    #[error("Error when parsing a URL (e.g., for a Flux URI): {0}")]
    UrlParseError(#[from] url::ParseError),

    /// Error when dealing with the Nix library
    #[error("Error occured in nix-specific operation: {0}")]
    NixError(#[from] nix::Error),

    /// Error when serializing or deserializing JSON data.
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// A custom error specific to your higher-level crate logic.
    #[error("Flux logic error: {0}")]
    Logic(String),

    /// A custom error for request/response messages
    #[error("Error occured in response/request.\nSystem Error: {0}\nError Message: {1}")]
    RequestResponseError(io::Error, String),
}

impl FluxError {
    /// Get an `errno` value for the FluxError object.
    pub fn to_errno(&self) -> i32 {
        match self {
            Self::System(err) => err.raw_os_error().unwrap_or(libc::EINVAL),
            Self::NixError(err) => *err as i32,
            Self::RequestResponseError(err, _) => err.raw_os_error().unwrap_or(libc::EINVAL),
            _ => libc::EINVAL,
        }
    }

    /// Get an `errno` value for the FluxError object and log the error with the FluxHandle.
    pub fn to_errno_with_flux_log(&self, handle: &FluxHandle) -> i32 {
        flux_log_error!(handle, "{}", self);
        self.to_errno()
    }

    /// Set `errno` based on the FluxError object.
    pub fn set_errno(&self, log_handle: Option<&FluxHandle>) {
        let errno_val = if let Some(h) = log_handle {
            self.to_errno_with_flux_log(h)
        } else {
            self.to_errno()
        };
        unsafe { *::libc::__errno_location() = errno_val };
    }
}

/// A convenient Result alias for the crate.
pub type Result<T> = std::result::Result<T, FluxError>;

/// Create a Flux C-style return code/`errno` pair from a `Result` object.
#[inline]
pub fn to_flux_rc(result: Result<()>, log_handle: Option<&FluxHandle>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            e.set_errno(log_handle);
            -1
        }
    }
}

/// A helper function to evaluate Flux C API integer return codes.
/// Flux typically returns 0 on success and -1 on failure, setting errno.
#[inline]
pub(crate) fn check_rc(rc: std::os::raw::c_int) -> Result<()> {
    if rc == -1 {
        // Captures the current `errno` and wraps it in std::io::Error
        Err(FluxError::System(io::Error::last_os_error()))
    } else {
        Ok(())
    }
}

/// A helper function for pointers returned by Flux.
/// Flux typically returns NULL on failure, setting errno.
#[inline]
pub(crate) fn check_ptr<T>(ptr: *mut T) -> Result<*mut T> {
    if ptr.is_null() {
        Err(FluxError::System(io::Error::last_os_error()))
    } else {
        Ok(ptr)
    }
}
