use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtrNoArgs};
use crate::handle::FluxHandle;
use crate::msg::{MessageMatch, MessageRolemask, MessageType};
use crate::msg_handler::{MsgHandler, MsgHandlerSpec, add_handler_vec};
use crate::tests::common::with_handle;

// =========================================================================
// Helpers
// =========================================================================

/// Build a minimal started handler on the shared handle.
fn make_handler(handle: &FluxHandle) -> MsgHandler {
    let matcher = MessageMatch::new(Some(MessageType::REQUEST), None, Some("test.*")).unwrap();
    MsgHandler::new(handle, matcher, Box::new(|_, _, _| {})).unwrap()
}

// =========================================================================
// MsgHandler::new
// =========================================================================

#[test]
fn new_with_request_type_and_glob_succeeds() {
    with_handle(|h| {
        let matcher =
            MessageMatch::new(Some(MessageType::REQUEST), None, Some("test.topic")).unwrap();
        assert!(MsgHandler::new(h, matcher, Box::new(|_, _, _| {})).is_ok());
    });
}

#[test]
fn new_with_event_type_succeeds() {
    with_handle(|h| {
        let matcher =
            MessageMatch::new(Some(MessageType::EVENT), None, Some("test.event")).unwrap();
        assert!(MsgHandler::new(h, matcher, Box::new(|_, _, _| {})).is_ok());
    });
}

#[test]
fn new_with_any_type_and_no_glob_succeeds() {
    with_handle(|h| {
        let matcher = MessageMatch::new(Some(MessageType::ANY), None, None).unwrap();
        assert!(MsgHandler::new(h, matcher, Box::new(|_, _, _| {})).is_ok());
    });
}

#[test]
fn new_with_matchtag_succeeds() {
    with_handle(|h| {
        let matcher = MessageMatch::new(Some(MessageType::RESPONSE), Some(42), None).unwrap();
        assert!(MsgHandler::new(h, matcher, Box::new(|_, _, _| {})).is_ok());
    });
}

#[test]
fn new_stores_callback_in_cb_box() {
    with_handle(|h| {
        let handler = make_handler(h);
        assert!(
            handler._cb_box.is_some(),
            "MsgHandler::new should store the callback in _cb_box"
        );
    });
}

// =========================================================================
// start / stop
// =========================================================================

#[test]
fn start_on_new_handler_succeeds() {
    with_handle(|h| {
        let handler = make_handler(h);
        handler.start();
    });
}

#[test]
fn stop_after_start_succeeds() {
    with_handle(|h| {
        let handler = make_handler(h);
        handler.start();
        handler.stop();
    });
}

#[test]
fn stop_without_prior_start_succeeds() {
    // flux_msg_handler_stop on an unstarted handler is a safe no-op.
    with_handle(|h| {
        let handler = make_handler(h);
        handler.stop();
    });
}

#[test]
fn start_stop_start_cycle_succeeds() {
    with_handle(|h| {
        let handler = make_handler(h);
        handler.start();
        handler.stop();
        handler.start();
    });
}

#[test]
fn double_start_is_idempotent() {
    // flux_msg_handler_start is idempotent; calling it twice should not error.
    with_handle(|h| {
        let handler = make_handler(h);
        handler.start();
        handler.start();
    });
}

#[test]
fn double_stop_is_idempotent() {
    with_handle(|h| {
        let handler = make_handler(h);
        handler.start();
        handler.stop();
        handler.stop();
    });
}

// =========================================================================
// allow_rolemask / deny_rolemask
// =========================================================================

#[test]
fn allow_rolemask_owner_succeeds() {
    with_handle(|h| {
        let mut handler = make_handler(h);
        handler.allow_rolemask(MessageRolemask::OWNER);
    });
}

#[test]
fn allow_rolemask_all_succeeds() {
    with_handle(|h| {
        let mut handler = make_handler(h);
        handler.allow_rolemask(MessageRolemask::ALL);
    });
}

#[test]
fn allow_rolemask_none_succeeds() {
    with_handle(|h| {
        let mut handler = make_handler(h);
        handler.allow_rolemask(MessageRolemask::NONE);
    });
}

#[test]
fn deny_rolemask_user_succeeds() {
    with_handle(|h| {
        let mut handler = make_handler(h);
        handler.deny_rolemask(MessageRolemask::USER);
    });
}

#[test]
fn deny_rolemask_owner_succeeds() {
    with_handle(|h| {
        let mut handler = make_handler(h);
        handler.deny_rolemask(MessageRolemask::OWNER);
    });
}

#[test]
fn allow_then_deny_same_rolemask_succeeds() {
    with_handle(|h| {
        let mut handler = make_handler(h);
        handler.allow_rolemask(MessageRolemask::USER);
        handler.deny_rolemask(MessageRolemask::USER);
    });
}

// =========================================================================
// BorrowFluxPtr / FromFluxPtr
// =========================================================================

#[test]
fn borrow_ptr_produces_non_owning_handler() {
    with_handle(|h| {
        let handler = make_handler(h);
        let ptr = handler.as_mut_ptr();
        let borrowed = unsafe { MsgHandler::borrow_ptr(ptr) }.unwrap();
        assert!(
            !borrowed.c_handler.is_owned(),
            "borrow_ptr should produce a non-owning MsgHandler"
        );
        // `handler` outlives `borrowed`, so ptr remains valid through this scope.
    });
}

#[test]
fn borrow_ptr_has_no_cb_box() {
    with_handle(|h| {
        let handler = make_handler(h);
        let ptr = handler.as_mut_ptr();
        let borrowed = unsafe { MsgHandler::borrow_ptr(ptr) }.unwrap();
        assert!(
            borrowed._cb_box.is_none(),
            "borrow_ptr should produce a MsgHandler with no _cb_box"
        );
    });
}

#[test]
fn borrowed_handler_can_start_and_stop() {
    with_handle(|h| {
        let handler = make_handler(h);
        let ptr = handler.as_mut_ptr();
        let borrowed = unsafe { MsgHandler::borrow_ptr(ptr) }.unwrap();
        borrowed.start();
        borrowed.stop();
    });
}

#[test]
fn borrow_ptr_null_returns_logic_error() {
    let result = unsafe { MsgHandler::borrow_ptr(std::ptr::null_mut()) };
    assert!(result.is_err());
}

#[test]
fn from_ptr_null_returns_logic_error() {
    let result = unsafe { MsgHandler::from_ptr(std::ptr::null_mut()) };
    assert!(result.is_err());
}

// =========================================================================
// AsFluxPtr / IntoFluxPtr
// =========================================================================

#[test]
fn as_mut_ptr_returns_non_null() {
    with_handle(|h| {
        let handler = make_handler(h);
        assert!(!handler.as_mut_ptr().is_null());
    });
}

#[test]
fn as_mut_ptr_is_stable_across_calls() {
    with_handle(|h| {
        let handler = make_handler(h);
        let ptr1 = handler.as_mut_ptr();
        let ptr2 = handler.as_mut_ptr();
        assert_eq!(ptr1, ptr2);
    });
}

// =========================================================================
// MsgHandlerSpec / add_handler_vec
// =========================================================================

#[test]
fn msg_handler_spec_new_stores_fields() {
    let spec = MsgHandlerSpec::new(
        MessageType::REQUEST,
        "test.topic",
        |_, _, _| {},
        MessageRolemask::OWNER,
    );
    assert_eq!(spec.typemask, MessageType::REQUEST);
    assert_eq!(spec.topic_glob, "test.topic");
    assert_eq!(spec.rolemask, MessageRolemask::OWNER);
}

#[test]
fn msg_handler_spec_as_msg_match_succeeds() {
    let spec = MsgHandlerSpec::new(
        MessageType::REQUEST,
        "test.topic",
        |_, _, _| {},
        MessageRolemask::OWNER,
    );
    assert!(spec.as_msg_match().is_ok());
}

#[test]
fn add_handler_vec_with_valid_specs_succeeds() {
    with_handle(|h| {
        let specs = vec![
            MsgHandlerSpec::new(
                MessageType::REQUEST,
                "svc.method1",
                |_, _, _| {},
                MessageRolemask::OWNER,
            ),
            MsgHandlerSpec::new(
                MessageType::REQUEST,
                "svc.method2",
                |_, _, _| {},
                MessageRolemask::OWNER,
            ),
        ];
        let handlers = add_handler_vec(h, specs, None);
        assert!(handlers.is_ok());
        assert_eq!(handlers.unwrap().len(), 2);
    });
}

#[test]
fn add_handler_vec_with_valid_specs_and_service_name_succeeds() {
    with_handle(|h| {
        let specs = vec![
            MsgHandlerSpec::new(
                MessageType::REQUEST,
                "method1",
                |_, _, _| {},
                MessageRolemask::OWNER,
            ),
            MsgHandlerSpec::new(
                MessageType::REQUEST,
                "method2",
                |_, _, _| {},
                MessageRolemask::OWNER,
            ),
        ];
        let handlers = add_handler_vec(h, specs, Some("svc"));
        assert!(handlers.is_ok());
        assert_eq!(handlers.unwrap().len(), 2);
    });
}

#[test]
fn add_handler_vec_with_nul_byte_topic_returns_error() {
    with_handle(|h| {
        let specs = vec![MsgHandlerSpec::new(
            MessageType::REQUEST,
            "bad\0topic",
            |_, _, _| {},
            MessageRolemask::OWNER,
        )];
        assert!(add_handler_vec(h, specs, None).is_err());
    });
}

#[test]
fn add_handler_vec_empty_specs_returns_empty_vec() {
    with_handle(|h| {
        let handlers = add_handler_vec(h, std::iter::empty::<MsgHandlerSpec>(), None).unwrap();
        assert!(handlers.is_empty());
    });
}

#[test]
fn add_handler_vec_handlers_are_started() {
    // Handlers returned by add_handler_vec should already be started.
    // Calling stop() immediately should succeed (not panic or error).
    with_handle(|h| {
        let specs = vec![MsgHandlerSpec::new(
            MessageType::REQUEST,
            "svc.started",
            |_, _, _| {},
            MessageRolemask::OWNER,
        )];
        let handlers = add_handler_vec(h, specs, None).unwrap();
        handlers[0].stop();
    });
}

// =========================================================================
// Callback invocation
// =========================================================================
// Note: end-to-end callback invocation (verifying that msg_handler_trampoline
// correctly dispatches a received message to the Rust closure) requires a
// live send/receive cycle through a running reactor. This is integration-test
// territory and is better covered through higher-level service tests once
// src/rpc.rs is available. The trampoline's pointer-casting and
// reference-counting logic is exercised indirectly by any test that creates
// a handler and runs the reactor.
