use flux_sys::core::flux_reactor_destroy;

use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtrNoArgs, IntoFluxPtr};
use crate::reactor::{FluxReactorThread, Reactor, ReactorFlags};

// =========================================================================
// Helpers
// =========================================================================

fn make_reactor() -> Reactor {
    Reactor::new().expect("Failed to create Reactor")
}

// =========================================================================
// ReactorFlags bitflag sanity
// =========================================================================

#[test]
fn reactor_flags_none_is_zero() {
    assert_eq!(ReactorFlags::NONE.bits(), 0);
}

#[test]
fn reactor_flags_nowait_is_nonzero() {
    assert_ne!(ReactorFlags::NOWAIT.bits(), 0);
}

#[test]
fn reactor_flags_once_is_nonzero() {
    assert_ne!(ReactorFlags::ONCE.bits(), 0);
}

#[test]
fn reactor_flags_combination_contains_both_members() {
    let combined = ReactorFlags::NOWAIT | ReactorFlags::ONCE;
    assert!(combined.contains(ReactorFlags::NOWAIT));
    assert!(combined.contains(ReactorFlags::ONCE));
}

// =========================================================================
// Reactor::new
// =========================================================================

#[test]
fn new_succeeds() {
    assert!(Reactor::new().is_ok());
}

// NOTE: FLUX_REACTOR_ONCE is only valid as a flag to flux_reactor_run,
// not to flux_reactor_create. Do not add new_with_once_flag_succeeds.

// =========================================================================
// Reactor::now / time / now_update
// =========================================================================

#[test]
fn now_returns_positive_value() {
    let reactor = make_reactor();
    let t = reactor.now().unwrap();
    assert!(t > 0.0, "Expected positive reactor time, got {t}");
}

#[test]
fn time_returns_positive_value() {
    let reactor = make_reactor();
    let t = reactor.time();
    assert!(t > 0.0, "Expected positive system time, got {t}");
}

#[test]
fn now_update_succeeds() {
    let mut reactor = make_reactor();
    assert!(reactor.now_update().is_ok());
}

#[test]
fn now_after_update_is_within_one_second_of_time() {
    // now() is libev's cached timestamp; time() is the live system clock.
    // After now_update() they must agree to within 1 s.
    let mut reactor = make_reactor();
    reactor.now_update().unwrap();
    let now = reactor.now().unwrap();
    let time = reactor.time();
    assert!(
        (now - time).abs() < 1.0,
        "now ({now}) and time ({time}) diverged by more than 1 second after now_update"
    );
}

// =========================================================================
// Reactor::run
// =========================================================================

#[test]
fn run_with_nowait_returns_ok() {
    // NOWAIT causes flux_reactor_run to return immediately when there are
    // no pending events, so this test must not block.
    let mut reactor = make_reactor();
    assert!(reactor.run(ReactorFlags::NOWAIT).is_ok());
}

#[test]
fn run_with_once_returns_ok() {
    // ONCE processes at most one pending event then returns.
    let mut reactor = make_reactor();
    assert!(reactor.run(ReactorFlags::ONCE).is_ok());
}

// =========================================================================
// Reactor::stop
// =========================================================================

#[test]
fn stop_none_on_idle_reactor_succeeds() {
    let mut reactor = make_reactor();
    assert!(reactor.stop(None).is_ok());
}

#[test]
fn stop_with_raw_os_error_calls_stop_error_path() {
    // error_code has a raw OS error → flux_reactor_stop_error branch.
    let mut reactor = make_reactor();
    let err = std::io::Error::from_raw_os_error(libc::ENOENT);
    assert!(reactor.stop(Some(err)).is_ok());
}

#[test]
fn stop_with_non_os_error_falls_through_to_normal_stop() {
    // error_code has no raw OS error → falls through to flux_reactor_stop.
    let mut reactor = make_reactor();
    let err = std::io::Error::other("custom error");
    assert!(reactor.stop(Some(err)).is_ok());
}

// =========================================================================
// Reactor::clone
//
// Clone is only available when flux_core_has_reactor_ref_count is set,
// meaning the Flux version provides flux_reactor_incref/decref with true
// memory reference counting semantics. On older Flux versions,
// flux_reactor_active_incref only calls ev_ref (a libev hint to prevent
// the loop from stopping) and does NOT prevent flux_reactor_destroy from
// freeing the allocation — making a safe owning Clone impossible.
// =========================================================================

#[test]
fn clone_is_usable() {
    let reactor = make_reactor();
    let clone = reactor.clone();
    assert!(clone.now().is_ok());
}

#[test]
fn clone_and_original_have_independent_stop_calls() {
    let reactor = make_reactor();
    let mut clone = reactor.clone();
    assert!(clone.stop(None).is_ok());
    // original is still alive and valid
    assert!(reactor.now().is_ok());
}

#[test]
fn clone_remains_valid_after_original_dropped() {
    // With true refcounting, the clone keeps the allocation alive after
    // the original is dropped. This test is only sound when
    // flux_reactor_incref/decref provide memory refcounting semantics.
    let clone = {
        let reactor = make_reactor();
        reactor.clone()
        // original dropped here — clone keeps it alive via refcount
    };
    assert!(
        clone.now().is_ok(),
        "Clone should remain valid after original is dropped (refcount > 0)"
    );
}

// =========================================================================
// BorrowFluxPtr / FromFluxPtr
// =========================================================================

#[test]
fn borrow_ptr_creates_non_owning_reactor() {
    let reactor = make_reactor();
    let ptr = reactor.as_mut_ptr();
    let borrowed = unsafe { Reactor::borrow_ptr(ptr) }.unwrap();
    assert!(
        !borrowed.c_reactor.is_owned(),
        "borrow_ptr should produce a non-owning Reactor"
    );
    // `reactor` outlives `borrowed`, so ptr remains valid through this scope.
}

#[test]
fn from_ptr_creates_owning_reactor() {
    // Consume the Reactor via IntoFluxPtr to transfer raw pointer ownership
    // cleanly (avoids a partial move of the c_reactor field).
    let reactor = make_reactor();
    let ptr = reactor.into_raw();
    let owned = unsafe { Reactor::from_ptr(ptr) }.unwrap();
    assert!(
        owned.c_reactor.is_owned(),
        "from_ptr should produce an owning Reactor"
    );
}

#[test]
fn borrowed_reactor_is_usable() {
    let reactor = make_reactor();
    let ptr = reactor.as_mut_ptr();
    let borrowed = unsafe { Reactor::borrow_ptr(ptr) }.unwrap();
    assert!(borrowed.now().is_ok());
}

// =========================================================================
// AsFluxPtr / IntoFluxPtr
// =========================================================================

#[test]
fn as_mut_ptr_returns_non_null() {
    let reactor = make_reactor();
    assert!(!reactor.as_mut_ptr().is_null());
}

#[test]
fn into_raw_returns_non_null_and_suppresses_destructor() {
    let reactor = make_reactor();
    let ptr = reactor.into_raw();
    assert!(!ptr.is_null());
    // Manually clean up to avoid leaking the C allocation.
    unsafe { flux_reactor_destroy(ptr) };
}

// =========================================================================
// FluxReactorThread
// =========================================================================

#[test]
fn reactor_thread_new_succeeds() {
    let reactor = make_reactor();
    let _thread = FluxReactorThread::new(reactor);
    // Construction must not panic.
}

#[test]
fn reactor_thread_stop_without_spawn_succeeds() {
    let reactor = make_reactor();
    let mut thread = FluxReactorThread::new(reactor);
    assert!(thread.stop().is_ok());
}

#[test]
fn reactor_thread_spawn_then_stop_succeeds() {
    let reactor = make_reactor();
    let mut thread = FluxReactorThread::new(reactor);
    thread.spawn().unwrap();
    assert!(thread.stop().is_ok());
}

#[test]
fn reactor_thread_spawn_then_stop_twice_is_idempotent() {
    let reactor = make_reactor();
    let mut thread = FluxReactorThread::new(reactor);
    thread.spawn().unwrap();
    thread.stop().unwrap();
    // Second stop should be a safe no-op since handle is already None.
    assert!(thread.stop().is_ok());
}

#[test]
fn reactor_thread_drop_does_not_panic() {
    let reactor = make_reactor();
    let mut thread = FluxReactorThread::new(reactor);
    thread.spawn().unwrap();
    // Drop fires stop() internally — must not panic.
}
