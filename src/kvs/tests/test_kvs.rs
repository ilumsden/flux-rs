use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::kvs::flags::KvsFlags;
use crate::kvs::kvs::Kvs;
use crate::kvs::txn::KvsTransaction;
use crate::tests::common::with_handle;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TestData {
    msg: String,
    val: u32,
}

// =========================================================================
// Construction & NUL-byte validation
// =========================================================================

#[test]
fn kvs_new_succeeds() {
    with_handle(|h| {
        let _kvs = Kvs::new(h);
    });
}

#[test]
fn lookup_nul_byte_key_returns_error() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        assert!(kvs.lookup("bad\0key", KvsFlags::NONE, None).is_err());
    });
}

#[test]
fn lookup_nul_byte_namespace_returns_error() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        assert!(kvs.lookup("key", KvsFlags::NONE, Some("bad\0ns")).is_err());
    });
}

#[test]
fn getroot_nul_byte_namespace_returns_error() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        assert!(kvs.getroot(Some("bad\0ns")).is_err());
    });
}

#[test]
fn create_namespace_nul_byte_returns_error() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        assert!(kvs
            .create_namespace("bad\0ns", KvsFlags::NONE, None)
            .is_err());
    });
}

#[test]
fn remove_namespace_nul_byte_returns_error() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        assert!(kvs.remove_namespace("bad\0ns").is_err());
    });
}

#[test]
fn copy_entry_nul_byte_returns_error() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        assert!(kvs
            .copy_entry("bad\0src", "dst", KvsFlags::NONE, None, None)
            .is_err());
    });
}

#[test]
fn move_entry_nul_byte_returns_error() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        assert!(kvs
            .move_entry("src", "bad\0dst", KvsFlags::NONE, None, None)
            .is_err());
    });
}

// =========================================================================
// Live KVS Commit & Lookup Cycle
// =========================================================================

#[test]
fn commit_and_lookup_raw_data() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        let mut txn = KvsTransaction::new().unwrap();
        let test_key = "test_raw_key";
        let test_val = b"hello_flux_kvs";

        txn.put(test_key, test_val, KvsFlags::NONE).unwrap();
        let mut commit = kvs.commit(&txn, KvsFlags::NONE, None).unwrap();
        assert!(commit.future.wait_for(5.0).unwrap());
        assert!(commit.get_sequence().is_ok());

        let mut lookup = kvs.lookup(test_key, KvsFlags::NONE, None).unwrap();
        assert!(lookup.future.wait_for(5.0).unwrap());

        let fetched_bytes = lookup.get().unwrap();
        assert_eq!(fetched_bytes, test_val);

        let key_opt = lookup.get_key().unwrap();
        assert_eq!(key_opt, Some(test_key));
    });
}

#[test]
fn commit_and_lookup_json_and_deserializable() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        let mut txn = KvsTransaction::new().unwrap();
        let test_key = "test_json_key";
        let data = TestData {
            msg: "kvs_json".to_string(),
            val: 12345,
        };

        txn.put_serializable(test_key, &data, KvsFlags::NONE)
            .unwrap();
        let mut commit = kvs.commit(&txn, KvsFlags::NONE, None).unwrap();
        assert!(commit.future.wait_for(5.0).unwrap());

        let mut lookup = kvs.lookup(test_key, KvsFlags::NONE, None).unwrap();
        assert!(lookup.future.wait_for(5.0).unwrap());

        let json_val = lookup.get_json().unwrap();
        assert_eq!(json_val, json!({"msg": "kvs_json", "val": 12345}));

        let struct_val: TestData = lookup.get_deserializable().unwrap();
        assert_eq!(struct_val, data);
    });
}

#[test]
fn getroot_on_default_namespace_succeeds() {
    with_handle(|h| {
        let mut kvs = Kvs::new(h);
        let mut getroot = kvs.getroot(None).unwrap();
        assert!(getroot.future.wait_for(5.0).unwrap());
        assert!(getroot.get_sequence().is_ok());
        assert!(getroot.get_owner().is_ok());
    });
}
