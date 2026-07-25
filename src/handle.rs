use std::any::Any;
use std::ffi::{c_char, c_int, c_void, CStr, CString};

use bitflags::bitflags;
use flux_sys::core::{
    flux_attr_get, flux_attr_set, flux_aux_get, flux_aux_set, flux_clone, flux_close,
    flux_comms_error_set, flux_get_rank, flux_get_reactor, flux_get_size, flux_incref, flux_log,
    flux_log_set_appname, flux_log_set_procid, flux_match, flux_msg_t, flux_open, flux_pollevents,
    flux_pollfd, flux_reconnect, flux_recv, flux_requeue, flux_respond, flux_respond_error,
    flux_respond_raw, flux_send_new, flux_set_reactor, flux_t, FLUX_O_CLONE, FLUX_O_MATCHDEBUG,
    FLUX_O_NONBLOCK, FLUX_O_RPCTRACK, FLUX_O_TEST_NOSUB, FLUX_O_TRACE, FLUX_POLLERR, FLUX_POLLIN,
    FLUX_POLLOUT,
};
use serde::Serialize;
use serde_json::Value;

use crate::error::{check_ptr, check_rc, FluxError, Result};
use crate::flux_ptr_management::{
    default_impl_as_flux_ptr, BorrowFluxPtr, FluxPtr, FromFluxPtr, FromFluxPtrNoArgs,
};
use crate::msg::{Message, MessageMatch};
use crate::reactor::Reactor;
use crate::request::Request;
use crate::rpc::{Rpc, RpcFlags, RpcNodeId};
use crate::uri::BaseUri;

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HandleFlags: u32 {
        const NONE = 0;
        const TRACE = FLUX_O_TRACE;
        const CLONE = FLUX_O_CLONE;
        const MATCHDEBUG = FLUX_O_MATCHDEBUG;
        const NONBLOCK = FLUX_O_NONBLOCK;
        const TEST_NOSUB = FLUX_O_TEST_NOSUB;
        const RPCTRACK = FLUX_O_RPCTRACK;
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    Emergency = libc::LOG_EMERG,
    Alert = libc::LOG_ALERT,
    Critical = libc::LOG_CRIT,
    Error = libc::LOG_ERR,
    Warning = libc::LOG_WARNING,
    Notice = libc::LOG_NOTICE,
    Info = libc::LOG_INFO,
    Debug = libc::LOG_DEBUG,
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PollEvents: u32 {
        const NONE = 0;
        const POLLIN = FLUX_POLLIN;
        const POLLOUT = FLUX_POLLOUT;
        const POLLERR = FLUX_POLLERR;
    }
}

#[macro_export]
macro_rules! flux_log {
    ($handle:expr, $level:expr, $($arg:tt)*) => {
        $handle.log($level, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! flux_log_emergency {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Emergency, $($arg)*)
    };
}

#[macro_export]
macro_rules! flux_log_alert {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Alert, $($arg)*)
    };
}

#[macro_export]
macro_rules! flux_log_critical {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Critical, $($arg)*)
    };
}

#[macro_export]
macro_rules! flux_log_error {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Error, $($arg)*)
    };
}

#[macro_export]
macro_rules! flux_log_warning {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Warning, $($arg)*)
    };
}

#[macro_export]
macro_rules! flux_log_notice {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Notice, $($arg)*)
    };
}

#[macro_export]
macro_rules! flux_log_info {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Info, $($arg)*)
    };
}

#[macro_export]
macro_rules! flux_log_debug {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Debug, $($arg)*)
    };
}

pub(crate) struct AuxThinPtrWrapper {
    pub(crate) inner: Box<dyn Any + Send + Sync>,
}

pub struct FluxHandle {
    pub(crate) h: FluxPtr<flux_t>,
    comm_error_handler_cb: Option<Box<dyn FnMut(FluxHandle) -> i32>>,
}

impl FluxHandle {
    // TODO remaining methods: flux_opt_set, flux_opt_get, flux_get_conf, flux_set_conf_new, flux_flags*, flux_send, flux_recv, flux_requeue

    pub fn new(uri: &BaseUri, flags: HandleFlags) -> Result<Self> {
        Self::new_from_str_uri(uri.uri.as_str(), flags)
    }

    pub fn new_from_str_uri(uri: &str, flags: HandleFlags) -> Result<Self> {
        let c_uri = CString::new(uri.to_owned())?;
        let flux_handle = unsafe { flux_open(c_uri.as_ptr(), flags.bits() as i32) };
        let flux_handle_rs = FluxPtr::create_owned(flux_handle, flux_close)?;
        Ok(Self {
            h: flux_handle_rs,
            comm_error_handler_cb: None,
        })
    }

    pub fn try_clone(&self) -> Result<Self> {
        Self::try_from(self)
    }

    pub fn reconnect(&mut self) -> Result<()> {
        let rc = unsafe { flux_reconnect(self.h.as_mut_ptr()) };
        check_rc(rc)
    }

    pub fn get_rank(&self) -> Result<u32> {
        let mut rank: u32 = 0;
        let rc = unsafe { flux_get_rank(self.h.as_mut_ptr(), &mut rank as *mut u32) };
        check_rc(rc)?;
        Ok(rank)
    }

    pub fn get_size(&self) -> Result<usize> {
        let mut size: u32 = 0;
        let rc = unsafe { flux_get_size(self.h.as_mut_ptr(), &mut size as *mut u32) };
        check_rc(rc)?;
        Ok(size as usize)
    }

    pub fn get_attr(&self, name: &str) -> Result<String> {
        let c_name = CString::new(name)?;
        let attr_ptr: *const c_char =
            unsafe { flux_attr_get(self.h.as_mut_ptr(), c_name.as_ptr()) };
        check_ptr(attr_ptr as *mut c_char)?;
        let c_attr_str = unsafe { CStr::from_ptr(attr_ptr) };
        let rust_attr_str = c_attr_str.to_str().map(|s| s.to_owned());
        unsafe {
            libc::free(attr_ptr as *mut c_void);
        }
        let rust_attr = rust_attr_str?;
        Ok(rust_attr)
    }

    pub fn set_attr(&mut self, name: &str, val: &str) -> Result<()> {
        let c_name = CString::new(name)?;
        let c_val = CString::new(val)?;
        let rc = unsafe { flux_attr_set(self.h.as_mut_ptr(), c_name.as_ptr(), c_val.as_ptr()) };
        check_rc(rc)
    }

    pub fn unset_attr(&mut self, name: &str) -> Result<()> {
        let c_name = CString::new(name)?;
        let rc = unsafe { flux_attr_set(self.h.as_mut_ptr(), c_name.as_ptr(), std::ptr::null()) };
        check_rc(rc)
    }

    pub fn set_aux<T: Send + Sync + 'static>(&mut self, name: &str, data: T) -> Result<()> {
        // Convert the name to a C String
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
                self.h.as_mut_ptr(),
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
        let raw_ptr = self.get_aux_raw(name)?;
        let wrapper = unsafe { &*(raw_ptr as *const AuxThinPtrWrapper) };
        match wrapper.inner.downcast_ref::<T>() {
            Some(typed_ref) => Ok(typed_ref),
            None => Err(FluxError::Logic(format!(
                "Type mismatch for aux key '{}'",
                name
            ))),
        }
    }

    pub fn get_aux_raw(&self, name: &str) -> Result<*mut c_void> {
        let c_name = CString::new(name)?;
        let raw_ptr = unsafe { flux_aux_get(self.h.as_mut_ptr(), c_name.as_ptr()) };
        check_ptr(raw_ptr)
    }

    pub fn set_comms_error_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: FnMut(FluxHandle) -> i32 + Send + 'static,
    {
        // Store the handler on the heap in FluxHandle to ensure it doesn't get deallocated
        self.comm_error_handler_cb = Some(Box::new(handler));
        // Get a mutable reference to the Box itself. By getting a reference to the Box instead of the closure,
        // we ensure we can get a thin pointer to pass to C.
        let box_ref = self
            .comm_error_handler_cb
            .as_mut()
            .ok_or(FluxError::Logic(String::from(
                "Cannot unpack the callback for interacting with Flux's C API",
            )))?;
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
            let handle = match unsafe { FluxHandle::from_ptr(h) } {
                Ok(handle) => handle,
                Err(err) => {
                    err.set_errno(None);
                    return -1;
                }
            };
            // Inovke the user closure
            closure(handle)
        }

        // Set the error handler
        unsafe {
            flux_comms_error_set(self.h.as_mut_ptr(), Some(trampoline), arg_ptr);
        }

        Ok(())
    }

    pub fn get_reactor(&self) -> Result<Reactor> {
        let reactor_ptr = unsafe { flux_get_reactor(self.h.as_mut_ptr()) };
        check_ptr(reactor_ptr)?;
        unsafe { Reactor::from_ptr(reactor_ptr) }
    }

    pub fn set_reactor(&mut self, reactor: &Reactor) -> Result<()> {
        let rc = unsafe { flux_set_reactor(self.h.as_mut_ptr(), reactor.c_reactor.as_mut_ptr()) };
        check_rc(rc)
    }

    pub fn send(&self, msg: Message, flags: HandleFlags) -> Result<()> {
        let mut raw_msg = msg.c_msg.as_mut_ptr();
        let rc = unsafe {
            flux_send_new(
                self.h.as_mut_ptr(),
                &mut raw_msg as *mut *mut flux_msg_t,
                flags.bits() as i32,
            )
        };
        // flux_send_new only frees the message if it succeeds.
        // To work around this, only assume ownership of the raw pointer
        // (which prevents freeing in Drop) if flux_send_new succeeds.
        // Otherwise, preserve ownership so that Drop frees the flux_msg_t pointer
        if rc == 0 {
            let _ = msg.c_msg.into_raw();
        }
        check_rc(rc)
    }

    pub fn recv(&self, msg_match: MessageMatch, flags: HandleFlags) -> Result<Message> {
        let c_match: flux_match = (&msg_match).into();
        let msg_ptr = unsafe { flux_recv(self.h.as_mut_ptr(), c_match, flags.bits() as i32) };
        check_ptr(msg_ptr)?;
        unsafe { Message::from_ptr(msg_ptr) }
    }

    pub fn requeue(&self, msg: Message, flags: HandleFlags) -> Result<()> {
        let rc = unsafe {
            flux_requeue(
                self.h.as_mut_ptr(),
                msg.c_msg.as_mut_ptr(),
                flags.bits() as i32,
            )
        };
        check_rc(rc)
    }

    pub fn get_pollfd(&self) -> Result<i32> {
        let fd = unsafe { flux_pollfd(self.h.as_mut_ptr()) };
        check_rc(fd)?;
        Ok(fd)
    }

    pub fn get_pollevents(&self) -> Result<PollEvents> {
        let bitmask = unsafe { flux_pollevents(self.h.as_mut_ptr()) };
        check_rc(bitmask)?;
        Ok(PollEvents::from_bits_retain(bitmask as u32))
    }

    pub fn set_log_appname(&mut self, appname: &str) -> Result<()> {
        let c_appname = CString::new(appname)?;
        unsafe {
            flux_log_set_appname(self.h.as_mut_ptr(), c_appname.as_ptr());
        }
        Ok(())
    }

    pub fn set_log_procid(&mut self, procid: &str) -> Result<()> {
        let c_procid = CString::new(procid)?;
        unsafe {
            flux_log_set_procid(self.h.as_mut_ptr(), c_procid.as_ptr());
        }
        Ok(())
    }

    pub fn log(&self, level: LogLevel, msg: &str) -> Result<()> {
        let c_fmt_str = CStr::from_bytes_with_nul(b"%s\0")?;
        let c_msg = CString::new(msg)?;
        let rc = unsafe {
            flux_log(
                self.h.as_mut_ptr(),
                level as i32,
                c_fmt_str.as_ptr(),
                c_msg.as_ptr(),
            )
        };
        check_rc(rc)
    }

    pub fn send_rpc(
        &self,
        topic: &str,
        data: &[u8],
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Rpc<'_>> {
        Rpc::create(self, topic, data, nodeid, flags)
    }

    pub fn send_rpc_json(
        &self,
        topic: &str,
        data: &Value,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Rpc<'_>> {
        Rpc::create_json(self, topic, data, nodeid, flags)
    }

    pub fn send_rpc_serializable<T: Serialize>(
        &self,
        topic: &str,
        data: &T,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Rpc<'_>> {
        Rpc::create_serializable(self, topic, data, nodeid, flags)
    }

    pub fn send_rpc_message(
        &self,
        msg: &Message,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Rpc<'_>> {
        Rpc::create_message(self, msg, nodeid, flags)
    }

    pub fn respond(&self, request: &Request, data: Option<&[u8]>) -> Result<()> {
        let (data_ptr, data_len) = match data {
            Some(data_slice) => (data_slice.as_ptr(), data_slice.len()),
            None => (std::ptr::null(), 0),
        };
        let rc = unsafe {
            flux_respond_raw(
                self.h.as_mut_ptr(),
                request.msg.c_msg.as_mut_ptr(),
                data_ptr as *const c_void,
                data_len as i32,
            )
        };
        check_rc(rc)
    }

    pub fn respond_json(&self, request: &Request, data: Option<&Value>) -> Result<()> {
        let data_vec_opt = data
            .map(|data_value| serde_json::to_vec(data_value))
            .transpose()?;
        let data_slice_opt = data_vec_opt.as_ref().map(|v| v.as_slice());
        self.respond(request, data_slice_opt)
    }

    pub fn respond_serializable<T: Serialize>(
        &self,
        request: &Request,
        data: Option<&T>,
    ) -> Result<()> {
        let data_vec_opt = data
            .map(|data_value| serde_json::to_vec(data_value))
            .transpose()?;
        let data_slice_opt = data_vec_opt.as_ref().map(|v| v.as_slice());
        self.respond(request, data_slice_opt)
    }

    pub fn respond_string(&self, request: &Request, data: Option<&str>) -> Result<()> {
        let c_data = data.map(|s| CString::new(s)).transpose()?;
        let c_data_ptr = c_data.as_ref().map(|cs| cs.as_ptr());
        let rc = unsafe {
            flux_respond(
                self.h.as_mut_ptr(),
                request.msg.c_msg.as_mut_ptr(),
                c_data_ptr.unwrap_or(std::ptr::null()),
            )
        };
        check_rc(rc)
    }

    pub fn respond_error(
        &self,
        request: &Request,
        error: std::io::Error,
        errmsg: Option<&str>,
    ) -> Result<()> {
        let errnum = error.raw_os_error().ok_or(FluxError::Logic(String::from(
            "Passed a std::io::Error to 'respond_error' that does not represent a OS error (i.e., errno). Consider using 'respond_raw_error'"
        )))?;
        self.respond_raw_error(request, errnum, errmsg)
    }

    pub fn respond_raw_error(
        &self,
        request: &Request,
        errnum: i32,
        errmsg: Option<&str>,
    ) -> Result<()> {
        let c_errmsg = errmsg.map(|s| CString::new(s)).transpose()?;
        let c_errmsg_ptr = c_errmsg.as_ref().map(|cs| cs.as_ptr());
        let rc = unsafe {
            flux_respond_error(
                self.h.as_mut_ptr(),
                request.msg.c_msg.as_mut_ptr(),
                errnum,
                c_errmsg_ptr.unwrap_or(std::ptr::null()),
            )
        };
        check_rc(rc)
    }
}

impl Clone for FluxHandle {
    /// Clone the FluxHandle.
    ///
    /// Under the hood, this method simply creates a new FluxHandle object
    /// with the same underlying `*mut flux_t` pointer. It also calls `flux_incref`
    /// to increment Flux's internal reference counter.
    fn clone(&self) -> Self {
        unsafe {
            flux_incref(self.h.as_mut_ptr());
        }
        Self {
            h: FluxPtr::create_owned(self.h.as_mut_ptr(), flux_close)
                .expect("flux_incref should never result in the flux_t pointer becoming NULL"),
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
        let cloned_flux_handle = unsafe { flux_clone(value.h.as_mut_ptr()) };
        check_ptr(cloned_flux_handle)?;
        Ok(FluxHandle {
            h: FluxPtr::create_owned(cloned_flux_handle, flux_close)?,
            comm_error_handler_cb: None,
        })
    }
}

unsafe impl BorrowFluxPtr for FluxHandle {
    type CType = flux_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            h: FluxPtr::create_borrowed(ptr, flux_close)?,
            comm_error_handler_cb: None,
        })
    }
}

unsafe impl FromFluxPtr for FluxHandle {
    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            h: FluxPtr::create_owned(ptr, flux_close)?,
            comm_error_handler_cb: None,
        })
    }
}

default_impl_as_flux_ptr!(FluxHandle, flux_t, h);

unsafe impl Send for FluxHandle {}
