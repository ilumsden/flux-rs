pub mod error;
pub mod future;
pub mod handle;
pub mod kvs;
pub mod msg;
pub mod reactor;
pub mod response;
pub mod rpc;
pub mod watcher;

pub trait AsRawFluxPtr<PtrType> {
    fn as_flux_ptr(&self) -> *mut PtrType;
}
