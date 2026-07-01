#[cfg(flux_core_has_module_loader_helpers)]
use flux_sys::core::{
    flux_error_t, flux_module_finalize, flux_module_initialize, flux_module_register_handlers,
};
#[cfg(flux_core_has_module_loader_helpers)]
use std::ffi::c_char;
#[cfg(flux_core_has_module_loader_helpers)]
use std::ffi::CStr;
#[cfg(flux_core_has_module_loader_helpers)]
use std::mem::MaybeUninit;

use flux_sys::core::{flux_module_debug_test, flux_module_set_running};

use crate::error::{check_rc, FluxError, Result};
use crate::handle::FluxHandle;

pub fn test_module_debug_bit(handle: &FluxHandle, flag: i32, clear: bool) -> bool {
    if handle.h.is_null() {
        return false;
    }
    unsafe { flux_module_debug_test(handle.h, flag, clear) }
}

pub fn set_module_running(handle: &FluxHandle) -> Result<()> {
    if handle.h.is_null() {
        return Err(FluxError::Logic(
            "Cannot set module to RUNNING state with a NULL Flux handle".to_string(),
        ));
    }
    let rc = unsafe { flux_module_set_running(handle.h) };
    check_rc(rc)
}

#[cfg(flux_core_has_module_loader_helpers)]
pub fn initialize_module(handle: &FluxHandle) -> Result<String> {
    if handle.h.is_null() {
        return Err(FluxError::Logic(
            "Cannot initialize module using a NULL Flux handle".to_string(),
        ));
    }
    let mut args_str_ptr: *mut c_char = std::ptr::null_mut();
    let mut err_buf: flux_error_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let rc = unsafe {
        flux_module_initialize(
            handle.h,
            &mut args_str_ptr as *mut *mut c_char,
            &mut err_buf as *mut flux_error_t,
        )
    };
    if rc == -1 {
        let err_msg = unsafe {
            CStr::from_bytes_until_nul(&err_buf.text)?
                .to_string_lossy()
                .to_owned()
        };
        Err(FluxError::Logic(err_msg))
    } else {
        if args_str_ptr.is_null() {
            Err(FluxError::Logic(
                "The 'flux_module_initialize' function produced a NULL args string".to_string(),
            ))
        } else {
            let args_str = unsafe { CStr::from_ptr(args_str_ptr).to_string_lossy().to_owned() };
            unsafe {
                libc::free(args_str_ptr);
            }
            Ok(args_str)
        }
    }
}

#[cfg(flux_core_has_module_loader_helpers)]
pub fn register_default_handlers(handle: &FluxHandle) -> Result<()> {
    if handle.h.is_null() {
        return Err(FluxError::Logic(
            "Cannot register default handlers using a NULL Flux handle".to_string(),
        ));
    }
    let mut err_buf: flux_error_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let rc = unsafe { flux_module_register_handlers(handle.h, &mut err_buf as *mut flux_error_t) };
    if rc == -1 {
        let err_msg = unsafe {
            CStr::from_bytes_until_nul(&err_buf.text)?
                .to_string_lossy()
                .to_owned()
        };
        Err(FluxError::Logic(err_msg))
    } else {
        Ok(())
    }
}

#[cfg(flux_core_has_module_loader_helpers)]
pub fn finalize_module(handle: &FluxHandle, error: Option<std::io::Error>) -> Result<()> {
    if handle.h.is_null() {
        return Err(FluxError::Logic(
            "Cannot finalize module using a NULL Flux handle".to_string(),
        ));
    }
    let errnum = if let Some(err) = error {
        err.raw_os_error()
    } else {
        0
    };
    let mut err_buf: flux_error_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let rc = unsafe { flux_module_finalize(handle.h, errnum, &mut err_buf as *mut flux_error_t) };
    if rc == -1 {
        let err_msg = unsafe {
            CStr::from_bytes_until_nul(&err_buf.text)?
                .to_string_lossy()
                .to_owned()
        };
        Err(FluxError::Logic(err_msg))
    } else {
        Ok(())
    }
}

#[macro_export]
macro_rules! __create_module_entrypoint_macro {
    ($user_main:path) => {
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
            let rust_handle = $crate::handle::FluxHandle::from(h);
            match $user_main(rust_handle.clone(), args) {
                Ok(_) => 0,
                Err(e) => {
                    let err_code = e.to_errno_with_flux_log(&rust_handle);
                    unsafe {
                        let errno_ptr = ::libc::__errno_location();
                        *errno_ptr = err_code;
                    }
                    -1
                }
            }
        }
    };
}

pub use crate::__create_module_entrypoint_macro as create_module_entrypoint;
