use std::sync::{Arc, Mutex};

use tokio::io::unix::AsyncFd;
use tokio::task::JoinHandle;

use crate::async_driver::base::{
    get_poll_fd_for_async, process_readable_event_for_async, AsyncDriver,
};
use crate::error::{FluxError, Result};
use crate::handle::FluxHandle;
use crate::reactor::FluxReactorThread;

pub struct TokioDriver {
    handle: Option<Arc<Mutex<FluxHandle>>>,
    task_handle: Option<JoinHandle<Result<()>>>,
}

impl TokioDriver {
    async fn drive_with_reactor_fd(handle: Arc<Mutex<FluxHandle>>) -> Result<()> {
        // Get the polling file descriptor
        let fd = get_poll_fd_for_async(handle.clone())?;
        // Wrap the file descriptor into a Tokio AsyncFd
        let async_fd = AsyncFd::new(fd)?;
        loop {
            // Yield the thread to Tokio until the file descriptor becomes readable
            let mut guard = async_fd.readable().await?;
            // Process the events by running the reactor without blocking
            process_readable_event_for_async(handle.clone())?;
            // Clear the readiness state so Tokio knows to wait for the next Flux edge-trigger
            guard.clear_ready();
        }
    }
}

impl TryFrom<Arc<Mutex<FluxHandle>>> for TokioDriver {
    type Error = FluxError;

    fn try_from(handle: Arc<Mutex<FluxHandle>>) -> Result<Self> {
        Ok(Self {
            handle: Some(handle),
            task_handle: None,
        })
    }
}

impl TryFrom<FluxReactorThread> for TokioDriver {
    type Error = FluxError;

    fn try_from(_: FluxReactorThread) -> Result<Self> {
        Err(FluxError::Logic(String::from(
            "Cannot create a TokioDriver with a FluxReactorThread. Use Arc<Mutex<FluxHandle>> instead.",
        )))
    }
}

impl AsyncDriver for TokioDriver {
    fn spawn(&mut self) -> Result<()> {
        let flux_handle = self.handle.take().ok_or(FluxError::Logic(String::from(
            "Cannot spawn a driver for Tokio when there is no underlying FluxHandle object",
        )))?;
        let handle = tokio::spawn(async move { Self::drive_with_reactor_fd(flux_handle).await });
        self.task_handle = Some(handle);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
        Ok(())
    }
}

impl Drop for TokioDriver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
