mod async_future;
mod sync_future;

pub use async_future::AsyncFluxFuture;
pub use sync_future::{create_wait_all_future, create_wait_any_future, FluxFuture};
