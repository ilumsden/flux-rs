use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};

use flux_sys::core::{
    flux_future_and_then, flux_future_continue, flux_future_continue_error, flux_future_destroy,
    flux_future_error_string, flux_future_fatal_error, flux_future_first_child,
    flux_future_fulfill, flux_future_fulfill_error, flux_future_fulfill_next,
    flux_future_fulfill_with, flux_future_get, flux_future_get_child, flux_future_has_error,
    flux_future_incref, flux_future_next_child, flux_future_or_then, flux_future_push,
    flux_future_reset, flux_future_t, flux_future_then, flux_future_wait_all_create,
    flux_future_wait_any_create, flux_future_wait_for,
};

use crate::error::{FluxError, Result};
use crate::AsRawFluxPtr;

pub struct FluxFuture {
    pub(crate) c_future: *mut flux_future_t,
}

impl FluxFuture {
    // TODO in the future, provide a method wrapping flux_future_create.

    /// Reset the Flux future.
    ///
    /// This method is a thin wrapper around `flux_future_reset`.
    pub fn reset(&mut self) -> Result<()> {
        unsafe { flux_future_reset(self.c_future) };
        Ok(())
    }

    /// Check if an error was reported on the future.
    ///
    /// This method is a thin wrapper around `flux_future_has_error` and `flux_future_error_string`.
    pub fn check_error(&mut self) -> Result<()> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot check error on a NULL future",
            )));
        }
        let has_error = unsafe { flux_future_has_error(self.c_future) };
        if has_error {
            let errstr = unsafe { flux_future_error_string(self.c_future) };
            if errstr.is_null() {
                return Err(FluxError::Logic(String::from(
                    "Error occured, but no error string is provided.",
                )));
            }
            let c_str = unsafe { CStr::from_ptr(errstr) };
            return Err(FluxError::Logic(c_str.to_str()?.to_owned()));
        }
        Ok(())
    }

    /// Prepare the C callback used in future chaining/continuation.
    fn prepare_callback<F>(
        &self,
        callback: F,
    ) -> (*mut c_void, extern "C" fn(*mut flux_future_t, *mut c_void))
    where
        F: FnOnce(FluxFuture) + Send + 'static,
    {
        // Get a stable C pointer to the user-provided callback
        let boxed_cb: Box<F> = Box::new(callback);
        let raw_ptr_cb = Box::into_raw(boxed_cb) as *mut c_void;
        // Call flux_future_incref to ensure the future will live long enough for
        // the callback to be invoked
        unsafe {
            flux_future_incref(self.c_future);
        }
        // Define the actual callback
        extern "C" fn trampoline_cb<F>(f: *mut flux_future_t, arg: *mut c_void)
        where
            F: FnOnce(FluxFuture) + Send + 'static,
        {
            // Get the user-provided closure from 'arg'
            let closure = unsafe { Box::from_raw(arg as *mut F) };
            // Create a new FluxFuture object from the C future
            let owned_future = FluxFuture::from(f);
            // Call the user callback. There is no need for 'arg' here because Rust
            // closures can capture
            closure(owned_future);
        }
        (raw_ptr_cb, trampoline_cb::<F>)
    }

    /// flux_future_then
    pub fn then<F>(&mut self, timeout: f64, callback: F) -> Result<()>
    where
        F: FnOnce(FluxFuture) + Send + 'static,
    {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot add continuation to a NULL future",
            )));
        }
        // Prepare the data needed for the call to flux_future_then
        let (raw_ptr_cb, trampoline) = self.prepare_callback(callback);
        // Call flux_future_then to register the callback
        let rc = unsafe { flux_future_then(self.c_future, timeout, Some(trampoline), raw_ptr_cb) };
        // If an error occured, clean up the boxed user callback and decrement the future's ref count
        if rc == -1 {
            let _ = unsafe { Box::from_raw(raw_ptr_cb as *mut F) };
            unsafe { flux_future_destroy(self.c_future) };
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn and_then<F>(&mut self, callback: F) -> Result<Self>
    where
        F: FnOnce(FluxFuture) + Send + 'static,
    {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot add 'and' continuation to a NULL future",
            )));
        }
        // Prepare the data needed for the call to flux_future_then
        let (raw_ptr_cb, trampoline) = self.prepare_callback(callback);
        // Call flux_future_then to register the callback
        let new_future =
            unsafe { flux_future_and_then(self.c_future, Some(trampoline), raw_ptr_cb) };
        if new_future as *const flux_future_t == std::ptr::null() {
            let _ = unsafe { Box::from_raw(raw_ptr_cb as *mut F) };
            unsafe { flux_future_destroy(self.c_future) };
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(Self::from(new_future))
    }

    pub fn or_then<F>(&mut self, callback: F) -> Result<Self>
    where
        F: FnOnce(FluxFuture) + Send + 'static,
    {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot add 'or' continuation to a NULL future",
            )));
        }
        // Prepare the data needed for the call to flux_future_then
        let (raw_ptr_cb, trampoline) = self.prepare_callback(callback);
        // Call flux_future_then to register the callback
        let new_future =
            unsafe { flux_future_or_then(self.c_future, Some(trampoline), raw_ptr_cb) };
        if new_future as *const flux_future_t == std::ptr::null() {
            let _ = unsafe { Box::from_raw(raw_ptr_cb as *mut F) };
            unsafe { flux_future_destroy(self.c_future) };
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(Self::from(new_future))
    }

    pub fn continue_with_future(&mut self, result_future: &FluxFuture) -> Result<()> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot continue a NULL future",
            )));
        }
        let rc = unsafe { flux_future_continue(self.c_future, result_future.c_future) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn continue_with_error(&mut self, errnum: i32, errstr: Option<&str>) -> Result<()> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot continue a NULL future",
            )));
        }
        let c_str = match errstr {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        let errstr_ptr = match &c_str {
            Some(cs) => cs.as_ptr(),
            None => std::ptr::null(),
        };
        unsafe { flux_future_continue_error(self.c_future, errnum, errstr_ptr) };
        Ok(())
    }

    extern "C" fn free_boxed_data_for_fulfill<T>(arg: *mut c_void) {
        let _ = unsafe { Box::from_raw(arg as *mut T) };
    }

    /// Fulfill the next future in the chain of futures.
    ///
    /// This method is a wrapper around `flux_future_fulfill_next`.
    ///
    /// # Returns:
    ///
    /// * `Ok(true)` if the next future was successfully fulfilled.
    /// * `Ok(false)` if there is not a next future to fulfill.
    pub fn fulfill_next<T>(&mut self, result: Box<T>) -> Result<bool> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot fulfill a NULL future",
            )));
        }
        let raw_ptr = Box::into_raw(result) as *mut c_void;
        let rc = unsafe {
            flux_future_fulfill_next(
                self.c_future,
                raw_ptr,
                Some(Self::free_boxed_data_for_fulfill::<T>),
            )
        };
        if rc == -1 {
            let last_os_error = std::io::Error::last_os_error();
            let errno_val = last_os_error.raw_os_error();
            if let Some(inner_errno_val) = errno_val {
                if inner_errno_val == libc::EINVAL {
                    return Ok(false);
                }
            }
            return Err(FluxError::System(last_os_error));
        }
        Ok(true)
    }

    pub fn fulfill<T>(&mut self, result: Box<T>) -> Result<()> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot fulfill a NULL future",
            )));
        }
        let raw_ptr = Box::into_raw(result) as *mut c_void;
        unsafe {
            flux_future_fulfill(
                self.c_future,
                raw_ptr,
                Some(Self::free_boxed_data_for_fulfill::<T>),
            )
        };
        Ok(())
    }

    pub fn fulfill_error(&mut self, errnum: i32, errstr: Option<&str>) -> Result<()> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot fulfill a NULL future",
            )));
        }
        let c_str = match errstr {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        let errstr_ptr = match &c_str {
            Some(cs) => cs.as_ptr(),
            None => std::ptr::null(),
        };
        unsafe { flux_future_fulfill_error(self.c_future, errnum, errstr_ptr) };
        Ok(())
    }

    pub fn fulfill_with(&mut self, other_future: &FluxFuture) -> Result<()> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot fulfill a NULL future",
            )));
        }
        unsafe { flux_future_fulfill_with(self.c_future, other_future.c_future) };
        Ok(())
    }

    pub fn fatal_error(&mut self, errnum: i32, errstr: Option<&str>) -> Result<()> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot trigger a fatal error on a NULL future",
            )));
        }
        let c_str = match errstr {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        let errstr_ptr = match &c_str {
            Some(cs) => cs.as_ptr(),
            None => std::ptr::null(),
        };
        unsafe { flux_future_fatal_error(self.c_future, errnum, errstr_ptr) };
        Ok(())
    }

    /// Block until the future is fulfilled or until a specified timeout, whichever happens first.
    ///
    /// # Returns
    /// * `Ok(true)` if the future was successfully waited on.
    /// * `Ok(false)` if the wait timed out.
    /// * `Err(FluxError)` on error.
    pub fn wait_for(&mut self, timeout: f64) -> Result<bool> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot wait for a NULL future",
            )));
        }
        let rc = unsafe { flux_future_wait_for(self.c_future, timeout) };
        if rc == -1 {
            let last_os_error = std::io::Error::last_os_error();
            let errno_val = last_os_error.raw_os_error();
            if let Some(inner_errno_val) = errno_val {
                if inner_errno_val == libc::ETIMEDOUT {
                    return Ok(false);
                }
            }
            return Err(FluxError::System(last_os_error));
        }
        Ok(true)
    }

    pub fn first_child(&self) -> Result<Option<String>> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get first child of a NULL future",
            )));
        }
        let c_str_ptr = unsafe { flux_future_first_child(self.c_future) };
        if c_str_ptr.is_null() {
            return Ok(None);
        }
        let c_str = unsafe { CStr::from_ptr(c_str_ptr) };
        Ok(Some(c_str.to_str()?.to_owned()))
    }

    pub fn next_child(&self) -> Result<Option<String>> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get next child of a NULL future",
            )));
        }
        let c_str_ptr = unsafe { flux_future_next_child(self.c_future) };
        if c_str_ptr.is_null() {
            return Ok(None);
        }
        let c_str = unsafe { CStr::from_ptr(c_str_ptr) };
        Ok(Some(c_str.to_str()?.to_owned()))
    }

    pub fn get_child(&self, name: &str) -> Result<Option<FluxFuture>> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get child of a NULL future",
            )));
        }
        let c_name = CString::new(name)?;
        let flux_future_ptr = unsafe { flux_future_get_child(self.c_future, c_name.as_ptr()) };
        if flux_future_ptr.is_null() {
            return Ok(None);
        }
        Ok(Some(FluxFuture::from(flux_future_ptr)))
    }

    pub fn get(&mut self) -> Result<*const c_void> {
        if self.c_future.is_null() {
            return Err(FluxError::Logic(String::from("Cannot get a NULL future")));
        }
        let mut result_ptr: *const c_void = std::ptr::null();
        let rc = unsafe { flux_future_get(self.c_future, &mut result_ptr) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(result_ptr)
    }
}

impl Drop for FluxFuture {
    fn drop(&mut self) {
        unsafe {
            if !self.c_future.is_null() {
                flux_future_destroy(self.c_future);
            }
        }
    }
}

impl Clone for FluxFuture {
    fn clone(&self) -> Self {
        unsafe {
            flux_future_incref(self.c_future);
        }
        Self::from(self.c_future)
    }
}

impl From<*mut flux_future_t> for FluxFuture {
    fn from(value: *mut flux_future_t) -> Self {
        Self { c_future: value }
    }
}

impl AsRawFluxPtr<flux_future_t> for FluxFuture {
    fn as_flux_ptr(&self) -> *mut flux_future_t {
        self.c_future
    }
}

fn push_to_collective_future(
    coll_future: &mut FluxFuture,
    name: &str,
    child_future: FluxFuture,
) -> Result<()> {
    let c_name = CString::new(name)?;
    // Increment the ref count of the future since it will be auto-decremented by drop at the
    // end of this function.
    unsafe { flux_future_incref(child_future.c_future) };
    let rc =
        unsafe { flux_future_push(coll_future.c_future, c_name.as_ptr(), child_future.c_future) };
    if rc == -1 {
        return Err(FluxError::System(std::io::Error::last_os_error()));
    }
    Ok(())
}

pub fn create_wait_all_future(futures: HashMap<String, FluxFuture>) -> Result<FluxFuture> {
    let raw_coll_future = unsafe { flux_future_wait_all_create() };
    if raw_coll_future.is_null() {
        return Err(FluxError::System(std::io::Error::last_os_error()));
    }
    let mut coll_future = FluxFuture::from(raw_coll_future);
    for (name, child_future) in futures.into_iter() {
        push_to_collective_future(&mut coll_future, &name, child_future)?;
    }
    Ok(coll_future)
}

pub fn create_wait_any_future(futures: HashMap<String, FluxFuture>) -> Result<FluxFuture> {
    let raw_coll_future = unsafe { flux_future_wait_any_create() };
    if raw_coll_future.is_null() {
        return Err(FluxError::System(std::io::Error::last_os_error()));
    }
    let mut coll_future = FluxFuture::from(raw_coll_future);
    for (name, child_future) in futures.into_iter() {
        push_to_collective_future(&mut coll_future, &name, child_future)?;
    }
    Ok(coll_future)
}
