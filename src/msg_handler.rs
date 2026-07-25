use std::ffi::c_void;

use flux_sys::core::{
    flux_incref, flux_match, flux_msg_handler_allow_rolemask, flux_msg_handler_create,
    flux_msg_handler_deny_rolemask, flux_msg_handler_destroy, flux_msg_handler_start,
    flux_msg_handler_stop, flux_msg_handler_t, flux_msg_incref, flux_msg_t, flux_t,
};

use crate::error::{check_ptr, Result};
use crate::flux_log_error;
use crate::flux_ptr_management::{
    default_impl_as_flux_ptr, BorrowFluxPtr, FluxPtr, FromFluxPtr, FromFluxPtrNoArgs,
};
use crate::handle::FluxHandle;
use crate::msg::{Message, MessageMatch, MessageRolemask, MessageType};

pub type MsgHandlerCallback = Box<dyn FnMut(FluxHandle, MsgHandler, Message)>;

pub struct MsgHandler {
    c_handler: FluxPtr<flux_msg_handler_t>,
    _cb_box: Option<MsgHandlerCallback>,
}

impl MsgHandler {
    pub(crate) extern "C" fn msg_handler_trampoline(
        h: *mut flux_t,
        mh: *mut flux_msg_handler_t,
        msg: *const flux_msg_t,
        arg: *mut c_void,
    ) {
        let closure = unsafe { &mut *(arg as *mut MsgHandlerCallback) };
        // Increment the reference counter on the flux_t pointer so that FluxHandle::drop
        // does not free the C-owned handle.
        unsafe {
            flux_incref(h);
        }
        let handle = if let Ok(fh) = unsafe { FluxHandle::from_ptr(h) } {
            fh
        } else {
            return;
        };
        // Increment the reference counter on the flux_msg_t pointer so that Message::drop
        // does not free the C-owned handle.
        unsafe {
            flux_msg_incref(msg);
        }
        let msg = match unsafe { Message::from_ptr(msg as *mut flux_msg_t) } {
            Ok(msg_obj) => msg_obj,
            Err(err) => {
                flux_log_error!(
                    handle,
                    "Error occured while wrapping the flux_msg_t in MessageHandler callback: {}",
                    err
                );
                return;
            }
        };
        // Create the Rust MsgHandler with 'should_drop' set to false so that
        // MsgHandler::drop does not free data owned by C
        let msg_handler = MsgHandler {
            c_handler: match FluxPtr::create_borrowed(mh, flux_msg_handler_destroy) {
                Ok(handler_ptr) => handler_ptr,
                Err(err) => {
                    flux_log_error!(handle, "Error occured in wrapping the flux_msg_handler_t in MessageHandler callback: {}", err);
                    return;
                }
            },
            _cb_box: None,
        };
        closure(handle, msg_handler, msg)
    }

    pub fn new(
        handle: &FluxHandle,
        matcher: MessageMatch,
        mut callback: MsgHandlerCallback,
    ) -> Result<Self> {
        let arg_ptr = &mut callback as *mut MsgHandlerCallback as *mut c_void;

        let c_match: flux_match = (&matcher).into();

        let handler_ptr = unsafe {
            flux_msg_handler_create(
                handle.h.as_mut_ptr(),
                c_match,
                Some(Self::msg_handler_trampoline),
                arg_ptr,
            )
        };
        check_ptr(handler_ptr)?;

        Ok(Self {
            c_handler: FluxPtr::create_owned(handler_ptr, flux_msg_handler_destroy)?,
            _cb_box: Some(callback),
        })
    }

    pub fn start(&self) -> Result<()> {
        unsafe {
            flux_msg_handler_start(self.c_handler.as_mut_ptr());
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        unsafe {
            flux_msg_handler_stop(self.c_handler.as_mut_ptr());
        }
        Ok(())
    }

    pub fn allow_rolemask(&mut self, rolemask: MessageRolemask) -> Result<()> {
        unsafe {
            flux_msg_handler_allow_rolemask(self.c_handler.as_mut_ptr(), rolemask.bits());
        }
        Ok(())
    }

    pub fn deny_rolemask(&mut self, rolemask: MessageRolemask) -> Result<()> {
        unsafe {
            flux_msg_handler_deny_rolemask(self.c_handler.as_mut_ptr(), rolemask.bits());
        }
        Ok(())
    }
}

unsafe impl BorrowFluxPtr for MsgHandler {
    type CType = flux_msg_handler_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_handler: FluxPtr::create_borrowed(ptr, flux_msg_handler_destroy)?,
            _cb_box: None,
        })
    }
}

unsafe impl FromFluxPtr for MsgHandler {
    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_handler: FluxPtr::create_owned(ptr, flux_msg_handler_destroy)?,
            _cb_box: None,
        })
    }
}

default_impl_as_flux_ptr!(MsgHandler, flux_msg_handler_t, c_handler);

pub struct MsgHandlerSpec {
    pub typemask: MessageType,
    pub topic_glob: String,
    pub cb: MsgHandlerCallback,
    pub rolemask: MessageRolemask,
}

impl MsgHandlerSpec {
    pub fn new<F>(
        typemask: MessageType,
        topic_glob: &str,
        callback: F,
        rolemask: MessageRolemask,
    ) -> Self
    where
        F: FnMut(FluxHandle, MsgHandler, Message) + Send + 'static,
    {
        Self {
            typemask,
            topic_glob: topic_glob.to_string(),
            cb: Box::new(callback),
            rolemask,
        }
    }

    pub fn as_msg_match(&self) -> Result<MessageMatch> {
        MessageMatch::new(Some(self.typemask), None, Some(self.topic_glob.as_str()))
    }
}

/// Create and start multiple MsgHandlers from a collection of MsgHandlerSpecs.
///
/// This function does the same thing `flux_msg_handler_addvec` from the C API.
///
/// Note: because `flux_msg_handler_destroy` is called automatically by MsgHandler::drop, there is no
/// equivalent to the `flux_msg_handler_delvec` from the C API. Simply call `drop` on the `Vec` returned
/// from this function to achieve the same thing.
pub fn add_handler_vec<I>(handle: &FluxHandle, specs: I) -> Result<Vec<MsgHandler>>
where
    I: IntoIterator<Item = MsgHandlerSpec>,
{
    specs
        .into_iter()
        .map(|spec| {
            let matcher = spec.as_msg_match()?;
            let mut handler = MsgHandler::new(handle, matcher, spec.cb)?;
            handler.allow_rolemask(spec.rolemask)?;
            handler.start()?;
            Ok(handler)
        })
        .collect()
}
