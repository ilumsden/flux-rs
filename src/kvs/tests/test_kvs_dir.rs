use crate::error::Result;
use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtr, FromFluxPtr, IntoFluxPtr};
use crate::kvs::flags::KvsFlags;
use crate::kvs::kvs::Kvs;
use crate::kvs::kvs_dir::KvsDir;
use crate::kvs::txn::KvsTransaction;
use crate::tests::common::with_handle;

// =========================================================================
// Live Directory Operations
// =========================================================================

#[test]
fn kvs_dir_read_and_inspect_entries() {
    with_handle(|h| {
        let kvs = Kvs::new(h);
        let mut txn = KvsTransaction::new().unwrap();

        txn.put("test_kvs_dir.file1", b"val1", KvsFlags::NONE)
            .unwrap();
        txn.put("test_kvs_dir.file2", b"val2", KvsFlags::NONE)
            .unwrap();
        let commit = kvs.commit(&txn, KvsFlags::NONE, None).unwrap();
        assert!(commit.future.wait_for(5.0).unwrap());

        let dir = KvsDir::new(h, Some("test_kvs_dir"), None).unwrap();

        assert_eq!(dir.len().unwrap(), 2);
        assert!(dir.contains("file1").unwrap());
        assert!(dir.contains("file2").unwrap());
        assert!(!dir.contains("nonexistent").unwrap());

        assert!(!dir.is_dir("file1").unwrap());
        assert!(!dir.is_symlink("file1").unwrap());

        let key_at = dir.get_key_at("file1").unwrap();
        assert!(key_at.contains("file1"));

        let clone1 = dir.clone();
        assert_eq!(clone1.len().unwrap(), 2);

        let clone2 = dir.try_clone().unwrap();
        assert_eq!(clone2.len().unwrap(), 2);
    });
}

#[test]
fn kvs_dir_cursor_and_iter() {
    with_handle(|h| {
        let kvs = Kvs::new(h);
        let mut txn = KvsTransaction::new().unwrap();

        txn.put("test_cursor_dir.a", b"1", KvsFlags::NONE).unwrap();
        txn.put("test_cursor_dir.b", b"2", KvsFlags::NONE).unwrap();
        let commit = kvs.commit(&txn, KvsFlags::NONE, None).unwrap();
        assert!(commit.future.wait_for(5.0).unwrap());

        let dir = KvsDir::new(h, Some("test_cursor_dir"), None).unwrap();

        // Cursor iteration & reset
        let mut cursor = dir.cursor().unwrap();
        let first_opt = cursor.move_next().unwrap();
        assert!(first_opt.is_some());
        let first = first_opt.unwrap().to_string();
        assert!(!first.is_empty());
        assert_eq!(cursor.current(), first.as_str());

        // Second item
        let second_opt = cursor.move_next().unwrap();
        assert!(second_opt.is_some());

        // EOF returns Ok(None)
        let eof_opt = cursor.move_next().unwrap();
        assert!(eof_opt.is_none());

        // Rewind / reset cursor back to beginning
        cursor.reset();
        let rewind_first = cursor.move_next().unwrap().unwrap().to_string();
        assert_eq!(first, rewind_first);

        // Iter
        let iter = dir.iter().unwrap();
        let items = iter.collect::<Result<Vec<String>>>().unwrap();
        assert_eq!(items.len(), 2);
    });
}

// =========================================================================
// Trait Conversions
// =========================================================================

#[test]
fn pointer_management_traits() {
    with_handle(|h| {
        let kvs = Kvs::new(h);
        let mut txn = KvsTransaction::new().unwrap();
        txn.put("test_ptr_dir.x", b"0", KvsFlags::NONE).unwrap();
        let commit = kvs.commit(&txn, KvsFlags::NONE, None).unwrap();
        assert!(commit.future.wait_for(5.0).unwrap());

        let dir = KvsDir::new(h, Some("test_ptr_dir"), None).unwrap();
        let ptr = dir.as_mut_ptr();

        let borrowed =
            unsafe { KvsDir::borrow_raw(ptr, Some("test_ptr_dir".to_string())) }.unwrap();
        assert!(!borrowed.c_kvsdir.is_owned());

        let raw = dir.into_raw();
        let owned = unsafe { KvsDir::from_raw(raw, Some("test_ptr_dir".to_string())) }.unwrap();
        assert!(owned.c_kvsdir.is_owned());
    });
}
