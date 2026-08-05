use serde_json::json;

use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtrNoArgs, IntoFluxPtr};
use crate::kvs::flags::KvsFlags;
use crate::kvs::txn::KvsTransaction;

// =========================================================================
// Construction
// =========================================================================

#[test]
fn new_creates_valid_transaction() {
    let txn = KvsTransaction::new();
    assert!(txn.is_ok());
    let txn = txn.unwrap();
    assert!(txn.base_path.is_none());
    assert!(!txn.c_txn.as_mut_ptr().is_null());
}

#[test]
fn from_path_sets_base_path() {
    let txn = KvsTransaction::from_path("a.b.c").unwrap();
    assert_eq!(txn.base_path.as_deref(), Some("a.b.c"));
}

// =========================================================================
// Staging Operations (put, put_json, put_serializable, mkdir, unlink, symlink)
// =========================================================================

#[test]
fn put_raw_bytes_succeeds() {
    let mut txn = KvsTransaction::new().unwrap();
    assert!(txn.put("test_key", b"hello world", KvsFlags::NONE).is_ok());
}

#[test]
fn put_with_base_path_succeeds() {
    let mut txn = KvsTransaction::from_path("sub_dir").unwrap();
    assert!(txn.put("key", b"val", KvsFlags::NONE).is_ok());
}

#[test]
fn put_json_succeeds() {
    let mut txn = KvsTransaction::new().unwrap();
    let val = json!({"foo": "bar", "num": 42});
    assert!(txn.put_json("json_key", &val, KvsFlags::NONE).is_ok());
}

#[test]
fn put_serializable_succeeds() {
    let mut txn = KvsTransaction::new().unwrap();
    let vec_data = vec![1, 2, 3];
    assert!(txn
        .put_serializable("vec_key", &vec_data, KvsFlags::NONE)
        .is_ok());
}

#[test]
fn mkdir_succeeds() {
    let mut txn = KvsTransaction::new().unwrap();
    assert!(txn.mkdir("dir_key", KvsFlags::NONE).is_ok());
}

#[test]
fn unlink_succeeds() {
    let mut txn = KvsTransaction::new().unwrap();
    assert!(txn.unlink("remove_key", KvsFlags::NONE).is_ok());
}

#[test]
fn symlink_without_namespace_succeeds() {
    let mut txn = KvsTransaction::new().unwrap();
    assert!(txn
        .symlink("link_key", "target_key", None, KvsFlags::NONE)
        .is_ok());
}

#[test]
fn symlink_with_namespace_succeeds() {
    let mut txn = KvsTransaction::new().unwrap();
    assert!(txn
        .symlink("link_key", "target_key", Some("guest"), KvsFlags::NONE)
        .is_ok());
}

// =========================================================================
// Error Handling
// =========================================================================

#[test]
fn put_nul_byte_key_returns_error() {
    let mut txn = KvsTransaction::new().unwrap();
    assert!(txn.put("bad\0key", b"data", KvsFlags::NONE).is_err());
}

#[test]
fn symlink_nul_byte_namespace_returns_error() {
    let mut txn = KvsTransaction::new().unwrap();
    assert!(txn
        .symlink("link", "target", Some("bad\0ns"), KvsFlags::NONE)
        .is_err());
}

// =========================================================================
// Pointer Management
// =========================================================================

#[test]
fn borrow_ptr_produces_non_owning_transaction() {
    let txn = KvsTransaction::new().unwrap();
    let ptr = txn.as_mut_ptr();
    let borrowed = unsafe { KvsTransaction::borrow_ptr(ptr) }.unwrap();
    assert!(!borrowed.c_txn.is_owned());
}

#[test]
fn from_ptr_produces_owning_transaction() {
    let txn = KvsTransaction::new().unwrap();
    let ptr = txn.into_raw();
    let owned = unsafe { KvsTransaction::from_ptr(ptr) }.unwrap();
    assert!(owned.c_txn.is_owned());
}
