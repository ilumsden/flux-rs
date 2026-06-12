use std::ffi::{c_int, c_void};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

use flux_sys::core::{
    flux_fd_watcher_create, flux_reactor_active_incref, flux_reactor_t, flux_watcher_t,
};

use crate::error::{check_ptr, FluxError, Result};
use crate::reactor::Reactor;
use crate::watcher::base::create_watcher_specialization;
use crate::watcher::{RawWatcher, WatcherEvents};

create_watcher_specialization! {
    pub struct FdWatcher<'a> {
        _borrowed_fd: Option<BorrowedFd<'a>>,
        _reactor: &'a Reactor,
        _watch_cb: Option<Box<dyn FnMut(&Reactor, &FdWatcher, WatcherEvents)>>,
    }
}

impl<'a> FdWatcher<'a> {
    extern "C" fn trampoline(
        reactor_ptr: *mut flux_reactor_t,
        watcher_ptr: *mut flux_watcher_t,
        revents: c_int,
        arg: *mut c_void,
    ) {
        let closure =
            unsafe { &mut *(arg as *mut Box<dyn FnMut(&Reactor, &FdWatcher, WatcherEvents)>) };
        unsafe {
            flux_reactor_active_incref(reactor_ptr);
        }
        let reactor = Reactor::from(reactor_ptr);
        let watcher = FdWatcher {
            watcher: RawWatcher::from(watcher_ptr),
            _borrowed_fd: None,
            _reactor: &reactor,
            _watch_cb: None,
        };
        closure(
            &reactor,
            &watcher,
            WatcherEvents::from_bits_retain(revents as u32),
        );
    }

    pub fn create<T, F>(
        reactor: &'a Reactor,
        fd_source: &'a T,
        events: WatcherEvents,
        callback: F,
    ) -> Result<Self>
    where
        T: AsFd,
        F: FnMut(&Reactor, &FdWatcher, WatcherEvents) + Send + 'static,
    {
        if reactor.c_reactor.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot create a file descriptor watcher with a NULL reactor",
            )));
        }
        let mut watch_cb: Option<Box<dyn FnMut(&Reactor, &FdWatcher, WatcherEvents)>> =
            Some(Box::new(callback));
        let cb_ref = watch_cb.as_mut().ok_or(FluxError::Logic(String::from(
            "Cannot unpack the callback for interactive with Flux's C API",
        )))?;
        let arg_ptr =
            cb_ref as *mut Box<dyn FnMut(&Reactor, &FdWatcher, WatcherEvents)> as *mut c_void;
        let borrowed_fd = fd_source.as_fd();
        let watcher_ptr = unsafe {
            flux_fd_watcher_create(
                reactor.c_reactor,
                borrowed_fd.as_raw_fd(),
                events.bits() as i32,
                Some(Self::trampoline),
                arg_ptr,
            )
        };
        check_ptr(watcher_ptr)?;
        Ok(Self {
            watcher: RawWatcher::from(watcher_ptr),
            _borrowed_fd: Some(borrowed_fd),
            _reactor: reactor,
            _watch_cb: watch_cb,
        })
    }
}
