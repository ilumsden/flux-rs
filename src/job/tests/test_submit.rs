use crate::job::jobspec::JobspecV1;
use crate::job::submit::{submit, submit_async, JobSubmitFlags};
use crate::job::urgency::JobUrgency;
use crate::tests::common::with_handle;

// =========================================================================
// Helpers
// =========================================================================

fn make_test_jobspec() -> JobspecV1 {
    JobspecV1::per_resource(["true"])
        .ncores(1)
        .build_jobspec()
        .expect("Failed to build test jobspec")
}

// =========================================================================
// JobSubmitFlags
// =========================================================================

#[test]
fn submit_flags_bitflags_sanity() {
    assert_ne!(JobSubmitFlags::WAITABLE.bits(), 0);
    assert_ne!(JobSubmitFlags::DEBUG.bits(), 0);
    assert_ne!(JobSubmitFlags::PRE_SIGNED.bits(), 0);
    assert_ne!(JobSubmitFlags::NOVALIDATE.bits(), 0);

    let combined = JobSubmitFlags::WAITABLE | JobSubmitFlags::DEBUG;
    assert!(combined.contains(JobSubmitFlags::WAITABLE));
    assert!(combined.contains(JobSubmitFlags::DEBUG));
}

// =========================================================================
// submit_async & submit
// =========================================================================

#[test]
fn submit_async_returns_valid_future() {
    with_handle(|h| {
        let js = make_test_jobspec();
        let future = submit_async(h, &js, None, None);
        assert!(future.is_ok());
    });
}

#[test]
fn submit_async_with_urgency_and_flags_succeeds() {
    with_handle(|h| {
        let js = make_test_jobspec();
        let future = submit_async(
            h,
            &js,
            Some(JobUrgency::DEFAULT),
            Some(JobSubmitFlags::WAITABLE),
        );
        assert!(future.is_ok());
    });
}

#[test]
fn submit_returns_valid_job() {
    with_handle(|h| {
        let js = make_test_jobspec();
        let job_res = submit(
            h,
            &js,
            Some(JobUrgency::DEFAULT),
            Some(JobSubmitFlags::WAITABLE),
        );
        assert!(job_res.is_ok());
        let job = job_res.unwrap();
        assert_ne!(**job.get_id(), 0);
    });
}
