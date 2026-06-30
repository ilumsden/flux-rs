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
use crate::handle::FluxHandle;
use crate::kvs::{Kvs, KvsFlags, KvsTransaction};

pub struct KvsDir {
    c_kvsdir: *mut flux_kvsdir_t,
    txn: Rc<KvsTransaction>,
    path: String,
}

impl KvsDir {
    pub fn new(handle: &FluxHandle, path: Option<&str>, namespace: Option<&str>) -> Result<Self> {
        let mut kvs_handle = Kvs::new(handle);
        let dir_path = path.unwrap_or(".");
        let mut lookup = kvs_handle.lookup(dir_path, KvsFlags::READDIR, namespace)?;
        lookup.get_dir()
    }

    pub fn from_ptr(ptr: *mut flux_kvsdir_t, path: Option<&str>) -> Result<Self> {
        let dir_path = path.unwrap_or(".").to_string();
        let txn = Rc::new(if dir_path == "." {
            KvsTransaction::new()?
        } else {
            KvsTransaction::from_path(&dir_path)?
        });
        Ok(Self {
            c_kvsdir: ptr,
            txn: txn,
            path: dir_path,
        })
    }

    pub fn try_clone(&self) -> Result<Self> {
        Self::try_from(self)
    }

    pub fn len(&self) -> Result<usize> {
        if self.c_kvsdir.is_null() {
            return Err(FluxError::Logic(
                "Cannot get size of KVS directory when the underlying pointer is NULL".to_string(),
            ));
        }
        let len_val = unsafe { flux_kvsdir_get_size(self.c_kvsdir) };
        check_rc(len_val)?;
        Ok(len_val as usize)
    }

    pub fn contains(&self, key: &str) -> Result<bool> {
        if self.c_kvsdir.is_null() {
            return Err(FluxError::Logic(
                "Cannot check if key exists in KVS directory when the underlying pointer is NULL"
                    .to_string(),
            ));
        }
        let c_key = CString::new(key)?;
        Ok(unsafe { flux_kvsdir_exists(self.c_kvsdir, c_key.as_ptr()) })
    }

    pub fn is_dir(&self, key: &str) -> Result<bool> {
        if self.c_kvsdir.is_null() {
            return Err(FluxError::Logic(
                "Cannot check if key under KVS directory is itself a directory when the underlying pointer is NULL"
                    .to_string(),
            ));
        }
        let c_key = CString::new(key)?;
        Ok(unsafe { flux_kvsdir_isdir(self.c_kvsdir, c_key.as_ptr()) })
    }

    pub fn is_symlink(&self, key: &str) -> Result<bool> {
        if self.c_kvsdir.is_null() {
            return Err(FluxError::Logic(
                "Cannot check if key under KVS directory is a symlink when the underlying pointer is NULL"
                    .to_string(),
            ));
        }
        let c_key = CString::new(key)?;
        Ok(unsafe { flux_kvsdir_issymlink(self.c_kvsdir, c_key.as_ptr()) })
    }

    pub fn get_key_at(&self, subkey: &str) -> Result<String> {
        if self.c_kvsdir.is_null() {
            return Err(FluxError::Logic(
                "Cannot get the fully-qualified key when the underlying KVSDir pointer is NULL"
                    .to_string(),
            ));
        }
        let c_key = CString::new(subkey)?;
        let qualified_ptr = unsafe { flux_kvsdir_key_at(self.c_kvsdir, c_key.as_ptr()) };
        check_ptr(qualified_ptr)?;
        let owned_str = unsafe { CStr::from_ptr(qualified_ptr).to_str()?.to_string() };
        unsafe {
            libc::free(qualified_ptr as *mut c_void);
        }
        Ok(owned_str)
    }

    pub fn cursor(&self) -> Result<KvsDirCursor<'_>> {
        if self.c_kvsdir.is_null() {
            return Err(FluxError::Logic(
                "Cannot create KVS directory cursor from NULL pointer".to_string(),
            ));
        }
        let iter_ptr = unsafe { flux_kvsitr_create(self.c_kvsdir) };
        check_ptr(iter_ptr)?;
        Ok(KvsDirCursor {
            _dir: self,
            iter: iter_ptr,
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
        if !self.c_kvsdir.is_null() {
            unsafe {
                flux_kvsdir_incref(self.c_kvsdir);
            }
        }
        Self {
            c_kvsdir: self.c_kvsdir,
            txn: self.txn.clone(),
            path: self.path.clone(),
        }
    }
}

impl Drop for KvsDir {
    fn drop(&mut self) {
        if !self.c_kvsdir.is_null() {
            unsafe {
                flux_kvsdir_destroy(self.c_kvsdir);
            }
        }
    }
}

impl TryFrom<&KvsDir> for KvsDir {
    type Error = FluxError;

    fn try_from(value: &KvsDir) -> Result<Self> {
        if value.c_kvsdir.is_null() {
            return Err(FluxError::Logic(
                "Cannot deep copy a KvsDir object when the underlying pointer is NULL".to_string(),
            ));
        }
        let cloned_handle = unsafe { flux_kvsdir_copy(value.c_kvsdir) };
        check_ptr(cloned_handle)?;
        Ok(Self {
            c_kvsdir: cloned_handle,
            txn: value.txn.clone(),
            path: value.path.clone(),
        })
    }
}

pub struct KvsDirCursor<'a> {
    _dir: &'a KvsDir,
    iter: *mut flux_kvsitr_t,
    current: String,
}

impl<'a> KvsDirCursor<'a> {
    pub fn next(&mut self) -> Result<&str> {
        if self.iter.is_null() {
            return Err(FluxError::Logic(
                "Cannot iterate over a KVS directory using a NULL pointer".to_string(),
            ));
        }
        let c_str = unsafe { flux_kvsitr_next(self.iter) };
        self.current = unsafe { CStr::from_ptr(c_str).to_str()?.to_string() };
        Ok(self.current.as_str())
    }

    pub fn current(&self) -> &str {
        self.current.as_str()
    }

    pub fn remove_current(&mut self) {
        if !self.iter.is_null() {
            unsafe {
                flux_kvsitr_destroy(self.iter);
            }
        }
    }

    pub fn reset(&mut self) {
        if !self.iter.is_null() {
            unsafe {
                flux_kvsitr_rewind(self.iter);
            }
        }
    }
}

impl<'a> Drop for KvsDirCursor<'a> {
    fn drop(&mut self) {
        if !self.iter.is_null() {
            unsafe {
                flux_kvsitr_destroy(self.iter);
            }
        }
    }
}

pub struct KvsDirIter<'a> {
    cursor: KvsDirCursor<'a>,
}

impl<'a> Iterator for KvsDirIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.cursor.next().ok().map(|s| s.to_string())
    }
}
