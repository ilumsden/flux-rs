use flux_sys::hostlist::{hostlist_create, hostlist_destroy};

use crate::error::Result;
use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtrNoArgs, IntoFluxPtr};
use crate::hostlist::Hostlist;

// =========================================================================
// Helpers
// =========================================================================

/// Build a `Hostlist` from a slice of host name strings.
fn make_hostlist(hosts: &[&str]) -> Hostlist {
    Hostlist::try_from_iter(hosts.iter().copied()).expect("Failed to build Hostlist")
}

// =========================================================================
// Hostlist::new
// =========================================================================

#[test]
fn new_creates_empty_hostlist() {
    let hl = Hostlist::new().unwrap();
    assert_eq!(hl.len(), 0);
}

// =========================================================================
// FromStr
// =========================================================================

#[test]
fn from_str_parses_range_notation() {
    let hl: Hostlist = "host[1-3]".parse().unwrap();
    assert_eq!(hl.len(), 3);
}

#[test]
fn from_str_parses_single_host() {
    let hl: Hostlist = "node0".parse().unwrap();
    assert_eq!(hl.len(), 1);
}

#[test]
fn from_str_parses_comma_separated_hosts() {
    let hl: Hostlist = "a,b,c".parse().unwrap();
    assert_eq!(hl.len(), 3);
}

#[test]
fn from_str_nul_byte_returns_error() {
    let result: Result<Hostlist> = "host\0list".parse();
    assert!(result.is_err());
}

// =========================================================================
// push / len
// =========================================================================

#[test]
fn push_increases_len() {
    let mut hl = Hostlist::new().unwrap();
    hl.push("node0").unwrap();
    assert_eq!(hl.len(), 1);
    hl.push("node1").unwrap();
    assert_eq!(hl.len(), 2);
}

#[test]
fn push_nul_byte_returns_error() {
    let mut hl = Hostlist::new().unwrap();
    assert!(hl.push("host\0name").is_err());
    // Hostlist should be unmodified after the error.
    assert_eq!(hl.len(), 0);
}

// =========================================================================
// encode / Display
// =========================================================================

#[test]
fn encode_single_host_round_trips() {
    let hl: Hostlist = "node0".parse().unwrap();
    assert_eq!(hl.encode().unwrap(), "node0");
}

#[test]
fn encode_non_empty_hostlist_is_non_empty_string() {
    let hl = make_hostlist(&["a", "b", "c"]);
    assert!(!hl.encode().unwrap().is_empty());
}

#[test]
fn display_matches_encode() {
    let hl: Hostlist = "host[1-3]".parse().unwrap();
    assert_eq!(hl.to_string(), hl.encode().unwrap());
}

// =========================================================================
// try_clone / TryFrom<&Hostlist>
// =========================================================================

#[test]
fn try_clone_produces_same_len() {
    let hl = make_hostlist(&["a", "b", "c"]);
    let clone = hl.try_clone().unwrap();
    assert_eq!(clone.len(), hl.len());
}

#[test]
fn try_clone_is_independent_of_original() {
    let hl = make_hostlist(&["a", "b"]);
    let mut clone = hl.try_clone().unwrap();
    clone.push("c").unwrap();
    // Original must be unaffected.
    assert_eq!(hl.len(), 2);
    assert_eq!(clone.len(), 3);
}

#[test]
fn try_from_ref_round_trips_content() {
    let hl: Hostlist = "host[0-4]".parse().unwrap();
    let clone = Hostlist::try_from(&hl).unwrap();
    assert_eq!(clone.encode().unwrap(), hl.encode().unwrap());
}

// =========================================================================
// append
// =========================================================================

#[test]
fn append_adds_all_hosts() {
    let mut hl = make_hostlist(&["a", "b"]);
    let other = make_hostlist(&["c", "d"]);
    hl.append(&other).unwrap();
    assert_eq!(hl.len(), 4);
}

#[test]
fn append_empty_other_does_not_change_len() {
    let mut hl = make_hostlist(&["a", "b"]);
    let empty = Hostlist::new().unwrap();
    hl.append(&empty).unwrap();
    assert_eq!(hl.len(), 2);
}

#[test]
fn append_to_empty_hostlist_equals_other_len() {
    let mut hl = Hostlist::new().unwrap();
    let other = make_hostlist(&["x", "y", "z"]);
    hl.append(&other).unwrap();
    assert_eq!(hl.len(), 3);
}

// =========================================================================
// find
// =========================================================================

#[test]
fn find_first_host_returns_zero() {
    let mut hl = make_hostlist(&["x", "y"]);
    assert_eq!(hl.find("x").unwrap(), Some(0));
}

#[test]
fn find_middle_host_returns_correct_position() {
    let mut hl = make_hostlist(&["a", "b", "c"]);
    assert_eq!(hl.find("b").unwrap(), Some(1));
}

#[test]
fn find_nonexistent_host_returns_none() {
    let mut hl = make_hostlist(&["a", "b"]);
    assert_eq!(hl.find("z").unwrap(), None);
}

#[test]
fn find_nul_byte_returns_error() {
    let mut hl = make_hostlist(&["a"]);
    assert!(hl.find("a\0b").is_err());
}

// =========================================================================
// nth
// =========================================================================

#[test]
fn nth_first_index_returns_first_host() {
    let mut hl = make_hostlist(&["first", "second"]);
    assert_eq!(hl.nth(0).as_deref(), Some("first"));
}

#[test]
fn nth_valid_middle_index_returns_correct_host() {
    let mut hl = make_hostlist(&["x", "y", "z"]);
    assert_eq!(hl.nth(1).as_deref(), Some("y"));
}

#[test]
fn nth_out_of_bounds_returns_none() {
    let mut hl = make_hostlist(&["a"]);
    assert!(hl.nth(99).is_none());
}

#[test]
fn nth_empty_hostlist_returns_none() {
    let mut hl = Hostlist::new().unwrap();
    assert!(hl.nth(0).is_none());
}

// =========================================================================
// remove (by index)
// =========================================================================

#[test]
fn remove_valid_index_returns_true_and_decreases_len() {
    let mut hl = make_hostlist(&["a", "b", "c"]);
    assert!(hl.remove(1));
    assert_eq!(hl.len(), 2);
}

#[test]
fn remove_first_element_decreases_len() {
    let mut hl = make_hostlist(&["a", "b"]);
    assert!(hl.remove(0));
    assert_eq!(hl.len(), 1);
}

#[test]
fn remove_out_of_bounds_returns_false_and_preserves_len() {
    let mut hl = make_hostlist(&["a"]);
    assert!(!hl.remove(99));
    assert_eq!(hl.len(), 1);
}

// =========================================================================
// remove_host (by name)
// =========================================================================

#[test]
fn remove_host_existing_returns_true_and_decreases_len() {
    let mut hl = make_hostlist(&["a", "b", "c"]);
    assert!(hl.remove_host("b").unwrap());
    assert_eq!(hl.len(), 2);
}

#[test]
fn remove_host_nonexistent_returns_false_and_preserves_len() {
    let mut hl = make_hostlist(&["a", "b"]);
    assert!(!hl.remove_host("z").unwrap());
    assert_eq!(hl.len(), 2);
}

#[test]
fn remove_host_nul_byte_returns_error() {
    let mut hl = make_hostlist(&["a"]);
    assert!(hl.remove_host("a\0b").is_err());
}

// =========================================================================
// dedup
// =========================================================================

#[test]
fn dedup_removes_duplicate_hosts() {
    let mut hl = make_hostlist(&["a", "b", "a", "c", "b"]);
    hl.dedup();
    assert_eq!(hl.len(), 3);
}

#[test]
fn dedup_no_duplicates_is_noop() {
    let mut hl = make_hostlist(&["a", "b", "c"]);
    hl.dedup();
    assert_eq!(hl.len(), 3);
}

#[test]
fn dedup_all_duplicates_leaves_one() {
    let mut hl = make_hostlist(&["x", "x", "x"]);
    hl.dedup();
    assert_eq!(hl.len(), 1);
}

// =========================================================================
// sort
// =========================================================================

#[test]
fn sort_orders_hosts() {
    let mut hl = make_hostlist(&["c", "a", "b"]);
    hl.sort();
    assert_eq!(hl.nth(0).as_deref(), Some("a"));
    assert_eq!(hl.nth(1).as_deref(), Some("b"));
    assert_eq!(hl.nth(2).as_deref(), Some("c"));
}

#[test]
fn sort_already_sorted_is_noop() {
    let mut hl = make_hostlist(&["a", "b", "c"]);
    hl.sort();
    assert_eq!(hl.nth(0).as_deref(), Some("a"));
    assert_eq!(hl.nth(2).as_deref(), Some("c"));
}

// =========================================================================
// try_from_iter
// =========================================================================

#[test]
fn try_from_iter_creates_correct_len() {
    let hl = Hostlist::try_from_iter(["node0", "node1", "node2"]).unwrap();
    assert_eq!(hl.len(), 3);
}

#[test]
fn try_from_iter_empty_iterator_creates_empty_hostlist() {
    let hl = Hostlist::try_from_iter(std::iter::empty::<&str>()).unwrap();
    assert_eq!(hl.len(), 0);
}

#[test]
fn try_from_iter_nul_byte_returns_error() {
    let result = Hostlist::try_from_iter(["valid", "in\0valid"]);
    assert!(result.is_err());
}

#[test]
fn try_from_iter_from_string_vec() {
    let hosts: Vec<String> = vec!["a".to_string(), "b".to_string()];
    let hl = Hostlist::try_from_iter(hosts).unwrap();
    assert_eq!(hl.len(), 2);
}

// =========================================================================
// HostlistCursor
// =========================================================================

#[test]
fn cursor_next_iterates_all_hosts_in_order() {
    let mut hl = make_hostlist(&["a", "b", "c"]);
    let mut cursor = hl.cursor_mut();
    assert_eq!(cursor.next().as_deref(), Some("a"));
    assert_eq!(cursor.next().as_deref(), Some("b"));
    assert_eq!(cursor.next().as_deref(), Some("c"));
    assert_eq!(cursor.next(), None);
}

#[test]
fn cursor_next_on_empty_hostlist_returns_none() {
    let mut hl = Hostlist::new().unwrap();
    let mut cursor = hl.cursor_mut();
    assert_eq!(cursor.next(), None);
}

#[test]
fn cursor_current_returns_last_visited_host() {
    let mut hl = make_hostlist(&["x", "y", "z"]);
    let mut cursor = hl.cursor_mut();
    cursor.next(); // "x"
    cursor.next(); // "y"
    assert_eq!(cursor.current().as_deref(), Some("y"));
}

#[test]
fn cursor_remove_current_shrinks_hostlist() {
    let mut hl = make_hostlist(&["a", "b", "c"]);
    {
        let mut cursor = hl.cursor_mut();
        cursor.next(); // "a"
        cursor.next(); // "b"
        assert!(cursor.remove_current());
    }
    assert_eq!(hl.len(), 2);
}

#[test]
fn cursor_next_after_remove_yields_remaining_hosts() {
    let mut hl = make_hostlist(&["a", "b", "c"]);
    {
        let mut cursor = hl.cursor_mut();
        cursor.next(); // "a"
        cursor.remove_current();
        // After removing "a", subsequent nexts yield the remaining hosts.
        let rest: Vec<String> = std::iter::from_fn(|| cursor.next()).collect();
        assert_eq!(rest, vec!["b", "c"]);
    }
}

// =========================================================================
// IntoIterator
// =========================================================================

#[test]
fn into_iter_yields_all_hosts_in_order() {
    let hl = make_hostlist(&["a", "b", "c"]);
    let collected: Vec<String> = hl.into_iter().collect();
    assert_eq!(collected, vec!["a", "b", "c"]);
}

#[test]
fn into_iter_empty_hostlist_yields_nothing() {
    let hl = Hostlist::new().unwrap();
    let collected: Vec<String> = hl.into_iter().collect();
    assert!(collected.is_empty());
}

#[test]
fn into_iter_count_matches_len() {
    let hl = make_hostlist(&["x", "y", "z"]);
    let len = hl.len();
    let count = hl.into_iter().count();
    assert_eq!(count, len);
}

// =========================================================================
// Serialize / Deserialize
// =========================================================================

#[test]
fn serialize_produces_encoded_json_string() {
    let hl = make_hostlist(&["a", "b"]);
    let encoded = hl.encode().unwrap();
    let json = serde_json::to_string(&hl).unwrap();
    // serde_json wraps the string value in quotes.
    assert_eq!(json, format!("\"{}\"", encoded));
}

#[test]
fn deserialize_from_valid_string_produces_correct_len() {
    let hl: Hostlist = serde_json::from_str("\"host[0-2]\"").unwrap();
    assert_eq!(hl.len(), 3);
}

#[test]
fn deserialize_from_single_host_string() {
    let hl: Hostlist = serde_json::from_str("\"node0\"").unwrap();
    assert_eq!(hl.len(), 1);
}

#[test]
fn serde_round_trip_preserves_content() {
    let original = make_hostlist(&["a", "b", "c"]);
    let json = serde_json::to_string(&original).unwrap();
    let restored: Hostlist = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.encode().unwrap(), original.encode().unwrap());
}

#[test]
fn deserialize_from_non_string_json_returns_error() {
    let result: std::result::Result<Hostlist, _> = serde_json::from_str("42");
    assert!(result.is_err());
}

// =========================================================================
// BorrowFluxPtr / FromFluxPtr (via NoArgs blanket impls)
// =========================================================================

#[test]
fn borrow_ptr_creates_non_owning_hostlist() {
    // The borrowed Hostlist must not free the pointer when dropped;
    // the owning `hl` will free it correctly afterwards (LIFO drop order).
    let hl = make_hostlist(&["a", "b"]);
    let ptr = hl.as_mut_ptr();
    let borrowed = unsafe { Hostlist::borrow_ptr(ptr) }.unwrap();
    assert_eq!(borrowed.len(), 2);
    // `borrowed` drops here (no-op destructor), then `hl` drops and frees.
}

#[test]
fn borrow_ptr_null_returns_error() {
    let result = unsafe { Hostlist::borrow_ptr(std::ptr::null_mut()) };
    assert!(result.is_err());
}

#[test]
fn from_ptr_creates_owning_hostlist() {
    // Create a raw pointer independently so from_ptr can take ownership.
    let ptr = unsafe { hostlist_create() };
    assert!(!ptr.is_null());
    let hl = unsafe { Hostlist::from_ptr(ptr) }.unwrap();
    assert_eq!(hl.len(), 0);
    // `hl` drops here and frees `ptr`.
}

#[test]
fn from_ptr_null_returns_error() {
    let result = unsafe { Hostlist::from_ptr(std::ptr::null_mut()) };
    assert!(result.is_err());
}

// =========================================================================
// AsFluxPtr / IntoFluxPtr (generated by default_impl_as_flux_ptr!)
// =========================================================================

#[test]
fn as_mut_ptr_returns_non_null() {
    let hl = make_hostlist(&["a"]);
    assert!(!hl.as_mut_ptr().is_null());
}

#[test]
fn as_mut_ptr_is_stable_across_calls() {
    let hl = make_hostlist(&["a"]);
    assert_eq!(hl.as_mut_ptr(), hl.as_mut_ptr());
}

#[test]
fn into_raw_returns_non_null_and_transfers_ownership() {
    let hl = make_hostlist(&["a"]);
    let ptr = hl.into_raw();
    assert!(!ptr.is_null());
    // We now own the pointer; destroy it manually to avoid a leak.
    unsafe { hostlist_destroy(ptr) };
}
