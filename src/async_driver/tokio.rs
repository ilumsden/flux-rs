use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::unix::AsyncFd;
use tokio::task::JoinHandle;

use crate::async_driver::base::{
    AsyncDriver, get_default_reactor_sleep_duration, get_poll_fd_for_async,
    process_readable_event_for_async,
};
use crate::error::{FluxError, Result};
use crate::handle::OwnedFluxHandle;
use crate::reactor::FluxReactorThread;

pub struct TokioDriver {
    pub(crate) handle: Option<Arc<Mutex<OwnedFluxHandle>>>,
    pub(crate) reactor_sleep_duration: Duration,
    pub(crate) task_handle: Option<JoinHandle<Result<()>>>,
}

impl TokioDriver {
    pub fn new(handle: Arc<Mutex<OwnedFluxHandle>>) -> Self {
        Self {
            handle: Some(handle),
            reactor_sleep_duration: get_default_reactor_sleep_duration("tokio"),
            task_handle: None,
        }
    }

    pub fn with_sleep_duration(
        handle: Arc<Mutex<OwnedFluxHandle>>,
        sleep_duration: Duration,
    ) -> Self {
        Self {
            handle: Some(handle),
            reactor_sleep_duration: sleep_duration,
            task_handle: None,
        }
    }

    pub(crate) async fn drive_with_reactor_fd(
        handle: Arc<Mutex<OwnedFluxHandle>>,
        reactor_sleep_time: Duration,
    ) -> Result<()> {
        // Get the polling file descriptor
        let fd = get_poll_fd_for_async(handle.clone())?;
        // Wrap the file descriptor into a Tokio AsyncFd
        let async_fd = AsyncFd::new(fd)?;

        // Drain any pre-existing events before awaiting I/O readiness
        process_readable_event_for_async(handle.clone(), false)?;

        loop {
            // Wait until either the pollfd is readable or enough time
            // has passed and then run the reactor
            tokio::select! {
                guard_res = async_fd.readable() => {
                    let mut guard = guard_res?;
                    process_readable_event_for_async(handle.clone(), true)?;
                    guard.clear_ready();
                }
                _ = tokio::time::sleep(reactor_sleep_time) => {
                    process_readable_event_for_async(handle.clone(), false)?;
                }
            }
        }
    }
}

impl TryFrom<Arc<Mutex<OwnedFluxHandle>>> for TokioDriver {
    type Error = FluxError;

    fn try_from(handle: Arc<Mutex<OwnedFluxHandle>>) -> Result<Self> {
        Ok(Self::new(handle))
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
        let reactor_sleep_duration = self.reactor_sleep_duration;
        let handle = tokio::spawn(async move {
            Self::drive_with_reactor_fd(flux_handle, reactor_sleep_duration).await
        });
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
