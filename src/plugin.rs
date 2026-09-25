use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;

use bitflags::bitflags;
use flux_sys::core::{
    FLUX_PLUGIN_ARG_IN, FLUX_PLUGIN_ARG_OUT, FLUX_PLUGIN_ARG_REPLACE, FLUX_PLUGIN_RTLD_DEEPBIND,
    FLUX_PLUGIN_RTLD_GLOBAL, FLUX_PLUGIN_RTLD_LAZY, FLUX_PLUGIN_RTLD_NOW, flux_plugin_add_handler,
    flux_plugin_arg_create, flux_plugin_arg_destroy, flux_plugin_arg_get, flux_plugin_arg_set,
    flux_plugin_arg_strerror, flux_plugin_arg_t, flux_plugin_aux_get, flux_plugin_aux_set,
    flux_plugin_call, flux_plugin_create, flux_plugin_destroy, flux_plugin_get_flags,
    flux_plugin_get_name, flux_plugin_get_path, flux_plugin_get_uuid, flux_plugin_load_dso,
    flux_plugin_remove_handler, flux_plugin_set_flags, flux_plugin_set_name, flux_plugin_t,
};
use indexmap::IndexMap;

use serde::{Deserialize, Serialize};
use serde_json::map::Entry;
use serde_json::{Map, Value};

use crate::error::{FluxError, Result, flux_try, to_flux_rc};
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, BorrowFluxPtrNoArgs, Borrowed, FluxPtr, FromFluxPtr, IntoFluxPtr,
    Owned, PossiblyDroppablePtr, define_as_flux_ptr_body, define_into_flux_ptr_body,
};
use crate::handle::AuxThinPtrWrapper;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
    pub struct PluginLoadFlags: u32 {
        const NONE = 0;
        const LAZY = FLUX_PLUGIN_RTLD_LAZY;
        const NOW = FLUX_PLUGIN_RTLD_NOW;
        const GLOBAL = FLUX_PLUGIN_RTLD_GLOBAL;
        const DEEPBIND = FLUX_PLUGIN_RTLD_DEEPBIND;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
    pub struct PluginArgFlag: u32 {
        const IN = FLUX_PLUGIN_ARG_IN;
        const OUT = FLUX_PLUGIN_ARG_OUT;
        const REPLACE = FLUX_PLUGIN_ARG_REPLACE;
    }
}

#[derive(Default)]
pub enum PluginArgUpdateDirection {
    #[default]
    PushToC,
    PullFromC,
}

pub struct PluginArgs<State: PossiblyDroppablePtr<flux_plugin_arg_t> = Owned<flux_plugin_arg_t>> {
    c_args: FluxPtr<flux_plugin_arg_t, State>,
    in_args: Map<String, Value>,
    out_args: Map<String, Value>,
}

pub type OwnedPluginArgs = PluginArgs<Owned<flux_plugin_arg_t>>;
pub type BorrowedPluginArgs<'a> = PluginArgs<Borrowed<'a, flux_plugin_arg_t>>;

impl OwnedPluginArgs {
    pub fn new() -> Result<Self> {
        let c_args = flux_try!(flux_plugin_arg_create())?;
        Ok(Self {
            c_args: FluxPtr::create_owned(c_args, flux_plugin_arg_destroy)?,
            in_args: Map::new(),
            out_args: Map::new(),
        })
    }
}

impl<State: PossiblyDroppablePtr<flux_plugin_arg_t>> PluginArgs<State> {
    pub fn input_args(&self) -> &Map<String, Value> {
        &self.in_args
    }

    pub fn input_args_mut(&mut self) -> &mut Map<String, Value> {
        &mut self.in_args
    }

    pub fn output_args(&self) -> &Map<String, Value> {
        &self.out_args
    }

    pub fn output_args_mut(&mut self) -> &mut Map<String, Value> {
        &mut self.out_args
    }

    pub fn insert<T: Serialize>(&mut self, key: &str, val: T, flag: PluginArgFlag) -> Result<bool> {
        let map_entry = if flag.contains(PluginArgFlag::IN) {
            self.in_args.entry(key)
        } else if flag.contains(PluginArgFlag::OUT) {
            self.out_args.entry(key)
        } else {
            return Err(FluxError::Logic(
                "The 'flag' argument must have either the IN or OUT bit set".to_string(),
            ));
        };
        match map_entry {
            Entry::Vacant(ve) => {
                ve.insert(serde_json::to_value(val)?);
                Ok(true)
            }
            Entry::Occupied(mut oe) => {
                if flag.contains(PluginArgFlag::REPLACE) {
                    oe.insert(serde_json::to_value(val)?);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    pub fn get_raw(&self, key: &str, flags: PluginArgFlag) -> Option<&Value> {
        if flags.contains(PluginArgFlag::IN) {
            self.in_args.get(key)
        } else if flags.contains(PluginArgFlag::OUT) {
            self.out_args.get(key)
        } else {
            None
        }
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str, flags: PluginArgFlag) -> Option<T> {
        serde_json::from_value(self.get_raw(key, flags)?.clone()).ok()
    }

    pub fn insert_input<T: Serialize>(
        &mut self,
        key: &str,
        val: T,
        override_entry: bool,
    ) -> Result<bool> {
        let flag = if override_entry {
            PluginArgFlag::IN | PluginArgFlag::REPLACE
        } else {
            PluginArgFlag::IN
        };
        self.insert(key, val, flag)
    }

    pub fn get_input<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.get(key, PluginArgFlag::IN)
    }

    pub fn insert_output<T: Serialize>(
        &mut self,
        key: &str,
        val: T,
        override_entry: bool,
    ) -> Result<bool> {
        let flag = if override_entry {
            PluginArgFlag::OUT | PluginArgFlag::REPLACE
        } else {
            PluginArgFlag::OUT
        };
        self.insert(key, val, flag)
    }

    pub fn get_output<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.get(key, PluginArgFlag::OUT)
    }

    pub fn update(&mut self, direction: PluginArgUpdateDirection) -> Result<()> {
        match direction {
            PluginArgUpdateDirection::PushToC => {
                let in_args_ser = serde_json::to_string(&self.in_args)?;
                let c_in_args = CString::new(in_args_ser)?;
                let rc = unsafe {
                    flux_plugin_arg_set(
                        self.c_args.as_mut_ptr(),
                        (PluginArgFlag::IN | PluginArgFlag::REPLACE).bits() as _,
                        c_in_args.as_ptr(),
                    )
                };
                if rc == -1 {
                    let err_ptr = unsafe { flux_plugin_arg_strerror(self.c_args.as_mut_ptr()) };
                    if err_ptr.is_null() {
                        return Err(FluxError::System(
                            "flux_plugin_arg_set",
                            std::io::Error::last_os_error(),
                        ));
                    } else {
                        return Err(FluxError::Logic(unsafe {
                            CStr::from_ptr(err_ptr).to_string_lossy().to_string()
                        }));
                    }
                }
                let out_args_ser = serde_json::to_string(&self.out_args)?;
                let c_out_args = CString::new(out_args_ser)?;
                let rc = unsafe {
                    flux_plugin_arg_set(
                        self.c_args.as_mut_ptr(),
                        (PluginArgFlag::OUT | PluginArgFlag::REPLACE).bits() as _,
                        c_out_args.as_ptr(),
                    )
                };
                if rc == -1 {
                    let err_ptr = unsafe { flux_plugin_arg_strerror(self.c_args.as_mut_ptr()) };
                    if err_ptr.is_null() {
                        Err(FluxError::System(
                            "flux_plugin_arg_set",
                            std::io::Error::last_os_error(),
                        ))
                    } else {
                        Err(FluxError::Logic(unsafe {
                            CStr::from_ptr(err_ptr).to_string_lossy().to_string()
                        }))
                    }
                } else {
                    Ok(())
                }
            }
            PluginArgUpdateDirection::PullFromC => {
                self.in_args =
                    Self::extract_args_from_c(self.c_args.as_mut_ptr(), PluginArgFlag::IN)?;
                self.out_args =
                    Self::extract_args_from_c(self.c_args.as_mut_ptr(), PluginArgFlag::OUT)?;
                Ok(())
            }
        }
    }

    fn extract_args_from_c(
        ptr: *mut flux_plugin_arg_t,
        flags: PluginArgFlag,
    ) -> Result<Map<String, Value>> {
        let mut json_c_str: *mut c_char = std::ptr::null_mut();
        let rc = unsafe {
            flux_plugin_arg_get(ptr, flags.bits() as _, &mut json_c_str as *mut *mut c_char)
        };
        let args = if rc == -1 {
            let last_system_error = std::io::Error::last_os_error();
            if let Some(raw_os_error) = last_system_error.raw_os_error()
                && raw_os_error == libc::ENOENT
            {
                Ok(Map::new())
            } else {
                let plugin_arg_strerror_ptr = unsafe { flux_plugin_arg_strerror(ptr) };
                if plugin_arg_strerror_ptr.is_null() {
                    return Err(FluxError::System(
                        "flux_plugin_arg_get",
                        std::io::Error::last_os_error(),
                    ));
                }
                return Err(FluxError::Logic(unsafe {
                    CStr::from_ptr(plugin_arg_strerror_ptr)
                        .to_string_lossy()
                        .to_string()
                }));
            }
        } else if json_c_str.is_null() {
            return Err(FluxError::Logic("flux_plugin_arg_get reported success, but returned a NULL pointer for the JSON string".to_string()));
        } else {
            unsafe {
                CStr::from_ptr(json_c_str)
                    .to_str()
                    .map_err(FluxError::from)
                    .and_then(|str_slice| {
                        serde_json::from_str::<Map<String, Value>>(str_slice)
                            .map_err(FluxError::from)
                    })
            }
        };
        unsafe {
            libc::free(json_c_str as *mut c_void);
        }
        args
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedPluginArgs<'a> {
    type CType = flux_plugin_arg_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        if ptr.is_null() {
            return Err(FluxError::Logic(
                "Cannot create a Rust PluginArgs object from a NULL flux_plugin_arg_t pointer"
                    .to_string(),
            ));
        }
        let in_args = Self::extract_args_from_c(ptr, PluginArgFlag::IN);
        let out_args = Self::extract_args_from_c(ptr, PluginArgFlag::OUT);
        Ok(Self {
            c_args: FluxPtr::create_borrowed(ptr)?,
            in_args: in_args?,
            out_args: out_args?,
        })
    }
}

unsafe impl FromFluxPtr for OwnedPluginArgs {
    type CType = flux_plugin_arg_t;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        if ptr.is_null() {
            return Err(FluxError::Logic(
                "Cannot create a Rust PluginArgs object from a NULL flux_plugin_arg_t pointer"
                    .to_string(),
            ));
        }
        let in_args = Self::extract_args_from_c(ptr, PluginArgFlag::IN);
        let out_args = Self::extract_args_from_c(ptr, PluginArgFlag::OUT);
        Ok(Self {
            c_args: FluxPtr::create_owned(ptr, flux_plugin_arg_destroy)?,
            in_args: in_args?,
            out_args: out_args?,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<flux_plugin_arg_t>> AsFluxPtr for PluginArgs<State> {
    define_as_flux_ptr_body!(flux_plugin_arg_t, c_args);
}

unsafe impl IntoFluxPtr for OwnedPluginArgs {
    define_into_flux_ptr_body!(c_args);
}

macro_rules! check_plugin_strerror {
    ($plugin_ptr:expr) => {{
        let check_plugin_strerror_raw_ptr =
            unsafe { ::flux_sys::core::flux_plugin_strerror($plugin_ptr) };
        if check_plugin_strerror_raw_ptr.is_null() {
            $crate::error::FluxReturnType::check_flux_return(
                check_plugin_strerror_raw_ptr as *mut ::std::ffi::c_char,
                "flux_plugin_strerror",
            )
        } else {
            let check_plugin_strerror_str = unsafe {
                ::std::ffi::CStr::from_ptr(check_plugin_strerror_raw_ptr)
                    .to_string_lossy()
                    .to_string()
            };
            Err($crate::error::FluxError::Logic(check_plugin_strerror_str))
        }
    }};
}

pub type PluginCallback =
    Box<dyn FnMut(BorrowedPlugin<'_>, &str, &mut BorrowedPluginArgs<'_>) -> Result<()>>;

pub struct Plugin<State: PossiblyDroppablePtr<flux_plugin_t> = Owned<flux_plugin_t>> {
    pub(crate) c_plugin: FluxPtr<flux_plugin_t, State>,
    pub(crate) _cb_boxes: IndexMap<String, Box<PluginCallback>>,
}

pub type OwnedPlugin = Plugin<Owned<flux_plugin_t>>;
pub type BorrowedPlugin<'a> = Plugin<Borrowed<'a, flux_plugin_t>>;

impl OwnedPlugin {
    pub fn new() -> Result<Self> {
        let plugin_ptr = flux_try!(flux_plugin_create())?;
        Ok(Self {
            c_plugin: FluxPtr::create_owned(plugin_ptr, flux_plugin_destroy)?,
            _cb_boxes: IndexMap::new(),
        })
    }

    pub fn load_dso(dso_path: &Path) -> Result<Self> {
        let new_plugin = Self::new()?;
        let ru_path = dso_path.to_str().ok_or(FluxError::Logic(
            "Cannot convert dso path into a Rust str".to_string(),
        ))?;
        let c_path = CString::new(ru_path)?;
        let rc = unsafe { flux_plugin_load_dso(new_plugin.c_plugin.as_mut_ptr(), c_path.as_ptr()) };
        if rc == -1 {
            check_plugin_strerror!(new_plugin.c_plugin.as_mut_ptr())?;
        }
        Ok(new_plugin)
    }
}

impl<State: PossiblyDroppablePtr<flux_plugin_t>> Plugin<State> {
    pub fn set_flags(&mut self, flags: PluginLoadFlags) -> Result<()> {
        let rc = unsafe { flux_plugin_set_flags(self.c_plugin.as_mut_ptr(), flags.bits() as _) };
        if rc == -1 {
            check_plugin_strerror!(self.c_plugin.as_mut_ptr()).map(|_| ())
        } else {
            Ok(())
        }
    }

    pub fn get_flags(&self) -> Result<PluginLoadFlags> {
        let flags_raw = unsafe { flux_plugin_get_flags(self.c_plugin.as_mut_ptr()) };
        if flags_raw == -1 {
            check_plugin_strerror!(self.c_plugin.as_mut_ptr()).map(|_| PluginLoadFlags::NONE)
        } else {
            Ok(PluginLoadFlags::from_bits_truncate(flags_raw as _))
        }
    }

    pub fn set_name(&mut self, name: &str) -> Result<()> {
        let c_name = CString::new(name)?;
        let rc = unsafe { flux_plugin_set_name(self.c_plugin.as_mut_ptr(), c_name.as_ptr()) };
        if rc == -1 {
            check_plugin_strerror!(self.c_plugin.as_mut_ptr()).map(|_| ())
        } else {
            Ok(())
        }
    }

    pub fn get_name(&self) -> Result<String> {
        let plugin_name_ptr = flux_try!(flux_plugin_get_name(self.c_plugin.as_mut_ptr()))?;
        let plugin_name = unsafe {
            CStr::from_ptr(plugin_name_ptr)
                .to_string_lossy()
                .to_string()
        };
        Ok(plugin_name)
    }

    pub fn get_uuid(&self) -> Result<String> {
        let uuid_name_ptr = flux_try!(flux_plugin_get_uuid(self.c_plugin.as_mut_ptr()))?;
        let uuid = unsafe { CStr::from_ptr(uuid_name_ptr).to_string_lossy().to_string() };
        Ok(uuid)
    }

    pub fn get_path(&self) -> Result<String> {
        let path_name_ptr = flux_try!(flux_plugin_get_path(self.c_plugin.as_mut_ptr()))?;
        let path = unsafe { CStr::from_ptr(path_name_ptr).to_string_lossy().to_string() };
        Ok(path)
    }

    pub fn set_aux<T: Send + Sync + 'static>(&mut self, key: &str, val: T) -> Result<()> {
        let c_key = CString::new(key)?;
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
            flux_plugin_aux_set(
                self.c_plugin.as_mut_ptr(),
                c_key.as_ptr(),
                raw_data_ptr,
                Some(destroy_aux_trampoline),
            )
        };

        if rc == -1 {
            let _ = unsafe { Box::from_raw(raw_data_ptr as *mut AuxThinPtrWrapper) };
            Err(FluxError::System(
                "flux_plugin_aux_set",
                std::io::Error::last_os_error(),
            ))
        } else {
            Ok(())
        }
    }

    pub fn get_aux<T: 'static>(&self, key: &str) -> Result<&T> {
        let raw_ptr = self.get_aux_raw(key)?;
        let wrapper = unsafe { &*(raw_ptr as *const AuxThinPtrWrapper) };
        match wrapper.inner.downcast_ref::<T>() {
            Some(typed_ref) => Ok(typed_ref),
            None => Err(FluxError::Logic(format!(
                "Type mismatch for aux key '{key}'",
            ))),
        }
    }

    pub fn get_aux_raw(&self, key: &str) -> Result<*mut c_void> {
        let c_key = CString::new(key)?;
        flux_try!(flux_plugin_aux_get(
            self.c_plugin.as_mut_ptr(),
            c_key.as_ptr()
        ))
    }

    extern "C" fn trampoline(
        p: *mut flux_plugin_t,
        topic: *const c_char,
        args: *mut flux_plugin_arg_t,
        data: *mut c_void,
    ) -> c_int {
        if data.is_null() {
            return -1;
        }
        let closure = unsafe { &mut *(data as *mut PluginCallback) };
        let rust_topic = unsafe { CStr::from_ptr(topic).to_string_lossy() };
        let mut plugin_args = match unsafe { PluginArgs::borrow_ptr(args) } {
            Ok(pa) => pa,
            Err(err) => {
                return to_flux_rc(Err(err), None);
            }
        };
        let plugin = match unsafe { Plugin::borrow_ptr(p) } {
            Ok(plug) => plug,
            Err(e) => return to_flux_rc(Err(e), None),
        };
        let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            closure(plugin, &rust_topic, &mut plugin_args)
        }));
        let update_res = plugin_args.update(PluginArgUpdateDirection::PushToC);
        match (panic_res, update_res) {
            (Ok(Ok(())), Err(e)) => to_flux_rc(Err(e), None),
            (Ok(res), _) => to_flux_rc(res, None),
            (Err(_panic), _) => -1,
        }
    }

    pub fn add_handler(&mut self, topic: &str, callback: PluginCallback) -> Result<()> {
        if self._cb_boxes.contains_key(topic) {
            return Err(FluxError::Logic(format!(
                "Handler for topic '{topic}' already exists",
            )));
        }
        let c_topic = CString::new(topic)?;
        let mut stable_cb = Box::new(callback);
        let arg_ptr = &mut *stable_cb as *mut PluginCallback as *mut c_void;
        flux_try!(flux_plugin_add_handler(
            self.c_plugin.as_mut_ptr(),
            c_topic.as_ptr(),
            Some(Self::trampoline),
            arg_ptr,
        ))?;
        self._cb_boxes.insert(topic.to_string(), stable_cb);
        Ok(())
    }

    pub fn get_handler(&self, topic: &str) -> Option<&PluginCallback> {
        self._cb_boxes.get(topic).map(|b| &**b)
    }

    pub fn match_handler(&self, topic: &str) -> Option<&PluginCallback> {
        let c_topic = CString::new(topic).ok()?;
        // Iterate over the topic/Rust callback pairs and check for a glob match using
        // `fnmatch`. This logic mirrors that in `flux_plugin_match_handler`.
        for (pattern, cb) in &self._cb_boxes {
            let c_pattern = CString::new(pattern.as_str()).ok()?;
            let match_result = unsafe { libc::fnmatch(c_pattern.as_ptr(), c_topic.as_ptr(), 0) };
            if match_result == 0 {
                return Some(cb);
            }
        }
        None
    }

    pub fn remove_handler(&mut self, topic: &str) -> Result<()> {
        let c_topic = CString::new(topic)?;
        flux_try!(flux_plugin_remove_handler(
            self.c_plugin.as_mut_ptr(),
            c_topic.as_ptr()
        ))?;
        // Do not handle the Option<> returned by shift_remove because it will only be None
        // when the topic was not previously recorded. That is not an error.
        self._cb_boxes.shift_remove(topic);
        Ok(())
    }

    /// Fully set up and register handlers for the plugin.
    ///
    /// This method is equivalent to `flux_plugin_register` from the C API.
    /// It is essentially just a wrapper that calls set_name and then calls
    /// add_handler for each key-value pair in `plugin_handlers`.
    pub fn register(
        &mut self,
        plugin_name: &str,
        plugin_handlers: HashMap<String, PluginCallback>,
    ) -> Result<()> {
        self.set_name(plugin_name)?;
        for (topic, cb) in plugin_handlers.into_iter() {
            self.add_handler(&topic, cb)?;
        }
        Ok(())
    }

    pub fn call(&self, name: &str, args: &mut PluginArgs) -> Result<()> {
        let c_name = CString::new(name)?;
        args.update(PluginArgUpdateDirection::PushToC)?;
        let rc = unsafe {
            flux_plugin_call(
                self.c_plugin.as_mut_ptr(),
                c_name.as_ptr(),
                args.as_mut_ptr(),
            )
        };
        if rc == -1 {
            check_plugin_strerror!(self.c_plugin.as_mut_ptr()).map(|_| ())
        } else if rc == 0 {
            Err(FluxError::Logic(format!(
                "Tried calling a plugin callback with an unknown name: {}",
                name
            )))
        } else {
            args.update(PluginArgUpdateDirection::PullFromC)
        }
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedPlugin<'a> {
    type CType = flux_plugin_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_plugin: FluxPtr::create_borrowed(ptr)?,
            _cb_boxes: IndexMap::new(),
        })
    }
}

unsafe impl FromFluxPtr for OwnedPlugin {
    type CType = flux_plugin_t;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_plugin: FluxPtr::create_owned(ptr, flux_plugin_destroy)?,
            _cb_boxes: IndexMap::new(),
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<flux_plugin_t>> AsFluxPtr for Plugin<State> {
    define_as_flux_ptr_body!(flux_plugin_t, c_plugin);
}

unsafe impl IntoFluxPtr for OwnedPlugin {
    define_into_flux_ptr_body!(c_plugin);
}
