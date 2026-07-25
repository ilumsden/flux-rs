use std::ffi::{c_char, c_void, CStr, CString};

use flux_sys::core::{
    flux_kvs_commit, flux_kvs_commit_get_sequence, flux_kvs_copy, flux_kvs_getroot,
    flux_kvs_getroot_get_owner, flux_kvs_getroot_get_sequence, flux_kvs_lookup,
    flux_kvs_lookup_cancel, flux_kvs_lookup_get_dir, flux_kvs_lookup_get_key,
    flux_kvs_lookup_get_raw, flux_kvs_lookup_get_symlink, flux_kvs_move, flux_kvs_namespace_create,
    flux_kvs_namespace_create_with, flux_kvs_namespace_remove, flux_kvsdir_copy, flux_kvsdir_t,
    FLUX_USERID_UNKNOWN,
};
use serde::Deserialize;
use serde_json::{from_slice, Value};

use crate::error::{check_ptr, check_rc, FluxError, Result};
use crate::flux_ptr_management::{FromFluxPtr, FromFluxPtrNoArgs};
use crate::future::FluxFuture;
use crate::handle::FluxHandle;
use crate::kvs::flags::KvsFlags;
use crate::kvs::kvs_dir::KvsDir;
use crate::kvs::txn::KvsTransaction;
use crate::utils::impl_async_future_wrapper;

pub struct Kvs<'a> {
    handle: &'a FluxHandle,
}

impl<'a> Kvs<'a> {
    // TODO implement support for functions related to treeobj and kvsdir

    pub const fn new(handle: &'a FluxHandle) -> Self {
        Self { handle }
    }

    pub fn create_namespace(
        &mut self,
        namespace: &str,
        flags: KvsFlags,
        owner: Option<u32>,
    ) -> Result<FluxFuture> {
        let c_namespace = CString::new(namespace)?;
        let c_owner = owner.unwrap_or(FLUX_USERID_UNKNOWN);
        let future_ptr = unsafe {
            flux_kvs_namespace_create(
                self.handle.h.as_mut_ptr(),
                c_namespace.as_ptr(),
                c_owner,
                flags.bits() as i32,
            )
        };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn create_namespace_with(
        &mut self,
        namespace: &str,
        rootref: &str,
        flags: KvsFlags,
        owner: Option<u32>,
    ) -> Result<FluxFuture> {
        let c_namespace = CString::new(namespace)?;
        let c_rootref = CString::new(rootref)?;
        let c_owner = owner.unwrap_or(FLUX_USERID_UNKNOWN);
        let future_ptr = unsafe {
            flux_kvs_namespace_create_with(
                self.handle.h.as_mut_ptr(),
                c_namespace.as_ptr(),
                c_rootref.as_ptr(),
                c_owner,
                flags.bits() as i32,
            )
        };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn remove_namespace(&mut self, namespace: &str) -> Result<FluxFuture> {
        let c_namespace = CString::new(namespace)?;
        let future_ptr =
            unsafe { flux_kvs_namespace_remove(self.handle.h.as_mut_ptr(), c_namespace.as_ptr()) };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn lookup(
        &mut self,
        key: &str,
        flags: KvsFlags,
        namespace: Option<&str>,
    ) -> Result<Lookup> {
        // Optionally convert 'namespace' to a C String.
        // If namespace is None, c_namespace will be None.
        // If namespace is Some(val), val will be converted to a C String.
        // If an error occurs during that conversion, it will be returned through this method's
        // Result<()> via '.transpose()?'.
        let c_namespace: Option<CString> = namespace
            .and_then(|ns| Some(CString::new(ns).map_err(|cstr_err| FluxError::NulError(cstr_err))))
            .transpose()?;
        let c_key = CString::new(key)?;
        let future_ptr = unsafe {
            flux_kvs_lookup(
                self.handle.h.as_mut_ptr(),
                c_namespace
                    .as_ref()
                    .map_or(std::ptr::null(), |cstr_ns| cstr_ns.as_ptr()),
                flags.bits() as i32,
                c_key.as_ptr(),
            )
        };
        check_ptr(future_ptr)?;
        Ok(Lookup::new(
            unsafe { FluxFuture::from_ptr(future_ptr)? },
            key,
        ))
    }

    pub fn getroot(&mut self, namespace: &str) -> Result<Getroot> {
        let c_namespace = CString::new(namespace)?;
        // TODO if flags are ever used with `flux_kvs_getroot`, update the call appropriately
        let future_ptr =
            unsafe { flux_kvs_getroot(self.handle.h.as_mut_ptr(), c_namespace.as_ptr(), 0) };
        if future_ptr.is_null() {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        }
        check_ptr(future_ptr)?;
        Ok(Getroot::new(unsafe { FluxFuture::from_ptr(future_ptr)? }))
    }

    pub fn copy_entry(
        &mut self,
        srckey: &str,
        dstkey: &str,
        commit_flags: KvsFlags,
        src_namespace: Option<&str>,
        dst_namespace: Option<&str>,
    ) -> Result<FluxFuture> {
        let c_srckey = CString::new(srckey)?;
        let c_dstkey = CString::new(dstkey)?;
        let c_src_namespace: Option<CString> = src_namespace
            .and_then(|ns| Some(CString::new(ns).map_err(|cstr_err| FluxError::NulError(cstr_err))))
            .transpose()?;
        let c_dst_namespace: Option<CString> = dst_namespace
            .and_then(|ns| Some(CString::new(ns).map_err(|cstr_err| FluxError::NulError(cstr_err))))
            .transpose()?;
        let future_ptr = unsafe {
            flux_kvs_copy(
                self.handle.h.as_mut_ptr(),
                c_src_namespace
                    .as_ref()
                    .map_or(std::ptr::null(), |cstr| cstr.as_ptr()),
                c_srckey.as_ptr(),
                c_dst_namespace
                    .as_ref()
                    .map_or(std::ptr::null(), |cstr| cstr.as_ptr()),
                c_dstkey.as_ptr(),
                commit_flags.bits() as i32,
            )
        };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn move_entry(
        &mut self,
        srckey: &str,
        dstkey: &str,
        commit_flags: KvsFlags,
        src_namespace: Option<&str>,
        dst_namespace: Option<&str>,
    ) -> Result<FluxFuture> {
        let c_srckey = CString::new(srckey)?;
        let c_dstkey = CString::new(dstkey)?;
        let c_src_namespace: Option<CString> = src_namespace
            .and_then(|ns| Some(CString::new(ns).map_err(|cstr_err| FluxError::NulError(cstr_err))))
            .transpose()?;
        let c_dst_namespace: Option<CString> = dst_namespace
            .and_then(|ns| Some(CString::new(ns).map_err(|cstr_err| FluxError::NulError(cstr_err))))
            .transpose()?;
        let future_ptr = unsafe {
            flux_kvs_move(
                self.handle.h.as_mut_ptr(),
                c_src_namespace
                    .as_ref()
                    .map_or(std::ptr::null(), |cstr| cstr.as_ptr()),
                c_srckey.as_ptr(),
                c_dst_namespace
                    .as_ref()
                    .map_or(std::ptr::null(), |cstr| cstr.as_ptr()),
                c_dstkey.as_ptr(),
                commit_flags.bits() as i32,
            )
        };
        check_ptr(future_ptr)?;
        unsafe { FluxFuture::from_ptr(future_ptr) }
    }

    pub fn commit(
        &mut self,
        txn: &KvsTransaction,
        flags: KvsFlags,
        namespace: Option<&str>,
    ) -> Result<Commit> {
        let c_namespace: Option<CString> = namespace
            .and_then(|ns| Some(CString::new(ns).map_err(|cstr_err| FluxError::NulError(cstr_err))))
            .transpose()?;
        let future_ptr = unsafe {
            flux_kvs_commit(
                self.handle.h.as_mut_ptr(),
                c_namespace
                    .as_ref()
                    .map_or(std::ptr::null(), |cstr| cstr.as_ptr()),
                flags.bits() as i32,
                txn.c_txn.as_mut_ptr(),
            )
        };
        check_ptr(future_ptr)?;
        Ok(Commit::new(unsafe { FluxFuture::from_ptr(future_ptr)? }))
    }
}

pub struct Lookup {
    future: FluxFuture<'static>,
    key: String,
}

impl Lookup {
    // TODO implement get_treeobj

    pub fn new(future: FluxFuture<'static>, key: &str) -> Self {
        Self {
            future,
            key: key.to_string(),
        }
    }

    pub fn get<'a>(&'a mut self) -> Result<&'a [u8]> {
        let mut value_ptr: *const c_void = std::ptr::null();
        let mut value_len: i32 = 0;
        let rc = unsafe {
            flux_kvs_lookup_get_raw(
                self.future.c_future.as_mut_ptr(),
                &mut value_ptr as *mut *const c_void,
                &mut value_len as *mut i32,
            )
        };
        check_rc(rc)?;
        if value_ptr.is_null() {
            return Err(FluxError::Logic(String::from(
                "Received a NULL value pointer from the Flux KVS",
            )));
        }
        Ok(unsafe { std::slice::from_raw_parts(value_ptr as *const u8, value_len as usize) })
    }

    pub fn get_json(&mut self) -> Result<Value> {
        let raw_value = self.get()?;
        Ok(from_slice(raw_value)?)
    }

    pub fn get_deserializable<'a, D: Deserialize<'a>>(&'a mut self) -> Result<D> {
        let raw_value = self.get()?;
        Ok(from_slice(raw_value)?)
    }

    pub fn get_dir(&mut self) -> Result<KvsDir> {
        let mut kvsdir: *const flux_kvsdir_t = std::ptr::null();
        let rc = unsafe {
            flux_kvs_lookup_get_dir(
                self.future.c_future.as_mut_ptr(),
                &mut kvsdir as *mut *const flux_kvsdir_t,
            )
        };
        check_rc(rc)?;
        let kvsdir_copy: *mut flux_kvsdir_t = unsafe { flux_kvsdir_copy(kvsdir) };
        check_ptr(kvsdir_copy)?;
        unsafe { KvsDir::from_raw(kvsdir_copy, Some(self.key.clone())) }
    }

    pub fn get_symlink<'a>(&'a mut self) -> Result<(&'a str, Option<&'a str>)> {
        let mut ns_val: *const c_char = std::ptr::null();
        let mut target_val: *const c_char = std::ptr::null();
        let rc = unsafe {
            flux_kvs_lookup_get_symlink(
                self.future.c_future.as_mut_ptr(),
                &mut ns_val as *mut *const c_char,
                &mut target_val as *mut *const c_char,
            )
        };
        check_rc(rc)?;
        if target_val.is_null() {
            return Err(FluxError::Logic(String::from(
                "Received a NULL pointer for 'target' from the Flux KVS",
            )));
        }
        let target_str = unsafe { CStr::from_ptr(target_val).to_str()? };
        let mut ns_str = None;
        if !ns_val.is_null() {
            ns_str = Some(unsafe { CStr::from_ptr(ns_val).to_str()? });
        }
        Ok((target_str, ns_str))
    }

    /// Get the key for the Flux KVS lookup.
    ///
    /// # Returns
    /// * `Ok(Some(&str))` when the key is successfully obtained from Flux.
    /// * `Ok(None)` when the future used to create the `Lookup` object did not come from a KVS lookup.
    /// * `Err(FluxError)` when an error occurs.
    pub fn get_key<'a>(&'a mut self) -> Result<Option<&'a str>> {
        let key_ptr = unsafe { flux_kvs_lookup_get_key(self.future.c_future.as_mut_ptr()) };
        if key_ptr.is_null() {
            let last_os_error = std::io::Error::last_os_error();
            let last_errno = last_os_error.raw_os_error();
            if let Some(errno_val) = last_errno {
                if errno_val == libc::EINVAL {
                    return Ok(None);
                }
            }
            return Err(FluxError::System(last_os_error));
        }
        Ok(Some(unsafe { CStr::from_ptr(key_ptr).to_str()? }))
    }

    pub fn cancel(&mut self) -> Result<()> {
        let rc = unsafe { flux_kvs_lookup_cancel(self.future.c_future.as_mut_ptr()) };
        check_rc(rc)
    }
}

impl_async_future_wrapper!(
    #[from_sync(Lookup)]
    pub struct AsyncLookup {
        #[from_sync(future)]
        future: AsyncFluxFuture,
        #[from_sync(key)]
        #[to_sync_action(Clone)]
        key: String,
    }
);

pub struct Getroot {
    future: FluxFuture<'static>,
}

impl Getroot {
    pub const fn new(future: FluxFuture<'static>) -> Self {
        Self { future }
    }

    // TODO implement get_treeobj and get_blobref

    pub fn get_sequence(&mut self) -> Result<i32> {
        let mut seq: i32 = 0;
        let rc = unsafe {
            flux_kvs_getroot_get_sequence(self.future.c_future.as_mut_ptr(), &mut seq as *mut i32)
        };
        check_rc(rc)?;
        Ok(seq)
    }

    pub fn get_owner(&mut self) -> Result<u32> {
        let mut owner: u32 = 0;
        let rc = unsafe {
            flux_kvs_getroot_get_owner(self.future.c_future.as_mut_ptr(), &mut owner as *mut u32)
        };
        check_rc(rc)?;
        Ok(owner)
    }
}

impl_async_future_wrapper!(
    #[from_sync(Getroot)]
    pub struct AsyncGetroot {
        #[from_sync(future)]
        future: AsyncFluxFuture,
    }
);

pub struct Commit {
    future: FluxFuture<'static>,
}

impl Commit {
    pub const fn new(future: FluxFuture<'static>) -> Self {
        Self { future }
    }

    // TODO implment get_treeobj and

    pub fn get_sequence(&mut self) -> Result<i32> {
        let mut seq: i32 = 0;
        let rc = unsafe {
            flux_kvs_commit_get_sequence(self.future.c_future.as_mut_ptr(), &mut seq as *mut i32)
        };
        check_rc(rc)?;
        Ok(seq)
    }
}

impl_async_future_wrapper!(
    #[from_sync(Commit)]
    pub struct AsyncCommit {
        #[from_sync(future)]
        future: AsyncFluxFuture,
    }
);
