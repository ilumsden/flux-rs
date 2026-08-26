use std::ffi::c_void;

use flux_sys::core::{
    flux_match, flux_msg_handler_allow_rolemask, flux_msg_handler_create,
    flux_msg_handler_deny_rolemask, flux_msg_handler_destroy, flux_msg_handler_start,
    flux_msg_handler_stop, flux_msg_handler_t, flux_msg_incref, flux_msg_t, flux_t,
};

use crate::error::{Result, flux_try};
use crate::flux_log_error;
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, BorrowFluxPtrNoArgs, Borrowed, FluxPtr, FromFluxPtr,
    FromFluxPtrNoArgs, Owned, PossiblyDroppablePtr, define_as_flux_ptr_body,
};
use crate::handle::{FluxHandle, OwnedFluxHandle};
use crate::msg::{Message, MessageMatch, MessageRolemask, MessageType};

pub type MsgHandlerCallback = Box<dyn Fn(OwnedFluxHandle, BorrowedMsgHandler<'_>, Message)>;

pub(crate) extern "C" fn msg_handler_trampoline(
    h: *mut flux_t,
    mh: *mut flux_msg_handler_t,
    msg: *const flux_msg_t,
    arg: *mut c_void,
) {
    let closure = unsafe { &mut *(arg as *mut MsgHandlerCallback) };
    let handle = if let Ok(fh) = unsafe { FluxHandle::borrow_ptr(h) } {
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
        c_handler: match FluxPtr::create_borrowed(mh) {
            Ok(handler_ptr) => handler_ptr,
            Err(err) => {
                flux_log_error!(
                    handle,
                    "Error occured in wrapping the flux_msg_handler_t in MessageHandler callback: {}",
                    err
                );
                return;
            }
        },
        _cb_box: None,
    };
    if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let owned_handle = handle.to_owned().expect(
            "Calling FluxHandle::to_owned failed, but it should never fail since flux_incref should never result in the flux_t pointer becoming NULL"
        );
        closure(owned_handle, msg_handler, msg)
    })) {
        flux_log_error!(
            handle,
            "A panic occured in the user-provided Rust callback for a MsgHandler: {e:?}"
        )
    }
}

pub struct MsgHandler<State: PossiblyDroppablePtr<flux_msg_handler_t> = Owned<flux_msg_handler_t>> {
    pub(crate) c_handler: FluxPtr<flux_msg_handler_t, State>,
    pub(crate) _cb_box: Option<Box<MsgHandlerCallback>>,
}

pub type OwnedMsgHandler = MsgHandler<Owned<flux_msg_handler_t>>;
pub type BorrowedMsgHandler<'a> = MsgHandler<Borrowed<'a, flux_msg_handler_t>>;

impl OwnedMsgHandler {
    pub fn new<FhState: PossiblyDroppablePtr<flux_t>>(
        handle: &FluxHandle<FhState>,
        matcher: MessageMatch,
        callback: MsgHandlerCallback,
    ) -> Result<Self> {
        // Wrap the callback in a Box<Box<FnMut>> to ensure we get a thin pointer that is valid for C
        let mut stable_cb = Box::new(callback);
        let arg_ptr = &mut *stable_cb as *mut MsgHandlerCallback as *mut c_void;

        let c_match: flux_match = (&matcher).into();

        let handler_ptr = flux_try!(flux_msg_handler_create(
            handle.h.as_mut_ptr(),
            c_match,
            Some(msg_handler_trampoline),
            arg_ptr,
        ))?;

        Ok(Self {
            c_handler: FluxPtr::create_owned(handler_ptr, flux_msg_handler_destroy)?,
            _cb_box: Some(stable_cb),
        })
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_handler_t>> MsgHandler<State> {
    pub fn start(&self) {
        unsafe {
            flux_msg_handler_start(self.c_handler.as_mut_ptr());
        }
    }

    pub fn stop(&self) {
        unsafe {
            flux_msg_handler_stop(self.c_handler.as_mut_ptr());
        }
    }

    pub fn allow_rolemask(&mut self, rolemask: MessageRolemask) {
        unsafe {
            flux_msg_handler_allow_rolemask(self.c_handler.as_mut_ptr(), rolemask.bits());
        }
    }

    pub fn deny_rolemask(&mut self, rolemask: MessageRolemask) {
        unsafe {
            flux_msg_handler_deny_rolemask(self.c_handler.as_mut_ptr(), rolemask.bits());
        }
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_handler_t>> Drop for MsgHandler<State> {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedMsgHandler<'a> {
    type CType = flux_msg_handler_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_handler: FluxPtr::create_borrowed(ptr)?,
            _cb_box: None,
        })
    }
}

unsafe impl FromFluxPtr for OwnedMsgHandler {
    type CType = flux_msg_handler_t;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_handler: FluxPtr::create_owned(ptr, flux_msg_handler_destroy)?,
            _cb_box: None,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<flux_msg_handler_t>> AsFluxPtr for MsgHandler<State> {
    define_as_flux_ptr_body!(flux_msg_handler_t, c_handler);
}

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
        F: Fn(OwnedFluxHandle, BorrowedMsgHandler<'_>, Message) + Send + 'static,
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
pub fn add_handler_vec<I>(handle: &FluxHandle, specs: I) -> Result<Vec<OwnedMsgHandler>>
where
    I: IntoIterator<Item = MsgHandlerSpec>,
{
    specs
        .into_iter()
        .map(|spec| {
            let matcher = spec.as_msg_match()?;
            let mut handler = MsgHandler::new(handle, matcher, spec.cb)?;
            handler.allow_rolemask(spec.rolemask);
            handler.start();
            Ok(handler)
        })
        .collect()
}
