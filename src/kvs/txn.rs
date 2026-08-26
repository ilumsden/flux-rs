use std::ffi::CString;

use flux_sys::core::{
    flux_kvs_txn_create, flux_kvs_txn_destroy, flux_kvs_txn_mkdir, flux_kvs_txn_put_raw,
    flux_kvs_txn_symlink, flux_kvs_txn_t, flux_kvs_txn_unlink,
};
use serde::Serialize;
use serde_json::Value;

use crate::error::{FluxError, Result, flux_try};
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, Borrowed, FluxPtr, FromFluxPtr, IntoFluxPtr, Owned,
    PossiblyDroppablePtr, define_as_flux_ptr_body, define_into_flux_ptr_body,
};
use crate::kvs::flags::KvsFlags;

pub struct KvsTransaction<State: PossiblyDroppablePtr<flux_kvs_txn_t> = Owned<flux_kvs_txn_t>> {
    pub(crate) c_txn: FluxPtr<flux_kvs_txn_t, State>,
    pub(crate) base_path: Option<String>,
}

pub type OwnedKvsTransaction = KvsTransaction<Owned<flux_kvs_txn_t>>;
pub type BorrowedKvsTransaction<'a> = KvsTransaction<Borrowed<'a, flux_kvs_txn_t>>;

impl OwnedKvsTransaction {
    pub fn new() -> Result<Self> {
        let txn = flux_try!(flux_kvs_txn_create())?;
        Ok(Self {
            c_txn: FluxPtr::create_owned(txn, flux_kvs_txn_destroy)?,
            base_path: None,
        })
    }

    pub fn from_path(path: &str) -> Result<Self> {
        let mut txn = Self::new()?;
        txn.base_path = Some(path.to_string());
        Ok(txn)
    }
}

impl<State: PossiblyDroppablePtr<flux_kvs_txn_t>> KvsTransaction<State> {
    pub fn put(&mut self, key: &str, data: &[u8], flags: KvsFlags) -> Result<()> {
        let full_key = if let Some(base) = &self.base_path {
            format!("{base}.{key}")
        } else {
            key.to_string()
        };
        let c_key = CString::new(full_key)?;
        flux_try!(empty_ok
            // TODO figure out why flux-sys has the length field be an int (i.e., i32) instead of size_t (i.e., usize)
            flux_kvs_txn_put_raw(
                self.c_txn.as_mut_ptr(),
                flags.bits() as _,
                c_key.as_ptr(),
                data.as_ptr() as *const _,
                data.len() as _,
            )
        )
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
        let full_key = if let Some(base) = &self.base_path {
            format!("{base}.{key}")
        } else {
            key.to_string()
        };
        let c_key = CString::new(full_key)?;
        flux_try!(empty_ok
            flux_kvs_txn_mkdir(self.c_txn.as_mut_ptr(), flags.bits() as _, c_key.as_ptr())
        )
    }

    pub fn unlink(&mut self, key: &str, flags: KvsFlags) -> Result<()> {
        let full_key = if let Some(base) = &self.base_path {
            format!("{base}.{key}")
        } else {
            key.to_string()
        };
        let c_key = CString::new(full_key)?;
        flux_try!(empty_ok flux_kvs_txn_unlink(
            self.c_txn.as_mut_ptr(),
            flags.bits() as _,
            c_key.as_ptr()
        ))
    }

    pub fn symlink(
        &mut self,
        key: &str,
        target: &str,
        namespace: Option<&str>,
        flags: KvsFlags,
    ) -> Result<()> {
        let full_key = if let Some(base) = &self.base_path {
            format!("{base}.{key}")
        } else {
            key.to_string()
        };
        // Convert 'key' to a C String
        let c_key = CString::new(full_key)?;
        // Optionally convert 'namespace' to a C String.
        // If namespace is None, c_namespace will be None.
        // If namespace is Some(val), val will be converted to a C String.
        // If an error occurs during that conversion, it will be returned through this method's
        // Result<()> via '.transpose()?'.
        let c_namespace: Option<CString> = namespace
            .map(|ns| CString::new(ns).map_err(FluxError::NulError))
            .transpose()?;
        // Convert 'target' to a C String
        let c_target = CString::new(target)?;
        // Call flux_kvs_txn_symlink.
        // Note that c_namespace is converted to either the C String pointer or a NULL pointer depending
        // on whether the Option is "Some" or "None".
        flux_try!(empty_ok
            flux_kvs_txn_symlink(
                self.c_txn.as_mut_ptr(),
                flags.bits() as _,
                c_key.as_ptr(),
                c_namespace
                    .as_ref()
                    .map_or(std::ptr::null(), |cstr_ns| cstr_ns.as_ptr()),
                c_target.as_ptr(),
            )
        )
    }

    // TODO add wrapper for put_treeobj, if possible
}

unsafe impl<'a> BorrowFluxPtr for BorrowedKvsTransaction<'a> {
    type CType = flux_kvs_txn_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_txn: FluxPtr::create_borrowed(ptr)?,
            base_path: None,
        })
    }
}

unsafe impl FromFluxPtr for OwnedKvsTransaction {
    type CType = flux_kvs_txn_t;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_txn: FluxPtr::create_owned(ptr, flux_kvs_txn_destroy)?,
            base_path: None,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<flux_kvs_txn_t>> AsFluxPtr for KvsTransaction<State> {
    define_as_flux_ptr_body!(flux_kvs_txn_t, c_txn);
}

unsafe impl IntoFluxPtr for OwnedKvsTransaction {
    define_into_flux_ptr_body!(c_txn);
}
