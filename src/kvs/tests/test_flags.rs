use crate::kvs::flags::KvsFlags;

// =========================================================================
// KvsFlags Constants & Sanity
// =========================================================================

#[test]
fn kvs_flags_none_is_zero() {
    assert_eq!(KvsFlags::NONE.bits(), 0);
}

#[test]
fn kvs_flags_individual_bits_are_nonzero() {
    assert_ne!(KvsFlags::APPEND.bits(), 0);
    assert_ne!(KvsFlags::READDIR.bits(), 0);
    assert_ne!(KvsFlags::READLINK.bits(), 0);
    assert_ne!(KvsFlags::TREEOBJ.bits(), 0);
    assert_ne!(KvsFlags::WAITCREATE.bits(), 0);
    assert_ne!(KvsFlags::WATCH.bits(), 0);
    assert_ne!(KvsFlags::WATCH_APPEND.bits(), 0);
    assert_ne!(KvsFlags::WATCH_FULL.bits(), 0);
    assert_ne!(KvsFlags::WATCH_UNIQ.bits(), 0);
}

#[test]
fn kvs_flags_combination_contains_members() {
    let combined = KvsFlags::APPEND | KvsFlags::WATCH;
    assert!(combined.contains(KvsFlags::APPEND));
    assert!(combined.contains(KvsFlags::WATCH));
    assert!(!combined.contains(KvsFlags::READDIR));
}

#[test]
fn kvs_flags_copy_and_clone() {
    let flags = KvsFlags::TREEOBJ | KvsFlags::READDIR;
    let copied = flags;
    assert_eq!(flags, copied);
}
