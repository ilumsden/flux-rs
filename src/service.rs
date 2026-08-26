use std::ffi::CString;

use flux_sys::core::{flux_service_register, flux_service_unregister, flux_t};

use crate::error::{FluxError, Result};
use crate::flux_ptr_management::{FromFluxPtrNoArgs, PossiblyDroppablePtr};
use crate::future::FluxFuture;
use crate::handle::FluxHandle;
use crate::rpc::Rpc;

pub fn register_service<State: PossiblyDroppablePtr<flux_t>>(
    handle: &FluxHandle<State>,
    name: &str,
) -> Result<()> {
    let c_name = CString::new(name)?;
    let ptr = unsafe { flux_service_register(handle.h.as_mut_ptr(), c_name.as_ptr()) };
    let rpc_future = Rpc::from(unsafe { FluxFuture::from_ptr(ptr)? });
    match rpc_future.get() {
        Ok(_) => Ok(()),
        Err(FluxError::System(c_func_name, io_error)) => match io_error.raw_os_error() {
            Some(errno) => {
                if errno == ::libc::EINVAL {
                    Err(FluxError::Logic("Invalid service name".to_string()))
                } else if errno == ::libc::EEXIST {
                    Err(FluxError::Logic(format!(
                        "Service already registered under name {name}"
                    )))
                } else if errno == ::libc::ENOENT {
                    Err(FluxError::Logic(
                        "Unable to lookup route to requesting sender".to_string(),
                    ))
                } else {
                    Err(FluxError::System(c_func_name, io_error))
                }
            }
            None => Err(FluxError::Logic(
                "Unknown error occured in registering service name".to_string(),
            )),
        },
        Err(err) => Err(err),
    }
}

pub fn unregister_service<State: PossiblyDroppablePtr<flux_t>>(
    handle: &FluxHandle<State>,
    name: &str,
) -> Result<()> {
    let c_name = CString::new(name)?;
    let ptr = unsafe { flux_service_unregister(handle.h.as_mut_ptr(), c_name.as_ptr()) };
    let rpc_future = Rpc::from(unsafe { FluxFuture::from_ptr(ptr)? });
    match rpc_future.get() {
        Ok(_) => Ok(()),
        Err(FluxError::System(c_func_name, io_error)) => match io_error.raw_os_error() {
            Some(errno) => {
                if errno == ::libc::ENOENT {
                    Err(FluxError::Logic(format!(
                        "No service registered as '{name}'"
                    )))
                } else if errno == ::libc::EINVAL {
                    Err(FluxError::Logic(
                        "Sender does not match current owner of service".to_string(),
                    ))
                } else {
                    Err(FluxError::System(c_func_name, io_error))
                }
            }
            None => Err(FluxError::Logic(
                "Unknown error occured in unregistering service name".to_string(),
            )),
        },
        Err(err) => Err(err),
    }
}
