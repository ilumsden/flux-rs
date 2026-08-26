#![allow(clippy::missing_safety_doc)]

use std::{marker::PhantomData, ops::Deref, ptr::NonNull};

use crate::error::{FluxError, Result};

/// A trait defining the interface for Rust objects to be built from raw Flux C pointers
/// without assuming ownership.
pub unsafe trait BorrowFluxPtr: Sized {
    /// The C pointer type, which should almost always come from flux-sys
    type CType;
    /// The type of additional arguments accepted
    type FromRawArgs;

    /// Constructs the Rust type from a Flux C pointer without assuming ownership over the pointer.
    ///
    /// When a Rust object is created with this method, its implementation of `Drop` is expected
    /// to **not** free the underlying C pointer.
    unsafe fn borrow_raw(ptr: *mut Self::CType, args: Self::FromRawArgs) -> Result<Self>;
}

/// A trait defining the interface for Rust objects to be built from raw Flux C pointers
/// while assuming ownership.
pub unsafe trait FromFluxPtr: Sized {
    /// The C pointer type, which should almost always come from flux-sys
    type CType;
    /// The type of additional arguments accepted
    type FromRawArgs;

    /// Constructs the Rust type from a Flux C pointer and assumes ownership over the pointer.
    ///
    /// When a Rust object is created with this method, its implementation of `Drop` is expected
    /// to free the underlying C pointer.
    unsafe fn from_raw(ptr: *mut Self::CType, args: Self::FromRawArgs) -> Result<Self>;
}

/// A trait defining convenience wrappers over BorrowFluxPtr when FromRawArgs is `()`
///
/// So long as this trait is brought into scope (i.e., with `use flux_core::FromFluxPtrNoArgs;`),
/// it will be auto-implemented for any type that implements FromFluxPtr with `FromRawArgs = ()`.
pub unsafe trait BorrowFluxPtrNoArgs: BorrowFluxPtr<FromRawArgs = ()> {
    unsafe fn borrow_ptr(ptr: *mut Self::CType) -> Result<Self> {
        unsafe { Self::borrow_raw(ptr, ()) }
    }
}

/// A trait defining convenience wrappers over FromFluxPtr when FromRawArgs is `()`
///
/// So long as this trait is brought into scope (i.e., with `use flux_core::FromFluxPtrNoArgs;`),
/// it will be auto-implemented for any type that implements FromFluxPtr with `FromRawArgs = ()`.
pub unsafe trait FromFluxPtrNoArgs: FromFluxPtr<FromRawArgs = ()> {
    unsafe fn from_ptr(ptr: *mut Self::CType) -> Result<Self> {
        unsafe { Self::from_raw(ptr, ()) }
    }
}

unsafe impl<T: BorrowFluxPtr<FromRawArgs = ()>> BorrowFluxPtrNoArgs for T {}
unsafe impl<T: FromFluxPtr<FromRawArgs = ()>> FromFluxPtrNoArgs for T {}

/// A trait defining the interface for obtaining raw Flux C pointers from Rust objects
/// without assuming ownership.
pub unsafe trait AsFluxPtr: Sized {
    type CType;

    /// Borrows the underlying Flux C pointer.
    fn as_mut_ptr(&self) -> *mut Self::CType;
}

/// A trait defining the interface for obtaining raw Flux C pointers from Rust objects
/// while assuming ownership.
pub unsafe trait IntoFluxPtr: AsFluxPtr {
    /// Consumes the Rust object and transfers ownership of the C pointer to the caller.
    fn into_raw(self) -> *mut Self::CType;
}

macro_rules! define_as_flux_ptr_body {
    ($c_type:ty, $flux_ptr_field:ident) => {
        type CType = $c_type;

        fn as_mut_ptr(&self) -> *mut Self::CType {
            self.$flux_ptr_field.as_mut_ptr()
        }
    };
}

macro_rules! define_into_flux_ptr_body {
    ($flux_ptr_field:ident) => {
        fn into_raw(self) -> *mut Self::CType {
            self.$flux_ptr_field.into_raw()
        }
    };
}

pub(crate) use define_as_flux_ptr_body;
pub(crate) use define_into_flux_ptr_body;

pub trait PossiblyDroppablePtr<T> {
    const IS_OWNED: bool;

    fn drop_ptr(&mut self, ptr: NonNull<T>);
}

/// The type of the destructor for a Flux C pointer of type `T`.
pub type FluxPtrDestructor<T> = unsafe extern "C" fn(*mut T);

pub struct Owned<T> {
    /// The destructor for the pointer.
    pub(crate) destructor: FluxPtrDestructor<T>,
}

impl<T> PossiblyDroppablePtr<T> for Owned<T> {
    const IS_OWNED: bool = true;

    fn drop_ptr(&mut self, ptr: NonNull<T>) {
        unsafe {
            (self.destructor)(ptr.as_ptr());
        }
    }
}

impl<T> Deref for Owned<T> {
    type Target = FluxPtrDestructor<T>;

    fn deref(&self) -> &Self::Target {
        &self.destructor
    }
}

pub struct Borrowed<'a, T> {
    /// Binds this struct to the lifetime 'a
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T> PossiblyDroppablePtr<T> for Borrowed<'a, T> {
    const IS_OWNED: bool = false;

    fn drop_ptr(&mut self, _ptr: NonNull<T>) {
        // Nothing to do for a borrowed value
    }
}

/// A struct for managing Flux C pointers that are either owned or borrowed by Rust.
pub struct FluxPtr<T, State: PossiblyDroppablePtr<T>> {
    /// The actual Flux C pointer.
    pub(crate) inner: NonNull<T>,
    /// The member indicating whether the pointer is owned or borrowed
    pub(crate) state: State,
}

impl<T, State> Drop for FluxPtr<T, State>
where
    State: PossiblyDroppablePtr<T>,
{
    /// Free the underlying C pointer using the destructor only if the pointer is owned.
    fn drop(&mut self) {
        self.state.drop_ptr(self.inner);
    }
}

impl<T> FluxPtr<T, Owned<T>> {
    /// Create a FluxPtr object that owns the underlying Flux C pointer.
    pub fn create_owned(ptr: *mut T, destructor: FluxPtrDestructor<T>) -> Result<Self> {
        let non_null_ptr = NonNull::new(ptr).ok_or_else(|| {
            FluxError::Logic("Cannot create a FluxPtr object from a NULL C pointer".to_string())
        })?;
        Ok(Self {
            inner: non_null_ptr,
            state: Owned { destructor },
        })
    }

    /// Get the underlying Flux C pointer and assume ownership.
    ///
    /// After calling this method, it is up to the user (or a Flux C API function)
    /// to deallocate the underlying C pointer.
    pub fn into_raw(self) -> *mut T {
        let ptr = self.inner.as_ptr();
        // Tell Rust to not invoke `drop` on `self`.
        // This effectively transfers ownership of the C pointer to the caller.
        std::mem::forget(self);
        ptr
    }
}

impl<'a, T> FluxPtr<T, Borrowed<'a, T>> {
    /// Create a FluxPtr object that borrows the underlying Flux C pointer.
    pub fn create_borrowed(ptr: *mut T) -> Result<Self> {
        let non_null_ptr = NonNull::new(ptr).ok_or_else(|| {
            FluxError::Logic("Cannot create a FluxPtr object from a NULL C pointer".to_string())
        })?;
        Ok(Self {
            inner: non_null_ptr,
            state: Borrowed {
                _marker: PhantomData,
            },
        })
    }
}

impl<T, State: PossiblyDroppablePtr<T>> FluxPtr<T, State> {
    /// Get the underlying Flux C pointer.
    pub fn as_mut_ptr(&self) -> *mut T {
        self.inner.as_ptr()
    }

    // TODO remove this lint once part of the crate actually has logic that
    // differs based on ownership of a Flux C pointer
    /// Check if the underlying pointer is owned or borrowed.
    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        State::IS_OWNED
    }
}
