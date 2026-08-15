use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void};
use std::marker::PhantomData;
use std::sync::Arc;

use flux_sys::core::{
    flux_future_and_then, flux_future_aux_get, flux_future_aux_set, flux_future_continue,
    flux_future_continue_error, flux_future_create, flux_future_destroy, flux_future_error_string,
    flux_future_fatal_error, flux_future_first_child, flux_future_fulfill,
    flux_future_fulfill_error, flux_future_fulfill_next, flux_future_fulfill_with, flux_future_get,
    flux_future_get_child, flux_future_get_flux, flux_future_get_reactor, flux_future_has_error,
    flux_future_incref, flux_future_next_child, flux_future_or_then, flux_future_push,
    flux_future_reset, flux_future_set_flux, flux_future_set_reactor, flux_future_t,
    flux_future_then, flux_future_wait_all_create, flux_future_wait_any_create,
    flux_future_wait_for,
};

use crate::error::{FluxError, Result, flux_try};
use crate::flux_ptr_management::{
    BorrowFluxPtr, BorrowFluxPtrNoArgs, FluxPtr, FromFluxPtr, FromFluxPtrNoArgs,
    default_impl_as_flux_ptr,
};
use crate::handle::{AuxThinPtrWrapper, FluxHandle};
use crate::reactor::Reactor;

pub(crate) type FluxFutureInitializer = Arc<dyn Fn(&mut FluxFuture<'_>) + Send + 'static>;

pub struct FluxFuture<'a> {
    /// The underlying flux_future_t C pointer wrapped in the FluxPtr struct
    pub(crate) c_future: FluxPtr<flux_future_t>,
    /// A PhantomData field to enforce borrowing rules on the flux_future_t pointer for child futures
    _phantom: PhantomData<&'a flux_future_t>,
    /// A field to store the callback passed to FluxFuture::new to ensure it isn't dropped
    pub(crate) _init_cb: Option<FluxFutureInitializer>,
}

impl<'a> FluxFuture<'a> {
    pub fn new<F>(initializer: F) -> Result<FluxFuture<'static>>
    where
        F: Fn(&mut FluxFuture<'_>) + Send + Sync + 'static,
    {
        let cb: FluxFutureInitializer = Arc::new(initializer);
        let raw_arg = Arc::into_raw(cb.clone()) as *mut c_void;

        extern "C" fn trampoline<F>(f: *mut flux_future_t, arg: *mut c_void)
        where
            F: Fn(&mut FluxFuture<'_>) + Send + Sync + 'static,
        {
            // Borrow the Arc without taking ownership — safe because _init_cb keeps it alive
            let cb = unsafe { &*(arg as *const F) };
            let mut future = match unsafe { FluxFuture::borrow_ptr(f) } {
                Ok(f) => f,
                Err(_) => return,
            };
            cb(&mut future);
        }

        let ptr = flux_try!(flux_future_create(Some(trampoline::<F>), raw_arg))?;
        Ok(FluxFuture {
            c_future: FluxPtr::create_owned(ptr, flux_future_destroy)?,
            _phantom: PhantomData,
            _init_cb: Some(cb),
        })
    }

    pub fn set_aux<T: Send + Sync + 'static>(&mut self, name: &str, data: T) -> Result<()> {
        let c_name = CString::new(name)?;
        let wrapper = Box::new(AuxThinPtrWrapper {
            inner: Box::new(data),
        });
        let raw_data_ptr = Box::into_raw(wrapper) as *mut c_void;

        extern "C" fn destroy_aux_trampoline(ptr: *mut c_void) {
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr as *mut AuxThinPtrWrapper);
                }
            }
        }

        let rc = unsafe {
            flux_future_aux_set(
                self.c_future.as_mut_ptr(),
                c_name.as_ptr(),
                raw_data_ptr,
                Some(destroy_aux_trampoline),
            )
        };
        if rc == -1 {
            let _ = unsafe { Box::from_raw(raw_data_ptr as *mut AuxThinPtrWrapper) };
            return Err(FluxError::System(
                "flux_future_aux_set",
                std::io::Error::last_os_error(),
            ));
        }
        Ok(())
    }

    pub fn get_aux<T: 'static>(&self, name: &str) -> Result<&T> {
        let raw_ptr = self.get_aux_raw(name)?;
        let wrapper = unsafe { &*(raw_ptr as *const AuxThinPtrWrapper) };
        match wrapper.inner.downcast_ref::<T>() {
            Some(typed_ref) => Ok(typed_ref),
            None => Err(FluxError::Logic(format!(
                "Type mismatch for aux key '{name}'",
            ))),
        }
    }

    pub fn get_aux_raw(&self, name: &str) -> Result<*mut c_void> {
        let c_name = CString::new(name)?;
        flux_try!(flux_future_aux_get(
            self.c_future.as_mut_ptr(),
            c_name.as_ptr()
        ))
    }

    pub fn set_reactor(&mut self, reactor: &Reactor) {
        unsafe {
            flux_future_set_reactor(self.c_future.as_mut_ptr(), reactor.c_reactor.as_mut_ptr());
        }
    }

    /// Get the Reactor associated with this future.
    ///
    /// # Safety
    /// The returned `Reactor` may share underlying C state
    /// with the handle and reactor that created this future.
    ///
    /// In multi-threaded environments (e.g., Tokio/Smol async runtimes), invoking
    /// methods on this reactor while an async driver is actively ticking the reactor
    /// on another thread causes an un-synchronized C data race and undefined behavior.
    ///
    /// You must ensure that no other thread is concurrently ticking the reactor
    /// or mutating the handle while using the returned `Reactor`.
    ///
    /// In single-threaded environments, this method is completely safe.
    pub unsafe fn get_reactor(&self) -> Result<Reactor> {
        let ptr = flux_try!(flux_future_get_reactor(self.c_future.as_mut_ptr()))?;
        unsafe { Reactor::borrow_ptr(ptr) }
    }

    pub fn set_flux(&mut self, handle: &FluxHandle) {
        unsafe {
            flux_future_set_flux(self.c_future.as_mut_ptr(), handle.h.as_mut_ptr());
        }
    }

    /// Get the FluxHandle associated with this future.
    ///
    /// # Safety
    /// The returned `FluxHandle` may share underlying C state
    /// with the handle and reactor that created this future.
    ///
    /// In multi-threaded environments (e.g., Tokio/Smol async runtimes), invoking
    /// methods on this handle while an async driver is actively ticking the reactor
    /// on another thread causes an un-synchronized C data race and undefined behavior.
    ///
    /// You must ensure that no other thread is concurrently ticking the reactor
    /// or mutating the handle while using the returned `FluxHandle`.
    ///
    /// In single-threaded environments, this method is completely safe.
    pub unsafe fn get_flux(&self) -> Result<FluxHandle> {
        let ptr = flux_try!(flux_future_get_flux(self.c_future.as_mut_ptr()))?;
        unsafe { FluxHandle::borrow_ptr(ptr) }
    }

    /// Created an Rust-owned version of this FluxFuture.
    ///
    /// Under the hood, this method does the same thing as FluxFuture's implementation
    /// of the Clone trait. However, Clone also copies the lifetime bound of the FluxFuture
    /// object. So, with Clone, a user can go from `FluxFuture<'a>` to `FluxFuture<'a>` or from
    /// `FluxFuture<'static>` to `FluxFuture<'static>`, but they cannot go from `FluxFuture<'a>`
    /// to `FluxFuture<'static>`. This method provides a way to do that lifetime upgrade.
    pub fn to_owned(&self) -> Result<FluxFuture<'static>> {
        unsafe {
            flux_future_incref(self.c_future.as_mut_ptr());
        }
        Ok(FluxFuture {
            c_future: FluxPtr::create_owned(self.c_future.as_mut_ptr(), flux_future_destroy)?,
            _phantom: PhantomData,
            _init_cb: self._init_cb.clone(),
        })
    }

    /// Reset the Flux future.
    ///
    /// This method is a thin wrapper around `flux_future_reset`.
    pub fn reset(&mut self) -> Result<()> {
        unsafe { flux_future_reset(self.c_future.as_mut_ptr()) };
        Ok(())
    }

    /// Check if an error was reported on the future.
    ///
    /// This method is a thin wrapper around `flux_future_has_error` and `flux_future_error_string`.
    pub fn check_error(&mut self) -> Result<()> {
        let has_error = unsafe { flux_future_has_error(self.c_future.as_mut_ptr()) };
        if has_error {
            let errstr = unsafe { flux_future_error_string(self.c_future.as_mut_ptr()) };
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
    #[inline]
    fn prepare_callback<F>(
        &self,
        callback: F,
    ) -> (*mut c_void, extern "C" fn(*mut flux_future_t, *mut c_void))
    where
        F: FnOnce(FluxFuture<'static>) + Send + 'static,
    {
        // Get a stable C pointer to the user-provided callback
        let boxed_cb: Box<F> = Box::new(callback);
        let raw_ptr_cb = Box::into_raw(boxed_cb) as *mut c_void;
        // Call flux_future_incref to ensure the future will live long enough for
        // the callback to be invoked
        unsafe {
            flux_future_incref(self.c_future.as_mut_ptr());
        }
        // Define the actual callback
        extern "C" fn trampoline_cb<F>(f: *mut flux_future_t, arg: *mut c_void)
        where
            F: FnOnce(FluxFuture<'static>) + Send + 'static,
        {
            // Get the user-provided closure from 'arg'
            let closure = unsafe { Box::from_raw(arg as *mut F) };
            // Create a new FluxFuture object from the C future
            let owned_future = match unsafe { FluxFuture::from_ptr(f) } {
                Ok(ff) => ff,
                Err(_) => return,
            };
            // Call the user callback. There is no need for 'arg' here because Rust
            // closures can capture
            closure(owned_future);
        }
        (raw_ptr_cb, trampoline_cb::<F>)
    }

    /// flux_future_then
    pub fn then<F>(&mut self, timeout: f64, callback: F) -> Result<()>
    where
        F: FnOnce(FluxFuture<'static>) + Send + 'static,
    {
        // Prepare the data needed for the call to flux_future_then
        let (raw_ptr_cb, trampoline) = self.prepare_callback(callback);
        // Call flux_future_then to register the callback
        let rc = unsafe {
            flux_future_then(
                self.c_future.as_mut_ptr(),
                timeout,
                Some(trampoline),
                raw_ptr_cb,
            )
        };
        // If an error occured, clean up the boxed user callback and decrement the future's ref count
        if rc == -1 {
            let _ = unsafe { Box::from_raw(raw_ptr_cb as *mut F) };
            unsafe { flux_future_destroy(self.c_future.as_mut_ptr()) };
            return Err(FluxError::System(
                "flux_future_then",
                std::io::Error::last_os_error(),
            ));
        }
        Ok(())
    }

    pub fn and_then<F>(&mut self, callback: F) -> Result<Self>
    where
        F: FnOnce(FluxFuture<'static>) + Send + 'static,
    {
        // Prepare the data needed for the call to flux_future_then
        let (raw_ptr_cb, trampoline) = self.prepare_callback(callback);
        // Call flux_future_then to register the callback
        let new_future = unsafe {
            flux_future_and_then(self.c_future.as_mut_ptr(), Some(trampoline), raw_ptr_cb)
        };
        if new_future.is_null() {
            let _ = unsafe { Box::from_raw(raw_ptr_cb as *mut F) };
            unsafe { flux_future_destroy(self.c_future.as_mut_ptr()) };
            return Err(FluxError::System(
                "flux_future_and_then",
                std::io::Error::last_os_error(),
            ));
        }
        unsafe { FluxFuture::from_ptr(new_future) }
    }

    pub fn or_then<F>(&mut self, callback: F) -> Result<Self>
    where
        F: FnOnce(FluxFuture<'static>) + Send + 'static,
    {
        // Prepare the data needed for the call to flux_future_then
        let (raw_ptr_cb, trampoline) = self.prepare_callback(callback);
        // Call flux_future_then to register the callback
        let new_future = unsafe {
            flux_future_or_then(self.c_future.as_mut_ptr(), Some(trampoline), raw_ptr_cb)
        };
        if new_future.is_null() {
            let _ = unsafe { Box::from_raw(raw_ptr_cb as *mut F) };
            unsafe { flux_future_destroy(self.c_future.as_mut_ptr()) };
            return Err(FluxError::System(
                "flux_future_or_then",
                std::io::Error::last_os_error(),
            ));
        }
        unsafe { FluxFuture::from_ptr(new_future) }
    }

    pub fn continue_with_future(&mut self, result_future: &FluxFuture) -> Result<()> {
        flux_try!(empty_ok
            flux_future_continue(
                self.c_future.as_mut_ptr(),
                result_future.c_future.as_mut_ptr(),
            )
        )
    }

    pub fn continue_with_error(&mut self, errnum: i32, errstr: Option<&str>) -> Result<()> {
        let c_str = match errstr {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        let errstr_ptr = match &c_str {
            Some(cs) => cs.as_ptr(),
            None => std::ptr::null(),
        };
        unsafe { flux_future_continue_error(self.c_future.as_mut_ptr(), errnum, errstr_ptr) };
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
        let raw_ptr = Box::into_raw(result) as *mut c_void;
        let rc = unsafe {
            flux_future_fulfill_next(
                self.c_future.as_mut_ptr(),
                raw_ptr,
                Some(Self::free_boxed_data_for_fulfill::<T>),
            )
        };
        if rc == -1 {
            let last_os_error = std::io::Error::last_os_error();
            let errno_val = last_os_error.raw_os_error();
            if let Some(inner_errno_val) = errno_val
                && inner_errno_val == libc::EINVAL
            {
                return Ok(false);
            }
            return Err(FluxError::System("flux_future_fulfill_next", last_os_error));
        }
        Ok(true)
    }

    pub fn fulfill<T>(&mut self, result: Option<Box<T>>) -> Result<()> {
        let raw_ptr = result
            .map(|r| Box::into_raw(r) as *mut c_void)
            .unwrap_or(std::ptr::null_mut());
        let free_fn: Option<unsafe extern "C" fn(*mut c_void)> = if raw_ptr.is_null() {
            None
        } else {
            Some(Self::free_boxed_data_for_fulfill::<T>)
        };
        unsafe { flux_future_fulfill(self.c_future.as_mut_ptr(), raw_ptr, free_fn) };
        Ok(())
    }

    pub fn fulfill_error(&mut self, errnum: i32, errstr: Option<&str>) -> Result<()> {
        let c_str = match errstr {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        let errstr_ptr = match &c_str {
            Some(cs) => cs.as_ptr(),
            None => std::ptr::null(),
        };
        unsafe { flux_future_fulfill_error(self.c_future.as_mut_ptr(), errnum, errstr_ptr) };
        Ok(())
    }

    pub fn fulfill_with(&mut self, other_future: &FluxFuture) -> Result<()> {
        unsafe {
            flux_future_fulfill_with(
                self.c_future.as_mut_ptr(),
                other_future.c_future.as_mut_ptr(),
            )
        };
        Ok(())
    }

    pub fn fatal_error(&mut self, errnum: i32, errstr: Option<&str>) -> Result<()> {
        let c_str = match errstr {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        let errstr_ptr = match &c_str {
            Some(cs) => cs.as_ptr(),
            None => std::ptr::null(),
        };
        unsafe { flux_future_fatal_error(self.c_future.as_mut_ptr(), errnum, errstr_ptr) };
        Ok(())
    }

    /// Block until the future is fulfilled or until a specified timeout, whichever happens first.
    ///
    /// # Returns
    /// * `Ok(true)` if the future was successfully waited on.
    /// * `Ok(false)` if the wait timed out.
    /// * `Err(FluxError)` on error.
    pub fn wait_for(&mut self, timeout: f64) -> Result<bool> {
        let rc = unsafe { flux_future_wait_for(self.c_future.as_mut_ptr(), timeout) };
        if rc == -1 {
            let last_os_error = std::io::Error::last_os_error();
            let errno_val = last_os_error.raw_os_error();
            if let Some(inner_errno_val) = errno_val
                && inner_errno_val == libc::ETIMEDOUT
            {
                return Ok(false);
            }
            return Err(FluxError::System("flux_future_wait_for", last_os_error));
        }
        Ok(true)
    }

    pub fn first_child(&self) -> Result<Option<String>> {
        let c_str_ptr = unsafe { flux_future_first_child(self.c_future.as_mut_ptr()) };
        if c_str_ptr.is_null() {
            return Ok(None);
        }
        let c_str = unsafe { CStr::from_ptr(c_str_ptr) };
        Ok(Some(c_str.to_str()?.to_owned()))
    }

    pub fn next_child(&self) -> Result<Option<String>> {
        let c_str_ptr = unsafe { flux_future_next_child(self.c_future.as_mut_ptr()) };
        if c_str_ptr.is_null() {
            return Ok(None);
        }
        let c_str = unsafe { CStr::from_ptr(c_str_ptr) };
        Ok(Some(c_str.to_str()?.to_owned()))
    }

    pub fn get_child<'b>(&'b self, name: &str) -> Result<Option<FluxFuture<'b>>> {
        let c_name = CString::new(name)?;
        let flux_future_ptr =
            unsafe { flux_future_get_child(self.c_future.as_mut_ptr(), c_name.as_ptr()) };
        if flux_future_ptr.is_null() {
            return Ok(None);
        }
        Ok(Some(unsafe { FluxFuture::borrow_ptr(flux_future_ptr)? }))
    }

    pub fn get(&mut self) -> Result<*const c_void> {
        let mut result_ptr: *const c_void = std::ptr::null();
        flux_try!(flux_future_get(self.c_future.as_mut_ptr(), &mut result_ptr))?;
        Ok(result_ptr)
    }
}

impl<'a> Clone for FluxFuture<'a> {
    fn clone(&self) -> Self {
        unsafe {
            flux_future_incref(self.c_future.as_mut_ptr());
        }
        Self {
            c_future: FluxPtr::create_owned(self.c_future.as_mut_ptr(), flux_future_destroy)
                .expect("The cloned Future has a NULL C pointer, which should never happen"),
            _phantom: PhantomData,
            _init_cb: self._init_cb.clone(),
        }
    }
}

unsafe impl<'a> BorrowFluxPtr for FluxFuture<'a> {
    type CType = flux_future_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_future: FluxPtr::create_borrowed(ptr, flux_future_destroy)?,
            _phantom: PhantomData,
            _init_cb: None,
        })
    }
}

unsafe impl FromFluxPtr for FluxFuture<'static> {
    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_future: FluxPtr::create_owned(ptr, flux_future_destroy)?,
            _phantom: PhantomData,
            _init_cb: None,
        })
    }
}

unsafe impl Send for FluxFuture<'_> {}
unsafe impl Sync for FluxFuture<'_> {}

default_impl_as_flux_ptr!(FluxFuture<'static>, flux_future_t, c_future);

fn push_to_collective_future(
    coll_future: &mut FluxFuture,
    name: &str,
    child_future: FluxFuture,
) -> Result<()> {
    let c_name = CString::new(name)?;
    // Increment the ref count of the future since it will be auto-decremented by drop at the
    // end of this function.
    unsafe { flux_future_incref(child_future.c_future.as_mut_ptr()) };
    flux_try!(empty_ok
        flux_future_push(
            coll_future.c_future.as_mut_ptr(),
            c_name.as_ptr(),
            child_future.c_future.as_mut_ptr(),
        )
    )
}

pub fn create_wait_all_future(futures: HashMap<String, FluxFuture>) -> Result<FluxFuture> {
    let raw_coll_future = flux_try!(flux_future_wait_all_create())?;
    let mut coll_future = unsafe { FluxFuture::from_ptr(raw_coll_future)? };
    for (name, child_future) in futures.into_iter() {
        push_to_collective_future(&mut coll_future, &name, child_future)?;
    }
    Ok(coll_future)
}

pub fn create_wait_any_future(futures: HashMap<String, FluxFuture>) -> Result<FluxFuture> {
    let raw_coll_future = flux_try!(flux_future_wait_any_create())?;
    let mut coll_future = unsafe { FluxFuture::from_ptr(raw_coll_future)? };
    for (name, child_future) in futures.into_iter() {
        push_to_collective_future(&mut coll_future, &name, child_future)?;
    }
    Ok(coll_future)
}
