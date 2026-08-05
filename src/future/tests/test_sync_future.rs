use std::collections::HashMap;

use flux_sys::core::flux_future_destroy;

use crate::error::FluxError;
use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtrNoArgs, IntoFluxPtr};
use crate::future::sync_future::{create_wait_all_future, create_wait_any_future, FluxFuture};

// =========================================================================
// Helpers
// =========================================================================

fn make_wait_all() -> FluxFuture<'static> {
    create_wait_all_future(HashMap::new()).expect("Failed to create empty wait_all future")
}

fn make_wait_any() -> FluxFuture<'static> {
    create_wait_any_future(HashMap::new()).expect("Failed to create empty wait_any future")
}

/// Build a wait_all future that has one unfulfilled child under the given name.
fn make_wait_all_with_child(name: &str) -> FluxFuture<'static> {
    let child = make_wait_all();
    let mut map = HashMap::new();
    map.insert(name.to_string(), child);
    create_wait_all_future(map).expect("Failed to create wait_all future with child")
}

// =========================================================================
// create_wait_all_future
// =========================================================================

#[test]
fn create_wait_all_empty_map_succeeds() {
    assert!(create_wait_all_future(HashMap::new()).is_ok());
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
    assert!(create_wait_any_future(HashMap::new()).is_ok());
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
    let owned: FluxFuture<'static> = {
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
    assert!(
        !child.c_future.is_owned(),
        "get_child should return a borrowed (non-owning) FluxFuture"
    );
}

// =========================================================================
// fulfill / get
//
// flux_future_fulfill marks the future as ready and stores the result.
// flux_future_get returns immediately if the future is already ready,
// without needing a reactor tick.
// =========================================================================

#[test]
fn fulfill_then_get_returns_stored_pointer() {
    let mut future = make_wait_all();
    let data: Box<u32> = Box::new(42u32);
    let expected_ptr = &*data as *const u32 as *const std::ffi::c_void;
    future.fulfill(data).unwrap();
    let result = future.get().unwrap();
    assert_eq!(result, expected_ptr);
}

#[test]
fn fulfill_with_data_then_get_returns_nonnull() {
    let mut future = make_wait_all();
    future.fulfill(Box::new(0u32)).unwrap();
    let ptr = future.get().unwrap();
    assert!(!ptr.is_null());
}

// =========================================================================
// fulfill_error / check_error
// =========================================================================

#[test]
fn check_error_on_fresh_future_returns_ok() {
    let mut future = make_wait_all();
    assert!(future.check_error().is_ok());
}

#[test]
fn fulfill_error_with_errstr_then_check_error_returns_err() {
    let mut future = make_wait_all();
    future
        .fulfill_error(libc::ENOENT, Some("file not found"))
        .unwrap();
    assert!(future.check_error().is_err());
}

#[test]
fn fulfill_error_message_is_preserved_in_check_error() {
    let mut future = make_wait_all();
    future
        .fulfill_error(libc::ENOENT, Some("file not found"))
        .unwrap();
    match future.check_error() {
        Err(FluxError::Logic(msg)) => {
            assert!(
                msg.contains("file not found"),
                "Expected 'file not found' in: {msg}"
            );
        }
        other => panic!("Expected FluxError::Logic, got {:?}", other),
    }
}

#[test]
fn fulfill_error_without_errstr_then_check_error_returns_logic_error() {
    // flux_future_error_string returns NULL when no errstr was provided;
    // check_error maps this to FluxError::Logic("Error occurred, but no error string is provided.")
    let mut future = make_wait_all();
    future.fulfill_error(libc::ENOENT, None).unwrap();
    assert!(matches!(future.check_error(), Err(FluxError::Logic(_))));
}

#[test]
fn fulfill_error_nul_byte_errstr_returns_error() {
    let mut future = make_wait_all();
    assert!(future
        .fulfill_error(libc::ENOENT, Some("bad\0msg"))
        .is_err());
}

// =========================================================================
// fatal_error / check_error
// =========================================================================

#[test]
fn fatal_error_with_errstr_then_check_error_returns_err() {
    let mut future = make_wait_all();
    future
        .fatal_error(libc::EPERM, Some("fatal error"))
        .unwrap();
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
    assert!(future.fatal_error(libc::EPERM, Some("bad\0msg")).is_err());
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
//
// NOTE: The source has a memory leak in this method — when
// flux_future_fulfill_next returns -1 with EINVAL (i.e., the future is not
// a streaming future), `raw_ptr` is never recovered via Box::from_raw before
// returning Ok(false). The same applies to the Err(...) path. Fix:
//
//   if rc == -1 {
//       let _ = unsafe { Box::from_raw(raw_ptr as *mut T) };
//       ...
//   }
// =========================================================================

#[test]
fn fulfill_next_on_non_streaming_future_returns_false() {
    // flux_future_fulfill_next returns EINVAL on non-streaming futures,
    // which the wrapper maps to Ok(false).
    let mut future = make_wait_all();
    // NOTE: the Box passed here is leaked due to the bug described above.
    let data: Box<u32> = Box::new(0u32);
    assert!(matches!(future.fulfill_next(data), Ok(false)));
}

// =========================================================================
// wait_for
// =========================================================================

#[test]
fn wait_for_zero_timeout_on_unfulfilled_child_returns_false() {
    // A wait_all future with an unfulfilled child cannot complete;
    // wait_for(0.0) should time out and return Ok(false).
    let mut future = make_wait_all_with_child("pending");
    assert!(matches!(future.wait_for(0.0), Ok(false)));
}

#[test]
fn wait_for_zero_timeout_on_fulfilled_future_returns_true() {
    // After explicit fulfill, the future is immediately ready;
    // wait_for(0.0) should return Ok(true) without blocking.
    let mut future = make_wait_all();
    future.fulfill(Box::new(0u32)).unwrap();
    assert!(matches!(future.wait_for(0.0), Ok(true)));
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
    future.fulfill(Box::new(0u32)).unwrap();
    assert!(future.reset().is_ok());
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
    assert!(future
        .continue_with_error(libc::ENOENT, Some("error msg"))
        .is_ok());
}

#[test]
fn continue_with_error_nul_byte_errstr_returns_error() {
    let mut future = make_wait_all();
    assert!(future
        .continue_with_error(libc::ENOENT, Some("bad\0str"))
        .is_err());
}

// =========================================================================
// BorrowFluxPtr / FromFluxPtr
// =========================================================================

#[test]
fn borrow_ptr_produces_non_owning_future() {
    let future = make_wait_all();
    let ptr = future.c_future.as_mut_ptr();
    let borrowed = unsafe { FluxFuture::borrow_ptr(ptr) }.unwrap();
    assert!(
        !borrowed.c_future.is_owned(),
        "borrow_ptr should produce a non-owning FluxFuture"
    );
    // `future` outlives `borrowed` so ptr remains valid through this scope.
}

#[test]
fn from_ptr_produces_owning_future() {
    let future = make_wait_all();
    let ptr = future.into_raw();
    let owned = unsafe { FluxFuture::from_ptr(ptr) }.unwrap();
    assert!(
        owned.c_future.is_owned(),
        "from_ptr should produce an owning FluxFuture"
    );
}

#[test]
fn borrowed_future_first_child_is_usable() {
    let future = make_wait_all_with_child("child");
    let ptr = future.c_future.as_mut_ptr();
    let borrowed = unsafe { FluxFuture::borrow_ptr(ptr) }.unwrap();
    assert!(borrowed.first_child().is_ok());
}

#[test]
fn borrow_ptr_null_returns_error() {
    let result = unsafe { FluxFuture::borrow_ptr(std::ptr::null_mut()) };
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn from_ptr_null_returns_error() {
    let result = unsafe { FluxFuture::from_ptr(std::ptr::null_mut()) };
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

// =========================================================================
// AsFluxPtr / IntoFluxPtr
// =========================================================================

#[test]
fn as_mut_ptr_returns_non_null() {
    let future = make_wait_all();
    assert!(!future.as_mut_ptr().is_null());
}

#[test]
fn into_raw_returns_non_null_and_suppresses_destructor() {
    let future = make_wait_all();
    let ptr = future.into_raw();
    assert!(!ptr.is_null());
    // Manually clean up to avoid leaking the C allocation.
    unsafe { flux_future_destroy(ptr) };
}

#[test]
fn as_mut_ptr_and_into_raw_return_same_pointer() {
    let future = make_wait_all();
    let as_ptr = future.as_mut_ptr();
    let raw_ptr = future.into_raw();
    assert_eq!(as_ptr, raw_ptr);
    // Manually free — no Rust type owns ptr at this point.
    unsafe { flux_future_destroy(raw_ptr) };
}
