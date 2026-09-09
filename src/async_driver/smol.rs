use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use async_io::Async;
use smol::channel::{Receiver, Sender, bounded};
use smol::future;
use smol::{Executor, Task, Timer};

use crate::async_driver::base::{
    AsyncDriver, WaitResult, get_default_reactor_sleep_duration, get_poll_fd_for_async,
    process_readable_event_for_async,
};
use crate::error::{FluxError, Result};
use crate::handle::OwnedFluxHandle;
use crate::reactor::FluxReactorThread;

pub struct SmolDriver {
    pub(crate) handle: Option<Arc<Mutex<OwnedFluxHandle>>>,
    pub(crate) reactor_sleep_duration: Duration,
    executor: Arc<Executor<'static>>,
    owns_executor: bool,
    pub(crate) task_handle: Option<Task<Result<()>>>,
    owned_runner: Option<(JoinHandle<()>, Sender<()>)>,
}

impl SmolDriver {
    pub fn new(handle: Arc<Mutex<OwnedFluxHandle>>) -> Self {
        Self {
            handle: Some(handle),
            reactor_sleep_duration: get_default_reactor_sleep_duration("smol"),
            executor: Arc::new(Executor::new()),
            owns_executor: true,
            task_handle: None,
            owned_runner: None,
        }
    }

    pub fn with_sleep_duration(
        handle: Arc<Mutex<OwnedFluxHandle>>,
        sleep_duration: Duration,
    ) -> Self {
        Self {
            handle: Some(handle),
            reactor_sleep_duration: sleep_duration,
            executor: Arc::new(Executor::new()),
            owns_executor: true,
            task_handle: None,
            owned_runner: None,
        }
    }

    pub fn with_executor(
        handle: Arc<Mutex<OwnedFluxHandle>>,
        executor: Arc<Executor<'static>>,
    ) -> Self {
        Self {
            handle: Some(handle),
            reactor_sleep_duration: get_default_reactor_sleep_duration("smol"),
            executor,
            owns_executor: false,
            task_handle: None,
            owned_runner: None,
        }
    }

    pub fn with_executor_and_sleep_duration(
        handle: Arc<Mutex<OwnedFluxHandle>>,
        executor: Arc<Executor<'static>>,
        sleep_duration: Duration,
    ) -> Self {
        Self {
            handle: Some(handle),
            reactor_sleep_duration: sleep_duration,
            executor,
            owns_executor: false,
            task_handle: None,
            owned_runner: None,
        }
    }

    pub(crate) async fn driver_with_reactor_fd(
        handle: Arc<Mutex<OwnedFluxHandle>>,
        started: Option<Sender<()>>,
        reactor_sleep_time: Duration,
    ) -> Result<()> {
        let fd = get_poll_fd_for_async(handle.clone())?;
        let async_fd = Async::new(fd)?;

        // Drain any pre-existing events before awaiting I/O readiness
        process_readable_event_for_async(handle.clone(), false)?;

        if let Some(started) = started {
            // Best-effort: if the receiver already gave up (e.g. spawn()
            // timed out waiting), there's nothing more to do here.
            let _ = started.try_send(());
        }

        loop {
            // Wait until either the pollfd is readable or enough time
            // has passed and then run the reactor
            eprintln!("[smol-driver] waiting for readable...");
            let await_branch = future::or(
                async {
                    async_fd.readable().await?;
                    Ok::<_, std::io::Error>(WaitResult::FdReadable)
                },
                async {
                    Timer::after(reactor_sleep_time).await;
                    Ok::<_, std::io::Error>(WaitResult::Timeout)
                },
            )
            .await?;
            eprintln!("[smol-driver] got readable, processing");
            process_readable_event_for_async(
                handle.clone(),
                matches!(await_branch, WaitResult::FdReadable),
            )?;
            eprintln!("[smol-driver] processed");
        }
    }

    pub fn get_executor(&self) -> &Executor<'static> {
        &self.executor
    }
}

impl TryFrom<Arc<Mutex<OwnedFluxHandle>>> for SmolDriver {
    type Error = FluxError;

    fn try_from(value: Arc<Mutex<OwnedFluxHandle>>) -> Result<Self> {
        Ok(SmolDriver::new(value))
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
            "Cannot spawn a driver for smol when there is no underlying FluxHandle object",
        )))?;

        // Signaled once the driver task has been polled for the first time
        // and has completed its pre-drain - only used when we're about to
        // spin up a fresh thread, since that's the only case where a
        // "hasn't started yet" race is possible.
        let (started_tx, started_rx) = bounded::<()>(1);
        let started_tx = self.owns_executor.then_some(started_tx);
        let sleep_duration = self.reactor_sleep_duration;

        let task = self.executor.spawn(async move {
            Self::driver_with_reactor_fd(flux_handle, started_tx, sleep_duration).await
        });
        self.task_handle = Some(task);

        if self.owns_executor && self.owned_runner.is_none() {
            let executor = self.executor.clone();
            let (stop_tx, stop_rx): (Sender<()>, Receiver<()>) = bounded(1);
            let thread = std::thread::Builder::new()
                .name("smol-driver".into())
                .spawn(move || {
                    smol::block_on(executor.run(async {
                        let _ = stop_rx.recv().await;
                    }));
                })
                .map_err(|e| {
                    FluxError::Logic(format!("Failed to spawn smol driver thread: {e}"))
                })?;

            // Block until the driver task has actually been polled once,
            // closing the race where a very-fast-completing future finishes
            // before the new background thread has even started ticking
            // the executor.
            started_rx.recv_blocking().map_err(|_| {
                FluxError::Logic(String::from(
                    "smol driver thread exited before completing startup",
                ))
            })?;

            self.owned_runner = Some((thread, stop_tx));
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            drop(handle);
        }
        if let Some((thread, tx)) = self.owned_runner.take() {
            let _ = tx.try_send(());
            let _ = thread.join();
        }
        Ok(())
    }
}

impl Drop for SmolDriver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
