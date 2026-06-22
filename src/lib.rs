pub mod async_driver;
pub mod duration;
pub mod error;
pub mod future;
pub mod handle;
pub mod hostlist;
pub mod idset;
pub mod job;
pub mod kvs;
pub mod msg;
pub mod reactor;
pub mod request;
pub mod response;
pub mod rpc;
pub mod uri;
pub(crate) mod utils;
pub mod watcher;

pub trait AsRawFluxPtr<PtrType> {
    fn as_flux_ptr(&self) -> *mut PtrType;
}
