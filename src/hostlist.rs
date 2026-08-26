use std::ffi::{CStr, CString, c_void};
use std::fmt::Display;
use std::str::FromStr;

use flux_sys::hostlist::{
    hostlist, hostlist_append, hostlist_append_list, hostlist_copy, hostlist_count,
    hostlist_create, hostlist_current, hostlist_decode, hostlist_destroy, hostlist_encode,
    hostlist_find, hostlist_first, hostlist_next, hostlist_nth, hostlist_remove_current,
    hostlist_sort, hostlist_uniq,
};
use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{self, Serialize, Serializer};

use crate::error::{FluxError, Result, flux_try};
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, Borrowed, FluxPtr, FromFluxPtr, IntoFluxPtr, Owned,
    PossiblyDroppablePtr, define_as_flux_ptr_body, define_into_flux_ptr_body,
};

/// A struct representing a Flux Idset.
///
/// The API for this struct is inspired by Rust's `Vec` and `LinkedList` collections.
pub struct Hostlist<State: PossiblyDroppablePtr<hostlist> = Owned<hostlist>> {
    c_hostlist: FluxPtr<hostlist, State>,
}

pub type OwnedHostlist = Hostlist<Owned<hostlist>>;
pub type BorrowedHostlist<'a> = Hostlist<Borrowed<'a, hostlist>>;

impl OwnedHostlist {
    pub fn new() -> Result<Self> {
        let ptr = flux_try!(hostlist_create())?;
        Ok(Self {
            c_hostlist: FluxPtr::create_owned(ptr, hostlist_destroy)?,
        })
    }

    pub fn try_from_iter<I, S>(iter: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut hostlist = Self::new()?;
        for host in iter {
            hostlist.push(host.as_ref())?;
        }
        Ok(hostlist)
    }
}

impl<State: PossiblyDroppablePtr<hostlist>> Hostlist<State> {
    pub fn try_clone(&self) -> Result<OwnedHostlist> {
        Hostlist::try_from(self)
    }

    pub fn encode(&self) -> Result<String> {
        let ptr = flux_try!(hostlist_encode(self.c_hostlist.as_mut_ptr()))?;
        let encoded_hostlist = unsafe { CStr::from_ptr(ptr).to_str().map(|s| s.to_owned()) };
        unsafe { ::libc::free(ptr as *mut c_void) };
        Ok(encoded_hostlist?)
    }

    pub fn push(&mut self, new_host: &str) -> Result<()> {
        let c_new_host = CString::new(new_host)?;
        flux_try!(empty_ok hostlist_append(self.c_hostlist.as_mut_ptr(), c_new_host.as_ptr()))
    }

    pub fn append<OtherState>(&mut self, other: &Hostlist<OtherState>) -> Result<usize>
    where
        OtherState: PossiblyDroppablePtr<hostlist>,
    {
        flux_try!(hostlist_append_list(
            self.c_hostlist.as_mut_ptr(),
            other.c_hostlist.as_mut_ptr()
        ))
        .map(|i| i as usize)
    }

    pub fn len(&self) -> usize {
        unsafe { hostlist_count(self.c_hostlist.as_mut_ptr()) as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dedup(&mut self) {
        unsafe {
            hostlist_uniq(self.c_hostlist.as_mut_ptr());
        }
    }

    pub fn sort(&mut self) {
        unsafe {
            hostlist_sort(self.c_hostlist.as_mut_ptr());
        }
    }

    pub fn find(&mut self, hostname: &str) -> Result<Option<usize>> {
        let c_hostname = CString::new(hostname)?;
        let pos = unsafe { hostlist_find(self.c_hostlist.as_mut_ptr(), c_hostname.as_ptr()) };
        if pos == -1 {
            Ok(None)
        } else {
            Ok(Some(pos as usize))
        }
    }

    pub fn nth(&mut self, n: usize) -> Option<String> {
        let ptr = unsafe { hostlist_nth(self.c_hostlist.as_mut_ptr(), n as _) };
        if ptr.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() })
    }

    pub fn remove(&mut self, n: usize) -> bool {
        if self.nth(n).is_none() {
            return false;
        }
        let rc = unsafe { hostlist_remove_current(self.c_hostlist.as_mut_ptr()) };
        rc == 1
    }

    pub fn remove_host(&mut self, hostname: &str) -> Result<bool> {
        if self.find(hostname)?.is_none() {
            return Ok(false);
        }
        let rc = unsafe { hostlist_remove_current(self.c_hostlist.as_mut_ptr()) };
        Ok(rc == 1)
    }

    pub fn cursor_mut(&mut self) -> HostlistCursor<'_, State> {
        HostlistCursor {
            hostlist: self,
            is_first: true,
        }
    }
}

impl std::str::FromStr for OwnedHostlist {
    type Err = FluxError;

    fn from_str(s: &str) -> Result<Self> {
        let c_str = CString::new(s)?;
        let ptr = flux_try!(hostlist_decode(c_str.as_ptr()))?;
        Ok(Self {
            c_hostlist: FluxPtr::create_owned(ptr, hostlist_destroy)?,
        })
    }
}

impl<State: PossiblyDroppablePtr<hostlist>> Display for Hostlist<State> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let encoded_hostlist = match self.encode() {
            Ok(s) => s,
            Err(e) => format!("UNKNOWN (Failed to convert C string to Rust string: {e})"),
        };
        write!(f, "{}", encoded_hostlist)
    }
}

impl<State: PossiblyDroppablePtr<hostlist>> TryFrom<&Hostlist<State>> for OwnedHostlist {
    type Error = FluxError;

    fn try_from(value: &Hostlist<State>) -> Result<Self> {
        let new_ptr = flux_try!(hostlist_copy(value.c_hostlist.as_mut_ptr()))?;
        Ok(Self {
            c_hostlist: FluxPtr::create_owned(new_ptr, hostlist_destroy)?,
        })
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedHostlist<'a> {
    type CType = hostlist;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_hostlist: FluxPtr::create_borrowed(ptr)?,
        })
    }
}

unsafe impl FromFluxPtr for OwnedHostlist {
    type CType = hostlist;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_hostlist: FluxPtr::create_owned(ptr, hostlist_destroy)?,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<hostlist>> AsFluxPtr for Hostlist<State> {
    define_as_flux_ptr_body!(hostlist, c_hostlist);
}

unsafe impl IntoFluxPtr for OwnedHostlist {
    define_into_flux_ptr_body!(c_hostlist);
}

impl<State: PossiblyDroppablePtr<hostlist>> Serialize for Hostlist<State> {
    fn serialize<S>(&self, serializer: S) -> std::prelude::v1::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded_data = self.encode().map_err(ser::Error::custom)?;
        serializer.serialize_str(&encoded_data)
    }
}

impl<'de> Deserialize<'de> for OwnedHostlist {
    fn deserialize<D>(deserializer: D) -> std::prelude::v1::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct HostlistVisitor;

        impl<'de> Visitor<'de> for HostlistVisitor {
            type Value = OwnedHostlist;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a Flux RFC 29 hostlist")
            }

            fn visit_str<E>(self, v: &str) -> std::prelude::v1::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Hostlist::from_str(v).map_err(|e| de::Error::custom(e))
            }
        }

        deserializer.deserialize_str(HostlistVisitor)
    }
}

pub struct HostlistCursor<'a, State: PossiblyDroppablePtr<hostlist>> {
    hostlist: &'a mut Hostlist<State>,
    is_first: bool,
}

impl<'a, State: PossiblyDroppablePtr<hostlist>> HostlistCursor<'a, State> {
    pub fn move_next(&mut self) -> Option<String> {
        let ptr = if self.is_first {
            self.is_first = false;
            unsafe { hostlist_first(self.hostlist.c_hostlist.as_mut_ptr()) }
        } else {
            unsafe { hostlist_next(self.hostlist.c_hostlist.as_mut_ptr()) }
        };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() })
        }
    }

    pub fn remove_current(&mut self) -> bool {
        let rc = unsafe { hostlist_remove_current(self.hostlist.c_hostlist.as_mut_ptr()) };
        rc == 1
    }

    pub fn current(&self) -> Option<String> {
        let ptr = unsafe { hostlist_current(self.hostlist.c_hostlist.as_mut_ptr()) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() })
        }
    }
}

pub struct IntoIter {
    hostlist: OwnedHostlist,
    is_first: bool,
}

impl Iterator for IntoIter {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = if self.is_first {
            self.is_first = false;
            unsafe { hostlist_first(self.hostlist.c_hostlist.as_mut_ptr()) }
        } else {
            unsafe { hostlist_next(self.hostlist.c_hostlist.as_mut_ptr()) }
        };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() })
        }
    }
}

impl IntoIterator for OwnedHostlist {
    type Item = String;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            hostlist: self,
            is_first: true,
        }
    }
}
