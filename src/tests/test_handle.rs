use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::FluxError;
use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtrNoArgs, IntoFluxPtr};
use crate::handle::{FluxHandle, HandleFlags, LogLevel, PollEvents};
use crate::msg::MessageMatch;
use crate::reactor::Reactor;
use crate::request::Request;
use crate::tests::common::with_handle;
use crate::{flux_log, flux_log_debug, flux_log_error, flux_log_info, flux_log_warning};

// =========================================================================
// Helpers
// =========================================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TestAux {
    value: u32,
    label: String,
}

/// Open a fresh independent handle for tests that need mutation.
/// Mutation tests (set_aux, set_reactor, set_log_appname, etc.) require
/// &mut FluxHandle, which the shared handle in common.rs does not provide.
/// Opening a fresh handle per test also keeps shared handle state clean.
fn open_fresh() -> FluxHandle {
    FluxHandle::new_from_str_uri("", HandleFlags::NONE)
        .expect("Failed to open a fresh Flux handle for testing")
}

/// A synthetically encoded request with no broker routing information.
/// Suitable for testing respond* argument plumbing but not end-to-end delivery.
fn make_request() -> Request {
    Request::encode("test.topic", b"hello").expect("Failed to encode request")
}

// =========================================================================
// HandleFlags bitflag sanity
// =========================================================================

#[test]
fn handle_flags_none_is_zero() {
    assert_eq!(HandleFlags::NONE.bits(), 0);
}

#[test]
fn handle_flags_trace_is_nonzero() {
    assert_ne!(HandleFlags::TRACE.bits(), 0);
}

#[test]
fn handle_flags_nonblock_is_nonzero() {
    assert_ne!(HandleFlags::NONBLOCK.bits(), 0);
}

#[test]
fn handle_flags_combination_contains_both_members() {
    let combined = HandleFlags::TRACE | HandleFlags::MATCHDEBUG;
    assert!(combined.contains(HandleFlags::TRACE));
    assert!(combined.contains(HandleFlags::MATCHDEBUG));
}

#[test]
fn handle_flags_members_are_disjoint() {
    // No two distinct non-NONE flags should share bits.
    assert_eq!(
        HandleFlags::TRACE & HandleFlags::NONBLOCK,
        HandleFlags::NONE
    );
    assert_eq!(
        HandleFlags::CLONE & HandleFlags::RPCTRACK,
        HandleFlags::NONE
    );
}

// =========================================================================
// LogLevel ordering
// =========================================================================

#[test]
fn log_level_emergency_is_less_than_debug() {
    // #[derive(PartialOrd)] uses declaration order:
    // Emergency(0) < Alert(1) < ... < Debug(7).
    assert!(LogLevel::Emergency < LogLevel::Debug);
}

#[test]
fn log_level_error_is_less_than_info() {
    assert!(LogLevel::Error < LogLevel::Info);
}

#[test]
fn log_level_warning_is_between_error_and_notice() {
    assert!(LogLevel::Error < LogLevel::Warning);
    assert!(LogLevel::Warning < LogLevel::Notice);
}

#[test]
fn log_level_critical_is_less_than_error() {
    assert!(LogLevel::Critical < LogLevel::Error);
}

// =========================================================================
// PollEvents bitflag sanity
// =========================================================================

#[test]
fn poll_events_none_is_zero() {
    assert_eq!(PollEvents::NONE.bits(), 0);
}

#[test]
fn poll_events_pollin_is_nonzero() {
    assert_ne!(PollEvents::POLLIN.bits(), 0);
}

#[test]
fn poll_events_combination_contains_both_members() {
    let combined = PollEvents::POLLIN | PollEvents::POLLOUT;
    assert!(combined.contains(PollEvents::POLLIN));
    assert!(combined.contains(PollEvents::POLLOUT));
}

#[test]
fn poll_events_pollin_and_pollerr_are_disjoint() {
    assert_eq!(PollEvents::POLLIN & PollEvents::POLLERR, PollEvents::NONE);
}

// =========================================================================
// FluxHandle::new_from_str_uri
// =========================================================================

#[test]
fn open_with_empty_uri_uses_flux_uri_env_var() {
    // Empty string → flux_open(NULL, 0) → resolves via FLUX_URI.
    assert!(FluxHandle::new_from_str_uri("", HandleFlags::NONE).is_ok());
}

#[test]
fn open_with_invalid_socket_path_returns_error() {
    assert!(
        FluxHandle::new_from_str_uri(
            "local:///definitely/nonexistent/flux/socket/xyzzy",
            HandleFlags::NONE
        )
        .is_err()
    );
}

#[test]
fn open_with_nul_byte_uri_returns_error() {
    assert!(FluxHandle::new_from_str_uri("local:///bad\0uri", HandleFlags::NONE).is_err());
}

// =========================================================================
// get_rank / get_size
// =========================================================================

#[test]
fn get_rank_returns_valid_u32() {
    with_handle(|h| {
        // In a single-node test instance, rank is 0. Always < u32::MAX.
        assert!(h.get_rank().unwrap() < u32::MAX);
    });
}

#[test]
fn get_size_returns_at_least_one() {
    with_handle(|h| {
        assert!(h.get_size().unwrap() >= 1);
    });
}

#[test]
fn get_rank_is_less_than_get_size() {
    with_handle(|h| {
        let rank = h.get_rank().unwrap() as usize;
        let size = h.get_size().unwrap();
        assert!(rank < size, "rank {rank} must be < size {size}");
    });
}

// =========================================================================
// get_attr
// =========================================================================

#[test]
fn get_attr_rank_matches_get_rank() {
    with_handle(|h| {
        let attr_rank: u32 = h.get_attr("rank").unwrap().parse().unwrap();
        assert_eq!(attr_rank, h.get_rank().unwrap());
    });
}

#[test]
fn get_attr_size_matches_get_size() {
    with_handle(|h| {
        let attr_size: usize = h.get_attr("size").unwrap().parse().unwrap();
        assert_eq!(attr_size, h.get_size().unwrap());
    });
}

#[test]
fn get_attr_nonexistent_key_returns_error() {
    with_handle(|h| {
        assert!(
            h.get_attr("this.attr.definitely.does.not.exist.xyzzy")
                .is_err()
        );
    });
}

#[test]
fn get_attr_nul_byte_key_returns_error() {
    with_handle(|h| {
        assert!(h.get_attr("bad\0key").is_err());
    });
}

// =========================================================================
// set_aux / get_aux / get_aux_raw
//
// Mutation tests open fresh handles to avoid modifying shared handle state.
// =========================================================================

#[test]
fn set_aux_and_get_aux_round_trips_struct() {
    let mut h = open_fresh();
    let data = TestAux {
        value: 42,
        label: "hello".to_string(),
    };
    h.set_aux("test.aux.struct", data).unwrap();
    let retrieved = h.get_aux::<TestAux>("test.aux.struct").unwrap();
    assert_eq!(retrieved.value, 42);
    assert_eq!(retrieved.label, "hello");
}

#[test]
fn set_aux_and_get_aux_round_trips_primitive() {
    let mut h = open_fresh();
    h.set_aux("test.aux.u32", 99u32).unwrap();
    let retrieved = h.get_aux::<u32>("test.aux.u32").unwrap();
    assert_eq!(*retrieved, 99u32);
}

#[test]
fn set_aux_and_get_aux_round_trips_string() {
    let mut h = open_fresh();
    h.set_aux("test.aux.string", "flux-test".to_string())
        .unwrap();
    let retrieved = h.get_aux::<String>("test.aux.string").unwrap();
    assert_eq!(retrieved, "flux-test");
}

#[test]
fn get_aux_type_mismatch_returns_logic_error() {
    let mut h = open_fresh();
    h.set_aux("test.aux.mismatch", 42u32).unwrap();
    assert!(matches!(
        h.get_aux::<String>("test.aux.mismatch"),
        Err(FluxError::Logic(_))
    ));
}

#[test]
fn get_aux_raw_returns_non_null_after_set() {
    let mut h = open_fresh();
    h.set_aux("test.aux.raw", 1u64).unwrap();
    assert!(h.get_aux_raw("test.aux.raw").is_ok());
}

#[test]
fn get_aux_raw_missing_key_returns_error() {
    let h = open_fresh();
    assert!(
        h.get_aux_raw("test.aux.definitely.missing.key.xyzzy")
            .is_err()
    );
}

#[test]
fn set_aux_nul_byte_key_returns_error() {
    let mut h = open_fresh();
    assert!(h.set_aux("bad\0key", 1u32).is_err());
}

#[test]
fn set_aux_overwrites_previous_value_for_same_key() {
    let mut h = open_fresh();
    h.set_aux("test.aux.overwrite", 1u32).unwrap();
    h.set_aux("test.aux.overwrite", 2u32).unwrap();
    let retrieved = h.get_aux::<u32>("test.aux.overwrite").unwrap();
    assert_eq!(*retrieved, 2u32);
}

// =========================================================================
// get_reactor / set_reactor
// =========================================================================

#[test]
fn get_reactor_returns_usable_reactor() {
    with_handle(|h| {
        let reactor = h.get_reactor().unwrap();
        assert!(reactor.now().is_ok());
    });
}

#[test]
fn get_reactor_produces_non_owning_reactor() {
    with_handle(|h| {
        // get_reactor uses Reactor::borrow_ptr internally, so the reactor
        // must not own the underlying C pointer (which belongs to the handle).
        let reactor = h.get_reactor().unwrap();
        assert!(
            !reactor.c_reactor.is_owned(),
            "get_reactor should produce a non-owning (borrowed) Reactor"
        );
    });
}

#[test]
fn set_reactor_succeeds() {
    let mut h = open_fresh();
    let reactor = Reactor::new().unwrap();
    assert!(h.set_reactor(&reactor).is_ok());
}

#[test]
fn get_reactor_after_set_reactor_is_usable() {
    let mut h = open_fresh();
    let reactor = Reactor::new().unwrap();
    h.set_reactor(&reactor).unwrap();
    let retrieved = h.get_reactor().unwrap();
    assert!(retrieved.now().is_ok());
}

// =========================================================================
// get_pollfd / get_pollevents
// =========================================================================

#[test]
fn get_pollfd_returns_non_negative_fd() {
    with_handle(|h| {
        let fd = h.get_pollfd().unwrap();
        assert!(fd >= 0, "Expected non-negative file descriptor, got {fd}");
    });
}

#[test]
fn get_pollfd_is_stable_across_calls() {
    with_handle(|h| {
        let fd1 = h.get_pollfd().unwrap();
        let fd2 = h.get_pollfd().unwrap();
        assert_eq!(fd1, fd2);
    });
}

#[test]
fn get_pollevents_returns_subset_of_known_flags() {
    with_handle(|h| {
        let events = h.get_pollevents().unwrap();
        let all_known = PollEvents::POLLIN | PollEvents::POLLOUT | PollEvents::POLLERR;
        assert!(
            all_known.contains(events),
            "Unexpected bits in pollevents: {:?}",
            events
        );
    });
}

// =========================================================================
// set_log_appname / set_log_procid / log / macros
// =========================================================================

#[test]
fn set_log_appname_succeeds() {
    let mut h = open_fresh();
    assert!(h.set_log_appname("flux-rs-test").is_ok());
}

#[test]
fn set_log_appname_nul_byte_returns_error() {
    let mut h = open_fresh();
    assert!(h.set_log_appname("bad\0name").is_err());
}

#[test]
fn set_log_procid_succeeds() {
    let mut h = open_fresh();
    assert!(h.set_log_procid("test-proc-1").is_ok());
}

#[test]
fn set_log_procid_nul_byte_returns_error() {
    let mut h = open_fresh();
    assert!(h.set_log_procid("bad\0procid").is_err());
}

#[test]
fn log_at_debug_level_succeeds() {
    with_handle(|h| {
        assert!(h.log(LogLevel::Debug, "unit test debug message").is_ok());
    });
}

#[test]
fn log_at_info_level_succeeds() {
    with_handle(|h| {
        assert!(h.log(LogLevel::Info, "unit test info message").is_ok());
    });
}

#[test]
fn log_at_warning_level_succeeds() {
    with_handle(|h| {
        assert!(
            h.log(LogLevel::Warning, "unit test warning message")
                .is_ok()
        );
    });
}

#[test]
fn log_at_error_level_succeeds() {
    with_handle(|h| {
        assert!(h.log(LogLevel::Error, "unit test error message").is_ok());
    });
}

#[test]
fn log_nul_byte_message_returns_error() {
    with_handle(|h| {
        assert!(h.log(LogLevel::Debug, "bad\0message").is_err());
    });
}

#[test]
fn flux_log_macro_does_not_panic() {
    with_handle(|h| {
        // The macro swallows the Result; we just verify no panic.
        flux_log!(h, LogLevel::Debug, "macro test value={}", 42);
    });
}

#[test]
fn flux_log_debug_macro_does_not_panic() {
    with_handle(|h| {
        flux_log_debug!(h, "debug macro test");
    });
}

#[test]
fn flux_log_info_macro_does_not_panic() {
    with_handle(|h| {
        flux_log_info!(h, "info macro test");
    });
}

#[test]
fn flux_log_warning_macro_does_not_panic() {
    with_handle(|h| {
        flux_log_warning!(h, "warning macro test");
    });
}

#[test]
fn flux_log_error_macro_does_not_panic() {
    with_handle(|h| {
        flux_log_error!(h, "error macro test");
    });
}

// =========================================================================
// Clone (incref-based) / try_clone (flux_clone-based)
// =========================================================================

#[test]
fn clone_is_usable() {
    with_handle(|h| {
        let cloned = h.clone();
        assert!(cloned.get_rank().is_ok());
    });
}

#[test]
fn clone_remains_valid_after_original_dropped() {
    // flux_incref ensures the underlying flux_t survives until the last
    // clone drops it.
    let clone = {
        let original = open_fresh();
        original.clone()
    }; // original dropped here; clone still holds an incref'd reference
    assert!(
        clone.get_rank().is_ok(),
        "Cloned handle should remain valid after original is dropped"
    );
}

#[test]
fn clone_and_original_see_same_rank() {
    with_handle(|h| {
        let rank = h.get_rank().unwrap();
        let cloned = h.clone();
        assert_eq!(cloned.get_rank().unwrap(), rank);
    });
}

#[test]
fn clone_has_no_comm_error_handler() {
    // Clone always resets comm_error_handler_cb to None; the clone should
    // remain fully functional despite the handler not being copied over.
    with_handle(|h| {
        let cloned = h.clone();
        assert!(cloned.get_size().is_ok());
    });
}

#[test]
fn try_clone_produces_usable_handle() {
    with_handle(|h| {
        let cloned = h.try_clone().unwrap();
        assert!(cloned.get_rank().is_ok());
    });
}

#[test]
fn try_from_ref_produces_usable_handle() {
    with_handle(|h| {
        let cloned = FluxHandle::try_from(h).unwrap();
        assert!(cloned.get_rank().is_ok());
    });
}

#[test]
fn try_clone_and_incref_clone_see_same_rank() {
    with_handle(|h| {
        let rank = h.get_rank().unwrap();
        assert_eq!(h.clone().get_rank().unwrap(), rank);
        assert_eq!(h.try_clone().unwrap().get_rank().unwrap(), rank);
    });
}

// =========================================================================
// BorrowFluxPtr / FromFluxPtr / AsFluxPtr / IntoFluxPtr
// =========================================================================

#[test]
fn as_mut_ptr_returns_non_null() {
    with_handle(|h| {
        assert!(!h.as_mut_ptr().is_null());
    });
}

#[test]
fn borrow_ptr_creates_non_owning_handle() {
    with_handle(|h| {
        let ptr = h.as_mut_ptr();
        let borrowed = unsafe { FluxHandle::borrow_ptr(ptr) }.unwrap();
        assert!(
            !borrowed.h.is_owned(),
            "borrow_ptr should produce a non-owning FluxHandle"
        );
    });
}

#[test]
fn borrowed_handle_is_usable() {
    with_handle(|h| {
        let ptr = h.as_mut_ptr();
        let borrowed = unsafe { FluxHandle::borrow_ptr(ptr) }.unwrap();
        assert!(borrowed.get_rank().is_ok());
    });
}

#[test]
fn from_ptr_creates_owning_handle() {
    // Transfer ownership out via IntoFluxPtr then back in via from_ptr.
    let original = open_fresh();
    let ptr = original.into_raw();
    let from_ptr = unsafe { FluxHandle::from_ptr(ptr) }.unwrap();
    assert!(
        from_ptr.h.is_owned(),
        "from_ptr should produce an owning FluxHandle"
    );
    assert!(from_ptr.get_rank().is_ok());
}

// =========================================================================
// recv (NONBLOCK — no pending messages)
// =========================================================================

#[test]
fn recv_with_nonblock_and_no_pending_messages_returns_error() {
    // With NONBLOCK and an empty queue, flux_recv returns -1 with EAGAIN.
    let h = open_fresh();
    let msg_match = MessageMatch::new(None, None, None).unwrap();
    assert!(
        h.recv(msg_match, HandleFlags::NONBLOCK).is_err(),
        "Expected error when no messages are pending with NONBLOCK"
    );
}

// =========================================================================
// respond / respond_json / respond_serializable / respond_string /
// respond_error / respond_raw_error
//
// NOTE: Requests below are synthetically encoded and lack valid broker
// routing information (matchtag, route frames). flux_respond_raw may
// succeed or fail depending on how strictly libflux validates the request.
// These tests verify correct argument plumbing and error handling in the
// Rust wrapper layer rather than end-to-end RPC delivery.
// =========================================================================

#[test]
fn respond_with_none_payload_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond(&req, None);
    });
}

#[test]
fn respond_with_raw_payload_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond_raw(&req, Some(b"response data"));
    });
}

#[test]
fn respond_with_payload_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond(&req, Some(c"response data"));
    });
}

#[test]
fn respond_json_with_some_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond_json(&req, Some(&json!({"key": "value"})));
    });
}

#[test]
fn respond_json_with_none_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond_json(&req, None);
    });
}

#[test]
fn respond_string_with_some_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond_string(&req, Some("hello"));
    });
}

#[test]
fn respond_string_with_none_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond_string(&req, None);
    });
}

#[test]
fn respond_string_nul_byte_returns_error() {
    with_handle(|h| {
        let req = make_request();
        assert!(h.respond_string(&req, Some("bad\0string")).is_err());
    });
}

#[test]
fn respond_error_with_os_error_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let err = std::io::Error::from_raw_os_error(libc::EPERM);
        let _ = h.respond_error(&req, err, None);
    });
}

#[test]
fn respond_error_with_non_os_error_returns_logic_error() {
    with_handle(|h| {
        let req = make_request();
        let err = std::io::Error::other("custom");
        assert!(matches!(
            h.respond_error(&req, err, None),
            Err(FluxError::Logic(_))
        ));
    });
}

#[test]
fn respond_error_with_nul_byte_errmsg_returns_error() {
    with_handle(|h| {
        let req = make_request();
        let err = std::io::Error::from_raw_os_error(libc::EPERM);
        assert!(h.respond_error(&req, err, Some("bad\0msg")).is_err());
    });
}

#[test]
fn respond_raw_error_with_valid_inputs_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond_raw_error(&req, libc::EPERM, None);
    });
}

#[test]
fn respond_raw_error_with_errmsg_does_not_panic() {
    with_handle(|h| {
        let req = make_request();
        let _ = h.respond_raw_error(&req, libc::ENOENT, Some("file not found"));
    });
}

#[test]
fn respond_raw_error_with_nul_byte_errmsg_returns_error() {
    with_handle(|h| {
        let req = make_request();
        assert!(
            h.respond_raw_error(&req, libc::EPERM, Some("bad\0msg"))
                .is_err()
        );
    });
}
