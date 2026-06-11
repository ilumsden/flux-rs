use std::ffi::CString;
use std::os::raw::c_void;

use bitflags::bitflags;
use flux_sys::core::kvs_op::{
    FLUX_KVS_APPEND, FLUX_KVS_READDIR, FLUX_KVS_READLINK, FLUX_KVS_TREEOBJ, FLUX_KVS_WAITCREATE,
    FLUX_KVS_WATCH, FLUX_KVS_WATCH_APPEND, FLUX_KVS_WATCH_FULL, FLUX_KVS_WATCH_UNIQ,
};
use flux_sys::core::{
    flux_kvs_txn_create, flux_kvs_txn_destroy, flux_kvs_txn_mkdir, flux_kvs_txn_put_raw,
    flux_kvs_txn_symlink, flux_kvs_txn_t, flux_kvs_txn_unlink,
};
use serde::Serialize;
use serde_json::Value;

use crate::error::{FluxError, Result};

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct KvsFlags: u32 {
        const NONE = 0;
        const APPEND = FLUX_KVS_APPEND;
        const READDIR = FLUX_KVS_READDIR;
        const READLINK = FLUX_KVS_READLINK;
        const TREEOBJ = FLUX_KVS_TREEOBJ;
        const WAITCREATE = FLUX_KVS_WAITCREATE;
        const WATCH = FLUX_KVS_WATCH;
        const WATCH_APPEND = FLUX_KVS_WATCH_APPEND;
        const WATCH_FULL = FLUX_KVS_WATCH_FULL;
        const WATCH_UNIQ = FLUX_KVS_WATCH_UNIQ;
    }
}

pub struct KvsTransaction {
    c_txn: *mut flux_kvs_txn_t,
}

impl KvsTransaction {
    pub fn new() -> Result<Self> {
        let txn = unsafe { flux_kvs_txn_create() };
        if txn.is_null() {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(Self { c_txn: txn })
    }

    pub fn from_ptr(txn: *mut flux_kvs_txn_t) -> Result<Self> {
        if txn.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot create a KvsTransaction from a null pointer",
            )));
        }
        Ok(Self { c_txn: txn })
    }

    pub fn put(&mut self, key: &str, data: &[u8], flags: KvsFlags) -> Result<()> {
        let c_key = CString::new(key)?;
        // TODO figure out why flux-sys has the length field be an int (i.e., i32) instead of size_t (i.e., usize)
        let rc = unsafe {
            flux_kvs_txn_put_raw(
                self.c_txn,
                flags.bits() as i32,
                c_key.as_ptr(),
                data.as_ptr() as *const c_void,
                data.len() as i32,
            )
        };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn put_json(&mut self, key: &str, value: &Value, flags: KvsFlags) -> Result<()> {
        let serialized_val = serde_json::to_vec(value)?;
        self.put(key, &serialized_val, flags)
    }

    pub fn put_serializable<S: Serialize>(
        &mut self,
        key: &str,
        value: &S,
        flags: KvsFlags,
    ) -> Result<()> {
        let serialized_val = serde_json::to_vec(value)?;
        self.put(key, &serialized_val, flags)
    }

    pub fn mkdir(&mut self, key: &str, flags: KvsFlags) -> Result<()> {
        let c_key = CString::new(key)?;
        let rc = unsafe { flux_kvs_txn_mkdir(self.c_txn, flags.bits() as i32, c_key.as_ptr()) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn unlink(&mut self, key: &str, flags: KvsFlags) -> Result<()> {
        let c_key = CString::new(key)?;
        let rc = unsafe { flux_kvs_txn_unlink(self.c_txn, flags.bits() as i32, c_key.as_ptr()) };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn symlink(
        &mut self,
        key: &str,
        target: &str,
        namespace: Option<&str>,
        flags: KvsFlags,
    ) -> Result<()> {
        // Convert 'key' to a C String
        let c_key = CString::new(key)?;
        // Optionally convert 'namespace' to a C String.
        // If namespace is None, c_namespace will be None.
        // If namespace is Some(val), val will be converted to a C String.
        // If an error occurs during that conversion, it will be returned through this method's
        // Result<()> via '.transpose()?'.
        let c_namespace: Option<CString> = namespace
            .and_then(|ns| Some(CString::new(ns).map_err(|cstr_err| FluxError::NulError(cstr_err))))
            .transpose()?;
        // Convert 'target' to a C String
        let c_target = CString::new(target)?;
        // Call flux_kvs_txn_symlink.
        // Note that c_namespace is converted to either the C String pointer or a NULL pointer depending
        // on whether the Option is "Some" or "None".
        let rc = unsafe {
            flux_kvs_txn_symlink(
                self.c_txn,
                flags.bits() as i32,
                c_key.as_ptr(),
                c_namespace
                    .as_ref()
                    .map_or(std::ptr::null(), |cstr_ns| cstr_ns.as_ptr()),
                c_target.as_ptr(),
            )
        };
        if rc == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    // TODO add wrapper for put_treeobj, if possible
}

impl Drop for KvsTransaction {
    fn drop(&mut self) {
        unsafe {
            if !self.c_txn.is_null() {
                flux_kvs_txn_destroy(self.c_txn);
            }
        }
    }
}
