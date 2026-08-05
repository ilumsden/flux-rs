use std::cell::Cell;

use crate::error::FluxError;
use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtr, FluxPtr, FromFluxPtr, IntoFluxPtr};
// =========================================================================
// Helpers
// =========================================================================

// Thread-local flag so parallel tests cannot interfere with each other's
// destructor-tracking state.
thread_local! {
    static DESTRUCTOR_CALLED: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "C" fn tracking_destructor<T>(_ptr: *mut T) {
    DESTRUCTOR_CALLED.with(|c| c.set(true));
}

unsafe extern "C" fn noop_destructor<T>(_ptr: *mut T) {}

fn reset_tracking() {
    DESTRUCTOR_CALLED.with(|c| c.set(false));
}

fn was_destructor_called() -> bool {
    DESTRUCTOR_CALLED.with(|c| c.get())
}

// A minimal local type that implements BorrowFluxPtr, FromFluxPtr,
// AsFluxPtr, and IntoFluxPtr so all four traits (and their blanket
// NoArgs wrappers) can be exercised without pulling in a real Flux type.
struct DummyFlux {
    ptr: FluxPtr<i32>,
}

unsafe impl BorrowFluxPtr for DummyFlux {
    type CType = i32;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut i32, _args: ()) -> crate::error::Result<Self> {
        Ok(Self {
            ptr: FluxPtr::create_borrowed(ptr, noop_destructor)?,
        })
    }
}

unsafe impl FromFluxPtr for DummyFlux {
    unsafe fn from_raw(ptr: *mut i32, _args: ()) -> crate::error::Result<Self> {
        Ok(Self {
            ptr: FluxPtr::create_owned(ptr, noop_destructor)?,
        })
    }
}

unsafe impl AsFluxPtr for DummyFlux {
    type CType = i32;

    fn as_mut_ptr(&self) -> *mut i32 {
        self.ptr.as_mut_ptr()
    }
}

unsafe impl IntoFluxPtr for DummyFlux {
    fn into_raw(self) -> *mut i32 {
        self.ptr.into_raw()
    }
}

// =========================================================================
// FluxPtr::create_owned
// =========================================================================

#[test]
fn create_owned_null_returns_logic_error() {
    let result = FluxPtr::<i32>::create_owned(std::ptr::null_mut(), noop_destructor);
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn create_owned_non_null_returns_ok() {
    let mut val: i32 = 1;
    let result = FluxPtr::create_owned(&mut val as *mut i32, noop_destructor);
    assert!(result.is_ok());
}

#[test]
fn create_owned_is_owned_true() {
    let mut val: i32 = 1;
    let fp = FluxPtr::create_owned(&mut val as *mut i32, noop_destructor).unwrap();
    assert!(fp.is_owned());
}

// =========================================================================
// FluxPtr::create_borrowed
// =========================================================================

#[test]
fn create_borrowed_null_returns_logic_error() {
    let result = FluxPtr::<i32>::create_borrowed(std::ptr::null_mut(), noop_destructor);
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn create_borrowed_non_null_returns_ok() {
    let mut val: i32 = 1;
    let result = FluxPtr::create_borrowed(&mut val as *mut i32, noop_destructor);
    assert!(result.is_ok());
}

#[test]
fn create_borrowed_is_owned_false() {
    let mut val: i32 = 1;
    let fp = FluxPtr::create_borrowed(&mut val as *mut i32, noop_destructor).unwrap();
    assert!(!fp.is_owned());
}

// =========================================================================
// FluxPtr::as_mut_ptr
// =========================================================================

#[test]
fn as_mut_ptr_owned_returns_original_pointer() {
    let mut val: i32 = 42;
    let ptr = &mut val as *mut i32;
    let fp = FluxPtr::create_owned(ptr, noop_destructor).unwrap();
    assert_eq!(fp.as_mut_ptr(), ptr);
}

#[test]
fn as_mut_ptr_borrowed_returns_original_pointer() {
    let mut val: i32 = 42;
    let ptr = &mut val as *mut i32;
    let fp = FluxPtr::create_borrowed(ptr, noop_destructor).unwrap();
    assert_eq!(fp.as_mut_ptr(), ptr);
}

// =========================================================================
// FluxPtr::into_raw
// =========================================================================

#[test]
fn into_raw_returns_original_pointer() {
    let mut val: i32 = 42;
    let ptr = &mut val as *mut i32;
    let fp = FluxPtr::create_owned(ptr, noop_destructor).unwrap();
    assert_eq!(fp.into_raw(), ptr);
}

#[test]
fn into_raw_does_not_call_destructor() {
    reset_tracking();
    let mut val: i32 = 0;
    let fp = FluxPtr::create_owned(&mut val as *mut i32, tracking_destructor::<i32>).unwrap();
    let _ = fp.into_raw();
    assert!(
        !was_destructor_called(),
        "destructor should NOT be called after into_raw"
    );
}

// =========================================================================
// Drop behavior
// =========================================================================

#[test]
fn drop_owned_calls_destructor() {
    reset_tracking();
    let mut val: i32 = 0;
    {
        let _fp = FluxPtr::create_owned(&mut val as *mut i32, tracking_destructor::<i32>).unwrap();
    } // _fp dropped here
    assert!(
        was_destructor_called(),
        "destructor SHOULD be called when an owned FluxPtr is dropped"
    );
}

#[test]
fn drop_borrowed_does_not_call_destructor() {
    reset_tracking();
    let mut val: i32 = 0;
    {
        let _fp =
            FluxPtr::create_borrowed(&mut val as *mut i32, tracking_destructor::<i32>).unwrap();
    } // _fp dropped here
    assert!(
        !was_destructor_called(),
        "destructor should NOT be called when a borrowed FluxPtr is dropped"
    );
}

// =========================================================================
// BorrowFluxPtrNoArgs / FromFluxPtrNoArgs blanket impls
// =========================================================================

#[test]
fn borrow_ptr_convenience_produces_borrowed_flux_ptr() {
    use crate::flux_ptr_management::BorrowFluxPtrNoArgs;
    let mut val: i32 = 99;
    let result = unsafe { DummyFlux::borrow_ptr(&mut val as *mut i32) };
    assert!(result.is_ok());
    assert!(!result.unwrap().ptr.is_owned());
}

#[test]
fn from_ptr_convenience_produces_owned_flux_ptr() {
    use crate::flux_ptr_management::FromFluxPtrNoArgs;
    let mut val: i32 = 99;
    let result = unsafe { DummyFlux::from_ptr(&mut val as *mut i32) };
    assert!(result.is_ok());
    assert!(result.unwrap().ptr.is_owned());
}

#[test]
fn borrow_ptr_null_returns_logic_error() {
    use crate::flux_ptr_management::BorrowFluxPtrNoArgs;
    let result = unsafe { DummyFlux::borrow_ptr(std::ptr::null_mut()) };
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn from_ptr_null_returns_logic_error() {
    use crate::flux_ptr_management::FromFluxPtrNoArgs;
    let result = unsafe { DummyFlux::from_ptr(std::ptr::null_mut()) };
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

// =========================================================================
// AsFluxPtr
// =========================================================================

#[test]
fn as_flux_ptr_returns_original_pointer() {
    let mut val: i32 = 55;
    let ptr = &mut val as *mut i32;
    let dummy = unsafe { DummyFlux::from_raw(ptr, ()) }.unwrap();
    // AsFluxPtr::as_mut_ptr should return the same address without
    // consuming or transferring ownership.
    assert_eq!(dummy.as_mut_ptr(), ptr);
}

#[test]
fn as_flux_ptr_borrowed_returns_original_pointer() {
    let mut val: i32 = 55;
    let ptr = &mut val as *mut i32;
    let dummy = unsafe { DummyFlux::borrow_raw(ptr, ()) }.unwrap();
    assert_eq!(dummy.as_mut_ptr(), ptr);
}

#[test]
fn as_flux_ptr_does_not_consume_object() {
    // Calling as_mut_ptr multiple times should always yield the same address,
    // confirming the object is not consumed by the call.
    let mut val: i32 = 7;
    let ptr = &mut val as *mut i32;
    let dummy = unsafe { DummyFlux::from_raw(ptr, ()) }.unwrap();
    assert_eq!(dummy.as_mut_ptr(), ptr);
    assert_eq!(dummy.as_mut_ptr(), ptr);
}

// =========================================================================
// IntoFluxPtr
// =========================================================================

#[test]
fn into_flux_ptr_returns_original_pointer() {
    let mut val: i32 = 77;
    let ptr = &mut val as *mut i32;
    let dummy = unsafe { DummyFlux::from_raw(ptr, ()) }.unwrap();
    assert_eq!(dummy.into_raw(), ptr);
}

#[test]
fn into_flux_ptr_does_not_call_destructor() {
    reset_tracking();
    let mut val: i32 = 0;
    let ptr = &mut val as *mut i32;
    // Manually construct with a tracking destructor since DummyFlux uses noop_destructor.
    let fp = FluxPtr::create_owned(ptr, tracking_destructor::<i32>).unwrap();
    let dummy = DummyFlux { ptr: fp };
    let _ = dummy.into_raw();
    assert!(
        !was_destructor_called(),
        "IntoFluxPtr::into_raw should NOT call the destructor"
    );
}
