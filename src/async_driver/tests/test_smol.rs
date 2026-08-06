use std::sync::{Arc, Mutex};

use crate::async_driver::AsyncDriver;
use crate::async_driver::smol::SmolDriver;
use crate::error::FluxError;
use crate::handle::FluxHandle;
use crate::handle::HandleFlags;
use crate::reactor::{FluxReactorThread, Reactor, ReactorFlags};

// =========================================================================
// Helpers
// =========================================================================

fn make_shared_handle() -> Arc<Mutex<FluxHandle>> {
    Arc::new(Mutex::new(
        FluxHandle::new_from_str_uri("", HandleFlags::NONE)
            .expect("Failed to open FluxHandle for SmolDriver test"),
    ))
}

fn make_reactor_thread() -> FluxReactorThread {
    let reactor =
        Reactor::new(ReactorFlags::NONE).expect("Failed to create Reactor for SmolDriver test");
    FluxReactorThread::new(reactor)
}

// =========================================================================
// TryFrom<Arc<Mutex<FluxHandle>>>
// =========================================================================

#[test]
fn try_from_shared_handle_succeeds() {
    let shared = make_shared_handle();
    assert!(SmolDriver::try_from(shared).is_ok());
}

#[test]
fn try_from_shared_handle_stores_handle() {
    let shared = make_shared_handle();
    let driver = SmolDriver::try_from(shared).unwrap();
    assert!(
        driver.handle.is_some(),
        "handle field should be Some after construction from Arc<Mutex<FluxHandle>>"
    );
}

#[test]
fn try_from_shared_handle_task_handle_is_none() {
    let shared = make_shared_handle();
    let driver = SmolDriver::try_from(shared).unwrap();
    assert!(
        driver.task_handle.is_none(),
        "task_handle field should be None before spawn() is called"
    );
}

// =========================================================================
// TryFrom<FluxReactorThread>
// =========================================================================

#[test]
fn try_from_reactor_thread_always_returns_logic_error() {
    assert!(matches!(
        SmolDriver::try_from(make_reactor_thread()),
        Err(FluxError::Logic(_))
    ));
}

#[test]
fn try_from_reactor_thread_error_message_mentions_reactor_thread() {
    match SmolDriver::try_from(make_reactor_thread()) {
        Err(FluxError::Logic(msg)) => {
            assert!(
                msg.contains("FluxReactorThread") || msg.contains("reactor"),
                "Expected error to mention FluxReactorThread, got: {msg}"
            );
        }
        Ok(_) => panic!("Expected FluxError::Logic, got actual SmolDriver object"),
        Err(other) => panic!("Expected FluxError::Logic, got {:?}", other),
    }
}

// =========================================================================
// AsyncDriver::spawn_reactor_thread (default method — always fails)
// =========================================================================

#[test]
fn spawn_reactor_thread_always_returns_logic_error() {
    let reactor = Reactor::new(ReactorFlags::NONE).unwrap();
    assert!(matches!(
        SmolDriver::spawn_reactor_thread(reactor),
        Err(FluxError::Logic(_))
    ));
}

// =========================================================================
// AsyncDriver::spawn / stop lifecycle
// =========================================================================

#[test]
fn spawn_succeeds() {
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    assert!(driver.spawn().is_ok());
    driver.stop().unwrap();
}

#[test]
fn spawn_takes_handle_leaving_none() {
    // spawn() calls self.handle.take(), so handle should be None afterwards.
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    driver.spawn().unwrap();
    assert!(
        driver.handle.is_none(),
        "handle field should be None after spawn() — it is consumed via Option::take"
    );
    driver.stop().unwrap();
}

#[test]
fn spawn_sets_task_handle() {
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    driver.spawn().unwrap();
    assert!(
        driver.task_handle.is_some(),
        "task_handle should be Some after spawn()"
    );
    driver.stop().unwrap();
}

#[test]
fn stop_after_spawn_succeeds() {
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    driver.spawn().unwrap();
    assert!(driver.stop().is_ok());
}

#[test]
fn stop_clears_task_handle() {
    // After stop(), task_handle should be None since the Task was dropped.
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    driver.spawn().unwrap();
    driver.stop().unwrap();
    assert!(
        driver.task_handle.is_none(),
        "task_handle should be None after stop()"
    );
}

#[test]
fn stop_without_spawn_succeeds() {
    // stop() on an unspawned driver should be a safe no-op since task_handle is None.
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    assert!(driver.stop().is_ok());
}

#[test]
fn stop_is_idempotent() {
    // After stop(), task_handle is None, so a second stop() is a safe no-op.
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    driver.spawn().unwrap();
    driver.stop().unwrap();
    assert!(
        driver.stop().is_ok(),
        "second stop() should be a safe no-op"
    );
}

#[test]
fn stop_without_spawn_is_idempotent() {
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    driver.stop().unwrap();
    assert!(driver.stop().is_ok());
}

#[test]
fn second_spawn_fails_with_logic_error() {
    // spawn() takes self.handle via Option::take, so a second call finds handle == None
    // and must return FluxError::Logic.
    let shared = make_shared_handle();
    let mut driver = SmolDriver::try_from(shared).unwrap();
    driver.spawn().unwrap();
    assert!(
        matches!(driver.spawn(), Err(FluxError::Logic(_))),
        "second spawn() should fail with FluxError::Logic since handle was taken"
    );
    driver.stop().unwrap();
}

// =========================================================================
// AsyncDriver::spawn_async_driver (default method)
// =========================================================================

#[test]
fn spawn_async_driver_succeeds() {
    let shared = make_shared_handle();
    let result = SmolDriver::spawn_async_driver(shared);
    assert!(result.is_ok());
    result.unwrap().stop().unwrap();
}

#[test]
fn spawn_async_driver_handle_is_none() {
    // spawn_async_driver calls spawn() which consumes the handle field.
    let shared = make_shared_handle();
    let driver = SmolDriver::spawn_async_driver(shared).unwrap();
    assert!(
        driver.handle.is_none(),
        "handle should be None after spawn_async_driver — spawn() consumed it"
    );
    // stop is called implicitly via Drop
}

#[test]
fn spawn_async_driver_task_handle_is_some() {
    let shared = make_shared_handle();
    let driver = SmolDriver::spawn_async_driver(shared).unwrap();
    assert!(
        driver.task_handle.is_some(),
        "task_handle should be Some after spawn_async_driver"
    );
    // stop is called implicitly via Drop
}

// =========================================================================
// Drop safety
// =========================================================================

#[test]
fn drop_without_stop_does_not_panic() {
    // Drop calls stop() internally via the Drop impl. A spawned driver that
    // is dropped without an explicit stop() call must not panic.
    {
        let mut driver = SmolDriver::try_from(make_shared_handle()).unwrap();
        driver.spawn().unwrap();
    } // driver dropped here — stop() fires in Drop
}

#[test]
fn drop_without_spawn_does_not_panic() {
    {
        let _driver = SmolDriver::try_from(make_shared_handle()).unwrap();
    } // driver dropped here without spawn() having been called
}

#[test]
fn drop_after_explicit_stop_does_not_panic() {
    // If stop() was already called, Drop should be a safe no-op since
    // task_handle is already None.
    {
        let mut driver = SmolDriver::try_from(make_shared_handle()).unwrap();
        driver.spawn().unwrap();
        driver.stop().unwrap();
    } // driver dropped here — Drop calls stop() again on None
}
