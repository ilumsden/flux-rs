use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::{Arc, Mutex};

use crate::error::{FluxError, Result};
use crate::handle::{OwnedFluxHandle, PollEvents};
use crate::reactor::{FluxReactorThread, OwnedReactor, ReactorFlags};

pub trait AsyncDriver:
    Sized
    + TryFrom<Arc<Mutex<OwnedFluxHandle>>, Error = FluxError>
    + TryFrom<FluxReactorThread, Error = FluxError>
{
    fn spawn(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;

    fn spawn_async_driver(handle: Arc<Mutex<OwnedFluxHandle>>) -> Result<Self> {
        let mut driver = Self::try_from(handle)?;
        driver.spawn()?;
        Ok(driver)
    }

    fn spawn_reactor_thread(reactor: OwnedReactor) -> Result<Self> {
        let flux_reactor_thread = FluxReactorThread::new(reactor);
        let mut driver = Self::try_from(flux_reactor_thread)?;
        driver.spawn()?;
        Ok(driver)
    }
}

pub struct RawFdWrapper(pub RawFd);

impl AsRawFd for RawFdWrapper {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl AsFd for RawFdWrapper {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.0) }
    }
}

pub(super) fn get_poll_fd_for_async(handle: Arc<Mutex<OwnedFluxHandle>>) -> Result<RawFdWrapper> {
    handle
        .lock()
        .map_err(|_| {
            FluxError::Logic(String::from(
                "Cannot get polling file descriptor because the mutex is poisoned",
            ))
        })?
        .get_pollfd()
        .map(RawFdWrapper)
}

pub(super) fn process_readable_event_for_async(handle: Arc<Mutex<OwnedFluxHandle>>) -> Result<()> {
    let locked_handle = handle.lock().map_err(|_| {
        FluxError::Logic(String::from(
            "Cannot get a reactor and pollevents because the mutex is poisoned",
        ))
    })?;
    // 1. Get events and clear the edge-triggered POLLIN state
    let events = locked_handle.get_pollevents()?;
    let mut reactor = locked_handle.get_reactor()?;
    // 2. If there is data to read, tick the reactor ONCE without blocking
    if (events & PollEvents::POLLIN) != PollEvents::NONE
        || (events & PollEvents::POLLOUT) != PollEvents::NONE
    {
        reactor.run(ReactorFlags::NOWAIT)?;
    }

    // 3. Handle errors
    if (events & PollEvents::POLLERR) != PollEvents::NONE {
        return Err(FluxError::Logic(
            "Flux handle experienced a POLLERR".to_string(),
        ));
    }

    Ok(())
}
