use std::collections::HashMap;
use std::sync::Arc;

use flux_sys::core::{flux_future_destroy, flux_future_incref, flux_future_t};

use crate::error::FluxError;
use crate::flux_ptr_management::{
    BorrowFluxPtrNoArgs, Borrowed, FromFluxPtrNoArgs, IntoFluxPtr, Owned,
};
use crate::future::sync_future::{FluxFuture, create_wait_all_future, create_wait_any_future};
use crate::reactor::Reactor;
use crate::tests::common::with_handle;

// =========================================================================
// Helpers
// =========================================================================

fn make_wait_all() -> FluxFuture {
    create_wait_all_future::<Owned<flux_future_t>>(HashMap::new())
        .expect("Failed to create empty wait_all future")
}

fn make_wait_any() -> FluxFuture {
    create_wait_any_future::<Owned<flux_future_t>>(HashMap::new())
        .expect("Failed to create empty wait_any future")
}

/// Build a wait_all future that has one unfulfilled child under the given name.
fn make_wait_all_with_child(name: &str) -> FluxFuture {
    let child = make_wait_all();
    let mut map = HashMap::new();
    map.insert(name.to_string(), child);
    create_wait_all_future(map).expect("Failed to create wait_all future with child")
}

#[derive(Debug, PartialEq)]
struct TestAux {
    value: u32,
    label: String,
}

// =========================================================================
// FluxFuture::new
// =========================================================================

#[test]
fn new_with_noop_callback_succeeds() {
    assert!(FluxFuture::new(|_| {}).is_ok());
}

#[test]
fn new_produces_non_null_future() {
    let future = FluxFuture::new(|_| {}).unwrap();
    assert!(!future.c_future.as_mut_ptr().is_null());
}

#[test]
fn new_stores_init_cb() {
    let future = FluxFuture::new(|_| {}).unwrap();
    assert!(future._init_cb.is_some());
}

#[test]
fn new_future_is_owning() {
    let future = FluxFuture::new(|_| {}).unwrap();
    assert!(future.c_future.is_owned());
}

// =========================================================================
// _init_cb preservation through Clone and to_owned
// =========================================================================

#[test]
fn clone_of_new_future_preserves_init_cb() {
    let future = FluxFuture::new(|_| {}).unwrap();
    let clone = future.clone();
    assert!(clone._init_cb.is_some());
}

#[test]
fn clone_init_cb_shares_same_arc() {
    let future = FluxFuture::new(|_| {}).unwrap();
    let clone = future.clone();
    assert!(Arc::ptr_eq(
        future._init_cb.as_ref().unwrap(),
        clone._init_cb.as_ref().unwrap()
    ));
}

#[test]
fn clone_of_wait_all_has_no_init_cb() {
    // Futures not created via new() propagate their None _init_cb through clone.
    let future = make_wait_all();
    let clone = future.clone();
    assert!(clone._init_cb.is_none());
}

#[test]
fn to_owned_of_new_future_preserves_init_cb() {
    let future = FluxFuture::new(|_| {}).unwrap();
    let owned = future.to_owned().unwrap();
    assert!(owned._init_cb.is_some());
}

#[test]
fn to_owned_init_cb_shares_same_arc() {
    let future = FluxFuture::new(|_| {}).unwrap();
    let owned = future.to_owned().unwrap();
    assert!(Arc::ptr_eq(
        future._init_cb.as_ref().unwrap(),
        owned._init_cb.as_ref().unwrap()
    ));
}

#[test]
fn to_owned_of_wait_all_has_no_init_cb() {
    let future = make_wait_all();
    let owned = future.to_owned().unwrap();
    assert!(owned._init_cb.is_none());
}

#[test]
fn borrow_ptr_has_no_init_cb() {
    let future = make_wait_all();
    let ptr = future.c_future.as_mut_ptr();
    let borrowed = unsafe { FluxFuture::borrow_ptr(ptr) }.unwrap();
    assert!(borrowed._init_cb.is_none());
}

#[test]
fn from_ptr_has_no_init_cb() {
    let future = make_wait_all();
    let ptr = future.c_future.as_mut_ptr();
    // incref so both future and owned can drop cleanly
    unsafe { flux_future_incref(ptr) };
    let owned = unsafe { FluxFuture::from_ptr(ptr) }.unwrap();
    assert!(owned._init_cb.is_none());
}

#[test]
fn get_child_has_no_init_cb() {
    let future = make_wait_all_with_child("child");
    let child = future.get_child("child").unwrap().unwrap();
    assert!(child._init_cb.is_none());
}

// =========================================================================
// set_aux / get_aux / get_aux_raw
// =========================================================================

#[test]
fn set_aux_and_get_aux_round_trips_struct() {
    let mut future = make_wait_all();
    let data = TestAux {
        value: 42,
        label: "hello".to_string(),
    };
    future.set_aux("test.aux.struct", data).unwrap();
    let retrieved = future.get_aux::<TestAux>("test.aux.struct").unwrap();
    assert_eq!(retrieved.value, 42);
    assert_eq!(retrieved.label, "hello");
}

#[test]
fn set_aux_and_get_aux_round_trips_primitive() {
    let mut future = make_wait_all();
    future.set_aux("test.aux.u32", 99u32).unwrap();
    let retrieved = future.get_aux::<u32>("test.aux.u32").unwrap();
    assert_eq!(*retrieved, 99u32);
}

#[test]
fn set_aux_nul_byte_key_returns_error() {
    let mut future = make_wait_all();
    assert!(future.set_aux("bad\0key", 42u32).is_err());
}

#[test]
fn get_aux_missing_key_returns_error() {
    let future = make_wait_all();
    assert!(future.get_aux::<u32>("nonexistent.key").is_err());
}

#[test]
fn get_aux_type_mismatch_returns_logic_error() {
    let mut future = make_wait_all();
    future.set_aux("test.typed", 42u32).unwrap();
    assert!(matches!(
        future.get_aux::<String>("test.typed"),
        Err(FluxError::Logic(_))
    ));
}

#[test]
fn get_aux_raw_returns_non_null_after_set() {
    let mut future = make_wait_all();
    future.set_aux("test.raw", 42u32).unwrap();
    let ptr = future.get_aux_raw("test.raw").unwrap();
    assert!(!ptr.is_null());
}

#[test]
fn get_aux_raw_nul_byte_key_returns_error() {
    let future = make_wait_all();
    assert!(future.get_aux_raw("bad\0key").is_err());
}

// =========================================================================
// set_reactor / get_reactor
// =========================================================================

#[test]
fn set_reactor_and_get_reactor_succeeds() {
    let mut future = make_wait_all();
    let reactor = Reactor::new().unwrap();
    future.set_reactor(&reactor).unwrap();
    assert!(unsafe { future.get_reactor().is_ok() });
}

#[test]
fn get_reactor_after_set_produces_non_owning_reactor() {
    let mut future = make_wait_all();
    let reactor = Reactor::new().unwrap();
    future.set_reactor(&reactor).unwrap();
    let borrowed = unsafe { future.get_reactor().unwrap() };
    assert!(!borrowed.c_reactor.is_owned());
}

#[test]
fn get_reactor_after_set_is_usable() {
    let mut future = make_wait_all();
    let reactor = Reactor::new().unwrap();
    future.set_reactor(&reactor).unwrap();
    let borrowed = unsafe { future.get_reactor().unwrap() };
    assert!(borrowed.now().is_ok());
}

// =========================================================================
// set_flux / get_flux
// =========================================================================

#[test]
fn set_flux_and_get_flux_succeeds() {
    with_handle(|h| {
        let mut future = make_wait_all();
        future.set_flux(h);
        assert!(unsafe { future.get_flux().is_ok() });
    });
}

#[test]
fn get_flux_after_set_produces_non_owning_handle() {
    with_handle(|h| {
        let mut future = make_wait_all();
        future.set_flux(h);
        let borrowed = unsafe { future.get_flux().unwrap() };
        assert!(!borrowed.h.is_owned());
    });
}

#[test]
fn get_flux_after_set_is_usable() {
    with_handle(|h| {
        let mut future = make_wait_all();
        future.set_flux(h);
        let borrowed = unsafe { future.get_flux().unwrap() };
        assert!(borrowed.get_rank().is_ok());
    });
}

// =========================================================================
// create_wait_all_future
// =========================================================================

#[test]
fn create_wait_all_empty_map_succeeds() {
    assert!(create_wait_all_future::<Owned<flux_future_t>>(HashMap::new()).is_ok());
    assert!(create_wait_all_future::<Borrowed<'_, flux_future_t>>(HashMap::new()).is_ok());
}

#[test]
fn create_wait_all_with_one_child_succeeds() {
    let child = make_wait_all();
    let mut map = HashMap::new();
    map.insert("child".to_string(), child);
    assert!(create_wait_all_future(map).is_ok());
}

#[test]
fn create_wait_all_with_multiple_children_succeeds() {
    let mut map = HashMap::new();
    map.insert("a".to_string(), make_wait_all());
    map.insert("b".to_string(), make_wait_all());
    map.insert("c".to_string(), make_wait_all());
    assert!(create_wait_all_future(map).is_ok());
}

#[test]
fn create_wait_all_nul_byte_child_name_returns_error() {
    let mut map = HashMap::new();
    map.insert("bad\0name".to_string(), make_wait_all());
    assert!(create_wait_all_future(map).is_err());
}

// =========================================================================
// create_wait_any_future
// =========================================================================

#[test]
fn create_wait_any_empty_map_succeeds() {
    assert!(create_wait_any_future::<Owned<flux_future_t>>(HashMap::new()).is_ok());
    assert!(create_wait_any_future::<Borrowed<'_, flux_future_t>>(HashMap::new()).is_ok());
}

#[test]
fn create_wait_any_with_one_child_succeeds() {
    let child = make_wait_any();
    let mut map = HashMap::new();
    map.insert("child".to_string(), child);
    assert!(create_wait_any_future(map).is_ok());
}

#[test]
fn create_wait_any_nul_byte_child_name_returns_error() {
    let mut map = HashMap::new();
    map.insert("bad\0name".to_string(), make_wait_any());
    assert!(create_wait_any_future(map).is_err());
}

// =========================================================================
// Clone
// =========================================================================

#[test]
fn clone_produces_non_null_future() {
    let future = make_wait_all();
    let clone = future.clone();
    assert!(!clone.c_future.as_mut_ptr().is_null());
}

#[test]
fn clone_and_original_have_same_pointer() {
    // incref-based clone shares the same underlying flux_future_t.
    let future = make_wait_all();
    let clone = future.clone();
    assert_eq!(future.c_future.as_mut_ptr(), clone.c_future.as_mut_ptr());
}

#[test]
fn clone_remains_valid_after_original_dropped() {
    let clone = {
        let future = make_wait_all();
        future.clone()
    }; // original dropped here; clone holds a ref
    assert!(!clone.c_future.as_mut_ptr().is_null());
}

// =========================================================================
// to_owned
// =========================================================================

#[test]
fn to_owned_succeeds() {
    let future = make_wait_all();
    assert!(future.to_owned().is_ok());
}

#[test]
fn to_owned_produces_non_null_future() {
    let future = make_wait_all();
    let owned = future.to_owned().unwrap();
    assert!(!owned.c_future.as_mut_ptr().is_null());
}

#[test]
fn to_owned_shares_pointer_with_source() {
    // to_owned uses incref; the pointer should be identical.
    let future = make_wait_all();
    let owned = future.to_owned().unwrap();
    assert_eq!(future.c_future.as_mut_ptr(), owned.c_future.as_mut_ptr());
}

#[test]
fn to_owned_remains_valid_after_source_dropped() {
    let owned: FluxFuture = {
        let future = make_wait_all();
        future.to_owned().unwrap()
    };
    assert!(!owned.c_future.as_mut_ptr().is_null());
}

// =========================================================================
// first_child / next_child
// =========================================================================

#[test]
fn first_child_on_empty_collective_returns_none() {
    let future = make_wait_all();
    assert!(future.first_child().unwrap().is_none());
}

#[test]
fn first_child_on_populated_collective_returns_some() {
    let future = make_wait_all_with_child("alpha");
    assert!(future.first_child().unwrap().is_some());
}

#[test]
fn first_child_name_matches_inserted_name() {
    let future = make_wait_all_with_child("myname");
    assert_eq!(future.first_child().unwrap().as_deref(), Some("myname"));
}

#[test]
fn next_child_on_empty_collective_returns_none() {
    let future = make_wait_all();
    future.first_child().unwrap(); // initialise the cursor
    assert!(future.next_child().unwrap().is_none());
}

#[test]
fn next_child_after_single_child_returns_none() {
    // first_child returns the only entry; next_child should exhaust the cursor.
    let future = make_wait_all_with_child("only");
    future.first_child().unwrap();
    assert!(future.next_child().unwrap().is_none());
}

#[test]
fn next_child_iterates_all_children() {
    let mut map = HashMap::new();
    map.insert("x".to_string(), make_wait_all());
    map.insert("y".to_string(), make_wait_all());
    let future = create_wait_all_future(map).unwrap();

    let first = future.first_child().unwrap();
    assert!(first.is_some());
    let second = future.next_child().unwrap();
    assert!(second.is_some());
    let third = future.next_child().unwrap();
    assert!(third.is_none());
}

// =========================================================================
// get_child
// =========================================================================

#[test]
fn get_child_nonexistent_returns_none() {
    let future = make_wait_all_with_child("real");
    assert!(future.get_child("nonexistent").unwrap().is_none());
}

#[test]
fn get_child_existing_returns_some() {
    let future = make_wait_all_with_child("present");
    assert!(future.get_child("present").unwrap().is_some());
}

#[test]
fn get_child_nul_byte_name_returns_error() {
    let future = make_wait_all_with_child("good");
    assert!(future.get_child("bad\0name").is_err());
}

#[test]
fn get_child_pointer_is_borrowed_not_owned() {
    // get_child uses BorrowFluxPtr; the returned future should be non-owning
    // so that the parent's destructor remains responsible for the child.
    let future = make_wait_all_with_child("child");
    let child = future.get_child("child").unwrap().unwrap();
    assert!(!child.c_future.is_owned());
}

// =========================================================================
// BorrowFluxPtr / FromFluxPtr
// =========================================================================

#[test]
fn borrow_ptr_produces_non_owning_future() {
    let future = make_wait_all();
    let ptr = future.c_future.as_mut_ptr();
    let borrowed = unsafe { FluxFuture::borrow_ptr(ptr) }.unwrap();
    assert!(!borrowed.c_future.is_owned());
}

#[test]
fn borrow_ptr_null_returns_error() {
    let result = unsafe { FluxFuture::borrow_ptr(std::ptr::null_mut()) };
    assert!(result.is_err());
}

#[test]
fn from_ptr_produces_owning_future() {
    let future = make_wait_all();
    let ptr = future.c_future.as_mut_ptr();
    // incref so both future and owned drop cleanly
    unsafe { flux_future_incref(ptr) };
    let owned = unsafe { FluxFuture::from_ptr(ptr) }.unwrap();
    assert!(owned.c_future.is_owned());
}

#[test]
fn from_ptr_null_returns_error() {
    let result = unsafe { FluxFuture::from_ptr(std::ptr::null_mut()) };
    assert!(result.is_err());
}

#[test]
fn borrowed_future_first_child_is_usable() {
    let future = make_wait_all_with_child("test");
    let ptr = future.c_future.as_mut_ptr();
    let borrowed = unsafe { FluxFuture::borrow_ptr(ptr) }.unwrap();
    assert!(borrowed.first_child().unwrap().is_some());
}

// =========================================================================
// AsFluxPtr / IntoFluxPtr
// =========================================================================

#[test]
fn as_mut_ptr_returns_non_null() {
    let future = make_wait_all();
    assert!(!future.c_future.as_mut_ptr().is_null());
}

#[test]
fn as_mut_ptr_and_into_raw_return_same_pointer() {
    let future = make_wait_all();
    let ptr = future.c_future.as_mut_ptr();
    let raw = future.into_raw();
    assert_eq!(ptr, raw);
    unsafe { flux_future_destroy(raw) };
}

#[test]
fn into_raw_returns_non_null_and_suppresses_destructor() {
    let future = make_wait_all();
    let ptr = future.into_raw();
    assert!(!ptr.is_null());
    unsafe { flux_future_destroy(ptr) };
}

// =========================================================================
// check_error
// =========================================================================

#[test]
fn check_error_on_fresh_future_returns_ok() {
    let future = make_wait_all();
    assert!(future.check_error().is_ok());
}

// =========================================================================
// reset
// =========================================================================

#[test]
fn reset_on_fresh_future_succeeds() {
    let mut future = make_wait_all();
    assert!(future.reset().is_ok());
}

#[test]
fn reset_on_fulfilled_future_succeeds() {
    let mut future = make_wait_all();
    future.fulfill(Some(Box::new(42u32))).unwrap();
    assert!(future.reset().is_ok());
}

// =========================================================================
// fulfill / get
// =========================================================================

#[test]
fn fulfill_then_get_returns_stored_pointer() {
    let mut future = make_wait_all();
    future.fulfill(Some(Box::new(42u32))).unwrap();
    let result = future.get().unwrap();
    assert!(!result.is_null());
}

#[test]
fn fulfill_with_data_then_get_returns_nonnull() {
    let mut future = make_wait_all();
    future.fulfill(Some(Box::new(99u64))).unwrap();
    assert!(!future.get().unwrap().is_null());
}

// =========================================================================
// fulfill_error / check_error
// =========================================================================

#[test]
fn fulfill_error_with_errstr_then_check_error_returns_err() {
    let mut future = make_wait_all();
    future
        .fulfill_error(libc::ENOENT, Some("not found"))
        .unwrap();
    assert!(future.check_error().is_err());
}

#[test]
fn fulfill_error_without_errstr_then_check_error_returns_logic_error() {
    // No error string → flux_future_error_string returns NULL → Logic error
    let mut future = make_wait_all();
    future.fulfill_error(libc::ENOENT, None).unwrap();
    assert!(matches!(future.check_error(), Err(FluxError::Logic(_))));
}

#[test]
fn fulfill_error_message_is_preserved_in_check_error() {
    let mut future = make_wait_all();
    future
        .fulfill_error(libc::ENOENT, Some("custom message"))
        .unwrap();
    match future.check_error() {
        Err(FluxError::Logic(msg)) => {
            assert!(msg.contains("custom message"), "Expected message in: {msg}")
        }
        other => panic!("Expected Logic error, got {:?}", other),
    }
}

#[test]
fn fulfill_error_nul_byte_errstr_returns_error() {
    let mut future = make_wait_all();
    assert!(
        future
            .fulfill_error(libc::ENOENT, Some("bad\0str"))
            .is_err()
    );
}

// =========================================================================
// fatal_error / check_error
// =========================================================================

#[test]
fn fatal_error_with_errstr_then_check_error_returns_err() {
    let mut future = make_wait_all();
    future.fatal_error(libc::EPERM, Some("fatal")).unwrap();
    assert!(future.check_error().is_err());
}

#[test]
fn fatal_error_without_errstr_then_check_error_returns_err() {
    let mut future = make_wait_all();
    future.fatal_error(libc::EPERM, None).unwrap();
    assert!(future.check_error().is_err());
}

#[test]
fn fatal_error_nul_byte_errstr_returns_error() {
    let mut future = make_wait_all();
    assert!(future.fatal_error(libc::EPERM, Some("bad\0str")).is_err());
}

// =========================================================================
// continue_with_error
// =========================================================================

#[test]
fn continue_with_error_without_errstr_succeeds() {
    let mut future = make_wait_all();
    assert!(future.continue_with_error(libc::ENOENT, None).is_ok());
}

#[test]
fn continue_with_error_with_errstr_succeeds() {
    let mut future = make_wait_all();
    assert!(
        future
            .continue_with_error(libc::ENOENT, Some("error"))
            .is_ok()
    );
}

#[test]
fn continue_with_error_nul_byte_errstr_returns_error() {
    let mut future = make_wait_all();
    assert!(
        future
            .continue_with_error(libc::ENOENT, Some("bad\0str"))
            .is_err()
    );
}

// =========================================================================
// fulfill_with
// =========================================================================

#[test]
fn fulfill_with_succeeds() {
    let mut future = make_wait_all();
    let other = make_wait_all();
    assert!(future.fulfill_with(&other).is_ok());
}

// =========================================================================
// fulfill_next
// =========================================================================

#[test]
fn fulfill_next_on_non_streaming_future_returns_false() {
    // Non-streaming futures return EINVAL from flux_future_fulfill_next,
    // which the wrapper maps to Ok(false).
    let mut future = make_wait_all();
    let result = future.fulfill_next(Box::new(42u32)).unwrap();
    assert!(!result);
}

// =========================================================================
// wait_for
// =========================================================================

#[test]
fn wait_for_zero_timeout_on_unfulfilled_child_returns_false() {
    // An empty wait_all with one unfulfilled child can never complete,
    // so wait_for(0.0) reliably times out.
    let future = make_wait_all_with_child("pending");
    assert!(!future.wait_for(0.0).unwrap());
}

#[test]
fn wait_for_zero_timeout_on_fulfilled_future_returns_true() {
    let mut future = make_wait_all();
    future.fulfill(Some(Box::new(42u32))).unwrap();
    assert!(future.wait_for(0.0).unwrap());
}
