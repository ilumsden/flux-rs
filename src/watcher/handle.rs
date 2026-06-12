use std::ffi::{c_int, c_void};

use flux_sys::core::{
    flux_handle_watcher_create, flux_handle_watcher_get_flux, flux_incref,
    flux_reactor_active_incref, flux_reactor_t, flux_watcher_t,
};

use crate::error::{check_ptr, FluxError, Result};
use crate::handle::FluxHandle;
use crate::reactor::Reactor;
use crate::watcher::base::create_watcher_specialization;
use crate::watcher::{RawWatcher, WatcherEvents};

create_watcher_specialization! {
    pub struct HandleWatcher<'a> {
        _flux_handle: Option<&'a FluxHandle>,
        _reactor: &'a Reactor,
        _watch_cb: Option<Box<dyn FnMut(&Reactor, &HandleWatcher, &Option<FluxHandle>, WatcherEvents)>>,
    }
}

impl<'a> HandleWatcher<'a> {
    extern "C" fn trampoline(
        reactor_ptr: *mut flux_reactor_t,
        watcher_ptr: *mut flux_watcher_t,
        revents: c_int,
        arg: *mut c_void,
    ) {
        let closure = unsafe {
            &mut *(arg as *mut Box<
                dyn FnMut(&Reactor, &HandleWatcher, &Option<FluxHandle>, WatcherEvents),
            >)
        };
        unsafe {
            flux_reactor_active_incref(reactor_ptr);
        }
        let reactor = Reactor::from(reactor_ptr);
        let handle_ptr = unsafe { flux_handle_watcher_get_flux(watcher_ptr) };
        let handle = if handle_ptr.is_null() {
            None
        } else {
            unsafe {
                flux_incref(handle_ptr);
            }
            Some(FluxHandle::from(handle_ptr))
        };
        let raw_watcher = RawWatcher::from(watcher_ptr);
        let mut watcher = HandleWatcher {
            watcher: raw_watcher,
            _flux_handle: handle.as_ref(),
            _reactor: &reactor,
            _watch_cb: None,
        };
        closure(
            &reactor,
            &watcher,
            &handle,
            WatcherEvents::from_bits_retain(revents as u32),
        );
        // Set the c_watcher field to NULL to avoid destroying a borrowed C pointer
        if !watcher.watcher.c_watcher.is_null() {
            watcher.watcher.c_watcher = std::ptr::null_mut();
        }
    }

    pub fn create<F>(
        reactor: &'a Reactor,
        handle: &'a FluxHandle,
        events: WatcherEvents,
        callback: F,
    ) -> Result<Self>
    where
        F: FnMut(&Reactor, &HandleWatcher, &Option<FluxHandle>, WatcherEvents) + Send + 'static,
    {
        if reactor.c_reactor.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot create a handle watcher with a NULL reactor",
            )));
        }
        if handle.h.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot create a handle watcher with a NULL handle",
            )));
        }
        let mut watch_cb: Option<
            Box<dyn FnMut(&Reactor, &HandleWatcher, &Option<FluxHandle>, WatcherEvents)>,
        > = Some(Box::new(callback));
        let cb_ref = watch_cb.as_mut().ok_or(FluxError::Logic(String::from(
            "Cannot unpack the callback for interacting with Flux's C API",
        )))?;
        let arg_ptr = cb_ref
            as *mut Box<dyn FnMut(&Reactor, &HandleWatcher, &Option<FluxHandle>, WatcherEvents)>
            as *mut c_void;
        let watcher_ptr = unsafe {
            flux_handle_watcher_create(
                reactor.c_reactor,
                handle.h,
                events.bits() as i32,
                Some(Self::trampoline),
                arg_ptr,
            )
        };
        check_ptr(watcher_ptr)?;
        Ok(Self {
            watcher: RawWatcher::from(watcher_ptr),
            _flux_handle: Some(handle),
            _reactor: reactor,
            _watch_cb: watch_cb,
        })
    }
}
