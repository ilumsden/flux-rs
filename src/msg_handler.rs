use std::ffi::c_void;

use flux_sys::core::{
    flux_incref, flux_match, flux_msg_handler_allow_rolemask, flux_msg_handler_create,
    flux_msg_handler_deny_rolemask, flux_msg_handler_destroy, flux_msg_handler_start,
    flux_msg_handler_stop, flux_msg_handler_t, flux_msg_incref, flux_msg_t, flux_t,
};

use crate::error::{FluxError, Result};
use crate::handle::FluxHandle;
use crate::msg::{Message, MessageMatch, MessageRolemask, MessageType};

pub struct MsgHandler {
    c_handler: *mut flux_msg_handler_t,
    _cb_box: Option<Box<dyn FnMut(FluxHandle, MsgHandler, Message)>>,
    should_drop: bool,
}

impl MsgHandler {
    pub fn new(
        handle: &FluxHandle,
        matcher: MessageMatch,
        mut callback: Box<dyn FnMut(FluxHandle, MsgHandler, Message)>,
    ) -> Result<Self> {
        if handle.h.is_null() {
            return Err(FluxError::Logic(
                "Cannot create a message handler with a NULL Flux handle".to_string(),
            ));
        }

        let arg_ptr =
            &mut callback as *mut Box<dyn FnMut(FluxHandle, MsgHandler, Message)> as *mut c_void;

        extern "C" fn trampoline(
            h: *mut flux_t,
            mh: *mut flux_msg_handler_t,
            msg: *const flux_msg_t,
            arg: *mut c_void,
        ) {
            let closure =
                unsafe { &mut *(arg as *mut Box<dyn FnMut(FluxHandle, MsgHandler, Message)>) };
            // Increment the reference counter on the flux_t pointer so that FluxHandle::drop
            // does not free the C-owned handle.
            unsafe {
                flux_incref(h);
            }
            let handle = FluxHandle::from(h);
            // Increment the reference counter on the flux_msg_t pointer so that Message::drop
            // does not free the C-owned handle.
            unsafe {
                flux_msg_incref(msg);
            }
            let msg = Message::from(msg as *mut flux_msg_t);
            // Create the Rust MsgHandler with 'should_drop' set to false so that
            // MsgHandler::drop does not free data owned by C
            let msg_handler = MsgHandler {
                c_handler: mh,
                _cb_box: None,
                should_drop: false,
            };
            closure(handle, msg_handler, msg)
        }

        let c_match: flux_match = (&matcher).into();

        let handler_ptr =
            unsafe { flux_msg_handler_create(handle.h, c_match, Some(trampoline), arg_ptr) };

        Ok(Self {
            c_handler: handler_ptr,
            _cb_box: Some(callback),
            should_drop: true,
        })
    }

    pub fn start(&self) -> Result<()> {
        if self.c_handler.is_null() {
            return Err(FluxError::Logic(
                "Cannot start a message handler with a NULL internal pointer".to_string(),
            ));
        }
        unsafe {
            flux_msg_handler_start(self.c_handler);
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        if self.c_handler.is_null() {
            return Err(FluxError::Logic(
                "Cannot stop a message handler with a NULL internal pointer".to_string(),
            ));
        }
        unsafe {
            flux_msg_handler_stop(self.c_handler);
        }
        Ok(())
    }

    pub fn allow_rolemask(&mut self, rolemask: MessageRolemask) -> Result<()> {
        if self.c_handler.is_null() {
            return Err(FluxError::Logic(
                "Cannot add to rolemask whitelist with a NULL internal pointer".to_string(),
            ));
        }
        unsafe {
            flux_msg_handler_allow_rolemask(self.c_handler, rolemask.bits());
        }
        Ok(())
    }

    pub fn deny_rolemask(&mut self, rolemask: MessageRolemask) -> Result<()> {
        if self.c_handler.is_null() {
            return Err(FluxError::Logic(
                "Cannot add to rolemask blacklist with a NULL internal pointer".to_string(),
            ));
        }
        unsafe {
            flux_msg_handler_deny_rolemask(self.c_handler, rolemask.bits());
        }
        Ok(())
    }
}

impl Drop for MsgHandler {
    fn drop(&mut self) {
        if !self.c_handler.is_null() && self.should_drop {
            unsafe {
                flux_msg_handler_destroy(self.c_handler);
            }
        }
    }
}

pub struct MsgHandlerSpec {
    pub typemask: MessageType,
    pub topic_glob: String,
    pub cb: Box<dyn FnMut(FluxHandle, MsgHandler, Message)>,
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
