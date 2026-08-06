use std::ffi::{CStr, CString, c_void};
use std::fmt::Display;
use std::ops::Range;

use bitflags::bitflags;
use flux_sys::idset::idset_flags::{
    IDSET_FLAG_AUTOGROW, IDSET_FLAG_BRACKETS, IDSET_FLAG_COUNT_LAZY, IDSET_FLAG_INITFULL,
    IDSET_FLAG_RANGE,
};
use flux_sys::idset::{
    idset, idset_copy, idset_count, idset_create, idset_decode, idset_destroy, idset_difference,
    idset_empty, idset_encode, idset_equal, idset_first, idset_has_intersection, idset_intersect,
    idset_last, idset_next, idset_range_set, idset_set, idset_test, idset_union,
};
use serde::de::{self, Visitor};
use serde::{Deserialize, Serialize};

use crate::error::{FluxError, Result, check_ptr, check_rc};
use crate::flux_ptr_management::{BorrowFluxPtr, FluxPtr, FromFluxPtr, default_impl_as_flux_ptr};
use crate::utils::impl_serde_repr_str;

pub(crate) const IDSET_INVALID_ID: u32 = u32::MAX - 1;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct IdsetFlags: u32 {
        const NONE = 0;
        const AUTOGROW = IDSET_FLAG_AUTOGROW;
        const BRACKETS = IDSET_FLAG_BRACKETS;
        const RANGE = IDSET_FLAG_RANGE;
        const INITFULL = IDSET_FLAG_INITFULL;
        const COUNT_LAZY = IDSET_FLAG_COUNT_LAZY;
    }
}

/// A struct representing a Flux Idset.
///
/// The API for this struct is inspired by Rust's `BTreeSet`.
pub struct Idset {
    c_idset: FluxPtr<idset>,
}

impl Idset {
    /// Create a new Idset with the specified size and flags.
    pub fn new(size: usize, flags: IdsetFlags) -> Result<Self> {
        let idset_ptr = unsafe { idset_create(size, flags.bits() as _) };
        check_ptr(idset_ptr)?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(idset_ptr, idset_destroy)?,
        })
    }

    /// Copy the Idset.
    ///
    /// If an error occurs in the underlying `idset_copy` function, this method returns
    /// an error.
    ///
    /// This method is the same as calling Idset::try_from.
    pub fn try_clone(&self) -> Result<Self> {
        Self::try_from(self)
    }

    /// Create a string encoding of the Idset using the specified flags.
    pub fn encode(&self, flags: IdsetFlags) -> Result<String> {
        let encoded_ptr = unsafe { idset_encode(self.c_idset.as_mut_ptr(), flags.bits() as _) };
        check_ptr(encoded_ptr)?;
        let owned_str = unsafe { CStr::from_ptr(encoded_ptr).to_str()?.to_owned() };
        unsafe { libc::free(encoded_ptr as *mut c_void) };
        Ok(owned_str)
    }

    /// Check if an id is in the Idset.
    pub fn contains(&self, value: u32) -> bool {
        unsafe { idset_test(self.c_idset.as_mut_ptr(), value) }
    }

    /// Get the size of the Idset
    pub fn len(&self) -> usize {
        unsafe { idset_count(self.c_idset.as_mut_ptr()) }
    }

    /// Check if the Idset is empty.
    pub fn is_empty(&self) -> bool {
        unsafe { idset_empty(self.c_idset.as_mut_ptr()) }
    }

    /// Add an id into the Idset.
    pub fn insert(&self, value: u32) -> Result<()> {
        let rc = unsafe { idset_set(self.c_idset.as_mut_ptr(), value) };
        check_rc(rc)
    }

    /// Insert a range of ids into the Idset.
    pub fn insert_range(&self, range: Range<u32>) -> Result<()> {
        let range_start = range.start;
        // We subtract 1 here because Rust Range represents the open interval
        // [start, end)
        let range_end = range.end - 1;
        if range_end == range_start {
            return self.insert(range_start);
        }
        let rc = unsafe { idset_range_set(self.c_idset.as_mut_ptr(), range_start, range_end) };
        check_rc(rc)
    }

    /// Get the first id in the Idset.
    pub fn first(&self) -> Option<u32> {
        let first_id = unsafe { idset_first(self.c_idset.as_mut_ptr()) };
        if first_id == IDSET_INVALID_ID {
            return None;
        }
        Some(first_id)
    }

    /// Get the last id in the Idset.
    pub fn last(&self) -> Option<u32> {
        let last_id = unsafe { idset_last(self.c_idset.as_mut_ptr()) };
        if last_id == IDSET_INVALID_ID {
            return None;
        }
        Some(last_id)
    }

    pub fn union(&self, other: &Self) -> Result<Self> {
        let union_ptr =
            unsafe { idset_union(self.c_idset.as_mut_ptr(), other.c_idset.as_mut_ptr()) };
        check_ptr(union_ptr)?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(union_ptr, idset_destroy)?,
        })
    }

    pub fn intersection(&self, other: &Self) -> Result<Self> {
        let intersection_ptr =
            unsafe { idset_intersect(self.c_idset.as_mut_ptr(), other.c_idset.as_mut_ptr()) };
        check_ptr(intersection_ptr)?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(intersection_ptr, idset_destroy)?,
        })
    }

    pub fn difference(&self, other: &Self) -> Result<Self> {
        let difference_ptr =
            unsafe { idset_difference(self.c_idset.as_mut_ptr(), other.c_idset.as_mut_ptr()) };
        check_ptr(difference_ptr)?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(difference_ptr, idset_destroy)?,
        })
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        unsafe { !idset_has_intersection(self.c_idset.as_mut_ptr(), other.c_idset.as_mut_ptr()) }
    }

    /// Create an iterator over the ids in the Idset.
    ///
    /// This method is used to create an iterator for a reference to Idset.
    /// If you want to create an Iterator that assumes ownership, use into_iter instead.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            idset: self,
            current: self.first().unwrap_or(IDSET_INVALID_ID),
        }
    }
}

impl PartialEq for Idset {
    fn eq(&self, other: &Self) -> bool {
        unsafe { idset_equal(self.c_idset.as_mut_ptr(), other.c_idset.as_mut_ptr()) }
    }
}

impl std::str::FromStr for Idset {
    type Err = FluxError;

    /// Decode a string representation into an Idset object.
    fn from_str(s: &str) -> Result<Self> {
        let c_str = CString::new(s)?;
        let ptr = unsafe { idset_decode(c_str.as_ptr()) };
        check_ptr(ptr)?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(ptr, idset_destroy)?,
        })
    }
}

impl Display for Idset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let encoded_idset = match self.encode(IdsetFlags::RANGE | IdsetFlags::BRACKETS) {
            Ok(s) => s,
            Err(e) => format!("UNKNOWN (Failed to convert C string to Rust string: {e})"),
        };
        write!(f, "{}", encoded_idset)
    }
}

impl TryFrom<&Idset> for Idset {
    type Error = FluxError;

    /// Copy an Idset.
    ///
    /// This function is used to create a copy of an Idset because the underlying
    /// `idset_copy` function may fail.
    fn try_from(value: &Idset) -> Result<Self> {
        let new_ptr = unsafe { idset_copy(value.c_idset.as_mut_ptr()) };
        check_ptr(new_ptr)?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(new_ptr, idset_destroy)?,
        })
    }
}

unsafe impl BorrowFluxPtr for Idset {
    type CType = idset;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_idset: FluxPtr::create_borrowed(ptr, idset_destroy)?,
        })
    }
}

unsafe impl FromFluxPtr for Idset {
    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_idset: FluxPtr::create_owned(ptr, idset_destroy)?,
        })
    }
}

default_impl_as_flux_ptr!(Idset, idset, c_idset);

impl Serialize for Idset {
    fn serialize<S>(&self, serializer: S) -> std::prelude::v1::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Idset {
    fn deserialize<D>(deserializer: D) -> std::prelude::v1::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct IdsetVisitor;

        impl<'de> Visitor<'de> for IdsetVisitor {
            type Value = Idset;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a Flux idset string (e.g., '1-3,5-6,42')")
            }

            fn visit_str<E>(self, v: &str) -> std::prelude::v1::Result<Self::Value, E>
            where
                E: de::Error,
            {
                v.parse::<Idset>().map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(IdsetVisitor)
    }
}

impl_serde_repr_str!(no_display Idset);

pub struct Iter<'a> {
    idset: &'a Idset,
    current: u32,
}

impl<'a> Iterator for Iter<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == IDSET_INVALID_ID {
            return None;
        }
        let result = self.current;
        self.current = unsafe { idset_next(self.idset.c_idset.as_mut_ptr(), result) };
        Some(result)
    }
}

impl<'a> IntoIterator for &'a Idset {
    type Item = u32;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct IntoIter {
    idset: Idset,
    current: u32,
}

impl Iterator for IntoIter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == IDSET_INVALID_ID {
            return None;
        }
        let result = self.current;
        self.current = unsafe { idset_next(self.idset.c_idset.as_mut_ptr(), result) };
        Some(result)
    }
}

impl IntoIterator for Idset {
    type Item = u32;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        let current = self.first().unwrap_or(IDSET_INVALID_ID);
        IntoIter {
            idset: self,
            current,
        }
    }
}
