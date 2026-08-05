use std::ffi::{c_void, CStr, CString};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use flux_sys::core::{
    flux_kvsdir_copy, flux_kvsdir_destroy, flux_kvsdir_exists, flux_kvsdir_get_size,
    flux_kvsdir_incref, flux_kvsdir_isdir, flux_kvsdir_issymlink, flux_kvsdir_key_at,
    flux_kvsdir_t, flux_kvsitr_create, flux_kvsitr_destroy, flux_kvsitr_next, flux_kvsitr_rewind,
    flux_kvsitr_t,
};

use crate::error::{check_ptr, check_rc, FluxError, Result};
use crate::flux_ptr_management::{default_impl_as_flux_ptr, BorrowFluxPtr, FluxPtr, FromFluxPtr};
use crate::handle::FluxHandle;
use crate::kvs::{Kvs, KvsFlags, KvsTransaction};

pub struct KvsDir {
    pub(crate) c_kvsdir: FluxPtr<flux_kvsdir_t>,
    pub(crate) txn: Rc<KvsTransaction>,
    pub(crate) path: String,
}

impl KvsDir {
    pub fn new(handle: &FluxHandle, path: Option<&str>, namespace: Option<&str>) -> Result<Self> {
        let mut kvs_handle = Kvs::new(handle);
        let dir_path = path.unwrap_or(".");
        let mut lookup = kvs_handle.lookup(dir_path, KvsFlags::READDIR, namespace)?;
        lookup.get_dir()
    }

    pub fn try_clone(&self) -> Result<Self> {
        Self::try_from(self)
    }

    pub fn len(&self) -> Result<usize> {
        let len_val = unsafe { flux_kvsdir_get_size(self.c_kvsdir.as_mut_ptr()) };
        check_rc(len_val)?;
        Ok(len_val as usize)
    }

    pub fn contains(&self, key: &str) -> Result<bool> {
        let c_key = CString::new(key)?;
        Ok(unsafe { flux_kvsdir_exists(self.c_kvsdir.as_mut_ptr(), c_key.as_ptr()) })
    }

    pub fn is_dir(&self, key: &str) -> Result<bool> {
        let c_key = CString::new(key)?;
        Ok(unsafe { flux_kvsdir_isdir(self.c_kvsdir.as_mut_ptr(), c_key.as_ptr()) })
    }

    pub fn is_symlink(&self, key: &str) -> Result<bool> {
        let c_key = CString::new(key)?;
        Ok(unsafe { flux_kvsdir_issymlink(self.c_kvsdir.as_mut_ptr(), c_key.as_ptr()) })
    }

    pub fn get_key_at(&self, subkey: &str) -> Result<String> {
        let c_key = CString::new(subkey)?;
        let qualified_ptr =
            unsafe { flux_kvsdir_key_at(self.c_kvsdir.as_mut_ptr(), c_key.as_ptr()) };
        check_ptr(qualified_ptr)?;
        let owned_str = unsafe { CStr::from_ptr(qualified_ptr).to_str()?.to_string() };
        unsafe {
            libc::free(qualified_ptr as *mut c_void);
        }
        Ok(owned_str)
    }

    pub fn cursor(&self) -> Result<KvsDirCursor<'_>> {
        let iter_ptr = unsafe { flux_kvsitr_create(self.c_kvsdir.as_mut_ptr()) };
        check_ptr(iter_ptr)?;
        Ok(KvsDirCursor {
            _dir: self,
            iter: FluxPtr::create_owned(iter_ptr, flux_kvsitr_destroy)?,
            current: String::new(),
        })
    }

    pub fn iter(&self) -> Result<KvsDirIter<'_>> {
        let cursor = self.cursor()?;
        Ok(KvsDirIter { cursor })
    }
}

impl Deref for KvsDir {
    type Target = Rc<KvsTransaction>;

    fn deref(&self) -> &Self::Target {
        &self.txn
    }
}

impl DerefMut for KvsDir {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.txn
    }
}

impl Clone for KvsDir {
    fn clone(&self) -> Self {
        unsafe {
            flux_kvsdir_incref(self.c_kvsdir.as_mut_ptr());
        }
        Self {
            c_kvsdir: FluxPtr::create_owned(self.c_kvsdir.as_mut_ptr(), flux_kvsdir_destroy).expect("Tried to clone a KvsDir object where the underlying pointer has an invalid NULL state"),
            txn: self.txn.clone(),
            path: self.path.clone(),
        }
    }
}

impl TryFrom<&KvsDir> for KvsDir {
    type Error = FluxError;

    fn try_from(value: &KvsDir) -> Result<Self> {
        let cloned_handle = unsafe { flux_kvsdir_copy(value.c_kvsdir.as_mut_ptr()) };
        check_ptr(cloned_handle)?;
        Ok(Self {
            c_kvsdir: FluxPtr::create_owned(cloned_handle, flux_kvsdir_destroy)?,
            txn: value.txn.clone(),
            path: value.path.clone(),
        })
    }
}

unsafe impl BorrowFluxPtr for KvsDir {
    type CType = flux_kvsdir_t;
    type FromRawArgs = Option<String>;

    unsafe fn borrow_raw(ptr: *mut Self::CType, args: Self::FromRawArgs) -> Result<Self> {
        let dir_path = args.unwrap_or(".".to_string());
        let txn = Rc::new(if dir_path == "." {
            KvsTransaction::new()?
        } else {
            KvsTransaction::from_path(&dir_path)?
        });
        Ok(Self {
            c_kvsdir: FluxPtr::create_borrowed(ptr, flux_kvsdir_destroy)?,
            txn: txn,
            path: dir_path,
        })
    }
}

unsafe impl FromFluxPtr for KvsDir {
    unsafe fn from_raw(ptr: *mut Self::CType, args: Self::FromRawArgs) -> Result<Self> {
        let dir_path = args.unwrap_or(".".to_string());
        let txn = Rc::new(if dir_path == "." {
            KvsTransaction::new()?
        } else {
            KvsTransaction::from_path(&dir_path)?
        });
        Ok(Self {
            c_kvsdir: FluxPtr::create_owned(ptr, flux_kvsdir_destroy)?,
            txn: txn,
            path: dir_path,
        })
    }
}

default_impl_as_flux_ptr!(KvsDir, flux_kvsdir_t, c_kvsdir);

pub struct KvsDirCursor<'a> {
    _dir: &'a KvsDir,
    iter: FluxPtr<flux_kvsitr_t>,
    current: String,
}

impl<'a> KvsDirCursor<'a> {
    pub fn next(&mut self) -> Result<Option<&str>> {
        let c_str = unsafe { flux_kvsitr_next(self.iter.as_mut_ptr()) };
        if c_str.is_null() {
            return Ok(None);
        }
        self.current = unsafe { CStr::from_ptr(c_str).to_str()?.to_string() };
        Ok(Some(self.current.as_str()))
    }

    pub fn current(&self) -> &str {
        self.current.as_str()
    }

    pub fn reset(&mut self) {
        unsafe {
            flux_kvsitr_rewind(self.iter.as_mut_ptr());
        }
    }
}

default_impl_as_flux_ptr!(KvsDirCursor<'a>, flux_kvsitr_t, iter);

pub struct KvsDirIter<'a> {
    cursor: KvsDirCursor<'a>,
}

impl<'a> Iterator for KvsDirIter<'a> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.cursor.next() {
            Ok(Some(s)) => Some(Ok(s.to_string())),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}
