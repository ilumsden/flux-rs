use std::collections::HashMap;

use flux_sys::core::flux_future_t;

use crate::flux_ptr_management::Owned;
use crate::future::create_wait_all_future;
use crate::job::status::JobStatus;

// =========================================================================
// Helpers
// =========================================================================

/// Create a JobStatus wrapping a dummy empty wait_all future.
fn make_dummy_status() -> JobStatus {
    let future = create_wait_all_future::<Owned<flux_future_t>>(HashMap::new())
        .expect("Failed to create dummy wait_all future");
    JobStatus::from(future)
}

// =========================================================================
// From<FluxFuture>
// =========================================================================

#[test]
fn from_flux_future_creates_job_status() {
    let status = make_dummy_status();
    // Verifies From<FluxFuture> completes without panicking
    assert!(!status.c_future.as_mut_ptr().is_null());
}

// =========================================================================
// Getters on uncached dummy future (error propagation)
// =========================================================================

#[test]
fn get_id_uncached_dummy_future_returns_error() {
    let mut status = make_dummy_status();
    // flux_job_wait_get_status fails on dummy non-job-wait future
    assert!(status.get_id().is_err());
}

#[test]
fn get_success_uncached_dummy_future_returns_error() {
    let mut status = make_dummy_status();
    assert!(status.get_success().is_err());
}

#[test]
fn get_errstr_uncached_dummy_future_returns_error() {
    let mut status = make_dummy_status();
    assert!(status.get_errstr().is_err());
}

// =========================================================================
// Deref & DerefMut
// =========================================================================

#[test]
fn deref_exposes_inner_future_methods() {
    let status = make_dummy_status();
    // Deref allows calling check_error directly on JobStatus
    assert!(status.check_error().is_ok());
}

#[test]
fn deref_mut_allows_mutating_inner_future() {
    let mut status = make_dummy_status();
    // DerefMut allows calling reset directly on JobStatus
    assert!(status.reset().is_ok());
}
