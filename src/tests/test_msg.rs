use std::io;

use flux_sys::core::{flux_match, flux_msg_create, flux_msg_destroy};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::FluxError;
use crate::flux_ptr_management::{AsFluxPtr, BorrowFluxPtrNoArgs, FromFluxPtrNoArgs, IntoFluxPtr};
use crate::msg::{Message, MessageFlag, MessageMatch, MessageRolemask, MessageType};

// =========================================================================
// Helpers
// =========================================================================

fn make_request() -> Message {
    let mut msg = Message::new(MessageType::REQUEST).expect("Failed to create REQUEST message");
    msg.set_topic("test.topic")
        .expect("Failed to set topic on REQUEST message");
    msg
}

fn make_event() -> Message {
    let mut msg = Message::new(MessageType::EVENT).expect("Failed to create EVENT message");
    msg.set_topic("test.event")
        .expect("Failed to set topic on EVENT message");
    msg
}

fn make_response() -> Message {
    Message::new(MessageType::RESPONSE).expect("Failed to create RESPONSE message")
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TestPayload {
    x: u32,
    label: String,
}

// =========================================================================
// MessageType Display
// =========================================================================

#[test]
fn message_type_request_display_is_not_unknown() {
    let s = MessageType::REQUEST.to_string();
    assert!(!s.is_empty());
    assert_ne!(s, "unknown");
}

#[test]
fn message_type_event_display_is_not_unknown() {
    let s = MessageType::EVENT.to_string();
    assert!(!s.is_empty());
    assert_ne!(s, "unknown");
}

#[test]
fn message_type_none_display_returns_unknown() {
    // bits() = 0 is not a recognized Flux message type; typestr returns NULL.
    let s = MessageType::NONE.to_string();
    assert_eq!(s, "unknown");
}

// =========================================================================
// MessageFlag bitflag sanity
// =========================================================================

#[test]
fn message_flag_none_is_zero() {
    assert_eq!(MessageFlag::NONE.bits(), 0);
}

#[test]
fn message_flag_combination_contains_both_members() {
    let combined = MessageFlag::TOPIC | MessageFlag::PAYLOAD;
    assert!(combined.contains(MessageFlag::TOPIC));
    assert!(combined.contains(MessageFlag::PAYLOAD));
}

// =========================================================================
// MessageRolemask bitflag sanity
// =========================================================================

#[test]
fn message_rolemask_all_contains_owner_and_user() {
    assert!(MessageRolemask::ALL.contains(MessageRolemask::OWNER));
    assert!(MessageRolemask::ALL.contains(MessageRolemask::USER));
}

#[test]
fn message_rolemask_none_is_not_owner() {
    assert!(!MessageRolemask::NONE.contains(MessageRolemask::OWNER));
}

// =========================================================================
// MessageMatch::new
// =========================================================================

#[test]
fn message_match_new_all_none_converts_to_zeroed_flux_match() {
    let mm = MessageMatch::new(None, None, None).unwrap();
    let fm: flux_match = (&mm).into();
    assert_eq!(fm.typemask, 0);
    assert_eq!(fm.matchtag, 0);
    assert!(fm.topic_glob.is_null());
}

#[test]
fn message_match_new_with_all_fields_converts_correctly() {
    let mm = MessageMatch::new(Some(MessageType::REQUEST), Some(42), Some("test.*")).unwrap();
    let fm: flux_match = (&mm).into();
    assert_eq!(fm.typemask, MessageType::REQUEST.bits() as i32);
    assert_eq!(fm.matchtag, 42);
    assert!(!fm.topic_glob.is_null());
}

#[test]
fn message_match_new_nul_byte_topic_returns_error() {
    let result = MessageMatch::new(None, None, Some("bad\0topic"));
    assert!(result.is_err());
}

// =========================================================================
// MessageMatch setters
// =========================================================================

#[test]
fn set_typemask_updates_converted_flux_match() {
    let mut mm = MessageMatch::new(None, None, None).unwrap();
    mm.set_typemask(Some(MessageType::EVENT));
    let fm: flux_match = (&mm).into();
    assert_eq!(fm.typemask, MessageType::EVENT.bits() as i32);
}

#[test]
fn set_typemask_to_none_yields_zero_in_flux_match() {
    let mut mm = MessageMatch::new(Some(MessageType::REQUEST), None, None).unwrap();
    mm.set_typemask(None);
    let fm: flux_match = (&mm).into();
    assert_eq!(fm.typemask, 0);
}

#[test]
fn set_matchtag_updates_converted_flux_match() {
    let mut mm = MessageMatch::new(None, None, None).unwrap();
    mm.set_matchtag(Some(99));
    let fm: flux_match = (&mm).into();
    assert_eq!(fm.matchtag, 99);
}

#[test]
fn set_topic_glob_valid_string_succeeds() {
    let mut mm = MessageMatch::new(None, None, None).unwrap();
    assert!(mm.set_topic_glob(Some("foo.*")).is_ok());
}

#[test]
fn set_topic_glob_nul_byte_returns_error() {
    let mut mm = MessageMatch::new(None, None, None).unwrap();
    assert!(mm.set_topic_glob(Some("bad\0topic")).is_err());
}

#[test]
fn set_topic_glob_none_clears_topic_in_flux_match() {
    let mut mm = MessageMatch::new(None, None, Some("foo.*")).unwrap();
    mm.set_topic_glob(None).unwrap();
    let fm: flux_match = (&mm).into();
    assert!(fm.topic_glob.is_null());
}

// =========================================================================
// From<&MessageMatch> for flux_match / TryFrom<flux_match> for MessageMatch
// =========================================================================

#[test]
fn from_message_match_ref_all_none_yields_zeroed_flux_match() {
    let mm = MessageMatch::new(None, None, None).unwrap();
    let fm: flux_match = (&mm).into();
    assert_eq!(fm.typemask, 0);
    assert_eq!(fm.matchtag, 0);
    assert!(fm.topic_glob.is_null());
}

#[test]
fn try_from_flux_match_round_trips_via_message_match() {
    // MessageMatch → flux_match → MessageMatch must preserve all fields.
    let original = MessageMatch::new(Some(MessageType::REQUEST), Some(5), Some("foo.*")).unwrap();
    let fm: flux_match = (&original).into();
    // `original` must remain alive so that fm.topic_glob is a valid pointer.
    let round_tripped = MessageMatch::try_from(fm).unwrap();
    assert_eq!(original, round_tripped);
}

#[test]
fn try_from_flux_match_null_topic_glob_yields_none_topic() {
    let fm = flux_match {
        typemask: MessageType::EVENT.bits() as i32,
        matchtag: 0,
        topic_glob: std::ptr::null(),
    };
    let mm = MessageMatch::try_from(fm).unwrap();
    let fm2: flux_match = (&mm).into();
    assert!(fm2.topic_glob.is_null());
}

// =========================================================================
// Message::new
// =========================================================================

#[test]
fn new_request_message_succeeds() {
    assert!(Message::new(MessageType::REQUEST).is_ok());
}

#[test]
fn new_event_message_succeeds() {
    assert!(Message::new(MessageType::EVENT).is_ok());
}

#[test]
fn new_response_message_succeeds() {
    assert!(Message::new(MessageType::RESPONSE).is_ok());
}

#[test]
fn new_control_message_succeeds() {
    assert!(Message::new(MessageType::CONTROL).is_ok());
}

// =========================================================================
// Message::try_clone
// =========================================================================

#[test]
fn try_clone_without_payload_preserves_topic() {
    let msg = make_request();
    let clone = msg.try_clone(false).unwrap();
    assert_eq!(clone.get_topic().unwrap(), "test.topic");
}

#[test]
fn try_clone_with_payload_true_copies_payload() {
    let mut msg = make_request();
    msg.set_payload(b"hello").unwrap();
    let clone = msg.try_clone(true).unwrap();
    assert_eq!(clone.get_payload().unwrap(), b"hello");
}

#[test]
fn try_clone_with_payload_false_excludes_payload() {
    let mut msg = make_request();
    msg.set_payload(b"hello").unwrap();
    let clone = msg.try_clone(false).unwrap();
    assert!(!clone.has_payload());
}

// =========================================================================
// Message::clone (flux_msg_incref)
// =========================================================================

#[test]
fn clone_produces_message_with_same_topic() {
    let msg = make_request();
    let clone = msg.clone();
    assert_eq!(clone.get_topic().unwrap(), msg.get_topic().unwrap());
}

#[test]
fn clone_remains_valid_after_original_is_dropped() {
    // flux_msg_incref increments the refcount; dropping the original
    // decrements it to 1, leaving the clone's pointer still valid.
    let clone = {
        let msg = make_request();
        msg.clone()
    };
    assert_eq!(clone.get_topic().unwrap(), "test.topic");
}

// =========================================================================
// Message::has_flag / set_flag / clear_flag
// =========================================================================

#[test]
fn new_message_does_not_have_user1_flag() {
    let msg = Message::new(MessageType::REQUEST).unwrap();
    assert!(!msg.has_flag(MessageFlag::USER1));
}

#[test]
fn set_flag_user1_makes_has_flag_true() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_flag(MessageFlag::USER1).unwrap();
    assert!(msg.has_flag(MessageFlag::USER1));
}

#[test]
fn clear_flag_user1_after_set_makes_has_flag_false() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_flag(MessageFlag::USER1).unwrap();
    msg.clear_flag(MessageFlag::USER1).unwrap();
    assert!(!msg.has_flag(MessageFlag::USER1));
}

#[test]
fn set_topic_implicitly_sets_topic_flag() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_topic("a.b").unwrap();
    assert!(msg.has_flag(MessageFlag::TOPIC));
}

#[test]
fn set_payload_implicitly_sets_payload_flag() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_payload(b"data").unwrap();
    assert!(msg.has_flag(MessageFlag::PAYLOAD));
}

// =========================================================================
// Message::set_private / is_private
// =========================================================================

#[test]
fn new_message_is_not_private() {
    let msg = Message::new(MessageType::REQUEST).unwrap();
    assert!(!msg.is_private());
}

#[test]
fn set_private_makes_is_private_true() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_private().unwrap();
    assert!(msg.is_private());
}

// =========================================================================
// Message::set_streaming / is_streaming
// =========================================================================

#[test]
fn new_response_message_is_not_streaming() {
    let msg = make_response();
    assert!(!msg.is_streaming());
}

#[test]
fn set_streaming_makes_is_streaming_true() {
    let mut msg = make_response();
    msg.set_streaming().unwrap();
    assert!(msg.is_streaming());
}

// =========================================================================
// Message::set_noresponse / is_noresponse
// =========================================================================

#[test]
fn new_request_message_is_not_noresponse() {
    let msg = Message::new(MessageType::REQUEST).unwrap();
    assert!(!msg.is_noresponse());
}

#[test]
fn set_noresponse_makes_is_noresponse_true() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_noresponse().unwrap();
    assert!(msg.is_noresponse());
}

// =========================================================================
// Message::set_topic / get_topic / delete_topic
// =========================================================================

#[test]
fn set_topic_and_get_topic_round_trip() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_topic("my.service.method").unwrap();
    assert_eq!(msg.get_topic().unwrap(), "my.service.method");
}

#[test]
fn set_topic_nul_byte_returns_error() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    assert!(msg.set_topic("bad\0topic").is_err());
}

#[test]
fn delete_topic_makes_get_topic_fail() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_topic("foo").unwrap();
    msg.delete_topic().unwrap();
    assert!(msg.get_topic().is_err());
}

// =========================================================================
// Message::has_payload / set_payload / get_payload
// =========================================================================

#[test]
fn new_message_has_no_payload() {
    let msg = Message::new(MessageType::REQUEST).unwrap();
    assert!(!msg.has_payload());
}

#[test]
fn set_payload_makes_has_payload_true() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_payload(b"data").unwrap();
    assert!(msg.has_payload());
}

#[test]
fn set_payload_and_get_payload_round_trip() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    let data = b"hello world";
    msg.set_payload(data).unwrap();
    assert_eq!(msg.get_payload().unwrap(), data);
}

#[test]
fn get_payload_without_payload_returns_error() {
    let msg = Message::new(MessageType::REQUEST).unwrap();
    assert!(msg.get_payload().is_err());
}

// =========================================================================
// Message::set_payload_json / get_payload_json
// =========================================================================

#[test]
fn set_payload_json_object_round_trips() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    let val = json!({"key": "value", "num": 42});
    msg.set_payload_json(&val).unwrap();
    assert_eq!(msg.get_payload_json().unwrap(), val);
}

#[test]
fn set_payload_json_array_round_trips() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    let val = json!([1, 2, 3]);
    msg.set_payload_json(&val).unwrap();
    assert_eq!(msg.get_payload_json().unwrap(), val);
}

// =========================================================================
// Message::set_payload_serializable / get_payload_deserializable
// =========================================================================

#[test]
fn set_payload_serializable_and_get_payload_deserializable_round_trip() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    let payload = TestPayload {
        x: 7,
        label: "hello".to_string(),
    };
    msg.set_payload_serializable(&payload).unwrap();
    let got: TestPayload = msg.get_payload_deserializable().unwrap();
    assert_eq!(got, payload);
}

// =========================================================================
// Message::set_string / get_string
// =========================================================================

#[test]
fn set_string_and_get_string_round_trip() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_string("hello flux").unwrap();
    assert_eq!(msg.get_string().unwrap(), "hello flux");
}

#[test]
fn set_string_nul_byte_returns_error() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    assert!(msg.set_string("bad\0string").is_err());
}

// =========================================================================
// Message::set_nodeid / get_nodeid
// =========================================================================

#[test]
fn set_nodeid_and_get_nodeid_round_trip() {
    let mut msg = make_request();
    msg.set_nodeid(42).unwrap();
    assert_eq!(msg.get_nodeid().unwrap(), 42);
}

#[test]
fn set_nodeid_zero_round_trips() {
    let mut msg = make_request();
    msg.set_nodeid(0).unwrap();
    assert_eq!(msg.get_nodeid().unwrap(), 0);
}

// =========================================================================
// Message::set_cred / get_cred
// =========================================================================

#[test]
fn set_cred_user_role_and_get_cred_round_trip() {
    let mut msg = make_request();
    msg.set_cred(1000, MessageRolemask::USER).unwrap();
    let (userid, rolemask) = msg.get_cred().unwrap();
    assert_eq!(userid, 1000);
    assert_eq!(rolemask, MessageRolemask::USER);
}

#[test]
fn set_cred_owner_role_and_get_cred_round_trip() {
    let mut msg = make_request();
    msg.set_cred(0, MessageRolemask::OWNER).unwrap();
    let (userid, rolemask) = msg.get_cred().unwrap();
    assert_eq!(userid, 0);
    assert!(rolemask.contains(MessageRolemask::OWNER));
}

// =========================================================================
// Message::authorize
// =========================================================================

#[test]
fn authorize_owner_role_allows_any_userid() {
    let mut msg = make_request();
    msg.set_cred(0, MessageRolemask::OWNER).unwrap();
    // OWNER role bypasses the userid check entirely.
    assert_eq!(msg.authorize(9999).unwrap(), true);
}

#[test]
fn authorize_user_role_matching_userid_returns_true() {
    let mut msg = make_request();
    msg.set_cred(1000, MessageRolemask::USER).unwrap();
    assert_eq!(msg.authorize(1000).unwrap(), true);
}

#[test]
fn authorize_user_role_mismatched_userid_returns_false() {
    let mut msg = make_request();
    msg.set_cred(1000, MessageRolemask::USER).unwrap();
    assert_eq!(msg.authorize(9999).unwrap(), false);
}

// =========================================================================
// Message::authorize_cred
// =========================================================================

#[test]
fn authorize_cred_owner_rolemask_allows_any_userid() {
    let msg = make_request();
    // OWNER role in the cred bypasses the userid check.
    assert_eq!(
        msg.authorize_cred(9999, 0, MessageRolemask::OWNER).unwrap(),
        true
    );
}

#[test]
fn authorize_cred_user_rolemask_matching_userid_returns_true() {
    let msg = make_request();
    assert_eq!(
        msg.authorize_cred(1000, 1000, MessageRolemask::USER)
            .unwrap(),
        true
    );
}

#[test]
fn authorize_cred_user_rolemask_mismatched_userid_returns_false() {
    let msg = make_request();
    assert_eq!(
        msg.authorize_cred(9999, 1000, MessageRolemask::USER)
            .unwrap(),
        false
    );
}

// =========================================================================
// Message::set_error / set_error_raw / get_error
// =========================================================================

#[test]
fn set_error_raw_and_get_error_round_trip() {
    let mut msg = make_response();
    msg.set_error_raw(libc::ENOENT).unwrap();
    let err = msg.get_error().unwrap();
    assert_eq!(err.raw_os_error(), Some(libc::ENOENT));
}

#[test]
fn set_error_with_raw_os_error_round_trips() {
    let mut msg = make_response();
    let io_err = io::Error::from_raw_os_error(libc::EPERM);
    msg.set_error(io_err).unwrap();
    let got = msg.get_error().unwrap();
    assert_eq!(got.raw_os_error(), Some(libc::EPERM));
}

#[test]
fn set_error_without_raw_os_error_returns_logic_error() {
    let mut msg = make_response();
    // io::Error::new does not carry a raw OS errno.
    let io_err = io::Error::new(io::ErrorKind::Other, "no errno attached");
    assert!(matches!(msg.set_error(io_err), Err(FluxError::Logic(_))));
}

// =========================================================================
// Message::set_sequence / get_sequence
// =========================================================================

#[test]
fn set_sequence_and_get_sequence_round_trip() {
    let mut msg = make_event();
    msg.set_sequence(42).unwrap();
    assert_eq!(msg.get_sequence().unwrap(), 42);
}

#[test]
fn set_sequence_zero_round_trips() {
    let mut msg = make_event();
    msg.set_sequence(0).unwrap();
    assert_eq!(msg.get_sequence().unwrap(), 0);
}

// =========================================================================
// Message::set_matchtag / get_matchtag / match_tag
// =========================================================================

#[test]
fn set_matchtag_and_get_matchtag_round_trip() {
    let mut msg = make_request();
    msg.set_matchtag(7).unwrap();
    assert_eq!(msg.get_matchtag().unwrap(), 7);
}

#[test]
fn match_tag_with_matching_tag_returns_true() {
    let mut msg = make_request();
    msg.set_matchtag(7).unwrap();
    assert!(msg.match_tag(7));
}

#[test]
fn match_tag_with_non_matching_tag_returns_false() {
    let mut msg = make_request();
    msg.set_matchtag(7).unwrap();
    assert!(!msg.match_tag(99));
}

// =========================================================================
// Message::encode / decode
// =========================================================================

#[test]
fn encode_produces_non_empty_bytes_for_request() {
    let msg = make_request();
    let encoded = msg.encode().unwrap();
    assert!(!encoded.is_empty());
}

#[test]
fn encode_decode_round_trips_topic() {
    let msg = make_request();
    let encoded = msg.encode().unwrap();
    let decoded = Message::decode(&encoded).unwrap();
    assert_eq!(decoded.get_topic().unwrap(), "test.topic");
}

#[test]
fn encode_decode_round_trips_payload() {
    let mut msg = make_request();
    msg.set_payload(b"encode test payload").unwrap();
    let encoded = msg.encode().unwrap();
    let decoded = Message::decode(&encoded).unwrap();
    assert_eq!(decoded.get_payload().unwrap(), b"encode test payload");
}

#[test]
fn encode_decode_round_trips_matchtag() {
    let mut msg = make_request();
    msg.set_matchtag(42).unwrap();
    let encoded = msg.encode().unwrap();
    let decoded = Message::decode(&encoded).unwrap();
    assert_eq!(decoded.get_matchtag().unwrap(), 42);
}

#[test]
fn decode_invalid_bytes_returns_error() {
    assert!(Message::decode(b"not a valid flux message at all").is_err());
}

// =========================================================================
// Message PartialEq<MessageMatch>
// =========================================================================

#[test]
fn message_matches_compatible_message_match() {
    let msg = make_request();
    let mm = MessageMatch::new(Some(MessageType::REQUEST), None, Some("test.topic")).unwrap();
    assert!(msg == mm);
}

#[test]
fn message_does_not_match_wrong_topic() {
    let msg = make_request();
    let mm = MessageMatch::new(Some(MessageType::REQUEST), None, Some("other.topic")).unwrap();
    assert!(!(msg == mm));
}

#[test]
fn message_does_not_match_wrong_type() {
    let msg = make_request();
    let mm = MessageMatch::new(Some(MessageType::EVENT), None, Some("test.topic")).unwrap();
    assert!(!(msg == mm));
}

#[test]
fn message_matches_any_typemask() {
    let msg = make_request();
    // ANY typemask should match a REQUEST message regardless of topic.
    let mm = MessageMatch::new(Some(MessageType::ANY), None, None).unwrap();
    assert!(msg == mm);
}

// =========================================================================
// BorrowFluxPtr / FromFluxPtr
// =========================================================================

#[test]
fn borrow_ptr_produces_non_owning_message() {
    let owner = make_request();
    let ptr = owner.c_msg.as_mut_ptr();
    let borrowed = unsafe { Message::borrow_ptr(ptr) }.unwrap();
    assert!(!borrowed.c_msg.is_owned());
    // LIFO drop order: `borrowed` (non-owning) drops first, then `owner`
    // (owning) drops and frees the pointer — no double-free.
}

#[test]
fn borrow_ptr_can_read_topic_from_borrowed_message() {
    let owner = make_request();
    let borrowed = unsafe { Message::borrow_ptr(owner.c_msg.as_mut_ptr()) }.unwrap();
    assert_eq!(borrowed.get_topic().unwrap(), "test.topic");
}

#[test]
fn from_ptr_produces_owning_message() {
    let raw_ptr = unsafe { flux_msg_create(MessageType::REQUEST.bits() as i32) };
    assert!(!raw_ptr.is_null());
    let msg = unsafe { Message::from_ptr(raw_ptr) }.unwrap();
    assert!(msg.c_msg.is_owned());
    // msg drops here and correctly destroys the pointer.
}

// =========================================================================
// AsFluxPtr / IntoFluxPtr
// =========================================================================

#[test]
fn as_mut_ptr_returns_non_null_pointer() {
    let msg = make_request();
    assert!(!msg.as_mut_ptr().is_null());
}

#[test]
fn as_mut_ptr_called_twice_returns_same_address() {
    let msg = make_request();
    assert_eq!(msg.as_mut_ptr(), msg.as_mut_ptr());
}

#[test]
fn into_raw_returns_non_null_pointer_and_does_not_destroy() {
    let msg = make_request();
    let ptr = msg.into_raw();
    assert!(!ptr.is_null());
    // Caller owns the pointer after into_raw; destroy it manually.
    unsafe { flux_msg_destroy(ptr) };
}
