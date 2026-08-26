use std::thread::{self, JoinHandle};

use bitflags::bitflags;
use errno::{Errno, set_errno};
use flux_sys::core::{
    FLUX_REACTOR_NOWAIT, FLUX_REACTOR_ONCE, flux_reactor_create, flux_reactor_destroy,
    flux_reactor_incref, flux_reactor_now, flux_reactor_now_update, flux_reactor_run,
    flux_reactor_stop, flux_reactor_stop_error, flux_reactor_t, flux_reactor_time,
};

use crate::error::{Result, flux_try};
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, Borrowed, FluxPtr, FromFluxPtr, IntoFluxPtr, Owned,
    PossiblyDroppablePtr, define_as_flux_ptr_body, define_into_flux_ptr_body,
};

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ReactorFlags: u32 {
        const NONE = 0;
        const NOWAIT = FLUX_REACTOR_NOWAIT;
        const ONCE = FLUX_REACTOR_ONCE;
    }
}

pub struct Reactor<State: PossiblyDroppablePtr<flux_reactor_t> = Owned<flux_reactor_t>> {
    pub(crate) c_reactor: FluxPtr<flux_reactor_t, State>,
}

pub type OwnedReactor = Reactor<Owned<flux_reactor_t>>;
pub type BorrowedReactor<'a> = Reactor<Borrowed<'a, flux_reactor_t>>;

unsafe impl<State: PossiblyDroppablePtr<flux_reactor_t>> Send for Reactor<State> {}

impl OwnedReactor {
    pub fn new() -> Result<Self> {
        let reactor_ptr = flux_try!(flux_reactor_create(0))?;
        Ok(Self {
            c_reactor: FluxPtr::create_owned(reactor_ptr, flux_reactor_destroy)?,
        })
    }
}

impl<State: PossiblyDroppablePtr<flux_reactor_t>> Reactor<State> {
    pub fn to_owned(&self) -> Result<OwnedReactor> {
        unsafe {
            flux_reactor_incref(self.c_reactor.as_mut_ptr());
        }
        Ok(Reactor {
            c_reactor: FluxPtr::create_owned(self.c_reactor.as_mut_ptr(), flux_reactor_destroy)?,
        })
    }

    pub fn run(&mut self, flags: ReactorFlags) -> Result<()> {
        flux_try!(empty_ok flux_reactor_run(
            self.c_reactor.as_mut_ptr(),
            flags.bits() as _
        ))
    }

    pub fn stop(&mut self, error_code: Option<std::io::Error>) -> Result<()> {
        if let Some(ec) = error_code
            && let Some(raw_errno_val) = ec.raw_os_error()
        {
            set_errno(Errno(raw_errno_val));
            unsafe {
                flux_reactor_stop_error(self.c_reactor.as_mut_ptr());
            }
            return Ok(());
        }
        unsafe {
            flux_reactor_stop(self.c_reactor.as_mut_ptr());
        }
        Ok(())
    }

    /// Get the current reactor time.
    ///
    /// *Note*: Flux's reactors are based on libev. For more information, see
    /// the documentation for `ev_now`.
    pub fn now(&self) -> Result<f64> {
        Ok(unsafe { flux_reactor_now(self.c_reactor.as_mut_ptr()) })
    }

    /// Update the current reactor time.
    ///
    /// *Note*: Flux's reactors are based on libev. For more information, see
    /// the documentation for `ev_now_update`.
    pub fn now_update(&mut self) -> Result<()> {
        unsafe { flux_reactor_now_update(self.c_reactor.as_mut_ptr()) }
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

impl Clone for OwnedReactor {
    fn clone(&self) -> Self {
        self.to_owned().expect(
            "The reactor being cloned has a NULL internal pointer, which shouldn't be possible",
        )
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedReactor<'a> {
    type CType = flux_reactor_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_reactor: FluxPtr::create_borrowed(ptr)?,
        })
    }
}

unsafe impl FromFluxPtr for OwnedReactor {
    type CType = flux_reactor_t;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_reactor: FluxPtr::create_owned(ptr, flux_reactor_destroy)?,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<flux_reactor_t>> AsFluxPtr for Reactor<State> {
    define_as_flux_ptr_body!(flux_reactor_t, c_reactor);
}

unsafe impl IntoFluxPtr for OwnedReactor {
    define_into_flux_ptr_body!(c_reactor);
}

/// A struct for launching a progress thread for the Flux reactor.
///
/// This struct allows for Rust async runtimes to satisfy AsyncFluxFuture without
/// requiring them to be wired up with the underlying `epoll` file descriptor from
/// the reactor. For more async runtime-native integration, see FluxAsyncDriver.
pub struct FluxReactorThread {
    /// A pointer to the Flux reactor.
    reactor: OwnedReactor,
    /// A handle to the spawned reactor thread.
    handle: Option<JoinHandle<Result<()>>>,
}

unsafe impl<'a> Send for FluxReactorThread {}
unsafe impl<'a> Sync for FluxReactorThread {}

impl FluxReactorThread {
    pub fn new(reactor: OwnedReactor) -> Self {
        Self {
            reactor,
            handle: None,
        }
    }

    /// Spawns a background thread to drive the Flux reactor.
    pub fn spawn(&mut self) -> Result<()> {
        let mut reactor_copy = Reactor {
            c_reactor: FluxPtr::create_borrowed(self.reactor.c_reactor.as_mut_ptr())?,
        };
        // This thread simply calls `flux_reactor_run` to run indefinitely.
        // If using an async runtime, this thread will ensure the callbacks registered
        // on flux_future_t objects by AsyncFluxFuture will fire.
        let handle = thread::spawn(move || reactor_copy.run(ReactorFlags::NONE));
        self.handle = Some(handle);
        Ok(())
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
