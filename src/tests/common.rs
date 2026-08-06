use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::handle::{FluxHandle, HandleFlags};

// -----------------------------------------------------------------------------
// Shared Flux handle
//
// A single FluxHandle is created on first use and reused for the lifetime of
// the test binary.  This avoids the overhead of opening/closing a handle per
// test and ensures all tests that need a live Flux instance share the same
// connection.
//
// NOTE: adjust `FluxHandle::open()` to whatever your actual constructor is
// called.  If `FluxHandle` is already `Sync`, you can drop the `Mutex` and use
// `OnceLock<FluxHandle>` directly.
// -----------------------------------------------------------------------------
static FLUX_HANDLE: OnceLock<Mutex<FluxHandle>> = OnceLock::new();

/// Acquire a lock on the shared `FluxHandle` and pass a reference to `f`.
///
/// Usage (from any test module in the crate):
/// ```ignore
/// use crate::tests::common::with_handle;
///
/// #[test]
/// fn my_test() {
///     with_handle(|handle| {
///         // use handle here
///     });
/// }
/// ```
pub fn with_handle<F, R>(f: F) -> R
where
    F: FnOnce(&FluxHandle) -> R,
{
    let guard: MutexGuard<FluxHandle> = FLUX_HANDLE
        .get_or_init(|| {
            let h = FluxHandle::new_from_str_uri("", HandleFlags::NONE)
                .expect("Failed to open Flux handle for testing");
            Mutex::new(h)
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    f(&guard)
}
