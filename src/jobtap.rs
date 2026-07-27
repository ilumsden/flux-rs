use std::ffi::{c_void, CString};
use std::ops::{Deref, DerefMut};

use flux_sys::core::{
    flux_incref, flux_job_result_t, flux_jobtap_dependency_add, flux_jobtap_dependency_remove,
    flux_jobtap_epilog_finish, flux_jobtap_epilog_start, flux_jobtap_error, flux_jobtap_get_flux,
    flux_jobtap_get_job_result, flux_jobtap_job_aux_get, flux_jobtap_job_aux_set,
    flux_jobtap_job_event_posted, flux_jobtap_job_lookup, flux_jobtap_job_set_flag,
    flux_jobtap_job_subscribe, flux_jobtap_job_unsubscribe, flux_jobtap_priority_unavail,
    flux_jobtap_prolog_finish, flux_jobtap_prolog_start, flux_jobtap_raise_exception,
    flux_jobtap_reject_job, flux_jobtap_reprioritize_all, flux_jobtap_reprioritize_job,
    flux_jobtap_service_register, flux_jobtap_service_register_ex, flux_plugin_t,
};
use indexmap::IndexMap;

use crate::error::{check_ptr, check_rc, FluxError, Result};
use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtr, FromFluxPtr, FromFluxPtrNoArgs};
use crate::handle::{AuxThinPtrWrapper, FluxHandle};
use crate::job::{JobEventSeverity, JobId, JobResultCode};
use crate::msg::MessageRolemask;
use crate::msg_handler::{MsgHandler, MsgHandlerCallback};
use crate::plugin::{Plugin, PluginArgs};

pub struct JobtapPlugin {
    plugin: Plugin,
    _cb_boxes: IndexMap<String, MsgHandlerCallback>,
}

impl JobtapPlugin {
    pub fn new() -> Result<Self> {
        Ok(Self {
            plugin: Plugin::new()?,
            _cb_boxes: IndexMap::new(),
        })
    }

    pub fn get_flux(&self) -> Result<FluxHandle> {
        // Get the raw flux_t pointer for the plugin
        let flux_ptr = unsafe { flux_jobtap_get_flux(self.plugin.as_mut_ptr()) };
        // Check the pointer's value and error out if needed
        check_ptr(flux_ptr)?;
        // Increment the reference count of flux_ptr so that FluxHandle::drop cannot fully free
        // the unowned flux_t pointer
        let flux_ptr_for_rust = unsafe { flux_incref(flux_ptr) };
        // Wrap the flux_t pointer in FluxHandle and return
        unsafe { FluxHandle::from_ptr(flux_ptr_for_rust) }
    }

    pub fn register_service(
        &mut self,
        method: &str,
        mut callback: MsgHandlerCallback,
        rolemask: Option<MessageRolemask>,
    ) -> Result<()> {
        let c_method = CString::new(method)?;
        let arg_ptr = &mut callback as *mut MsgHandlerCallback as *mut c_void;
        let rc = if let Some(rmask) = rolemask {
            unsafe {
                flux_jobtap_service_register_ex(
                    self.plugin.as_mut_ptr(),
                    c_method.as_ptr(),
                    rmask.bits(),
                    Some(MsgHandler::msg_handler_trampoline),
                    arg_ptr,
                )
            }
        } else {
            unsafe {
                flux_jobtap_service_register(
                    self.plugin.as_mut_ptr(),
                    c_method.as_ptr(),
                    Some(MsgHandler::msg_handler_trampoline),
                    arg_ptr,
                )
            }
        };
        check_rc(rc)?;
        self._cb_boxes.insert(method.to_string(), callback);
        Ok(())
    }

    pub fn reprioritize_all(&self) -> Result<()> {
        let rc = unsafe { flux_jobtap_reprioritize_all(self.plugin.as_mut_ptr()) };
        check_rc(rc)
    }

    pub fn reprioritize_job(&self, jobid: JobId, priority: u32) -> Result<()> {
        let rc =
            unsafe { flux_jobtap_reprioritize_job(self.plugin.as_mut_ptr(), jobid.0, priority) };
        check_rc(rc)
    }

    pub fn mark_priority_unavail(&self, args: &PluginArgs) -> Result<()> {
        let rc =
            unsafe { flux_jobtap_priority_unavail(self.plugin.as_mut_ptr(), args.as_mut_ptr()) };
        check_rc(rc)
    }

    pub fn return_error(&self, args: &PluginArgs, msg: &str) -> Result<()> {
        let c_msg = CString::new(msg)?;
        let rc = unsafe {
            flux_jobtap_error(
                self.plugin.as_mut_ptr(),
                args.as_mut_ptr(),
                c"%s".as_ptr(),
                c_msg.as_ptr(),
            )
        };
        check_rc(rc)
    }

    pub fn reject_job(&self, args: &PluginArgs, msg: Option<&str>) -> Result<()> {
        let fmt_opt = msg.map(|m| (c"%s", CString::new(m)));
        let rc = if let Some((fmt, c_msg_res)) = fmt_opt {
            let c_msg = c_msg_res?;
            unsafe {
                flux_jobtap_reject_job(
                    self.plugin.as_mut_ptr(),
                    args.as_mut_ptr(),
                    fmt.as_ptr(),
                    c_msg.as_ptr(),
                )
            }
        } else {
            unsafe {
                flux_jobtap_reject_job(
                    self.plugin.as_mut_ptr(),
                    args.as_mut_ptr(),
                    std::ptr::null(),
                )
            }
        };
        check_rc(rc)
    }

    pub fn add_dependency(&self, id: JobId, description: &str) -> Result<()> {
        // Convert the description to a C string
        let c_desc = CString::new(description)?;
        // Add the dependency with the C API
        let rc =
            unsafe { flux_jobtap_dependency_add(self.plugin.as_mut_ptr(), id.0, c_desc.as_ptr()) };
        // Some errno values have special meanings for this function.
        // Check for those errno values, and return descriptive errors, if needed
        if rc == -1 {
            let last_os_error = std::io::Error::last_os_error();
            let errno_val = last_os_error.raw_os_error();
            if let Some(inner_errno_val) = errno_val {
                if inner_errno_val == libc::ENOENT {
                    return Err(FluxError::Logic("Provided job ID not found".to_string()));
                } else if inner_errno_val == libc::EEXIST {
                    return Err(FluxError::Logic(
                        "A dependency with the provided description has already been used"
                            .to_string(),
                    ));
                }
            }
        }
        // Handle all other errors with check_rc
        check_rc(rc)
    }

    pub fn remove_dependency(&self, id: JobId, description: &str) -> Result<()> {
        // Convert the description to a C string
        let c_desc = CString::new(description)?;
        // Remove the dependency with the C API
        let rc = unsafe {
            flux_jobtap_dependency_remove(self.plugin.as_mut_ptr(), id.0, c_desc.as_ptr())
        };
        // Some errno values have special meanings for this function.
        // Check for those errno values, and return descriptive errors, if needed
        if rc == -1 {
            let last_os_error = std::io::Error::last_os_error();
            let errno_val = last_os_error.raw_os_error();
            if let Some(inner_errno_val) = errno_val {
                if inner_errno_val == libc::ENOENT {
                    return Err(FluxError::Logic("Provided job ID not found".to_string()));
                }
            }
        }
        // Handle all other errors with check_rc
        check_rc(rc)
    }

    pub fn set_job_aux<T: Send + Sync + 'static>(
        &self,
        id: JobId,
        name: &str,
        val: T,
    ) -> Result<()> {
        let c_name = CString::new(name)?;
        let wrapper = Box::new(AuxThinPtrWrapper {
            inner: Box::new(val),
        });
        let raw_data_ptr = Box::into_raw(wrapper) as *mut c_void;

        extern "C" fn destroy_aux_trampoline(ptr: *mut c_void) {
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr as *mut AuxThinPtrWrapper);
                }
            }
        }

        let rc = unsafe {
            flux_jobtap_job_aux_set(
                self.plugin.as_mut_ptr(),
                id.0,
                c_name.as_ptr(),
                raw_data_ptr,
                Some(destroy_aux_trampoline),
            )
        };
        if rc == -1 {
            let _ = unsafe { Box::from_raw(raw_data_ptr as *mut AuxThinPtrWrapper) };
            Err(FluxError::System(std::io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    pub fn get_job_aux<T: 'static>(&self, id: JobId, key: &str) -> Result<&T> {
        let raw_ptr = self.get_job_aux_raw(id, key)?;
        let wrapper = unsafe { &*(raw_ptr as *const AuxThinPtrWrapper) };
        match wrapper.inner.downcast_ref::<T>() {
            Some(typed_ref) => Ok(typed_ref),
            None => Err(FluxError::Logic(format!(
                "Type mismatch for aux key '{}' and job '{}'",
                key,
                id.f58().ok().unwrap_or_else(|| id.0.to_string())
            ))),
        }
    }

    pub fn get_job_aux_raw(&self, id: JobId, key: &str) -> Result<*mut c_void> {
        let c_key = CString::new(key)?;
        let raw_ptr =
            unsafe { flux_jobtap_job_aux_get(self.plugin.as_mut_ptr(), id.0, c_key.as_ptr()) };
        check_ptr(raw_ptr)?;
        Ok(raw_ptr)
    }

    pub fn delete_job_aux(&self, id: JobId, name: &str) -> Result<()> {
        let c_name = CString::new(name)?;
        let rc = unsafe {
            flux_jobtap_job_aux_set(
                self.plugin.as_mut_ptr(),
                id.0,
                c_name.as_ptr(),
                std::ptr::null_mut(),
                None,
            )
        };
        check_rc(rc)
    }

    pub fn set_job_flag(&self, id: JobId, flag: &str) -> Result<()> {
        let c_flag = CString::new(flag)?;
        let rc =
            unsafe { flux_jobtap_job_set_flag(self.plugin.as_mut_ptr(), id.0, c_flag.as_ptr()) };
        check_rc(rc)
    }

    pub fn raise_job_execption(
        &self,
        id: JobId,
        exception_type: &str,
        severity: JobEventSeverity,
        message: Option<&str>,
    ) -> Result<()> {
        let c_type = CString::new(exception_type)?;
        let msg_tup = message.map(|m| (c"%s", CString::new(m)));
        let rc = if let Some((fmt, c_msg_res)) = msg_tup {
            let c_msg = c_msg_res?;
            unsafe {
                flux_jobtap_raise_exception(
                    self.plugin.as_mut_ptr(),
                    id.0,
                    c_type.as_ptr(),
                    *severity as i32,
                    fmt.as_ptr(),
                    c_msg.as_ptr(),
                )
            }
        } else {
            unsafe {
                flux_jobtap_raise_exception(
                    self.plugin.as_mut_ptr(),
                    id.0,
                    c_type.as_ptr(),
                    *severity as i32,
                    std::ptr::null(),
                )
            }
        };
        check_rc(rc)
    }

    // TODO implement these once one of the following changes are made:
    //   1. Update the minimum required Rust version to ... (released early 2026)
    //   2. Update Flux-Core itself to have versions of these functions that don't require Jansson format strings
    //   3. Make our own bindings around Jansson
    //
    // pub fn post_job_event<S: Serialize>(
    //     &self,
    //     id: JobId,
    //     name: &str,
    //     context: Option<S>,
    // ) -> Result<()> {
    //     let c_name = CString::new(name)?;
    //     let rc = if let Some(ctx) = context {
    //         let json_val = serde_json::to_value(ctx)?;
    //     } else {
    //         unsafe {
    //             flux_jobtap_event_post_pack(
    //                 self.plugin.as_mut_ptr,
    //                 id.0,
    //                 c_name.as_ptr(),
    //                 std::ptr::null(),
    //             )
    //         }
    //     };
    // }

    // pub fn update_jobspec(&self, updates: Map<String, Value>) -> Result<()> {
    //     unimplemented!()
    // }

    // pub fn update_jobspec_for_id(&self, id: JobId, updates: Map<String, Value>) -> Result<()> {
    //     unimplemented!()
    // }

    pub fn lookup_job(&self, id: JobId) -> Result<PluginArgs> {
        let plugin_arg_ptr = unsafe { flux_jobtap_job_lookup(self.plugin.as_mut_ptr(), id.0) };
        if plugin_arg_ptr.is_null() {
            let last_os_error = std::io::Error::last_os_error();
            let errno_val = last_os_error.raw_os_error();
            if let Some(inner_errno_val) = errno_val {
                if inner_errno_val == libc::ENOENT {
                    return Err(FluxError::Logic("Provided job ID not found".to_string()));
                }
            }
        }
        check_ptr(plugin_arg_ptr)?;
        unsafe { PluginArgs::from_ptr(plugin_arg_ptr) }
    }

    pub fn get_job_result(&self, id: JobId) -> Result<JobResultCode> {
        let mut job_result: flux_job_result_t = JobResultCode::NONE.bits();
        let rc = unsafe {
            flux_jobtap_get_job_result(
                self.plugin.as_mut_ptr(),
                id.0,
                &mut job_result as *mut flux_job_result_t,
            )
        };
        check_rc(rc)?;
        Ok(JobResultCode::from(job_result))
    }

    pub fn check_for_posted_event(&self, id: JobId, event_name: &str) -> Result<bool> {
        let c_event_name = CString::new(event_name)?;
        let rc = unsafe {
            flux_jobtap_job_event_posted(self.plugin.as_mut_ptr(), id.0, c_event_name.as_ptr())
        };
        check_rc(rc)?;
        Ok(rc == 1)
    }

    pub fn subscribe_to_job_events(&self, id: JobId) -> Result<()> {
        let rc = unsafe { flux_jobtap_job_subscribe(self.plugin.as_mut_ptr(), id.0) };
        check_rc(rc)
    }

    pub fn unsubscribe_from_job_events(&self, id: JobId) {
        unsafe {
            flux_jobtap_job_unsubscribe(self.plugin.as_mut_ptr(), id.0);
        }
    }

    pub fn start_prolog(&self, description: &str) -> Result<()> {
        let c_desc = CString::new(description)?;
        let rc = unsafe { flux_jobtap_prolog_start(self.plugin.as_mut_ptr(), c_desc.as_ptr()) };
        check_rc(rc)
    }

    pub fn finish_prolog(&self, id: JobId, description: &str, status: i32) -> Result<()> {
        let c_desc = CString::new(description)?;
        let rc = unsafe {
            flux_jobtap_prolog_finish(self.plugin.as_mut_ptr(), id.0, c_desc.as_ptr(), status)
        };
        check_rc(rc)
    }

    pub fn start_epilog(&self, description: &str) -> Result<()> {
        let c_desc = CString::new(description)?;
        let rc = unsafe { flux_jobtap_epilog_start(self.plugin.as_mut_ptr(), c_desc.as_ptr()) };
        check_rc(rc)
    }

    pub fn finish_epilog(&self, id: JobId, description: &str, status: i32) -> Result<()> {
        let c_desc = CString::new(description)?;
        let rc = unsafe {
            flux_jobtap_epilog_finish(self.plugin.as_mut_ptr(), id.0, c_desc.as_ptr(), status)
        };
        check_rc(rc)
    }

    // TODO consider whether to add a wrapper to flux_jobtap_call
}

impl Deref for JobtapPlugin {
    type Target = Plugin;

    fn deref(&self) -> &Self::Target {
        &self.plugin
    }
}

impl DerefMut for JobtapPlugin {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.plugin
    }
}

unsafe impl BorrowFluxPtr for JobtapPlugin {
    type CType = flux_plugin_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            plugin: Plugin::borrow_raw(ptr, args)?,
            _cb_boxes: IndexMap::new(),
        })
    }
}

unsafe impl FromFluxPtr for JobtapPlugin {
    unsafe fn from_raw(ptr: *mut Self::CType, args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            plugin: Plugin::from_raw(ptr, args)?,
            _cb_boxes: IndexMap::new(),
        })
    }
}

pub type JobtapPluginEntrypoint = fn(JobtapPlugin) -> Result<()>;

#[macro_export]
macro_rules! __create_jobtap_entrypoint_macro {
    ($user_jobtap_init:path) => {
        const _: $crate::jobtap::JobtapPluginEntrypoint = $user_jobtap_init;

        #[no_mangle]
        pub extern "C" fn flux_plugin_init(
            p: *mut ::flux_sys::core::flux_plugin_t,
        ) -> ::std::ffi::c_int {
            let flux_plugin = match unsafe {
                <$crate::jobtap::JobtapPlugin as $crate::BorrowFluxPtrNoArgs>::borrow_ptr(p)
            } {
                Ok(jp) => jp,
                Err(e) => return $crate::error::to_flux_rc(Err(e), None),
            };
            let flux_handle = flux_plugin.get_flux().ok();
            $crate::error::to_flux_rc($user_jobtap_init(flux_plugin), flux_handle.as_ref())
        }
    };
}

pub use crate::__create_jobtap_entrypoint_macro as create_jobtap_entrypoint;
