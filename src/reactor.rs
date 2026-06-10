use std::io;
use std::os::raw::c_int;
use std::thread::{self, JoinHandle};

use flux_sys::core::{
    flux_pollevents, flux_pollfd, flux_reactor_run, flux_reactor_stop, flux_reactor_t, flux_t,
    FLUX_POLLERR, FLUX_POLLIN, FLUX_POLLOUT, FLUX_REACTOR_NOWAIT,
};

use crate::error::{check_rc, FluxError, Result};

/// A thin wrapper around a raw C-pointer to flux_reactor_t to implement Send.
#[derive(Clone, Copy)]
struct FluxReactor(*mut flux_reactor_t);

unsafe impl Send for FluxReactor {}

impl FluxReactor {
    /// Extract the C-pointer to the Flux reactor.
    ///
    /// This method is used to keep Rust 2021 from trying to over-optimize the handle thread
    /// in FluxReactorThread. By providing this method, the compiler is forced to capture the whole
    /// FluxReactor object instead of optimizing to just capturing the `!Send` raw pointer field.
    #[inline(always)]
    fn as_ptr(self) -> *mut flux_reactor_t {
        self.0
    }
}

/// A struct for launching a progress thread for the Flux reactor.
///
/// This struct allows for Rust async runtimes to satisfy AsyncFluxFuture without
/// requiring them to be wired up with the underlying `epoll` file descriptor from
/// the reactor. For more async runtime-native integration, see FluxAsyncDriver.
pub struct FluxReactorThread {
    /// A pointer to the Flux reactor.
    reactor: *mut flux_reactor_t,
    /// A handle to the spawned reactor thread.
    handle: Option<JoinHandle<Result<()>>>,
}

unsafe impl Send for FluxReactorThread {}
unsafe impl Sync for FluxReactorThread {}

impl FluxReactorThread {
    /// Spawns a background thread to drive the Flux reactor.
    pub fn spawn(reactor: *mut flux_reactor_t) -> Self {
        // Wrap the reactor pointer in a Send-safe type
        let safe_reactor = FluxReactor(reactor);
        // Spawn the reactor thread
        // This thread simply calls `flux_reactor_run` to run indefinitely.
        // If using an async runtime, this thread will ensure the callbacks registered
        // on flux_future_t objects by AsyncFluxFuture will fire.
        let handle = thread::spawn(move || {
            let ptr = safe_reactor.as_ptr();
            let rc = unsafe { flux_reactor_run(ptr, 0) };
            check_rc(rc)?;
            Ok(())
        });

        Self {
            reactor,
            handle: Some(handle),
        }
    }

    /// Stops the reactor and joins the background thread.
    pub fn stop(&mut self) -> Result<()> {
        unsafe {
            flux_reactor_stop(self.reactor);
        }
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
pub struct FluxAsyncDriver {
    h: *mut flux_t,
    reactor: *mut flux_reactor_t,
}

impl FluxAsyncDriver {
    pub fn new(h: *mut flux_t, reactor: *mut flux_reactor_t) -> Self {
        Self { h, reactor }
    }

    /// Returns the edge-triggered file descriptor to register with your async runtime.
    pub fn get_poll_fd(&self) -> Result<c_int> {
        let fd = unsafe { flux_pollfd(self.h) };
        if fd == -1 {
            Err(FluxError::System(io::Error::last_os_error()))
        } else {
            Ok(fd)
        }
    }

    /// Call this when your async runtime indicates the FD is readable.
    /// It clears the edge-triggered state and processes pending callbacks without blocking.
    pub fn process_readable_event(&self) -> Result<()> {
        // 1. Get events and clear the edge-triggered POLLIN state
        let events = unsafe { flux_pollevents(self.h) };
        if events == -1 {
            return Err(FluxError::System(io::Error::last_os_error()));
        }

        // 2. If there is data to read, tick the reactor ONCE without blocking
        if (events as u32 & FLUX_POLLIN) != 0 || (events as u32 & FLUX_POLLOUT) != 0 {
            let rc = unsafe { flux_reactor_run(self.reactor, FLUX_REACTOR_NOWAIT as i32) };
            check_rc(rc)?;
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
