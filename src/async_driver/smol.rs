use std::sync::{Arc, Mutex};

use async_io::Async;
use smol::Task;

use crate::async_driver::base::{
    AsyncDriver, get_poll_fd_for_async, process_readable_event_for_async,
};
use crate::error::{FluxError, Result};
use crate::handle::OwnedFluxHandle;
use crate::reactor::FluxReactorThread;

pub struct SmolDriver {
    pub(crate) handle: Option<Arc<Mutex<OwnedFluxHandle>>>,
    pub(crate) task_handle: Option<Task<Result<()>>>,
}

impl SmolDriver {
    pub(crate) async fn driver_with_reactor_fd(handle: Arc<Mutex<OwnedFluxHandle>>) -> Result<()> {
        let fd = get_poll_fd_for_async(handle.clone())?;
        let async_fd = Async::new(fd)?;

        // Drain any pre-existing events before awaiting I/O readiness
        process_readable_event_for_async(handle.clone())?;

        loop {
            async_fd.readable().await?;
            process_readable_event_for_async(handle.clone())?;
        }
    }
}

impl TryFrom<Arc<Mutex<OwnedFluxHandle>>> for SmolDriver {
    type Error = FluxError;

    fn try_from(value: Arc<Mutex<OwnedFluxHandle>>) -> Result<Self> {
        Ok(Self {
            handle: Some(value),
            task_handle: None,
        })
    }
}

impl TryFrom<FluxReactorThread> for SmolDriver {
    type Error = FluxError;

    fn try_from(_: FluxReactorThread) -> Result<Self> {
        Err(FluxError::Logic(String::from(
            "Cannot create a SmolDriver with a FluxReactorThread. Use Arc<Mutex<FluxHandle>> instead.",
        )))
    }
}

impl AsyncDriver for SmolDriver {
    fn spawn(&mut self) -> Result<()> {
        let flux_handle = self.handle.take().ok_or(FluxError::Logic(String::from(
            "Cannot spawn a driver for async-std when there is no underlying FluxHandle object",
        )))?;
        let handle = smol::spawn(async move { Self::driver_with_reactor_fd(flux_handle).await });
        self.task_handle = Some(handle);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            drop(handle);
        }
        Ok(())
    }
}

impl Drop for SmolDriver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
