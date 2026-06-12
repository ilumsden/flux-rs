use std::io;
use std::os::raw::c_int;
use std::thread::{self, JoinHandle};

use bitflags::bitflags;
use errno::{set_errno, Errno};
use flux_sys::core::{
    flux_reactor_active_incref, flux_reactor_create, flux_reactor_destroy, flux_reactor_now,
    flux_reactor_now_update, flux_reactor_run, flux_reactor_stop, flux_reactor_stop_error,
    flux_reactor_t, flux_reactor_time, FLUX_POLLERR, FLUX_POLLIN, FLUX_POLLOUT,
    FLUX_REACTOR_NOWAIT, FLUX_REACTOR_ONCE,
};

use crate::error::{FluxError, Result};
use crate::handle::FluxHandle;
use crate::AsRawFluxPtr;

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ReactorFlags: u32 {
        const NONE = 0;
        const NOWAIT = FLUX_REACTOR_NOWAIT;
        const ONCE = FLUX_REACTOR_ONCE;
    }
}

pub struct Reactor {
    pub(crate) c_reactor: *mut flux_reactor_t,
}

unsafe impl Send for Reactor {}

impl Reactor {
    pub fn new(flags: ReactorFlags) -> Result<Self> {
        let reactor_ptr = unsafe { flux_reactor_create(flags.bits() as i32) };
        if reactor_ptr.is_null() {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(Self {
            c_reactor: reactor_ptr,
        })
    }

    pub fn run(&mut self, flags: ReactorFlags) -> Result<()> {
        if self.c_reactor.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot run a reactor where the internal pointer is NULL",
            )));
        }
        let rc = unsafe { flux_reactor_run(self.c_reactor, flags.bits() as i32) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn stop(&mut self, error_code: Option<std::io::Error>) -> Result<()> {
        if self.c_reactor.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot stop a reactor where the internal pointer is NULL",
            )));
        }
        if let Some(ec) = error_code {
            if let Some(raw_errno_val) = ec.raw_os_error() {
                set_errno(Errno(raw_errno_val));
                unsafe {
                    flux_reactor_stop_error(self.c_reactor);
                }
                return Ok(());
            }
        }
        unsafe {
            flux_reactor_stop(self.c_reactor);
        }
        Ok(())
    }

    /// Get the current reactor time.
    ///
    /// *Note*: Flux's reactors are based on libev. For more information, see
    /// the documentation for `ev_now`.
    pub fn now(&self) -> Result<f64> {
        if self.c_reactor.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot query a reactor's time when the internal pointer is NULL",
            )));
        }
        Ok(unsafe { flux_reactor_now(self.c_reactor) })
    }

    /// Update the current reactor time.
    ///
    /// *Note*: Flux's reactors are based on libev. For more information, see
    /// the documentation for `ev_now_update`.
    pub fn now_update(&mut self) -> Result<()> {
        if self.c_reactor.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot update a reactor's time when the internal pointer is NULL",
            )));
        }
        unsafe { flux_reactor_now_update(self.c_reactor) }
        Ok(())
    }

    /// Get the current system time.
    ///
    /// *Note*: Flux's reactors are based on libev. For more information, see
    /// the documentation for `ev_time`.
    pub fn time(&self) -> f64 {
        unsafe { flux_reactor_time() }
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        unsafe {
            if !self.c_reactor.is_null() {
                flux_reactor_destroy(self.c_reactor);
            }
        }
    }
}

impl From<*mut flux_reactor_t> for Reactor {
    fn from(value: *mut flux_reactor_t) -> Self {
        Self { c_reactor: value }
    }
}

impl Clone for Reactor {
    fn clone(&self) -> Self {
        if !self.c_reactor.is_null() {
            unsafe {
                flux_reactor_active_incref(self.c_reactor);
            }
        }
        Self {
            c_reactor: self.c_reactor,
        }
    }
}

impl AsRawFluxPtr<flux_reactor_t> for Reactor {
    fn as_flux_ptr(&self) -> *mut flux_reactor_t {
        self.c_reactor
    }
}

/// A struct for launching a progress thread for the Flux reactor.
///
/// This struct allows for Rust async runtimes to satisfy AsyncFluxFuture without
/// requiring them to be wired up with the underlying `epoll` file descriptor from
/// the reactor. For more async runtime-native integration, see FluxAsyncDriver.
pub struct FluxReactorThread {
    /// A pointer to the Flux reactor.
    reactor: Reactor,
    /// A handle to the spawned reactor thread.
    handle: Option<JoinHandle<Result<()>>>,
}

unsafe impl Send for FluxReactorThread {}
unsafe impl Sync for FluxReactorThread {}

impl FluxReactorThread {
    /// Spawns a background thread to drive the Flux reactor.
    pub fn spawn(mut reactor: Reactor) -> Self {
        let reactor_copy = Reactor {
            c_reactor: reactor.c_reactor,
        };
        // This thread simply calls `flux_reactor_run` to run indefinitely.
        // If using an async runtime, this thread will ensure the callbacks registered
        // on flux_future_t objects by AsyncFluxFuture will fire.
        let handle = thread::spawn(move || reactor.run(ReactorFlags::NONE));

        Self {
            reactor: reactor_copy,
            handle: Some(handle),
        }
    }

    /// Stops the reactor and joins the background thread.
    pub fn stop(&mut self) -> Result<()> {
        self.reactor.stop(None)?;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join().expect("Reactor thread panicked");
        }
        Ok(())
    }
}

impl Drop for FluxReactorThread {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// If you want to use your async runtime's native epoll (e.g., Tokio's AsyncFd),
/// use `flux_pollfd` to get the FD, register it with your runtime, and call
/// this function whenever the runtime indicates the FD is readable.
pub struct FluxAsyncDriver<'a> {
    handle: &'a FluxHandle,
    reactor: Reactor,
}

impl<'a> FluxAsyncDriver<'a> {
    pub fn new(handle: &'a FluxHandle) -> Result<Self> {
        let reactor = handle.get_reactor()?;
        Ok(Self { handle, reactor })
    }

    /// Returns the edge-triggered file descriptor to register with your async runtime.
    pub fn get_poll_fd(&self) -> Result<c_int> {
        self.handle.get_pollfd()
    }

    /// Call this when your async runtime indicates the FD is readable.
    /// It clears the edge-triggered state and processes pending callbacks without blocking.
    pub fn process_readable_event(&mut self) -> Result<()> {
        // 1. Get events and clear the edge-triggered POLLIN state
        let events = self.handle.get_pollevents()?;
        if events == -1 {
            return Err(FluxError::System(io::Error::last_os_error()));
        }

        // 2. If there is data to read, tick the reactor ONCE without blocking
        if (events as u32 & FLUX_POLLIN) != 0 || (events as u32 & FLUX_POLLOUT) != 0 {
            self.reactor.run(ReactorFlags::NOWAIT)?;
        }

        // 3. Handle errors
        if (events as u32 & FLUX_POLLERR) != 0 {
            return Err(FluxError::Logic(
                "Flux handle experienced a POLLERR".to_string(),
            ));
        }

        Ok(())
    }
}
