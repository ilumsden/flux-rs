use std::sync::{Arc, Mutex};

use crate::async_driver::AsyncDriver;
use crate::error::{FluxError, Result};
use crate::handle::OwnedFluxHandle;
use crate::reactor::FluxReactorThread;

/// An async driver intended for actor architectures.
///
/// # Safety
///
/// This driver can easily cause data races and undefined behavior if
/// other threads are using the same `Reactor` or if those threads are using
/// the `FluxHandle` used to create/obtain the `Reactor`. This driver is
/// mainly intended to be used in actor architectures where Flux events
/// are handled by reactor callbacks (e.g., via `MessageHandler`), and those
/// callbacks dispatch other, non-Flux related work to an async runtime
/// via channels, message passing, etc.
pub struct ThreadDriver {
    pub(crate) driver: Option<FluxReactorThread>,
}

impl TryFrom<FluxReactorThread> for ThreadDriver {
    type Error = FluxError;

    fn try_from(driver: FluxReactorThread) -> Result<Self> {
        Ok(Self {
            driver: Some(driver),
        })
    }
}

impl TryFrom<Arc<Mutex<OwnedFluxHandle>>> for ThreadDriver {
    type Error = FluxError;

    fn try_from(_: Arc<Mutex<OwnedFluxHandle>>) -> Result<Self> {
        Err(FluxError::Logic(String::from(
            "ThreadDriver does not use file descriptor polling",
        )))
    }
}

impl AsyncDriver for ThreadDriver {
    fn spawn(&mut self) -> Result<()> {
        if let Some(driver) = self.driver.as_mut() {
            driver.spawn()?;
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(mut driver) = self.driver.take() {
            driver.stop()?;
        }
        Ok(())
    }
}

impl Drop for ThreadDriver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
