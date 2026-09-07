use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use async_io::Async;
use smol::channel::{Receiver, Sender, bounded};
use smol::{Executor, Task};

use crate::async_driver::base::{
    AsyncDriver, get_poll_fd_for_async, process_readable_event_for_async,
};
use crate::error::{FluxError, Result};
use crate::handle::OwnedFluxHandle;
use crate::reactor::FluxReactorThread;

pub struct SmolDriver {
    pub(crate) handle: Option<Arc<Mutex<OwnedFluxHandle>>>,
    executor: Arc<Executor<'static>>,
    /// True if this driver created its own private `Executor` (via `new`),
    /// in which case it is also responsible for running it on a dedicated
    /// background thread. False if the executor was supplied by the caller
    /// (via `with_executor`), in which case the caller is responsible for
    /// actually driving it - `SmolDriver` will only ever spawn a task onto
    /// it, never a thread.
    owns_executor: bool,
    pub(crate) task_handle: Option<Task<Result<()>>>,
    /// Present only when `owns_executor` is true and `spawn()` has run:
    /// the background thread driving the private executor, plus a sender
    /// used to signal it to stop.
    owned_runner: Option<(JoinHandle<()>, Sender<()>)>,
}

impl SmolDriver {
    /// Creates a driver that will own a private `Executor` and a dedicated
    /// background thread to run it, created when `spawn()` is called. This
    /// is the path used by `AsyncDriver::spawn_async_driver` and gives each
    /// `SmolDriver` instance its own isolated executor, with no state shared
    /// between separate instances.
    pub fn new(handle: Arc<Mutex<OwnedFluxHandle>>) -> Result<Self> {
        Ok(Self {
            handle: Some(handle),
            executor: Arc::new(Executor::new()),
            owns_executor: true,
            task_handle: None,
            owned_runner: None,
        })
    }

    /// Creates a driver that spawns its polling task onto a caller-supplied
    /// `Executor` instead of a private one.
    ///
    /// The caller remains fully responsible for actually running this
    /// executor (e.g. via their own `smol::block_on(executor.run(..))` loop
    /// on some thread) for the lifetime of the driver - `SmolDriver` will
    /// not spawn a background thread for it, and the polling task will
    /// simply never make progress if nothing ever ticks the executor.
    pub fn with_executor(
        handle: Arc<Mutex<OwnedFluxHandle>>,
        executor: Arc<Executor<'static>>,
    ) -> Result<Self> {
        Ok(Self {
            handle: Some(handle),
            executor,
            owns_executor: false,
            task_handle: None,
            owned_runner: None,
        })
    }

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

    /// Returns a reference to the `Executor` driving this instance's polling
    /// task - either the private one created by `new`, or the one supplied
    /// via `with_executor`.
    pub fn get_executor(&self) -> &Executor<'static> {
        &self.executor
    }
}

impl TryFrom<Arc<Mutex<OwnedFluxHandle>>> for SmolDriver {
    type Error = FluxError;

    fn try_from(value: Arc<Mutex<OwnedFluxHandle>>) -> Result<Self> {
        SmolDriver::new(value)
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

        let task = self
            .executor
            .spawn(async move { Self::driver_with_reactor_fd(flux_handle).await });
        self.task_handle = Some(task);

        // Only spin up a dedicated background thread when we privately own
        // the executor - a caller-supplied executor (via `with_executor`) is
        // the caller's responsibility to drive.
        if self.owns_executor && self.owned_runner.is_none() {
            let executor = self.executor.clone();
            let (tx, rx): (Sender<()>, Receiver<()>) = bounded(1);
            let thread = std::thread::Builder::new()
                .name("smol-driver".into())
                .spawn(move || {
                    smol::block_on(executor.run(async {
                        let _ = rx.recv().await;
                    }));
                })
                .map_err(|e| {
                    FluxError::Logic(format!("Failed to spawn smol driver thread: {e}"))
                })?;
            self.owned_runner = Some((thread, tx));
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
