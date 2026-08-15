#[cfg(flux_core_has_module_loader_helpers)]
use flux_sys::core::{
    flux_error_t, flux_module_finalize, flux_module_initialize, flux_module_register_handlers,
};
#[cfg(flux_core_has_module_loader_helpers)]
use std::ffi::{CStr, c_char, c_void};
#[cfg(flux_core_has_module_loader_helpers)]
use std::mem::MaybeUninit;

use std::alloc::{GlobalAlloc, Layout, System};

use flux_sys::core::{flux_module_debug_test, flux_module_set_running};

#[allow(unused_imports)]
use crate::error::{FluxError, Result, flux_try};
use crate::handle::FluxHandle;

pub struct PanickingAllocator;

unsafe impl GlobalAlloc for PanickingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if ptr.is_null() {
            panic!("Failed to allocate {} bytes of memory", layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if ptr.is_null() {
            panic!("Failed to allocate {} bytes of memory", layout.size());
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if ptr.is_null() {
            panic!("Failed to reallocate to {} bytes of memory", new_size);
        }
        ptr
    }
}

pub fn test_module_debug_bit(handle: &FluxHandle, flag: i32, clear: bool) -> bool {
    unsafe { flux_module_debug_test(handle.h.as_mut_ptr(), flag, clear) }
}

pub fn set_module_running(handle: &FluxHandle) -> Result<()> {
    flux_try!(empty_ok flux_module_set_running(handle.h.as_mut_ptr()))
}

#[cfg(flux_core_has_module_loader_helpers)]
pub fn initialize_module(handle: &FluxHandle) -> Result<String> {
    let mut args_str_ptr: *mut c_char = std::ptr::null_mut();
    let mut err_buf: flux_error_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let rc = unsafe {
        flux_module_initialize(
            handle.h.as_mut_ptr(),
            &mut args_str_ptr as *mut *mut c_char,
            &mut err_buf as *mut flux_error_t,
        )
    };
    if rc == -1 {
        let text_bytes = unsafe {
            std::slice::from_raw_parts(err_buf.text.as_ptr() as *const _, err_buf.text.len())
        };
        let err_msg = CStr::from_bytes_until_nul(text_bytes)?
            .to_string_lossy()
            .to_string();
        Err(FluxError::Logic(err_msg))
    } else if args_str_ptr.is_null() {
        Err(FluxError::Logic(
            "The 'flux_module_initialize' function produced a NULL args string".to_string(),
        ))
    } else {
        let args_str = unsafe { CStr::from_ptr(args_str_ptr).to_string_lossy().to_string() };
        unsafe {
            libc::free(args_str_ptr as *mut c_void);
        }
        Ok(args_str)
    }
}

#[cfg(flux_core_has_module_loader_helpers)]
pub fn register_default_handlers(handle: &FluxHandle) -> Result<()> {
    let mut err_buf: flux_error_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let rc = unsafe {
        flux_module_register_handlers(handle.h.as_mut_ptr(), &mut err_buf as *mut flux_error_t)
    };
    if rc == -1 {
        let text_bytes = unsafe {
            std::slice::from_raw_parts(err_buf.text.as_ptr() as *const _, err_buf.text.len())
        };
        let err_msg = CStr::from_bytes_until_nul(text_bytes)?
            .to_string_lossy()
            .to_string();
        Err(FluxError::Logic(err_msg))
    } else {
        Ok(())
    }
}

#[cfg(flux_core_has_module_loader_helpers)]
pub fn finalize_module(handle: &FluxHandle, error: Option<std::io::Error>) -> Result<()> {
    let errnum = if let Some(err) = error {
        err.raw_os_error().unwrap_or(0)
    } else {
        0
    };
    let mut err_buf: flux_error_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let rc = unsafe {
        flux_module_finalize(
            handle.h.as_mut_ptr(),
            errnum,
            &mut err_buf as *mut flux_error_t,
        )
    };
    if rc == -1 {
        let text_bytes = unsafe {
            std::slice::from_raw_parts(err_buf.text.as_ptr() as *mut u8, err_buf.text.len())
        };
        let err_msg = CStr::from_bytes_until_nul(text_bytes)?
            .to_string_lossy()
            .to_string();
        Err(FluxError::Logic(err_msg))
    } else {
        Ok(())
    }
}

/// A type alias for the signature of the expected broker module entrypoint.
pub type BrokerModuleEntrypoint = fn(FluxHandle, Vec<String>) -> Result<()>;

#[macro_export]
macro_rules! __set_global_allocator_to_panicking {
    () => {
        #[cfg(panic = "abort")]
        ::std::compile_error!(
            r#"Calling `set_global_panicking_allocator` requires `panic = "unwind"` in your Cargo.toml.
Add the following to your profile sections:

[profile.dev]
panic = "unwind"

[profile.release]
panic = "unwind"
"#
        );

        #[global_allocator]
        static __FLUX_PANICKING_ALLOCATOR: $crate::module::PanickingAllocator =
            $crate::module::PanickingAllocator;
    };
}

#[macro_export]
macro_rules! __create_module_entrypoint_macro {
    ($user_main:path) => {
        const _: $crate::module::BrokerModuleEntrypoint = $user_main;

        #[no_mangle]
        pub extern "C" fn mod_main(
            h: *mut ::flux_sys::core::flux_t,
            argc: ::std::ffi::c_int,
            argv: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let mut args = Vec::with_capacity(argc as usize);
            if argc > 0 && !argv.is_null() {
                for i in 0..argc {
                    let arg_ptr = unsafe { *argv.offset(i as isize) };
                    if !arg_ptr.is_null() {
                        let c_str = unsafe { ::std::ffi::CStr::from_ptr(arg_ptr) };
                        args.push(c_str.to_string_lossy().into_owned());
                    }
                }
            }
            unsafe {
                ::flux_sys::core::flux_incref(h);
            }
            let rust_handle = match unsafe {
                <$crate::handle::FluxHandle as $crate::FromFluxPtrNoArgs>::from_ptr(h)
            } {
                Ok(rh) => rh,
                Err(e) => {
                    return $crate::error::to_flux_rc(Err(e), None);
                }
            };
            $crate::error::to_flux_rc($user_main(rust_handle.clone(), args), Some(&rust_handle))
        }
    };
}

pub use crate::__create_module_entrypoint_macro as create_module_entrypoint;
pub use crate::__set_global_allocator_to_panicking as set_global_panicking_allocator;
