use std::future::Future;
use std::os::raw::c_void;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use flux_sys::core::{flux_future_destroy, flux_future_get, flux_future_t, flux_future_then};

use crate::error::{FluxError, Result};
use crate::future::sync_future::FluxFuture;

/// A struct providing the shared state between the Flux runtime and Rust async runtime
struct SharedState {
    /// The waker for the Rust async runtime
    waker: Option<Waker>,
    /// The result of the Flux future. This will either store the real result or an error message.
    result: Option<Result<*const c_void>>,
}

/// A Rust Future wrapper around `flux_future_t`.
pub struct AsyncFluxFuture {
    inner: *mut flux_future_t,
    state: Arc<Mutex<SharedState>>,
}

impl AsyncFluxFuture {
    /// Creates a new AsyncFluxFuture from a raw Flux future.
    ///
    /// # Safety
    /// `future_ptr` must be a valid pointer to a `flux_future_t`.
    pub unsafe fn new(future_ptr: *mut flux_future_t) -> Result<Self> {
        // Create the shared state between Flux runtime and Rust async runtime
        let state = Arc::new(Mutex::new(SharedState {
            waker: None,
            result: None,
        }));

        // Clone the Arc to pass ownership to the C callback
        let state_for_c = Arc::clone(&state);
        // Convert the Arc into a C "void *" to satisfy the Flux API
        let arg = Arc::into_raw(state_for_c) as *mut c_void;

        // Register the continuation callback with no timeout (-1.0)
        let rc = flux_future_then(future_ptr, -1.0, Some(Self::c_callback), arg);

        if rc == -1 {
            // If `then` fails, we must reclaim the Arc to avoid a memory leak
            let _ = Arc::from_raw(arg as *const Mutex<SharedState>);
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }

        Ok(Self {
            inner: future_ptr,
            state,
        })
    }

    /// The continuation callback invoked by the Flux reactor.
    extern "C" fn c_callback(f: *mut flux_future_t, arg: *mut c_void) {
        // Reclaim the Arc from the raw pointer to prevent memory leaks.
        let state = unsafe { Arc::from_raw(arg as *const Mutex<SharedState>) };

        // Extract the result using flux_future_get
        let mut result_ptr: *const c_void = std::ptr::null();
        let rc = unsafe { flux_future_get(f, &mut result_ptr) };

        // Convert the C return code + errno to a Rust Result
        let final_result = if rc == -1 {
            Err(FluxError::System(std::io::Error::last_os_error()))
        } else {
            Ok(result_ptr)
        };

        // Update the shared state with the result
        let mut lock = state.lock().unwrap();
        lock.result = Some(final_result);

        // Wake the Rust async runtime to make progress
        if let Some(waker) = lock.waker.take() {
            waker.wake();
        }
    }
}

impl Future for AsyncFluxFuture {
    type Output = Result<*const c_void>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Lock the shared state for the async runtime
        let mut state = self.state.lock().unwrap();

        // If there is a result in the shared state, tell the async runtime that the future has been
        // fulfilled with the result. Otherwise, update the shared state with the async runtime's
        // waker so the Flux callback can alert the async runtime upon completion.
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            // Store the waker so the C callback can wake this task later
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl Drop for AsyncFluxFuture {
    fn drop(&mut self) {
        // When destroying the AsyncFluxFuture object, destroy the C-level flux_future_t too.
        unsafe {
            if !self.inner.is_null() {
                flux_future_destroy(self.inner);
            }
        }
    }
}

impl TryFrom<FluxFuture> for AsyncFluxFuture {
    type Error = FluxError;

    fn try_from(value: FluxFuture) -> Result<Self> {
        unsafe { Self::new(value.c_future) }
    }
}
