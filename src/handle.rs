use std::any::Any;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::Arc;

use bitflags::bitflags;
use flux_sys::core::{
    FLUX_O_CLONE, FLUX_O_MATCHDEBUG, FLUX_O_NONBLOCK, FLUX_O_RPCTRACK, FLUX_O_TEST_NOSUB,
    FLUX_O_TRACE, FLUX_POLLERR, FLUX_POLLIN, FLUX_POLLOUT, flux_attr_get, flux_attr_set,
    flux_aux_get, flux_aux_set, flux_clone, flux_close, flux_comms_error_set, flux_get_rank,
    flux_get_reactor, flux_get_size, flux_incref, flux_log, flux_log_set_appname,
    flux_log_set_procid, flux_match, flux_msg_t, flux_open, flux_pollevents, flux_pollfd,
    flux_reactor_t, flux_reconnect, flux_recv, flux_requeue, flux_respond, flux_respond_error,
    flux_respond_raw, flux_send, flux_set_reactor, flux_t,
};
use serde::Serialize;
use serde_json::Value;

use crate::error::{FluxError, Result, flux_try};
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, BorrowFluxPtrNoArgs, Borrowed, FluxPtr, FromFluxPtr,
    FromFluxPtrNoArgs, IntoFluxPtr, Owned, PossiblyDroppablePtr, define_as_flux_ptr_body,
    define_into_flux_ptr_body,
};
use crate::msg::{Message, MessageMatch};
use crate::reactor::{BorrowedReactor, OwnedReactor, Reactor};
use crate::request::Request;
use crate::rpc::{Rpc, RpcFlags, RpcNodeId};
use crate::uri::BaseUri;

bitflags! {
    /// A `bitflags` struct for flags that can be passed to the Flux handle.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HandleFlags: u32 {
        /// No flags set.
        const NONE = 0;
        /// Dump message trace to stderr.
        const TRACE = FLUX_O_TRACE;
        /// Clone the handle.
        ///
        /// This should not be used directly. To use this flag, use FluxHandle::try_clone instead.
        const CLONE = FLUX_O_CLONE;
        /// Print diagnostic to stderr when matchtags are leaked.
        const MATCHDEBUG = FLUX_O_MATCHDEBUG;
        /// Do not block underlying calls to `flux_send` and `flux_recv`.
        const NONBLOCK = FLUX_O_NONBLOCK;
        /// Make event subscription and unsubscription no-ops.
        const TEST_NOSUB = FLUX_O_TEST_NOSUB;
        /// Track pending RPCs so they can receive automatic `ECONNRESET` failures if the
        /// broker connection is re-established with FluxHandle::reconnect.
        const RPCTRACK = FLUX_O_RPCTRACK;
    }
}

/// An enum for Flux logging levels.
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
    /// A `bitflags` struct for polling events.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PollEvents: u32 {
        /// No events.
        const NONE = 0;
        /// Ready for reading.
        const POLLIN = FLUX_POLLIN;
        /// Ready for writing.
        const POLLOUT = FLUX_POLLOUT;
        /// An error has occured.
        const POLLERR = FLUX_POLLERR;
    }
}

/// A macro for writing a log to the provided handle with the provided log level.
///
/// This macro is a wrapper around FluxHandle::log that allows for message formatting
/// similar to Rust's `format!()` macro.
#[macro_export]
macro_rules! flux_log {
    ($handle:expr, $level:expr, $($arg:tt)*) => {{
        let _ = $handle.log($level, &format!($($arg)*));
    }};
}

/// A macro for writing emergency logs to the provided handle.
///
/// This macro is a wrapper around `flux_log!()` that sets the log level to LogLevel::Emergency.
#[macro_export]
macro_rules! flux_log_emergency {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Emergency, $($arg)*)
    };
}

/// A macro for writing alert logs to the provided handle.
///
/// This macro is a wrapper around `flux_log!()` that sets the log level to LogLevel::Alert.
#[macro_export]
macro_rules! flux_log_alert {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Alert, $($arg)*)
    };
}

/// A macro for writing critical logs to the provided handle.
///
/// This macro is a wrapper around `flux_log!()` that sets the log level to LogLevel::Critical.
#[macro_export]
macro_rules! flux_log_critical {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Critical, $($arg)*)
    };
}

/// A macro for writing error logs to the provided handle.
///
/// This macro is a wrapper around `flux_log!()` that sets the log level to LogLevel::Error.
#[macro_export]
macro_rules! flux_log_error {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Error, $($arg)*)
    };
}

/// A macro for writing warning logs to the provided handle.
///
/// This macro is a wrapper around `flux_log!()` that sets the log level to LogLevel::Warning.
#[macro_export]
macro_rules! flux_log_warning {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Warning, $($arg)*)
    };
}

/// A macro for writing notice logs to the provided handle.
///
/// This macro is a wrapper around `flux_log!()` that sets the log level to LogLevel::Notice.
#[macro_export]
macro_rules! flux_log_notice {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Notice, $($arg)*)
    };
}

/// A macro for writing info logs to the provided handle.
///
/// This macro is a wrapper around `flux_log!()` that sets the log level to LogLevel::Info.
#[macro_export]
macro_rules! flux_log_info {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Info, $($arg)*)
    };
}

/// A macro for writing debug logs to the provided handle.
///
/// This macro is a wrapper around `flux_log!()` that sets the log level to LogLevel::Debug.
#[macro_export]
macro_rules! flux_log_debug {
    ($handle:expr, $($arg:tt)*) => {
        $crate::flux_log!($handle, $crate::handle::LogLevel::Debug, $($arg)*)
    };
}

/// A thin wrapper to enable getting thin pointers for `Box<dyn Any>`.
pub(crate) struct AuxThinPtrWrapper {
    pub(crate) inner: Box<dyn Any + Send + Sync>,
}

pub(crate) type CommErrHandler = Box<dyn Fn(OwnedFluxHandle) -> i32 + Send + Sync>;

/// A handle to a Flux message broker.
pub struct FluxHandle<State: PossiblyDroppablePtr<flux_t> = Owned<flux_t>> {
    /// The underlying `flux_t` C pointer.
    pub(crate) h: FluxPtr<flux_t, State>,
    /// A field to store the user provided comm error handler to make sure it
    /// does not get deallocated too early.
    comm_error_handler_cb: Option<Arc<CommErrHandler>>,
    curr_set_reactor: Option<OwnedReactor>,
}

pub type OwnedFluxHandle = FluxHandle<Owned<flux_t>>;
pub type BorrowedFluxHandle<'a> = FluxHandle<Borrowed<'a, flux_t>>;

impl OwnedFluxHandle {
    /// Create a handle for the Flux broker described by the URI object
    pub fn new(uri: &BaseUri, flags: HandleFlags) -> Result<Self> {
        Self::new_from_str_uri(uri.uri.as_str(), flags)
    }

    /// Create a handle for the Flux broker described by the URI string.
    pub fn new_from_str_uri(uri: &str, flags: HandleFlags) -> Result<Self> {
        // Treat an empty string URI as the equivalent of a NULL URI in the C API
        let flux_handle = if uri.is_empty() {
            unsafe { flux_open(std::ptr::null(), flags.bits() as _) }
        } else {
            let c_uri = CString::new(uri.to_owned())?;
            unsafe { flux_open(c_uri.as_ptr(), flags.bits() as _) }
        };
        let flux_handle_rs = FluxPtr::create_owned(flux_handle, flux_close)?;
        Ok(Self {
            h: flux_handle_rs,
            comm_error_handler_cb: None,
            curr_set_reactor: None,
        })
    }
}

impl<State: PossiblyDroppablePtr<flux_t>> FluxHandle<State> {
    // TODO remaining methods: flux_opt_set, flux_opt_get, flux_get_conf, flux_set_conf_new, flux_flags*

    /// Clone the Flux handle.
    ///
    /// Under the hood, this method calls the `flux_clone` C function.
    /// This method is equivalent to FluxHandle's implementation of `TryFrom<&FluxHandle>`.
    /// If you want to get a new handle by simply incrementing the reference count, use
    /// `FluxHandle`'s implementation of `Clone` instead.
    pub fn try_clone(&self) -> Result<FluxHandle<Owned<flux_t>>> {
        FluxHandle::try_from(self)
    }

    /// Clone the Flux handle.
    ///
    /// Under the hood, this method simply increments the reference count on the current
    /// FluxHandle's `flux_t` pointer. It then creates a new `FluxHandle` object using the
    /// same `flux_t` pointer. If you want to get a new handle using `flux_clone`, use
    /// the `try_clone` method or the implementation of `TryFrom<&FluxHandle>` instead.
    pub fn to_owned(&self) -> Result<OwnedFluxHandle> {
        unsafe {
            flux_incref(self.h.as_mut_ptr());
        }
        Ok(FluxHandle {
            h: FluxPtr::create_owned(self.h.as_mut_ptr(), flux_close)?,
            comm_error_handler_cb: self.comm_error_handler_cb.clone(),
            curr_set_reactor: self.curr_set_reactor.clone(),
        })
    }

    /// Reconnect to the Flux broker.
    ///
    /// This is primarily meant to be used by the `set_comms_error_handler` method.
    pub fn reconnect(&mut self) -> Result<()> {
        flux_try!(flux_reconnect(self.h.as_mut_ptr())).map(|_| ())
    }

    /// Get the rank of the connected Flux broker in the Flux instance.
    pub fn get_rank(&self) -> Result<u32> {
        let mut rank: u32 = 0;
        flux_try!(flux_get_rank(self.h.as_mut_ptr(), &mut rank as *mut _))?;
        Ok(rank)
    }

    /// Get the number of brokers in the Flux instance.
    pub fn get_size(&self) -> Result<usize> {
        let mut size: u32 = 0;
        flux_try!(flux_get_size(self.h.as_mut_ptr(), &mut size as *mut _))?;
        Ok(size as usize)
    }

    /// Get the Flux broker attribute under the provided name.
    pub fn get_attr(&self, name: &str) -> Result<String> {
        let c_name = CString::new(name)?;
        let attr_ptr: *const c_char =
            flux_try!(flux_attr_get(self.h.as_mut_ptr(), c_name.as_ptr()))?;
        let c_attr_str = unsafe { CStr::from_ptr(attr_ptr) };
        let rust_attr_str = c_attr_str.to_str().map(|s| s.to_owned());
        let rust_attr = rust_attr_str?;
        Ok(rust_attr)
    }

    /// Set the Flux broker attribute for the provided name with the provided value.
    pub fn set_attr(&mut self, name: &str, val: &str) -> Result<()> {
        let c_name = CString::new(name)?;
        let c_val = CString::new(val)?;
        flux_try!(empty_ok flux_attr_set(
            self.h.as_mut_ptr(),
            c_name.as_ptr(),
            c_val.as_ptr()
        ))
    }

    /// Unset the Flux broker attribute for the provided name.
    pub fn unset_attr(&mut self, name: &str) -> Result<()> {
        let c_name = CString::new(name)?;
        flux_try!(empty_ok flux_attr_set(self.h.as_mut_ptr(), c_name.as_ptr(), std::ptr::null()))
    }

    /// Attach application-specific data to the Flux handle under the provided name.
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
            return Err(FluxError::System(
                "flux_aux_set",
                std::io::Error::last_os_error(),
            ));
        }
        Ok(())
    }

    /// Get application-specific data from the Flux handle under the provided name.
    ///
    /// This method requires that the generic type for this method matches the type of the data passed
    /// to `set_aux`. If it does not, an error will be returned.
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

    /// Get the underlying C pointer representing application-specific data from the Flux handle under
    /// the provided name.
    ///
    /// This method simply returns the raw C pointer obtained through `flux_aux_get`. It is intended for
    /// the unlikely scenario of obtaining application-specfiic data that was set by another programming
    /// language (e.g., C). If the data was set by Rust, use `get_aux` instead.
    pub fn get_aux_raw(&self, name: &str) -> Result<*mut c_void> {
        let c_name = CString::new(name)?;
        flux_try!(flux_aux_get(self.h.as_mut_ptr(), c_name.as_ptr()))
    }

    /// Configure a callback to be run internally by `libflux_core` if an error
    /// occurs when sending or receiving messages on this handle.
    pub fn set_comms_error_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(FluxHandle) -> i32 + Send + Sync + 'static,
    {
        let cb: CommErrHandler = Box::new(handler);
        let wrapped_handler = Arc::new(cb);

        self.comm_error_handler_cb = Some(wrapped_handler.clone());

        // Get a thin pointer to the closure.
        let arg_ptr = Arc::as_ptr(&wrapped_handler) as *mut c_void;

        // Actual C callback function
        extern "C" fn trampoline(h: *mut flux_t, arg: *mut c_void) -> c_int {
            // Cast arg back to the real Rust callback
            let closure = unsafe { &*(arg as *const CommErrHandler) };
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
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| closure(handle.clone())))
            {
                Ok(rc) => rc,
                Err(e) => {
                    let panic_msg = if let Some(s) = e.downcast_ref::<&str>() {
                        *s
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.as_str()
                    } else {
                        "UNKNOWN PANIC"
                    };
                    flux_log_error!(
                        handle,
                        "Panic occured in the comms error handler: {}",
                        panic_msg
                    );
                    -1
                }
            }
        }

        // Set the error handler
        unsafe {
            flux_comms_error_set(self.h.as_mut_ptr(), Some(trampoline), arg_ptr);
        }

        Ok(())
    }

    /// Get the Reactor object associated with this handle.
    ///
    /// If a Reactor was never set on this handle previously, this method will create a Reactor.
    /// The underlying C pointer stored in the Reactor object is either owned by the handle
    /// or by the Reactor that was previously passed to `set_reactor`.
    pub fn get_reactor<'r>(&'r self) -> Result<BorrowedReactor<'r>> {
        let reactor_ptr = flux_try!(flux_get_reactor(self.h.as_mut_ptr()))?;
        unsafe { Reactor::borrow_ptr(reactor_ptr) }
    }

    /// Set the Reactor object associated with this handle.
    ///
    /// This method does not assume any ownership over the Reactor. The reactor will only
    /// be valid in the handle for the lifetime of the `reactor` argument.
    pub fn set_reactor<ReactState: PossiblyDroppablePtr<flux_reactor_t>>(
        &mut self,
        reactor: &Reactor<ReactState>,
    ) -> Result<()> {
        flux_try!(empty_ok flux_set_reactor(self.h.as_mut_ptr(), reactor.c_reactor.as_mut_ptr()))?;
        self.curr_set_reactor = Some(reactor.to_owned()?);
        Ok(())
    }

    /// Send the provided message with the Flux broker associated with the handle.
    pub fn send<MsgState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        msg: Message<MsgState>,
        flags: HandleFlags,
    ) -> Result<()> {
        let raw_msg = msg.c_msg.as_mut_ptr();
        flux_try!(empty_ok flux_send(self.h.as_mut_ptr(), raw_msg as *const flux_msg_t, flags.bits() as _))
    }

    /// Receive a message using the Flux broker associated with the handle.
    pub fn recv(&self, msg_match: MessageMatch, flags: HandleFlags) -> Result<Message> {
        let c_match: flux_match = (&msg_match).into();
        let msg_ptr = flux_try!(flux_recv(self.h.as_mut_ptr(), c_match, flags.bits() as _))?;
        unsafe { Message::from_ptr(msg_ptr) }
    }

    /// Requeue the provided Message using the handle.
    pub fn requeue<MsgState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        msg: Message<MsgState>,
        flags: HandleFlags,
    ) -> Result<()> {
        flux_try!(empty_ok
            flux_requeue(
                self.h.as_mut_ptr(),
                msg.c_msg.as_mut_ptr(),
                flags.bits() as _,
            )
        )
    }

    /// Get a C-style file descriptor that becomes readable when the Flux handle needs attention.
    ///
    /// Signalling on this file descriptor is edge-triggered, which means that PollEvents::POLLIN is
    /// raised when the handle becomes ready for reading or writing, but is not re-raised if those
    /// conditions are still true when polling is re-entered.
    ///
    /// The file descriptor is created on the first call to `get_pollfd` or `get_pollevents`. It is
    /// used for signalling only, and it must not be read, written, or closed.
    pub fn get_pollfd(&self) -> Result<i32> {
        flux_try!(flux_pollfd(self.h.as_mut_ptr()))
    }

    /// Get a bitmask of poll events for the Flux handle.
    ///
    /// This method is normally called after a `POLLIN` event on `get_pollfd`. If there is any
    /// pending `POLLIN` event, this method clears that event.
    pub fn get_pollevents(&self) -> Result<PollEvents> {
        let bitmask = flux_try!(flux_pollevents(self.h.as_mut_ptr()))?;
        Ok(PollEvents::from_bits_retain(bitmask as _))
    }

    /// Set the application name for the Flux log.
    ///
    /// By default, the application name is initialized to the value of the `__progname` symbol.
    /// This method overrides that name.
    pub fn set_log_appname(&mut self, appname: &str) -> Result<()> {
        let c_appname = CString::new(appname)?;
        unsafe {
            flux_log_set_appname(self.h.as_mut_ptr(), c_appname.as_ptr());
        }
        Ok(())
    }

    /// Set the application's process ID for the Flux log.
    ///
    /// By default, the application process ID is initialized to the calling process's PID.
    /// This method overides that process ID.
    pub fn set_log_procid(&mut self, procid: &str) -> Result<()> {
        let c_procid = CString::new(procid)?;
        unsafe {
            flux_log_set_procid(self.h.as_mut_ptr(), c_procid.as_ptr());
        }
        Ok(())
    }

    /// Send log messages to the Flux broker connected to the handle.
    pub fn log(&self, level: LogLevel, msg: &str) -> Result<()> {
        let c_fmt_str = c"%s";
        let c_msg = CString::new(msg)?;
        flux_try!(empty_ok
            flux_log(
                self.h.as_mut_ptr(),
                level as _,
                c_fmt_str.as_ptr(),
                c_msg.as_ptr(),
            )
        )
    }

    /// Send an RPC request message with an arbitrary binary payload using the Flux handle.
    ///
    /// This method sends the request to the Flux service identified by the provided topic string
    /// and node ID.
    pub fn send_rpc<'a>(
        &'a self,
        topic: &str,
        data: &[u8],
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Rpc<'a, State>> {
        Rpc::create(self, topic, data, nodeid, flags)
    }

    /// Send an RPC request message with a `serde`-serialized JSON payload using the Flux handle.
    ///
    /// This method sends the request to the Flux service identified by the provided topic string
    /// and node ID.
    pub fn send_rpc_json<'a>(
        &'a self,
        topic: &str,
        data: &Value,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Rpc<'a, State>> {
        Rpc::create_json(self, topic, data, nodeid, flags)
    }

    /// Send an RPC request message with an arbitrary `serde`-serializable payload using the Flux handle.
    ///
    /// This method sends the request to the Flux service identified by the provided topic string
    /// and node ID.
    pub fn send_rpc_serializable<'a, T: Serialize>(
        &'a self,
        topic: &str,
        data: &T,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Rpc<'a, State>> {
        Rpc::create_serializable(self, topic, data, nodeid, flags)
    }

    /// Send an RPC request message with a Message payload using the Flux handle.
    ///
    /// This method sends the request to the Flux service identified by the provided topic string
    /// and node ID.
    pub fn send_rpc_message<'a>(
        &'a self,
        msg: &Message,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Rpc<'a, State>> {
        Rpc::create_message(self, msg, nodeid, flags)
    }

    /// Respond to the provided Request with an optional data payload consisting of arbitrary bytes.
    pub fn respond_raw<ReqState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        request: &Request<ReqState>,
        data: Option<&[u8]>,
    ) -> Result<()> {
        let (data_ptr, data_len) = match data {
            Some(data_slice) => (data_slice.as_ptr(), data_slice.len()),
            None => (std::ptr::null(), 0),
        };
        flux_try!(empty_ok
            flux_respond_raw(
                self.h.as_mut_ptr(),
                request.msg.c_msg.as_mut_ptr(),
                data_ptr as *const c_void,
                data_len as _,
            )
        )
    }

    pub fn respond<ReqState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        request: &Request<ReqState>,
        data: Option<&CStr>,
    ) -> Result<()> {
        let data_ptr = match data {
            Some(s) => s.as_ptr(),
            None => std::ptr::null(),
        };
        flux_try!(flux_respond(
            self.h.as_mut_ptr(),
            request.msg.c_msg.as_mut_ptr(),
            data_ptr
        ))
        .map(|_| ())
    }

    /// Respond to the provided Request with an optional data payload consisting of `serde`-serialized JSON data.
    pub fn respond_json<ReqState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        request: &Request<ReqState>,
        data: Option<&Value>,
    ) -> Result<()> {
        self.respond_serializable(request, data)
    }

    /// Respond to the provided Request with an optional data payload consisting of arbitrary `serde`-serializable data.
    pub fn respond_serializable<T: Serialize, ReqState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        request: &Request<ReqState>,
        data: Option<&T>,
    ) -> Result<()> {
        let data_cstring = data
            .map(serde_json::to_vec)
            .transpose()?
            .map(CString::new)
            .transpose()?;
        self.respond(request, data_cstring.as_deref())
    }

    /// Respond to the provided Request with an optional data payload consisting of a UTF-8 string.
    pub fn respond_string<ReqState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        request: &Request<ReqState>,
        data: Option<&str>,
    ) -> Result<()> {
        let c_data = data.map(CString::new).transpose()?;
        self.respond(request, c_data.as_deref())
    }

    /// Respond to the provided Request with an error code and optional error message.
    ///
    /// This method represents the error code with a `std::io::Error` object. To use a C-style
    /// `errno` value, use `respond_raw_error` instead.
    pub fn respond_error<ReqState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        request: &Request<ReqState>,
        error: std::io::Error,
        errmsg: Option<&str>,
    ) -> Result<()> {
        let errnum = error.raw_os_error().ok_or(FluxError::Logic(String::from(
            "Passed a std::io::Error to 'respond_error' that does not represent a OS error (i.e., errno). Consider using 'respond_raw_error'"
        )))?;
        self.respond_raw_error(request, errnum, errmsg)
    }

    /// Respond to the provided Request with an error code and optional error message.
    ///
    /// This method represents the error code with a C-style `errno` value. To use a
    /// `std::io::Error` object, use `respond_error` instead.
    pub fn respond_raw_error<ReqState: PossiblyDroppablePtr<flux_msg_t>>(
        &self,
        request: &Request<ReqState>,
        errnum: i32,
        errmsg: Option<&str>,
    ) -> Result<()> {
        let c_errmsg = errmsg.map(CString::new).transpose()?;
        let c_errmsg_ptr = c_errmsg.as_ref().map(|cs| cs.as_ptr());
        flux_try!(empty_ok
            flux_respond_error(
                self.h.as_mut_ptr(),
                request.msg.c_msg.as_mut_ptr(),
                errnum,
                c_errmsg_ptr.unwrap_or(std::ptr::null()),
            )
        )
    }
}

impl Clone for OwnedFluxHandle {
    /// Clone the Flux handle.
    ///
    /// Under the hood, this method simply increments the reference count on the current
    /// FluxHandle's `flux_t` pointer. It then creates a new `FluxHandle` object using the
    /// same `flux_t` pointer. If you want to get a new handle using `flux_clone`, use
    /// the `try_clone` method or the implementation of `TryFrom<&FluxHandle>` instead.
    fn clone(&self) -> Self {
        self.to_owned()
            .expect("flux_incref should never result in the flux_t pointer becoming NULL")
    }
}

impl<State: PossiblyDroppablePtr<flux_t>> TryFrom<&FluxHandle<State>> for OwnedFluxHandle {
    type Error = FluxError;

    /// Clone the Flux handle.
    ///
    /// Under the hood, this method calls the `flux_clone` C function.
    /// This method is equivalent to FluxHandle's `try_clone` method.
    /// If you want to get a new handle by simply incrementing the reference count, use
    /// `FluxHandle`'s implementation of `Clone` instead.
    fn try_from(value: &FluxHandle<State>) -> Result<OwnedFluxHandle> {
        let cloned_flux_handle = flux_try!(flux_clone(value.h.as_mut_ptr()))?;
        Ok(FluxHandle {
            h: FluxPtr::create_owned(cloned_flux_handle, flux_close)?,
            comm_error_handler_cb: value.comm_error_handler_cb.clone(),
            curr_set_reactor: None,
        })
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedFluxHandle<'a> {
    type CType = flux_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            h: FluxPtr::create_borrowed(ptr)?,
            comm_error_handler_cb: None,
            curr_set_reactor: None,
        })
    }
}

unsafe impl FromFluxPtr for OwnedFluxHandle {
    type CType = flux_t;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            h: FluxPtr::create_owned(ptr, flux_close)?,
            comm_error_handler_cb: None,
            curr_set_reactor: None,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<flux_t>> AsFluxPtr for FluxHandle<State> {
    define_as_flux_ptr_body!(flux_t, h);
}

unsafe impl IntoFluxPtr for OwnedFluxHandle {
    define_into_flux_ptr_body!(h);
}

unsafe impl<State: PossiblyDroppablePtr<flux_t>> Send for FluxHandle<State> {}
