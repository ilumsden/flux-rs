use std::collections::HashMap;

use flux_sys::core::flux_job_result_t;

use crate::future::create_wait_all_future;
use crate::job::result::{JobResult, JobResultCode};

// =========================================================================
// Helpers
// =========================================================================

/// Create a JobResult wrapping a dummy empty wait_all future.
fn make_dummy_result() -> JobResult {
    let future =
        create_wait_all_future(HashMap::new()).expect("Failed to create dummy wait_all future");
    JobResult::from(future)
}

// =========================================================================
// JobResultCode bitflags sanity & From
// =========================================================================

#[test]
fn job_result_code_none_is_zero() {
    assert_eq!(JobResultCode::NONE.bits(), 0);
}

#[test]
fn job_result_code_flags_are_disjoint() {
    assert_eq!(
        JobResultCode::COMPLETED & JobResultCode::FAILED,
        JobResultCode::NONE
    );
    assert_eq!(
        JobResultCode::CANCELED & JobResultCode::TIMEOUT,
        JobResultCode::NONE
    );
}

#[test]
fn from_flux_job_result_t_converts_correctly() {
    let raw: flux_job_result_t = JobResultCode::COMPLETED.bits();
    let code = JobResultCode::from(raw);
    assert_eq!(code, JobResultCode::COMPLETED);
}

// =========================================================================
// JobResultCode::decode
// =========================================================================

#[test]
fn decode_valid_completed_string_succeeds() {
    let decoded = JobResultCode::decode("COMPLETED").unwrap();
    assert_eq!(decoded, JobResultCode::COMPLETED);
}

#[test]
fn decode_valid_failed_string_succeeds() {
    let decoded = JobResultCode::decode("FAILED").unwrap();
    assert_eq!(decoded, JobResultCode::FAILED);
}

#[test]
fn decode_invalid_string_returns_error() {
    assert!(JobResultCode::decode("NOT_A_VALID_RESULT").is_err());
}

#[test]
fn decode_nul_byte_returns_error() {
    assert!(JobResultCode::decode("COMPLETED\0FAILED").is_err());
}

// =========================================================================
// JobResult Struct & Traits (From<FluxFuture>, Deref, DerefMut)
// =========================================================================

#[test]
fn from_flux_future_creates_job_result() {
    let result = make_dummy_result();
    assert!(!result.c_future.as_mut_ptr().is_null());
}

#[test]
fn deref_exposes_inner_future_methods() {
    let mut result = make_dummy_result();
    // Deref allows calling check_error directly on JobResult
    assert!(result.check_error().is_ok());
}

#[test]
fn deref_mut_allows_mutating_inner_future() {
    let mut result = make_dummy_result();
    // DerefMut allows calling reset directly on JobResult
    assert!(result.reset().is_ok());
}
