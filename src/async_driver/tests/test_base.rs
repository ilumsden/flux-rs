use std::os::fd::{AsFd, AsRawFd};
use std::sync::{Arc, Mutex};

use crate::async_driver::base::{
    AsyncDriver, RawFdWrapper, get_poll_fd_for_async, process_readable_event_for_async,
};
use crate::error::{FluxError, Result};
use crate::handle::FluxHandle;
use crate::handle::HandleFlags;
use crate::reactor::{FluxReactorThread, Reactor};

// =========================================================================
// Helpers
// =========================================================================

/// Open a fresh handle wrapped in Arc<Mutex<>> for async driver tests.
fn make_shared_handle() -> Arc<Mutex<FluxHandle>> {
    Arc::new(Mutex::new(
        FluxHandle::new_from_str_uri("", HandleFlags::NONE)
            .expect("Failed to open FluxHandle for async driver test"),
    ))
}

/// Poison a mutex by panicking inside a thread that holds it.
fn make_poisoned_mutex() -> Arc<Mutex<FluxHandle>> {
    let shared = make_shared_handle();
    let shared_clone = shared.clone();
    let _ = std::thread::spawn(move || {
        let _guard = shared_clone.lock().unwrap();
        panic!("intentional panic to poison mutex");
    })
    .join();
    shared
}

/// A minimal AsyncDriver that records whether spawn was called.
struct MockDriver {
    #[allow(dead_code)]
    handle: Arc<Mutex<FluxHandle>>,
    spawned: bool,
}

impl TryFrom<Arc<Mutex<FluxHandle>>> for MockDriver {
    type Error = FluxError;
    fn try_from(handle: Arc<Mutex<FluxHandle>>) -> Result<Self> {
        Ok(Self {
            handle,
            spawned: false,
        })
    }
}

impl TryFrom<FluxReactorThread> for MockDriver {
    type Error = FluxError;
    fn try_from(_: FluxReactorThread) -> Result<Self> {
        Ok(Self {
            handle: make_shared_handle(),
            spawned: false,
        })
    }
}

impl AsyncDriver for MockDriver {
    fn spawn(&mut self) -> Result<()> {
        self.spawned = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

/// An AsyncDriver whose spawn always fails.
struct FailingMockDriver;

impl TryFrom<Arc<Mutex<FluxHandle>>> for FailingMockDriver {
    type Error = FluxError;
    fn try_from(_: Arc<Mutex<FluxHandle>>) -> Result<Self> {
        Ok(Self)
    }
}

impl TryFrom<FluxReactorThread> for FailingMockDriver {
    type Error = FluxError;
    fn try_from(_: FluxReactorThread) -> Result<Self> {
        Ok(Self)
    }
}

impl AsyncDriver for FailingMockDriver {
    fn spawn(&mut self) -> Result<()> {
        Err(FluxError::Logic("spawn always fails".to_string()))
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

/// An AsyncDriver whose try_from(Arc<Mutex<FluxHandle>>) always fails.
struct FailingFromHandleMockDriver;

impl TryFrom<Arc<Mutex<FluxHandle>>> for FailingFromHandleMockDriver {
    type Error = FluxError;
    fn try_from(_: Arc<Mutex<FluxHandle>>) -> Result<Self> {
        Err(FluxError::Logic("try_from handle always fails".to_string()))
    }
}

impl TryFrom<FluxReactorThread> for FailingFromHandleMockDriver {
    type Error = FluxError;
    fn try_from(_: FluxReactorThread) -> Result<Self> {
        Err(FluxError::Logic(
            "try_from reactor thread always fails".to_string(),
        ))
    }
}

impl AsyncDriver for FailingFromHandleMockDriver {
    fn spawn(&mut self) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

// =========================================================================
// RawFdWrapper
// =========================================================================

#[test]
fn raw_fd_wrapper_as_raw_fd_returns_stored_value() {
    let wrapper = RawFdWrapper(42);
    assert_eq!(wrapper.as_raw_fd(), 42);
}

#[test]
fn raw_fd_wrapper_as_fd_returns_borrowed_fd_with_correct_raw() {
    // Use stdin (fd 0) which is always open in a test process.
    let wrapper = RawFdWrapper(0);
    let borrowed = wrapper.as_fd();
    assert_eq!(borrowed.as_raw_fd(), 0);
}

#[test]
fn raw_fd_wrapper_stores_negative_one() {
    // -1 is a sentinel "invalid fd" value; the wrapper should store it as-is.
    let wrapper = RawFdWrapper(-1);
    assert_eq!(wrapper.as_raw_fd(), -1);
}

// =========================================================================
// get_poll_fd_for_async
// =========================================================================

#[test]
fn get_poll_fd_returns_non_negative_fd() {
    let shared = make_shared_handle();
    let result = get_poll_fd_for_async(shared);
    assert!(result.is_ok());
    assert!(result.unwrap().as_raw_fd() >= 0);
}

#[test]
fn get_poll_fd_returns_raw_fd_wrapper() {
    let shared = make_shared_handle();
    let wrapper = get_poll_fd_for_async(shared).unwrap();
    // Verify it implements AsRawFd correctly by calling it twice.
    assert_eq!(wrapper.as_raw_fd(), wrapper.as_raw_fd());
}

#[test]
fn get_poll_fd_poisoned_mutex_returns_logic_error() {
    let poisoned = make_poisoned_mutex();
    let result = get_poll_fd_for_async(poisoned);
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn get_poll_fd_is_stable_across_two_calls() {
    // The pollfd should be the same file descriptor on repeated calls.
    let shared = make_shared_handle();
    let fd1 = get_poll_fd_for_async(shared.clone()).unwrap().as_raw_fd();
    let fd2 = get_poll_fd_for_async(shared).unwrap().as_raw_fd();
    assert_eq!(fd1, fd2);
}

// =========================================================================
// process_readable_event_for_async
// =========================================================================

#[test]
fn process_readable_event_on_fresh_handle_succeeds() {
    let shared = make_shared_handle();
    // Trigger pollfd creation first (required before get_pollevents works).
    let _ = get_poll_fd_for_async(shared.clone());
    let result = process_readable_event_for_async(shared, true);
    assert!(result.is_ok());
}

#[test]
fn process_readable_event_poisoned_mutex_returns_logic_error() {
    let poisoned = make_poisoned_mutex();
    let result = process_readable_event_for_async(poisoned, true);
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn process_readable_event_can_be_called_multiple_times() {
    let shared = make_shared_handle();
    let _ = get_poll_fd_for_async(shared.clone());
    assert!(process_readable_event_for_async(shared.clone(), true).is_ok());
    assert!(process_readable_event_for_async(shared, true).is_ok());
}

// =========================================================================
// AsyncDriver::spawn_async_driver (default method)
// =========================================================================

#[test]
fn spawn_async_driver_calls_spawn() {
    let shared = make_shared_handle();
    let driver = MockDriver::spawn_async_driver(shared).unwrap();
    assert!(driver.spawned, "spawn_async_driver must call spawn()");
}

#[test]
fn spawn_async_driver_succeeds_with_valid_handle() {
    let shared = make_shared_handle();
    assert!(MockDriver::spawn_async_driver(shared).is_ok());
}

#[test]
fn spawn_async_driver_propagates_spawn_error() {
    let shared = make_shared_handle();
    assert!(matches!(
        FailingMockDriver::spawn_async_driver(shared),
        Err(FluxError::Logic(_))
    ));
}

#[test]
fn spawn_async_driver_propagates_try_from_error() {
    let shared = make_shared_handle();
    assert!(matches!(
        FailingFromHandleMockDriver::spawn_async_driver(shared),
        Err(FluxError::Logic(_))
    ));
}

// =========================================================================
// AsyncDriver::spawn_reactor_thread (default method)
// =========================================================================

#[test]
fn spawn_reactor_thread_calls_spawn() {
    let reactor = Reactor::new().unwrap();
    let driver = MockDriver::spawn_reactor_thread(reactor).unwrap();
    assert!(driver.spawned, "spawn_reactor_thread must call spawn()");
}

#[test]
fn spawn_reactor_thread_succeeds_with_valid_reactor() {
    let reactor = Reactor::new().unwrap();
    assert!(MockDriver::spawn_reactor_thread(reactor).is_ok());
}

#[test]
fn spawn_reactor_thread_propagates_spawn_error() {
    let reactor = Reactor::new().unwrap();
    assert!(matches!(
        FailingMockDriver::spawn_reactor_thread(reactor),
        Err(FluxError::Logic(_))
    ));
}

#[test]
fn spawn_reactor_thread_propagates_try_from_error() {
    let reactor = Reactor::new().unwrap();
    assert!(matches!(
        FailingFromHandleMockDriver::spawn_reactor_thread(reactor),
        Err(FluxError::Logic(_))
    ));
}
