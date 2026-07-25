use bitflags::bitflags;
use flux_sys::core::{
    flux_watcher_destroy, flux_watcher_next_wakeup, flux_watcher_start, flux_watcher_stop,
    flux_watcher_t, FLUX_POLLERR, FLUX_POLLIN, FLUX_POLLOUT,
};

use crate::error::{FluxError, Result};
use crate::flux_ptr_management::{
    default_impl_as_flux_ptr, AsFluxPtr, BorrowFluxPtr, FluxPtr, FromFluxPtr,
};

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WatcherEvents: u32 {
        const NONE = 0;
        const POLLIN = FLUX_POLLIN;
        const POLLOUT = FLUX_POLLOUT;
        const POLLERR = FLUX_POLLERR;
    }
}

pub trait Watcher: AsFluxPtr<CType = flux_watcher_t> {
    fn start(&self) -> Result<()> {
        let watcher_ptr = self.as_mut_ptr();
        if watcher_ptr.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot start a watcher with a NULL pointer",
            )));
        }
        unsafe {
            flux_watcher_start(watcher_ptr);
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let watcher_ptr = self.as_mut_ptr();
        if watcher_ptr.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot stop a watcher with a NULL pointer",
            )));
        }
        unsafe {
            flux_watcher_stop(watcher_ptr);
        }
        Ok(())
    }

    fn next_wakeup(&self) -> Result<f64> {
        let watcher_ptr = self.as_mut_ptr();
        if watcher_ptr.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get next wakup from a watcher with a NULL pointer",
            )));
        }
        Ok(unsafe { flux_watcher_next_wakeup(watcher_ptr) })
    }
}

// TODO consider making RawWatcher support custom creation with flux_watcher_create
pub struct RawWatcher {
    pub(crate) c_watcher: FluxPtr<flux_watcher_t>,
}

unsafe impl BorrowFluxPtr for RawWatcher {
    type CType = flux_watcher_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_watcher: FluxPtr::create_borrowed(ptr, flux_watcher_destroy)?,
        })
    }
}

unsafe impl FromFluxPtr for RawWatcher {
    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_watcher: FluxPtr::create_owned(ptr, flux_watcher_destroy)?,
        })
    }
}

default_impl_as_flux_ptr!(RawWatcher, flux_watcher_t, c_watcher);

impl Watcher for RawWatcher {}

macro_rules! create_watcher_specialization {
    (
        $vis:vis struct $struct_name:ident $(<$life:lifetime>)? {
            $( $field_vis:vis $field_name:ident : $field_type:ty ),* $(,)?
        }
    ) => {
        $vis struct $struct_name $(<$life>)? {
            watcher: $crate::watcher::base::RawWatcher,
            $( $field_vis $field_name : $field_type, )*
        }

        unsafe impl $(<$life>)? $crate::AsFluxPtr for $struct_name $(<$life>)? {
            type CType = flux_sys::core::flux_watcher_t;

            fn as_mut_ptr(&self) -> *mut flux_sys::core::flux_watcher_t {
                self.watcher.as_mut_ptr()
            }
        }

        impl $(<$life>)? $crate::watcher::base::Watcher for $struct_name $(<$life>)? {}
    };
}

pub(crate) use create_watcher_specialization;
