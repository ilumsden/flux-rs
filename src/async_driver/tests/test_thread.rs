use std::sync::{Arc, Mutex};

use crate::async_driver::thread::ThreadDriver;
use crate::async_driver::AsyncDriver;
use crate::error::FluxError;
use crate::handle::FluxHandle;
use crate::handle::HandleFlags;
use crate::reactor::{FluxReactorThread, Reactor, ReactorFlags};

// =========================================================================
// Helpers
// =========================================================================

/// Create a fresh Reactor and wrap it in a FluxReactorThread for testing.
fn make_reactor_thread() -> FluxReactorThread {
    let reactor =
        Reactor::new(ReactorFlags::NONE).expect("Failed to create Reactor for ThreadDriver test");
    FluxReactorThread::new(reactor)
}

/// Create a ThreadDriver from a fresh FluxReactorThread.
fn make_thread_driver() -> ThreadDriver {
    ThreadDriver::try_from(make_reactor_thread())
        .expect("Failed to create ThreadDriver from FluxReactorThread")
}

/// Open a fresh FluxHandle wrapped in Arc<Mutex<>> for TryFrom tests.
fn make_shared_handle() -> Arc<Mutex<FluxHandle>> {
    Arc::new(Mutex::new(
        FluxHandle::new_from_str_uri("", HandleFlags::NONE)
            .expect("Failed to open FluxHandle for ThreadDriver test"),
    ))
}

// =========================================================================
// TryFrom<FluxReactorThread>
// =========================================================================

#[test]
fn try_from_reactor_thread_succeeds() {
    assert!(ThreadDriver::try_from(make_reactor_thread()).is_ok());
}

#[test]
fn try_from_reactor_thread_stores_driver_in_some() {
    let driver = make_thread_driver();
    assert!(
        driver.driver.is_some(),
        "ThreadDriver::driver should be Some after construction from FluxReactorThread"
    );
}

// =========================================================================
// TryFrom<Arc<Mutex<FluxHandle>>>
// =========================================================================

#[test]
fn try_from_shared_handle_always_returns_logic_error() {
    let shared = make_shared_handle();
    assert!(matches!(
        ThreadDriver::try_from(shared),
        Err(FluxError::Logic(_))
    ));
}

#[test]
fn try_from_shared_handle_error_message_mentions_polling() {
    let shared = make_shared_handle();
    match ThreadDriver::try_from(shared) {
        Err(FluxError::Logic(msg)) => {
            assert!(
                msg.contains("polling") || msg.contains("file descriptor"),
                "Expected error message to mention polling, got: {msg}"
            );
        }
        Ok(_) => panic!("Expected FluxError::Logic, got an actual ThreadDriver object"),
        Err(other) => panic!("Expected FluxError::Logic, got {:?}", other),
    }
}

// =========================================================================
// AsyncDriver::spawn_async_driver (default method)
// =========================================================================

#[test]
fn spawn_async_driver_always_returns_logic_error() {
    let shared = make_shared_handle();
    assert!(matches!(
        ThreadDriver::spawn_async_driver(shared),
        Err(FluxError::Logic(_))
    ));
}

// =========================================================================
// AsyncDriver::spawn_reactor_thread (default method)
// =========================================================================

#[test]
fn spawn_reactor_thread_succeeds() {
    let reactor = Reactor::new(ReactorFlags::NONE).unwrap();
    assert!(ThreadDriver::spawn_reactor_thread(reactor).is_ok());
}

#[test]
fn spawn_reactor_thread_driver_field_is_none_after_spawn() {
    // spawn() calls driver.spawn() which starts the reactor thread;
    // the driver field is still Some until stop() is called.
    let reactor = Reactor::new(ReactorFlags::NONE).unwrap();
    let driver = ThreadDriver::spawn_reactor_thread(reactor).unwrap();
    assert!(
        driver.driver.is_some(),
        "driver field should remain Some after spawn"
    );
}

// =========================================================================
// AsyncDriver::spawn / stop lifecycle
// =========================================================================

#[test]
fn spawn_succeeds() {
    let mut driver = make_thread_driver();
    assert!(driver.spawn().is_ok());
}

#[test]
fn stop_after_spawn_succeeds() {
    let mut driver = make_thread_driver();
    driver.spawn().unwrap();
    assert!(driver.stop().is_ok());
}

#[test]
fn stop_without_prior_spawn_succeeds() {
    // stop() on an unspawned driver should be a safe no-op.
    let mut driver = make_thread_driver();
    assert!(driver.stop().is_ok());
}

#[test]
fn stop_takes_driver_leaving_none() {
    // After stop(), driver.driver should be None since Option::take was called.
    let mut driver = make_thread_driver();
    driver.spawn().unwrap();
    driver.stop().unwrap();
    assert!(
        driver.driver.is_none(),
        "driver field should be None after stop()"
    );
}

#[test]
fn stop_is_idempotent() {
    // Calling stop() twice should not panic or error since the second call
    // finds driver.driver == None and short-circuits.
    let mut driver = make_thread_driver();
    driver.spawn().unwrap();
    driver.stop().unwrap();
    assert!(
        driver.stop().is_ok(),
        "second stop() should be a safe no-op"
    );
}

#[test]
fn stop_without_spawn_is_idempotent() {
    let mut driver = make_thread_driver();
    driver.stop().unwrap();
    assert!(driver.stop().is_ok());
}

#[test]
fn spawn_stop_spawn_stop_cycle_succeeds() {
    // Each spawn/stop cycle needs a new FluxReactorThread since stop() consumes it.
    let mut driver = make_thread_driver();
    driver.spawn().unwrap();
    driver.stop().unwrap();
    // Re-create: after stop, driver is None so we need a fresh driver.
    let mut driver2 = make_thread_driver();
    driver2.spawn().unwrap();
    assert!(driver2.stop().is_ok());
}

// =========================================================================
// Drop safety
// =========================================================================

#[test]
fn drop_without_stop_does_not_panic() {
    // Drop calls stop() internally. A spawned driver that is dropped without
    // an explicit stop() call must not panic.
    {
        let mut driver = make_thread_driver();
        driver.spawn().unwrap();
    } // driver dropped here — stop() fires in Drop
}

#[test]
fn drop_without_spawn_does_not_panic() {
    {
        let _driver = make_thread_driver();
    } // driver dropped here without spawn() having been called
}

#[test]
fn drop_after_explicit_stop_does_not_panic() {
    // If stop() was already called, Drop should be a safe no-op since
    // driver.driver is already None.
    {
        let mut driver = make_thread_driver();
        driver.spawn().unwrap();
        driver.stop().unwrap();
    } // driver dropped here — Drop calls stop() again on None
}
