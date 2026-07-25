use std::ptr::NonNull;

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
pub unsafe trait FromFluxPtr: BorrowFluxPtr {
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
pub unsafe trait FromFluxPtrNoArgs:
    FromFluxPtr<FromRawArgs = ()> + BorrowFluxPtrNoArgs
{
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

macro_rules! default_impl_as_flux_ptr {
    ($implementor_type:ident<'static>, $c_type:ty, $flux_ptr_field:ident) => {
        unsafe impl $crate::AsFluxPtr for $implementor_type<'static> {
            type CType = $c_type;

            fn as_mut_ptr(&self) -> *mut Self::CType {
                self.$flux_ptr_field.as_mut_ptr()
            }
        }

        unsafe impl $crate::IntoFluxPtr for $implementor_type<'static> {
            fn into_raw(self) -> *mut Self::CType {
                self.$flux_ptr_field.into_raw()
            }
        }
    };
    ($implementor_type:ident<$lt:lifetime>, $c_type:ty, $flux_ptr_field:ident) => {
        unsafe impl<$lt> $crate::AsFluxPtr for $implementor_type<$lt> {
            type CType = $c_type;

            fn as_mut_ptr(&self) -> *mut Self::CType {
                self.$flux_ptr_field.as_mut_ptr()
            }
        }

        unsafe impl<$lt> $crate::IntoFluxPtr for $implementor_type<$lt> {
            fn into_raw(self) -> *mut Self::CType {
                self.$flux_ptr_field.into_raw()
            }
        }
    };
    ($implementor_type:ident, $c_type:ty, $flux_ptr_field:ident) => {
        unsafe impl $crate::AsFluxPtr for $implementor_type {
            type CType = $c_type;

            fn as_mut_ptr(&self) -> *mut Self::CType {
                self.$flux_ptr_field.as_mut_ptr()
            }
        }

        unsafe impl $crate::IntoFluxPtr for $implementor_type {
            fn into_raw(self) -> *mut Self::CType {
                self.$flux_ptr_field.into_raw()
            }
        }
    };
}

pub(crate) use default_impl_as_flux_ptr;

pub(crate) type FluxPtrDestructor<T> = unsafe extern "C" fn(*mut T);

pub(crate) struct FluxPtr<T> {
    pub(crate) inner: NonNull<T>,
    pub(crate) owned: Option<NonNull<T>>,
    pub(crate) destructor: FluxPtrDestructor<T>,
}

impl<T> FluxPtr<T> {
    pub fn create_owned(ptr: *mut T, destructor: FluxPtrDestructor<T>) -> Result<Self> {
        let non_null_ptr = NonNull::new(ptr).ok_or_else(|| {
            FluxError::Logic("Cannot create a FluxPtr object from a NULL C pointer".to_string())
        })?;
        Ok(Self {
            inner: non_null_ptr,
            owned: Some(non_null_ptr),
            destructor,
        })
    }

    pub fn create_borrowed(ptr: *mut T, destructor: FluxPtrDestructor<T>) -> Result<Self> {
        let non_null_ptr = NonNull::new(ptr).ok_or_else(|| {
            FluxError::Logic("Cannot create a FluxPtr object from a NULL C pointer".to_string())
        })?;
        Ok(Self {
            inner: non_null_ptr,
            owned: None,
            destructor,
        })
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        self.inner.as_ptr()
    }

    // TODO remove this lint once part of the crate actually has logic that
    // differs based on ownership of a Flux C pointer
    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        self.owned.is_some()
    }

    pub fn into_raw(mut self) -> *mut T {
        // Take the NonNull out of the Option to prevent pointer destruction
        self.owned.take();
        self.inner.as_ptr()
    }
}

impl<T> Drop for FluxPtr<T> {
    fn drop(&mut self) {
        if let Some(ptr) = self.owned.take() {
            unsafe {
                (self.destructor)(ptr.as_ptr());
            }
        }
    }
}
