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

use crate::error::{FluxError, Result, flux_try};
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, Borrowed, FluxPtr, FromFluxPtr, IntoFluxPtr, Owned,
    PossiblyDroppablePtr, define_as_flux_ptr_body, define_into_flux_ptr_body,
};
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
pub struct Idset<State: PossiblyDroppablePtr<idset> = Owned<idset>> {
    c_idset: FluxPtr<idset, State>,
}

pub type OwnedIdset = Idset<Owned<idset>>;
pub type BorrowedIdset<'a> = Idset<Borrowed<'a, idset>>;

impl OwnedIdset {
    /// Create a new Idset with the specified size and flags.
    pub fn new(size: usize, flags: IdsetFlags) -> Result<Self> {
        let idset_ptr = flux_try!(idset_create(size, flags.bits() as _))?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(idset_ptr, idset_destroy)?,
        })
    }
}

impl<State: PossiblyDroppablePtr<idset>> Idset<State> {
    /// Copy the Idset.
    ///
    /// If an error occurs in the underlying `idset_copy` function, this method returns
    /// an error.
    ///
    /// This method is the same as calling Idset::try_from.
    pub fn try_clone(&self) -> Result<OwnedIdset> {
        Idset::try_from(self)
    }

    /// Create a string encoding of the Idset using the specified flags.
    pub fn encode(&self, flags: IdsetFlags) -> Result<String> {
        let encoded_ptr = flux_try!(idset_encode(self.c_idset.as_mut_ptr(), flags.bits() as _))?;
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
    pub fn insert(&mut self, value: u32) -> Result<()> {
        flux_try!(empty_ok idset_set(self.c_idset.as_mut_ptr(), value))
    }

    /// Insert a range of ids into the Idset.
    pub fn insert_range(&mut self, range: Range<u32>) -> Result<()> {
        let range_start = range.start;
        // We subtract 1 here because Rust Range represents the open interval
        // [start, end)
        let range_end = range.end - 1;
        if range_end == range_start {
            return self.insert(range_start);
        }
        flux_try!(empty_ok idset_range_set(self.c_idset.as_mut_ptr(), range_start, range_end))
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

    pub fn union<OtherState: PossiblyDroppablePtr<idset>>(
        &self,
        other: &Idset<OtherState>,
    ) -> Result<OwnedIdset> {
        let union_ptr = flux_try!(idset_union(
            self.c_idset.as_mut_ptr(),
            other.c_idset.as_mut_ptr()
        ))?;
        Ok(Idset {
            c_idset: FluxPtr::create_owned(union_ptr, idset_destroy)?,
        })
    }

    pub fn intersection<OtherState: PossiblyDroppablePtr<idset>>(
        &self,
        other: &Idset<OtherState>,
    ) -> Result<OwnedIdset> {
        let intersection_ptr = flux_try!(idset_intersect(
            self.c_idset.as_mut_ptr(),
            other.c_idset.as_mut_ptr()
        ))?;
        Ok(Idset {
            c_idset: FluxPtr::create_owned(intersection_ptr, idset_destroy)?,
        })
    }

    pub fn difference<OtherState: PossiblyDroppablePtr<idset>>(
        &self,
        other: &Idset<OtherState>,
    ) -> Result<OwnedIdset> {
        let difference_ptr = flux_try!(idset_difference(
            self.c_idset.as_mut_ptr(),
            other.c_idset.as_mut_ptr()
        ))?;
        Ok(Idset {
            c_idset: FluxPtr::create_owned(difference_ptr, idset_destroy)?,
        })
    }

    pub fn is_disjoint<OtherState: PossiblyDroppablePtr<idset>>(
        &self,
        other: &Idset<OtherState>,
    ) -> bool {
        unsafe { !idset_has_intersection(self.c_idset.as_mut_ptr(), other.c_idset.as_mut_ptr()) }
    }

    /// Create an iterator over the ids in the Idset.
    ///
    /// This method is used to create an iterator for a reference to Idset.
    /// If you want to create an Iterator that assumes ownership, use into_iter instead.
    pub fn iter(&self) -> Iter<'_, State> {
        Iter {
            idset: self,
            current: self.first().unwrap_or(IDSET_INVALID_ID),
        }
    }
}

impl<SelfState: PossiblyDroppablePtr<idset>, OtherState: PossiblyDroppablePtr<idset>>
    PartialEq<Idset<OtherState>> for Idset<SelfState>
{
    fn eq(&self, other: &Idset<OtherState>) -> bool {
        unsafe { idset_equal(self.c_idset.as_mut_ptr(), other.c_idset.as_mut_ptr()) }
    }
}

impl std::str::FromStr for OwnedIdset {
    type Err = FluxError;

    /// Decode a string representation into an Idset object.
    fn from_str(s: &str) -> Result<Self> {
        let c_str = CString::new(s)?;
        let ptr = flux_try!(idset_decode(c_str.as_ptr()))?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(ptr, idset_destroy)?,
        })
    }
}

impl<State: PossiblyDroppablePtr<idset>> Display for Idset<State> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let encoded_idset = match self.encode(IdsetFlags::RANGE | IdsetFlags::BRACKETS) {
            Ok(s) => s,
            Err(e) => format!("UNKNOWN (Failed to convert C string to Rust string: {e})"),
        };
        write!(f, "{}", encoded_idset)
    }
}

impl<State: PossiblyDroppablePtr<idset>> TryFrom<&Idset<State>> for OwnedIdset {
    type Error = FluxError;

    /// Copy an Idset.
    ///
    /// This function is used to create a copy of an Idset because the underlying
    /// `idset_copy` function may fail.
    fn try_from(value: &Idset<State>) -> Result<Self> {
        let new_ptr = flux_try!(idset_copy(value.c_idset.as_mut_ptr()))?;
        Ok(Self {
            c_idset: FluxPtr::create_owned(new_ptr, idset_destroy)?,
        })
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedIdset<'a> {
    type CType = idset;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_idset: FluxPtr::create_borrowed(ptr)?,
        })
    }
}

unsafe impl FromFluxPtr for OwnedIdset {
    type CType = idset;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_idset: FluxPtr::create_owned(ptr, idset_destroy)?,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<idset>> AsFluxPtr for Idset<State> {
    define_as_flux_ptr_body!(idset, c_idset);
}

unsafe impl IntoFluxPtr for OwnedIdset {
    define_into_flux_ptr_body!(c_idset);
}

impl<State: PossiblyDroppablePtr<idset>> Serialize for Idset<State> {
    fn serialize<S>(&self, serializer: S) -> std::prelude::v1::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for OwnedIdset {
    fn deserialize<D>(deserializer: D) -> std::prelude::v1::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct IdsetVisitor;

        impl<'de> Visitor<'de> for IdsetVisitor {
            type Value = OwnedIdset;

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

impl_serde_repr_str!(no_display BorrowedIdset<'a>);
impl_serde_repr_str!(no_display OwnedIdset);

pub struct Iter<'r, State: PossiblyDroppablePtr<idset>> {
    idset: &'r Idset<State>,
    current: u32,
}

impl<'r, State: PossiblyDroppablePtr<idset>> Iterator for Iter<'r, State> {
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

pub struct IntoIter {
    idset: OwnedIdset,
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

impl IntoIterator for OwnedIdset {
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
