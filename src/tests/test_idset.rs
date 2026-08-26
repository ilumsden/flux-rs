use flux_sys::idset::{idset_create, idset_destroy};

use crate::error::Result;
use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtrNoArgs, IntoFluxPtr};
use crate::idset::{Idset, IdsetFlags};

// =========================================================================
// Helpers
// =========================================================================

/// Build an auto-growing `Idset` pre-populated with the given ids.
fn make_idset(ids: &[u32]) -> Idset {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).expect("Failed to create Idset");
    for &id in ids {
        idset.insert(id).expect("Failed to insert id");
    }
    idset
}

// =========================================================================
// Idset::new
// =========================================================================

#[test]
fn new_with_autogrow_creates_empty_idset() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert!(idset.is_empty());
    assert_eq!(idset.len(), 0);
}

#[test]
fn new_with_fixed_size_creates_empty_idset() {
    let idset = Idset::new(64, IdsetFlags::NONE).unwrap();
    assert!(idset.is_empty());
}

#[test]
fn new_with_initfull_creates_full_idset() {
    // INITFULL pre-populates the entire idset up to `size`.
    let idset = Idset::new(8, IdsetFlags::INITFULL).unwrap();
    assert!(!idset.is_empty());
    assert_eq!(idset.len(), 8);
}

// =========================================================================
// FromStr
// =========================================================================

#[test]
fn from_str_parses_single_id() {
    let idset: Idset = "5".parse().unwrap();
    assert_eq!(idset.len(), 1);
    assert!(idset.contains(5));
}

#[test]
fn from_str_parses_range_notation() {
    let idset: Idset = "1-3".parse().unwrap();
    assert_eq!(idset.len(), 3);
    assert!(idset.contains(1));
    assert!(idset.contains(2));
    assert!(idset.contains(3));
}

#[test]
fn from_str_parses_comma_separated() {
    let idset: Idset = "1,3,5".parse().unwrap();
    assert_eq!(idset.len(), 3);
    assert!(idset.contains(1));
    assert!(idset.contains(3));
    assert!(idset.contains(5));
}

#[test]
fn from_str_nul_byte_returns_error() {
    let result: Result<Idset> = "1\x002".parse();
    assert!(result.is_err());
}

#[test]
fn from_str_invalid_string_returns_error() {
    let result: Result<Idset> = "not_an_idset!@#".parse();
    assert!(result.is_err());
}

// =========================================================================
// insert / contains
// =========================================================================

#[test]
fn insert_makes_id_present() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    idset.insert(42).unwrap();
    assert!(idset.contains(42));
}

#[test]
fn contains_absent_id_returns_false() {
    let idset = make_idset(&[1, 2, 3]);
    assert!(!idset.contains(99));
}

#[test]
fn insert_duplicate_does_not_increase_len() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    idset.insert(7).unwrap();
    idset.insert(7).unwrap();
    assert_eq!(idset.len(), 1);
}

// =========================================================================
// insert_range
// =========================================================================

#[test]
fn insert_range_multi_element_inserts_all() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    idset.insert_range(0..5).unwrap();
    assert_eq!(idset.len(), 5);
    for i in 0..5 {
        assert!(idset.contains(i));
    }
}

#[test]
fn insert_range_single_element_uses_insert_path() {
    // 5..6 → range_end (5) == range_start (5) → delegates to insert(5)
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    idset.insert_range(5..6).unwrap();
    assert_eq!(idset.len(), 1);
    assert!(idset.contains(5));
}

#[test]
fn insert_range_zero_based_single_element() {
    // 0..1 → range_end (0) == range_start (0) → delegates to insert(0)
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    idset.insert_range(0..1).unwrap();
    assert_eq!(idset.len(), 1);
    assert!(idset.contains(0));
}

// =========================================================================
// len / is_empty
// =========================================================================

#[test]
fn len_reflects_number_of_insertions() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert_eq!(idset.len(), 0);
    idset.insert(1).unwrap();
    assert_eq!(idset.len(), 1);
    idset.insert(2).unwrap();
    assert_eq!(idset.len(), 2);
}

#[test]
fn is_empty_true_for_new_idset() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert!(idset.is_empty());
}

#[test]
fn is_empty_false_after_insert() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    idset.insert(0).unwrap();
    assert!(!idset.is_empty());
}

// =========================================================================
// first / last
// =========================================================================

#[test]
fn first_on_empty_returns_none() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert!(idset.first().is_none());
}

#[test]
fn first_returns_minimum_id() {
    let idset = make_idset(&[5, 1, 9, 3]);
    assert_eq!(idset.first(), Some(1));
}

#[test]
fn last_on_empty_returns_none() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert!(idset.last().is_none());
}

#[test]
fn last_returns_maximum_id() {
    let idset = make_idset(&[5, 1, 9, 3]);
    assert_eq!(idset.last(), Some(9));
}

// =========================================================================
// encode
// =========================================================================

#[test]
fn encode_with_range_flag_uses_range_notation() {
    let idset = make_idset(&[0, 1, 2]);
    let encoded = idset.encode(IdsetFlags::RANGE).unwrap();
    assert_eq!(encoded, "0-2");
}

#[test]
fn encode_with_brackets_and_range_flags() {
    let idset = make_idset(&[0, 1, 2]);
    let encoded = idset
        .encode(IdsetFlags::RANGE | IdsetFlags::BRACKETS)
        .unwrap();
    assert_eq!(encoded, "[0-2]");
}

#[test]
fn encode_without_range_flag_uses_comma_notation() {
    let idset = make_idset(&[0, 1, 2]);
    let encoded = idset.encode(IdsetFlags::NONE).unwrap();
    assert_eq!(encoded, "0,1,2");
}

#[test]
fn encode_single_id_no_range_notation() {
    let idset = make_idset(&[7]);
    let encoded = idset.encode(IdsetFlags::RANGE).unwrap();
    assert_eq!(encoded, "7");
}

// =========================================================================
// try_clone / TryFrom<&Idset>
// =========================================================================

#[test]
fn try_clone_produces_equal_idset() {
    let idset = make_idset(&[1, 2, 3]);
    let clone = idset.try_clone().unwrap();
    assert_eq!(clone, idset);
}

#[test]
fn try_clone_is_independent_of_original() {
    let idset = make_idset(&[1, 2]);
    let clone = idset.try_clone().unwrap();
    clone.insert(3).unwrap();
    // Original must be unaffected.
    assert_eq!(idset.len(), 2);
    assert_eq!(clone.len(), 3);
}

#[test]
fn try_from_ref_round_trips_content() {
    let idset: Idset = "1-5".parse().unwrap();
    let clone = Idset::try_from(&idset).unwrap();
    assert_eq!(
        clone.encode(IdsetFlags::RANGE).unwrap(),
        idset.encode(IdsetFlags::RANGE).unwrap()
    );
}

// =========================================================================
// PartialEq
// =========================================================================

#[test]
fn equal_idsets_compare_equal() {
    let a = make_idset(&[1, 2, 3]);
    let b = make_idset(&[1, 2, 3]);
    assert_eq!(a, b);
}

#[test]
fn unequal_idsets_compare_not_equal() {
    let a = make_idset(&[1, 2, 3]);
    let b = make_idset(&[1, 2, 4]);
    assert_ne!(a, b);
}

#[test]
fn empty_idsets_compare_equal() {
    let a = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    let b = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert_eq!(a, b);
}

// =========================================================================
// union
// =========================================================================

#[test]
fn union_of_disjoint_sets_contains_all_ids() {
    let a = make_idset(&[1, 2]);
    let b = make_idset(&[3, 4]);
    let u = a.union(&b).unwrap();
    assert_eq!(u.len(), 4);
    for i in 1..=4 {
        assert!(u.contains(i));
    }
}

#[test]
fn union_of_overlapping_sets_deduplicates() {
    let a = make_idset(&[1, 2, 3]);
    let b = make_idset(&[2, 3, 4]);
    let u = a.union(&b).unwrap();
    assert_eq!(u.len(), 4);
}

#[test]
fn union_with_empty_set_equals_original() {
    let a = make_idset(&[1, 2, 3]);
    let empty = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    let u = a.union(&empty).unwrap();
    assert_eq!(u, a);
}

// =========================================================================
// intersection
// =========================================================================

#[test]
fn intersection_of_overlapping_sets_contains_common_ids() {
    let a = make_idset(&[1, 2, 3, 4]);
    let b = make_idset(&[3, 4, 5, 6]);
    let inter = a.intersection(&b).unwrap();
    assert_eq!(inter.len(), 2);
    assert!(inter.contains(3));
    assert!(inter.contains(4));
}

#[test]
fn intersection_of_disjoint_sets_is_empty() {
    let a = make_idset(&[1, 2]);
    let b = make_idset(&[3, 4]);
    let inter = a.intersection(&b).unwrap();
    assert!(inter.is_empty());
}

#[test]
fn intersection_with_identical_set_equals_self() {
    let a = make_idset(&[1, 2, 3]);
    let b = make_idset(&[1, 2, 3]);
    let inter = a.intersection(&b).unwrap();
    assert_eq!(inter, a);
}

// =========================================================================
// difference
// =========================================================================

#[test]
fn difference_removes_common_ids() {
    let a = make_idset(&[1, 2, 3, 4]);
    let b = make_idset(&[3, 4]);
    let diff = a.difference(&b).unwrap();
    assert_eq!(diff.len(), 2);
    assert!(diff.contains(1));
    assert!(diff.contains(2));
    assert!(!diff.contains(3));
    assert!(!diff.contains(4));
}

#[test]
fn difference_with_empty_other_equals_self() {
    let a = make_idset(&[1, 2, 3]);
    let empty = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    let diff = a.difference(&empty).unwrap();
    assert_eq!(diff, a);
}

#[test]
fn difference_with_superset_is_empty() {
    let a = make_idset(&[1, 2]);
    let b = make_idset(&[1, 2, 3]);
    let diff = a.difference(&b).unwrap();
    assert!(diff.is_empty());
}

// =========================================================================
// is_disjoint
// =========================================================================

#[test]
fn is_disjoint_with_non_overlapping_sets() {
    let a = make_idset(&[1, 2]);
    let b = make_idset(&[3, 4]);
    assert!(a.is_disjoint(&b));
}

#[test]
fn is_not_disjoint_with_overlapping_sets() {
    let a = make_idset(&[1, 2, 3]);
    let b = make_idset(&[3, 4, 5]);
    assert!(!a.is_disjoint(&b));
}

#[test]
fn is_disjoint_with_empty_set() {
    let a = make_idset(&[1, 2, 3]);
    let empty = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert!(a.is_disjoint(&empty));
}

// =========================================================================
// Display
// =========================================================================

#[test]
fn display_matches_encode_with_range_and_brackets() {
    let idset = make_idset(&[1, 2, 3]);
    let expected = idset
        .encode(IdsetFlags::RANGE | IdsetFlags::BRACKETS)
        .unwrap();
    assert_eq!(idset.to_string(), expected);
}

#[test]
fn display_non_empty_is_non_empty_string() {
    let idset = make_idset(&[5]);
    assert!(!idset.to_string().is_empty());
}

// =========================================================================
// Iter (borrowing iterator)
// =========================================================================

#[test]
fn iter_empty_idset_yields_nothing() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert_eq!(idset.iter().count(), 0);
}

#[test]
fn iter_yields_all_ids_in_ascending_order() {
    // Idset stores ids in sorted order; insertion order doesn't matter.
    let idset = make_idset(&[3, 1, 5, 2]);
    let ids: Vec<u32> = idset.iter().collect();
    assert_eq!(ids, vec![1, 2, 3, 5]);
}

#[test]
fn iter_count_matches_len() {
    let idset = make_idset(&[10, 20, 30]);
    assert_eq!(idset.iter().count(), idset.len());
}

#[test]
fn ref_into_iterator_yields_all_ids() {
    let idset = make_idset(&[1, 2, 3]);
    let ids: Vec<u32> = idset.into_iter().collect();
    assert_eq!(ids, vec![1, 2, 3]);
}

// =========================================================================
// IntoIter (consuming iterator)
// =========================================================================

#[test]
fn into_iter_yields_all_ids_in_ascending_order() {
    let idset = make_idset(&[4, 2, 6]);
    let ids: Vec<u32> = idset.into_iter().collect();
    assert_eq!(ids, vec![2, 4, 6]);
}

#[test]
fn into_iter_empty_idset_yields_nothing() {
    let idset = Idset::new(0, IdsetFlags::AUTOGROW).unwrap();
    assert_eq!(idset.into_iter().count(), 0);
}

// =========================================================================
// BorrowFluxPtr / FromFluxPtr
// =========================================================================

#[test]
fn borrow_ptr_creates_usable_non_owning_idset() {
    let owned = make_idset(&[1, 2, 3]);
    // `borrowed` is a non-owning view; it must be dropped before `owned`
    // (Rust's LIFO drop order guarantees this) to avoid a double-free.
    let borrowed = unsafe { Idset::borrow_ptr(owned.as_mut_ptr()) }.unwrap();
    assert_eq!(borrowed.len(), owned.len());
    assert!(borrowed.contains(1));
}

#[test]
fn from_ptr_creates_owning_idset() {
    // Create a raw pointer directly via the C API to transfer ownership cleanly.
    let raw_ptr = unsafe { idset_create(0, IdsetFlags::AUTOGROW.bits() as i32) };
    assert!(!raw_ptr.is_null());
    // from_ptr assumes ownership; idset_destroy is called automatically on drop.
    let idset = unsafe { Idset::from_ptr(raw_ptr) }.unwrap();
    assert!(idset.is_empty());
}

// =========================================================================
// AsFluxPtr / IntoFluxPtr
// =========================================================================

#[test]
fn as_flux_ptr_returns_non_null_pointer() {
    let idset = make_idset(&[1]);
    assert!(!idset.as_mut_ptr().is_null());
}

#[test]
fn as_flux_ptr_does_not_consume_idset() {
    let idset = make_idset(&[1, 2]);
    let _ = idset.as_mut_ptr();
    // Idset is still usable after as_mut_ptr.
    assert_eq!(idset.len(), 2);
}

#[test]
fn into_flux_ptr_returns_valid_pointer_and_suppresses_drop() {
    let idset = make_idset(&[1, 2, 3]);
    let raw = idset.into_raw();
    assert!(!raw.is_null());
    // Manually destroy to avoid a leak since no Rust type owns the pointer now.
    unsafe { idset_destroy(raw) };
}

// =========================================================================
// Serialize / Deserialize
// =========================================================================

#[test]
fn serialize_produces_json_string() {
    let idset = make_idset(&[1, 2, 3]);
    let json = serde_json::to_string(&idset).unwrap();
    // Serialized form must be a quoted JSON string, not an integer or object.
    assert!(json.starts_with('"') && json.ends_with('"'));
}

#[test]
fn deserialize_from_valid_json_string() {
    let idset: Idset = serde_json::from_str("\"1-3\"").unwrap();
    assert_eq!(idset.len(), 3);
    assert!(idset.contains(1));
    assert!(idset.contains(2));
    assert!(idset.contains(3));
}

#[test]
fn serde_round_trip_preserves_content() {
    let original = make_idset(&[1, 2, 3, 5, 8]);
    let json = serde_json::to_string(&original).unwrap();
    let restored: Idset = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, original);
}

#[test]
fn deserialize_wrong_type_returns_error() {
    // The Idset visitor expects a string; a bare JSON integer must fail.
    let result = serde_json::from_str::<Idset>("42");
    assert!(result.is_err());
}

// =========================================================================
// Debug — impl_serde_repr_str!(no_display Idset)
// =========================================================================

#[test]
fn debug_format_starts_with_type_name() {
    let idset = make_idset(&[1]);
    let debug_str = format!("{idset:?}");
    assert!(
        debug_str.starts_with("OwnedIdset("),
        "Expected debug to start with 'Idset(', got: {debug_str}"
    );
}

#[test]
fn debug_format_contains_serialized_value() {
    let idset = make_idset(&[1]);
    let serialized = serde_json::to_string(&idset).unwrap();
    let debug_str = format!("{idset:?}");
    assert!(
        debug_str.contains(&serialized),
        "Expected debug to contain serialized value '{serialized}', got: {debug_str}"
    );
}

// =========================================================================
// IdsetFlags
// =========================================================================

#[test]
fn idset_flags_none_bits_are_zero() {
    assert_eq!(IdsetFlags::NONE.bits(), 0);
}

#[test]
fn idset_flags_combine_correctly() {
    let combined = IdsetFlags::RANGE | IdsetFlags::BRACKETS;
    assert!(combined.contains(IdsetFlags::RANGE));
    assert!(combined.contains(IdsetFlags::BRACKETS));
    assert!(!combined.contains(IdsetFlags::AUTOGROW));
}

#[test]
fn idset_flags_autogrow_does_not_include_range() {
    let flags = IdsetFlags::AUTOGROW;
    assert!(flags.contains(IdsetFlags::AUTOGROW));
    assert!(!flags.contains(IdsetFlags::RANGE));
}
