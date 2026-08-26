use std::collections::HashMap;

use flux_sys::core::flux_future_t;

use crate::flux_ptr_management::Owned;
use crate::future::async_future::AsyncFluxFuture;
use crate::future::sync_future::{FluxFuture, create_wait_all_future, create_wait_any_future};
use crate::tests::common::with_handle;

// =========================================================================
// Helpers
// =========================================================================

fn make_wait_all() -> FluxFuture {
    create_wait_all_future::<Owned<flux_future_t>>(HashMap::new())
        .expect("Failed to create empty wait_all future")
}

fn make_wait_any() -> FluxFuture {
    create_wait_any_future::<Owned<flux_future_t>>(HashMap::new())
        .expect("Failed to create empty wait_any future")
}

// =========================================================================
// AsyncFluxFuture::new
//
// flux_future_then requires a reactor to be attached to the future.
// All construction tests use with_handle to obtain a live reactor and call
// set_reactor before constructing AsyncFluxFuture.
//
// These tests only verify construction — they do NOT await the future.
// Awaiting requires a live reactor thread; see the tokio/smol sections below.
// =========================================================================

#[test]
fn new_with_wait_all_future_succeeds() {
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        let mut future = make_wait_all();
        future.set_reactor(&reactor).unwrap();
        assert!(AsyncFluxFuture::new(future).is_ok());
    });
}

#[test]
fn new_with_wait_any_future_succeeds() {
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        let mut future = make_wait_any();
        future.set_reactor(&reactor).unwrap();
        assert!(AsyncFluxFuture::new(future).is_ok());
    });
}

#[test]
fn new_with_sync_future_new_succeeds() {
    // FluxFuture::new produces a future with an init callback — verify
    // AsyncFluxFuture can wrap it without error.
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        let mut future = FluxFuture::new(|_| {}).unwrap();
        future.set_reactor(&reactor).unwrap();
        assert!(AsyncFluxFuture::new(future).is_ok());
    });
}

#[test]
fn new_returns_ok_for_to_owned_future() {
    // to_owned() upgrades lifetime to 'static — the resulting future must
    // also be wrappable.
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        let base = make_wait_all();
        let mut owned = base.to_owned().unwrap();
        owned.set_reactor(&reactor).unwrap();
        assert!(AsyncFluxFuture::new(owned).is_ok());
    });
}

#[test]
fn new_returns_ok_for_cloned_future() {
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        let base = make_wait_all();
        let mut cloned = base.clone();
        cloned.set_reactor(&reactor).unwrap();
        // Both the original and clone can be independently wrapped.
        let mut base2 = make_wait_all();
        base2.set_reactor(&reactor).unwrap();
        assert!(AsyncFluxFuture::new(base2).is_ok());
        assert!(AsyncFluxFuture::new(cloned).is_ok());
    });
}

// =========================================================================
// Drop safety
//
// Dropping an AsyncFluxFuture before the callback fires must not panic or
// corrupt memory. The c_callback holds an Arc clone, so dropping _inner
// only decrements the Rust-side Arc; the C-side Arc is reclaimed in the
// callback if it fires later, or leaked if it never fires.
// =========================================================================

#[test]
fn drop_without_await_does_not_panic() {
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        let mut future = make_wait_all();
        future.set_reactor(&reactor).unwrap();
        let _af = AsyncFluxFuture::new(future).unwrap();
        // _af dropped here
    });
}

#[test]
fn drop_wait_any_without_await_does_not_panic() {
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        let mut future = make_wait_any();
        future.set_reactor(&reactor).unwrap();
        let _af = AsyncFluxFuture::new(future).unwrap();
    });
}

#[test]
fn multiple_constructions_do_not_panic() {
    // Construct and drop several AsyncFluxFuture objects to verify no
    // global state is corrupted between constructions.
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        for _ in 0..4 {
            let mut future = make_wait_all();
            future.set_reactor(&reactor).unwrap();
            let _af = AsyncFluxFuture::new(future).unwrap();
        }
    });
}

// =========================================================================
// Tokio integration tests
//
// These tests require the "tokio" feature and use TokioDriver to drive
// the Flux reactor via the polling file descriptor rather than a
// dedicated reactor thread.
// =========================================================================

#[cfg(feature = "tokio")]
mod tokio_tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use crate::async_driver::AsyncDriver;
    use crate::async_driver::TokioDriver;
    use crate::future::BorrowedFluxFuture;
    use crate::handle::{FluxHandle, HandleFlags};

    fn open_shared_handle() -> Arc<Mutex<FluxHandle>> {
        Arc::new(Mutex::new(
            FluxHandle::new_from_str_uri("", HandleFlags::NONE)
                .expect("Failed to open FluxHandle for AsyncFluxFuture tokio test"),
        ))
    }

    // ---- Construction within a Tokio runtime (no await) ----

    #[tokio::test]
    async fn new_succeeds_in_tokio_context() {
        with_handle(|h| {
            let reactor = h.get_reactor().unwrap();
            let mut future = make_wait_all();
            future.set_reactor(&reactor).unwrap();
            assert!(AsyncFluxFuture::new(future).is_ok());
        });
    }

    #[tokio::test]
    async fn drop_in_tokio_context_does_not_panic() {
        with_handle(|h| {
            let reactor = h.get_reactor().unwrap();
            let mut future = make_wait_all();
            future.set_reactor(&reactor).unwrap();
            let _af = AsyncFluxFuture::new(future).unwrap();
            // dropped here inside a tokio task — no await needed
        });
    }

    // ---- Actual await tests with TokioDriver ----

    /// An empty wait_all is immediately fulfilled. With TokioDriver running,
    /// the Flux polling fd becomes readable, the driver ticks the reactor via
    /// process_readable_event_for_async, the c_callback fires and wakes the
    /// Tokio task, causing poll() to return Poll::Ready.
    #[tokio::test]
    async fn await_empty_wait_all_completes_with_tokio_driver() {
        let shared = open_shared_handle();
        let mut driver = TokioDriver::spawn_async_driver(shared.clone()).unwrap();

        let mut future = make_wait_all();
        {
            let handle = shared.lock().unwrap();
            let reactor = handle.get_reactor().unwrap();
            future.set_reactor(&reactor).unwrap();
        }

        let af = AsyncFluxFuture::new(future).unwrap();

        let result = tokio::time::timeout(Duration::from_secs(10), af).await;

        driver.stop().unwrap();
        assert!(
            result.is_ok(),
            "AsyncFluxFuture (wait_all) did not complete within 5 s"
        );
    }

    /// A FluxFuture created with FluxFuture::new and an init callback that
    /// immediately fulfills the future should complete when awaited.
    /// The callback now receives &mut FluxFuture so it can call fulfill directly.
    #[tokio::test]
    async fn await_flux_future_new_completes_with_tokio_driver() {
        let shared = open_shared_handle();
        let mut driver = TokioDriver::spawn_async_driver(shared.clone()).unwrap();

        let mut future = FluxFuture::new(|f: &mut BorrowedFluxFuture<'_>| {
            let _ = f.fulfill(None::<Box<()>>);
        })
        .unwrap();
        {
            let handle = shared.lock().unwrap();
            let reactor = handle.get_reactor().unwrap();
            future.set_reactor(&reactor).unwrap();
        }

        let af = AsyncFluxFuture::new(future).unwrap();

        let result = tokio::time::timeout(Duration::from_secs(10), af).await;

        driver.stop().unwrap();
        assert!(
            result.is_ok(),
            "AsyncFluxFuture (FluxFuture::new) did not complete within 5 s"
        );
    }

    #[tokio::test]
    async fn drop_before_completion_with_tokio_driver_does_not_panic() {
        let shared = open_shared_handle();
        let mut driver = TokioDriver::spawn_async_driver(shared.clone()).unwrap();

        let mut future = make_wait_all();
        {
            let handle = shared.lock().unwrap();
            let reactor = handle.get_reactor().unwrap();
            future.set_reactor(&reactor).unwrap();
        }

        {
            let _af = AsyncFluxFuture::new(future).unwrap();
            // dropped here without awaiting
        }

        driver.stop().unwrap();
    }
}

// =========================================================================
// Smol integration tests
//
// These tests require the "smol" feature and use SmolDriver to drive
// the Flux reactor via the polling file descriptor.
// smol has no #[smol::test] attribute, so plain #[test] + smol::block_on
// is used for await tests, with a thread + channel timeout for safety.
// =========================================================================

#[cfg(feature = "smol")]
mod smol_tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use crate::async_driver::AsyncDriver;
    use crate::async_driver::SmolDriver;
    use crate::future::BorrowedFluxFuture;
    use crate::handle::{FluxHandle, HandleFlags};

    fn open_shared_handle() -> Arc<Mutex<FluxHandle>> {
        Arc::new(Mutex::new(
            FluxHandle::new_from_str_uri("", HandleFlags::NONE)
                .expect("Failed to open FluxHandle for AsyncFluxFuture smol test"),
        ))
    }

    // ---- Construction within smol context (no await) ----

    #[test]
    fn new_succeeds_in_smol_context() {
        with_handle(|h| {
            let reactor = h.get_reactor().unwrap();
            let mut future = make_wait_all();
            future.set_reactor(&reactor).unwrap();
            assert!(AsyncFluxFuture::new(future).is_ok());
        });
    }

    #[test]
    fn drop_in_smol_context_does_not_panic() {
        with_handle(|h| {
            let reactor = h.get_reactor().unwrap();
            let mut future = make_wait_all();
            future.set_reactor(&reactor).unwrap();
            let _af = AsyncFluxFuture::new(future).unwrap();
            // dropped here — no await
        });
    }

    // ---- Actual await tests with SmolDriver ----

    /// Runs `f` inside `smol::block_on` on a dedicated thread and asserts it
    /// completes within 5 seconds. This provides a timeout for smol await tests
    /// since smol has no built-in timeout primitive.
    fn run_with_timeout<F, T>(f: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = f();
            let _ = tx.send(result);
        });
        rx.recv_timeout(Duration::from_secs(5))
            .expect("smol await test did not complete within 5 s")
    }

    #[test]
    fn await_empty_wait_all_completes_with_smol_driver() {
        run_with_timeout(|| {
            let shared = open_shared_handle();
            let mut driver = SmolDriver::spawn_async_driver(shared.clone()).unwrap();

            let mut future = make_wait_all();
            {
                let handle = shared.lock().unwrap();
                let reactor = handle.get_reactor().unwrap();
                future.set_reactor(&reactor).unwrap();
            }

            let af = AsyncFluxFuture::new(future).unwrap();

            smol::block_on(af);

            driver.stop().unwrap();
        });
    }

    /// A FluxFuture created with FluxFuture::new and an init callback that
    /// immediately fulfills the future should complete when awaited.
    /// The callback now receives &mut FluxFuture so it can call fulfill directly.
    #[test]
    fn await_flux_future_new_completes_with_smol_driver() {
        run_with_timeout(|| {
            let shared = open_shared_handle();
            let mut driver = SmolDriver::spawn_async_driver(shared.clone()).unwrap();

            let mut future = FluxFuture::new(|f: &mut BorrowedFluxFuture<'_>| {
                let _ = f.fulfill(None::<Box<()>>);
            })
            .unwrap();
            {
                let handle = shared.lock().unwrap();
                let reactor = handle.get_reactor().unwrap();
                future.set_reactor(&reactor).unwrap();
            }

            let af = AsyncFluxFuture::new(future).unwrap();

            smol::block_on(af);

            driver.stop().unwrap();
        });
    }

    #[test]
    fn drop_before_completion_with_smol_driver_does_not_panic() {
        let shared = open_shared_handle();
        let mut driver = SmolDriver::spawn_async_driver(shared.clone()).unwrap();

        let mut future = make_wait_all();
        {
            let handle = shared.lock().unwrap();
            let reactor = handle.get_reactor().unwrap();
            future.set_reactor(&reactor).unwrap();
        }

        {
            let _af = AsyncFluxFuture::new(future).unwrap();
            // dropped here without awaiting
        }

        driver.stop().unwrap();
    }
}
