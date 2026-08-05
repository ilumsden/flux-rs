use std::sync::{Arc, Mutex};

use crate::async_driver::AsyncDriver;
use crate::error::{FluxError, Result};
use crate::handle::FluxHandle;
use crate::reactor::FluxReactorThread;

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

impl TryFrom<Arc<Mutex<FluxHandle>>> for ThreadDriver {
    type Error = FluxError;

    fn try_from(_: Arc<Mutex<FluxHandle>>) -> Result<Self> {
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
