#![allow(clippy::uninlined_format_args)]

pub mod async_driver;
pub mod duration;
pub mod error;
pub(crate) mod flux_ptr_management;
pub mod future;
pub mod handle;
pub mod hostlist;
pub mod idset;
pub mod job;
pub mod kvs;
pub mod module;
pub mod msg;
pub mod msg_handler;
pub mod plugin;
pub mod reactor;
pub mod request;
pub mod response;
pub mod rpc;
pub mod service;
pub mod uri;
pub(crate) mod utils;

#[cfg(feature = "jobtap")]
pub mod jobtap;

#[cfg(test)]
pub(crate) mod tests;

// Re-export the entire flux_sys crate so it can be used in
// downstream crates (e.g., via the flux_core::module::create_module_entrypoint macro)
#[doc(hidden)]
pub use flux_sys;

pub use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtr, FromFluxPtrNoArgs, IntoFluxPtr,
};

pub use crate::utils::SignalCode;
