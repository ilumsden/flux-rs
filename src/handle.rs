use std::any::Any;
use std::ffi::{c_char, c_int, c_void, CStr, CString};

use bitflags::bitflags;
use flux_sys::core::{
    flux_attr_get, flux_attr_set, flux_aux_get, flux_aux_set, flux_clone, flux_close,
    flux_comms_error_set, flux_get_rank, flux_get_reactor, flux_get_size, flux_incref, flux_open,
    flux_reconnect, flux_set_reactor, flux_t, FLUX_O_CLONE, FLUX_O_MATCHDEBUG, FLUX_O_NONBLOCK,
    FLUX_O_RPCTRACK, FLUX_O_TEST_NOSUB, FLUX_O_TRACE,
};

use crate::error::{FluxError, Result};
use crate::reactor::Reactor;

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct OpenFlags: u32 {
        const NONE = 0;
        const TRACE = FLUX_O_TRACE;
        const CLONE = FLUX_O_CLONE;
        const MATCHDEBUG = FLUX_O_MATCHDEBUG;
        const NONBLOCK = FLUX_O_NONBLOCK;
        const TEST_NOSUB = FLUX_O_TEST_NOSUB;
        const RPCTRACK = FLUX_O_RPCTRACK;
    }
}

struct AuxThinPtrWrapper {
    inner: Box<dyn Any + Send + Sync>,
}

pub struct FluxHandle {
    h: *mut flux_t,
    comm_error_handler_cb: Option<Box<dyn FnMut(FluxHandle) -> i32>>,
}

impl FluxHandle {
    // TODO remaining methods: flux_opt_set, flux_opt_get, flux_get_conf, flux_set_conf_new, flux_flags*, flux_send, flux_recv, flux_requeue

    pub fn new(uri: &str, flags: OpenFlags) -> Result<Self> {
        let c_uri = CString::new(uri)?;
        let flux_handle = unsafe { flux_open(c_uri.as_ptr(), flags.bits() as i32) };
        Ok(Self {
            h: flux_handle,
            comm_error_handler_cb: None,
        })
    }

    pub fn try_clone(&self) -> Result<Self> {
        Self::try_from(self)
    }

    pub fn close(&mut self) {
        if !self.h.is_null() {
            unsafe {
                flux_close(self.h);
            }
            self.h = std::ptr::null_mut() as *mut flux_t;
        }
    }

    pub fn reconnect(&mut self) -> Result<()> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot reconnect with a NULL handle",
            )));
        }
        let rc = unsafe { flux_reconnect(self.h) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn get_rank(&self) -> Result<u32> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get rank with a NULL handle",
            )));
        }
        let mut rank: u32 = 0;
        let rc = unsafe { flux_get_rank(self.h, &mut rank as *mut u32) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(rank)
    }

    pub fn get_size(&self) -> Result<usize> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get size with a NULL handle",
            )));
        }
        let mut size: u32 = 0;
        let rc = unsafe { flux_get_size(self.h, &mut size as *mut u32) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(size as usize)
    }

    pub fn get_attr(&self, name: &str) -> Result<String> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get attribute with a NULL handle",
            )));
        }
        let c_name = CString::new(name)?;
        let attr_ptr: *const c_char = unsafe { flux_attr_get(self.h, c_name.as_ptr()) };
        if attr_ptr.is_null() {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        let c_attr_str = unsafe { CStr::from_ptr(attr_ptr) };
        let rust_attr_str = c_attr_str.to_str().map(|s| s.to_owned());
        unsafe {
            libc::free(attr_ptr as *mut c_void);
        }
        let rust_attr = rust_attr_str?;
        Ok(rust_attr)
    }

    pub fn set_attr(&mut self, name: &str, val: &str) -> Result<()> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set attribute with a NULL handle",
            )));
        }
        let c_name = CString::new(name)?;
        let c_val = CString::new(val)?;
        let rc = unsafe { flux_attr_set(self.h, c_name.as_ptr(), c_val.as_ptr()) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn unset_attr(&mut self, name: &str) -> Result<()> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot unset attribute with a NULL handle",
            )));
        }
        let c_name = CString::new(name)?;
        let rc = unsafe { flux_attr_set(self.h, c_name.as_ptr(), std::ptr::null()) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn set_aux<T: Send + Sync + 'static>(&mut self, name: &str, data: T) -> Result<()> {
        // Check if the handle is NULL
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set aux with a NULL handle",
            )));
        }
        // Conver the name to a C String
        let c_name = CString::new(name)?;
        // Put the data on the heap, and wrap in an AuxThinPtrWrapper object to ensure
        // we have a thin pointer to pass to C.
        let wrapper = Box::new(AuxThinPtrWrapper {
            inner: Box::new(data),
        });
        let raw_data_ptr = Box::into_raw(wrapper) as *mut c_void;

        // Define the function for freeing the Boxed data
        extern "C" fn destroy_aux_trampoline(ptr: *mut c_void) {
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr as *mut AuxThinPtrWrapper);
                }
            }
        }

        // Set the aux
        let rc = unsafe {
            flux_aux_set(
                self.h,
                c_name.as_ptr(),
                raw_data_ptr,
                Some(destroy_aux_trampoline),
            )
        };
        // Handle failure
        if rc == -1 {
            let _ = unsafe { Box::from_raw(raw_data_ptr as *mut AuxThinPtrWrapper) };
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn get_aux<T: 'static>(&self, name: &str) -> Result<&T> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get aux with a NULL handle",
            )));
        }
        let c_name = CString::new(name)?;
        let raw_ptr = unsafe { flux_aux_get(self.h, c_name.as_ptr()) };
        if raw_ptr.is_null() {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        let wrapper = unsafe { &*(raw_ptr as *const AuxThinPtrWrapper) };
        match wrapper.inner.downcast_ref::<T>() {
            Some(typed_ref) => Ok(typed_ref),
            None => Err(FluxError::Logic(format!(
                "Type mismatch for aux key '{}'",
                name
            ))),
        }
    }

    pub fn set_comms_error_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: FnMut(FluxHandle) -> i32 + Send + 'static,
    {
        // Ensure the flux_t is not NULL
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set communication error handler with a NULL handle",
            )));
        }
        // Store the handler on the heap in FluxHandle to ensure it doesn't get deallocated
        self.comm_error_handler_cb = Some(Box::new(handler));
        // Get a mutable reference to the Box itself. By getting a reference to the Box instead of the closure,
        // we ensure we can get a thin pointer to pass to C.
        let box_ref = self.comm_error_handler_cb.as_mut().unwrap();
        // Get a thin pointer to the closure.
        let arg_ptr = box_ref as *mut Box<dyn FnMut(FluxHandle) -> i32> as *mut c_void;

        // Actual C callback function
        extern "C" fn trampoline(h: *mut flux_t, arg: *mut c_void) -> c_int {
            // Cast arg back to the real Rust callback
            let closure = unsafe { &mut *(arg as *mut Box<dyn FnMut(FluxHandle) -> i32>) };
            // Create a FluxHandle that will not call flux_close on drop to pass to the closure
            // We call flux_incref here so that the destructor of FluxHandle does not reduce the
            // reference count to 0.
            unsafe {
                flux_incref(h);
            }
            let handle = FluxHandle::from(h);
            // Inovke the user closure
            closure(handle)
        }

        // Set the error handler
        unsafe {
            flux_comms_error_set(self.h, Some(trampoline), arg_ptr);
        }

        Ok(())
    }

    pub fn get_reactor(&self) -> Result<Reactor> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get reactor from a NULL handle",
            )));
        }
        let reactor_ptr = unsafe { flux_get_reactor(self.h) };
        if reactor_ptr.is_null() {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(Reactor::from(reactor_ptr))
    }

    pub fn set_reactor(&mut self, reactor: &Reactor) -> Result<()> {
        if self.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set reactor with a NULL handle",
            )));
        }
        if reactor.c_reactor.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set reactor with a NULL reactor",
            )));
        }
        let rc = unsafe { flux_set_reactor(self.h, reactor.c_reactor) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

impl Drop for FluxHandle {
    fn drop(&mut self) {
        if !self.h.is_null() {
            unsafe {
                flux_close(self.h);
            }
        }
    }
}

impl From<*mut flux_t> for FluxHandle {
    fn from(value: *mut flux_t) -> Self {
        FluxHandle {
            h: value,
            comm_error_handler_cb: None,
        }
    }
}

impl Clone for FluxHandle {
    /// Clone the FluxHandle.
    ///
    /// Under the hood, this method simply creates a new FluxHandle object
    /// with the same underlying `*mut flux_t` pointer. It also calls `flux_incref`
    /// to increment Flux's internal reference counter.
    fn clone(&self) -> Self {
        if !self.h.is_null() {
            unsafe {
                flux_incref(self.h);
            }
        }
        Self {
            h: self.h,
            comm_error_handler_cb: None,
        }
    }
}

impl TryFrom<&FluxHandle> for FluxHandle {
    type Error = FluxError;

    /// Create a new FluxHandle from a reference to another.
    ///
    /// Unlike `FluxHandle.clone`, this method calls `flux_clone` to get a new
    /// underlying `*mut flux_t` pointer.
    fn try_from(value: &FluxHandle) -> Result<FluxHandle> {
        if value.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot clone a FluxHandle when the underlying pointer is NULL",
            )));
        }
        let cloned_flux_handle = unsafe { flux_clone(value.h) };
        if cloned_flux_handle.is_null() {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(FluxHandle {
            h: cloned_flux_handle,
            comm_error_handler_cb: None,
        })
    }
}
