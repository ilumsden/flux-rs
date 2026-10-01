use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::Arc;
use std::time::Duration;

use convert_case::ccase;
use parking_lot::ReentrantMutex;

use crate::duration::FluxDuration;
use crate::error::{FluxError, Result};
use crate::handle::{OwnedFluxHandle, PollEvents};
use crate::reactor::{FluxReactorThread, OwnedReactor, ReactorFlags};

pub trait AsyncDriver:
    Sized
    + TryFrom<Arc<ReentrantMutex<OwnedFluxHandle>>, Error = FluxError>
    + TryFrom<FluxReactorThread, Error = FluxError>
{
    fn spawn(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;

    fn spawn_async_driver(handle: Arc<ReentrantMutex<OwnedFluxHandle>>) -> Result<Self> {
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

pub trait AsyncWaitableDriver: AsyncDriver {
    fn wait(&mut self) -> impl std::future::Future<Output = Result<()>> + Send;
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

pub(super) enum WaitResult {
    FdReadable,
    Timeout,
}

pub(super) fn get_default_reactor_sleep_duration(driver_name: &str) -> Duration {
    let env_var_name = format!(
        "FLUX_CORE_RS_{}_DRIVER_REACTOR_SLEEP",
        ccase!(constant, driver_name)
    );
    if let Ok(val) = std::env::var(env_var_name) {
        let flux_dur = FluxDuration::from(val.as_str());
        match f64::try_from(flux_dur) {
            Ok(parsed_duration) => return Duration::from_secs_f64(parsed_duration),
            Err(e) => println!("Invalid reactor sleep duration, defaulting to 50ms:\n{e}"),
        }
    }
    Duration::from_millis(50)
}

pub(super) fn get_poll_fd_for_async(
    handle: Arc<ReentrantMutex<OwnedFluxHandle>>,
) -> Result<RawFdWrapper> {
    handle.lock().get_pollfd().map(RawFdWrapper)
}

pub(super) fn process_readable_event_for_async(
    handle: Arc<ReentrantMutex<OwnedFluxHandle>>,
    check_poll_events: bool,
) -> Result<()> {
    let locked_handle = handle.lock();
    // 1. Get reactor. Also, get pollevents if `check_poll_events` is `true`
    let mut reactor = locked_handle.get_reactor()?.to_owned()?;
    let pollevents = check_poll_events
        .then(|| locked_handle.get_pollevents())
        .transpose()?;
    // 2. Tick the reactor ONCE without blocking.
    if let Some(events) = pollevents {
        if (events & PollEvents::POLLIN) != PollEvents::NONE
            || (events & PollEvents::POLLOUT) != PollEvents::NONE
        {
            reactor.run(ReactorFlags::NOWAIT)?;
        }
    } else {
        reactor.run(ReactorFlags::NOWAIT)?;
    }

    // 3. Handle errors
    if let Some(events) = pollevents
        && (events & PollEvents::POLLERR) != PollEvents::NONE
    {
        return Err(FluxError::Logic(
            "Flux handle experienced a POLLERR".to_string(),
        ));
    }

    Ok(())
}
