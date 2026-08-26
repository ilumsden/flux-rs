use std::ffi::{CStr, CString, c_void};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use flux_sys::core::{
    flux_kvsdir_copy, flux_kvsdir_destroy, flux_kvsdir_exists, flux_kvsdir_get_size,
    flux_kvsdir_incref, flux_kvsdir_isdir, flux_kvsdir_issymlink, flux_kvsdir_key_at,
    flux_kvsdir_t, flux_kvsitr_create, flux_kvsitr_destroy, flux_kvsitr_next, flux_kvsitr_rewind,
    flux_kvsitr_t, flux_t,
};

use crate::error::{FluxError, Result, flux_try};
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, Borrowed, FluxPtr, FromFluxPtr, IntoFluxPtr, Owned,
    PossiblyDroppablePtr, define_as_flux_ptr_body, define_into_flux_ptr_body,
};
use crate::handle::FluxHandle;
use crate::kvs::txn::OwnedKvsTransaction;
use crate::kvs::{Kvs, KvsFlags, KvsTransaction};

pub struct KvsDir<State: PossiblyDroppablePtr<flux_kvsdir_t> = Owned<flux_kvsdir_t>> {
    pub(crate) c_kvsdir: FluxPtr<flux_kvsdir_t, State>,
    pub(crate) txn: Rc<OwnedKvsTransaction>,
    pub(crate) path: String,
}

pub type OwnedKvsDir = KvsDir<Owned<flux_kvsdir_t>>;
pub type BorrowedKvsDir<'a> = KvsDir<Borrowed<'a, flux_kvsdir_t>>;

impl OwnedKvsDir {
    pub fn new<FhState: PossiblyDroppablePtr<flux_t>>(
        handle: &FluxHandle<FhState>,
        path: Option<&str>,
        namespace: Option<&str>,
    ) -> Result<Self> {
        let mut kvs_handle = Kvs::new(handle);
        let dir_path = path.unwrap_or(".");
        let lookup = kvs_handle.lookup(dir_path, KvsFlags::READDIR, namespace)?;
        lookup.get_dir()
    }
}

impl<State: PossiblyDroppablePtr<flux_kvsdir_t>> KvsDir<State> {
    pub fn try_clone(&self) -> Result<OwnedKvsDir> {
        KvsDir::try_from(self)
    }

    pub fn to_owned(&self) -> Result<OwnedKvsDir> {
        unsafe {
            flux_kvsdir_incref(self.c_kvsdir.as_mut_ptr());
        }
        Ok(KvsDir {
            c_kvsdir: FluxPtr::create_owned(self.c_kvsdir.as_mut_ptr(), flux_kvsdir_destroy)?,
            txn: self.txn.clone(),
            path: self.path.clone(),
        })
    }

    pub fn len(&self) -> Result<usize> {
        let len_val = flux_try!(flux_kvsdir_get_size(self.c_kvsdir.as_mut_ptr()))?;
        Ok(len_val as usize)
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
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
        let qualified_ptr = flux_try!(flux_kvsdir_key_at(
            self.c_kvsdir.as_mut_ptr(),
            c_key.as_ptr()
        ))?;
        let owned_str = unsafe { CStr::from_ptr(qualified_ptr).to_str()?.to_string() };
        unsafe {
            libc::free(qualified_ptr as *mut c_void);
        }
        Ok(owned_str)
    }

    pub fn cursor<'b>(&'b self) -> Result<KvsDirCursor<'b, State>> {
        let iter_ptr = flux_try!(flux_kvsitr_create(self.c_kvsdir.as_mut_ptr()))?;
        Ok(KvsDirCursor {
            _dir: self,
            iter: FluxPtr::create_owned(iter_ptr, flux_kvsitr_destroy)?,
            current: String::new(),
        })
    }

    pub fn iter<'b>(&'b self) -> Result<KvsDirIter<'b, State>> {
        let cursor = self.cursor()?;
        Ok(KvsDirIter { cursor })
    }
}

impl<State: PossiblyDroppablePtr<flux_kvsdir_t>> Deref for KvsDir<State> {
    type Target = Rc<OwnedKvsTransaction>;

    fn deref(&self) -> &Self::Target {
        &self.txn
    }
}

impl<State: PossiblyDroppablePtr<flux_kvsdir_t>> DerefMut for KvsDir<State> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.txn
    }
}

impl Clone for OwnedKvsDir {
    fn clone(&self) -> Self {
        self.to_owned().expect(
            "Tried to clone a KvsDir object where the underlying pointer has an invalid NULL state",
        )
    }
}

impl<State: PossiblyDroppablePtr<flux_kvsdir_t>> TryFrom<&KvsDir<State>> for OwnedKvsDir {
    type Error = FluxError;

    fn try_from(value: &KvsDir<State>) -> Result<Self> {
        let cloned_handle = flux_try!(flux_kvsdir_copy(value.c_kvsdir.as_mut_ptr()))?;
        Ok(Self {
            c_kvsdir: FluxPtr::create_owned(cloned_handle, flux_kvsdir_destroy)?,
            txn: value.txn.clone(),
            path: value.path.clone(),
        })
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedKvsDir<'a> {
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
            c_kvsdir: FluxPtr::create_borrowed(ptr)?,
            txn,
            path: dir_path,
        })
    }
}

unsafe impl FromFluxPtr for OwnedKvsDir {
    type CType = flux_kvsdir_t;
    type FromRawArgs = Option<String>;

    unsafe fn from_raw(ptr: *mut Self::CType, args: Self::FromRawArgs) -> Result<Self> {
        let dir_path = args.unwrap_or(".".to_string());
        let txn = Rc::new(if dir_path == "." {
            KvsTransaction::new()?
        } else {
            KvsTransaction::from_path(&dir_path)?
        });
        Ok(Self {
            c_kvsdir: FluxPtr::create_owned(ptr, flux_kvsdir_destroy)?,
            txn,
            path: dir_path,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<flux_kvsdir_t>> AsFluxPtr for KvsDir<State> {
    define_as_flux_ptr_body!(flux_kvsdir_t, c_kvsdir);
}

unsafe impl IntoFluxPtr for OwnedKvsDir {
    define_into_flux_ptr_body!(c_kvsdir);
}

pub struct KvsDirCursor<'a, State: PossiblyDroppablePtr<flux_kvsdir_t>> {
    _dir: &'a KvsDir<State>,
    iter: FluxPtr<flux_kvsitr_t, Owned<flux_kvsitr_t>>,
    current: String,
}

impl<'a, State: PossiblyDroppablePtr<flux_kvsdir_t>> KvsDirCursor<'a, State> {
    pub fn move_next(&mut self) -> Result<Option<&str>> {
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

unsafe impl<'a, State: PossiblyDroppablePtr<flux_kvsdir_t>> AsFluxPtr for KvsDirCursor<'a, State> {
    define_as_flux_ptr_body!(flux_kvsitr_t, iter);
}

unsafe impl<'a, State: PossiblyDroppablePtr<flux_kvsdir_t>> IntoFluxPtr
    for KvsDirCursor<'a, State>
{
    define_into_flux_ptr_body!(iter);
}

pub struct KvsDirIter<'a, State: PossiblyDroppablePtr<flux_kvsdir_t>> {
    cursor: KvsDirCursor<'a, State>,
}

impl<'a, State: PossiblyDroppablePtr<flux_kvsdir_t>> Iterator for KvsDirIter<'a, State> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.cursor.move_next() {
            Ok(Some(s)) => Some(Ok(s.to_string())),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}
