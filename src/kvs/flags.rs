use bitflags::bitflags;
use flux_sys::core::kvs_op::{
    FLUX_KVS_APPEND, FLUX_KVS_READDIR, FLUX_KVS_READLINK, FLUX_KVS_TREEOBJ, FLUX_KVS_WAITCREATE,
    FLUX_KVS_WATCH, FLUX_KVS_WATCH_APPEND, FLUX_KVS_WATCH_FULL, FLUX_KVS_WATCH_UNIQ,
};

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct KvsFlags: u32 {
        const NONE = 0;
        const APPEND = FLUX_KVS_APPEND;
        const READDIR = FLUX_KVS_READDIR;
        const READLINK = FLUX_KVS_READLINK;
        const TREEOBJ = FLUX_KVS_TREEOBJ;
        const WAITCREATE = FLUX_KVS_WAITCREATE;
        const WATCH = FLUX_KVS_WATCH;
        const WATCH_APPEND = FLUX_KVS_WATCH_APPEND;
        const WATCH_FULL = FLUX_KVS_WATCH_FULL;
        const WATCH_UNIQ = FLUX_KVS_WATCH_UNIQ;
    }
}
