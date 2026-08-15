use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::FluxError;
use crate::msg::Message;
use crate::request::Request;
use crate::response::Response;

// =========================================================================
// Helpers
// =========================================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TestPayload {
    x: u32,
    label: String,
}

/// A minimal pre-encoded request used by derive tests.
fn make_request() -> Request {
    Request::encode("test.topic", b"hello").expect("Failed to encode request")
}

/// A minimal pre-encoded response used by conversion and decode tests.
fn make_encoded_response() -> Response {
    Response::encode("test.topic", b"hello").expect("Failed to encode response")
}

// =========================================================================
// Response::encode (raw bytes)
// =========================================================================

#[test]
fn encode_with_valid_topic_and_payload_succeeds() {
    assert!(Response::encode("test.topic", b"payload data").is_ok());
}

#[test]
fn encode_with_nul_byte_topic_returns_error() {
    assert!(Response::encode("bad\0topic", b"payload").is_err());
}

#[test]
fn encode_with_empty_payload_succeeds() {
    assert!(Response::encode("test.empty", b"").is_ok());
}

#[test]
fn encode_with_binary_payload_succeeds() {
    let payload: Vec<u8> = (0u8..=255).collect();
    assert!(Response::encode("test.binary", &payload).is_ok());
}

// =========================================================================
// Response::encode_json
// =========================================================================

#[test]
fn encode_json_with_object_payload_succeeds() {
    let payload = json!({"key": "value", "num": 42});
    assert!(Response::encode_json("test.json", &payload).is_ok());
}

#[test]
fn encode_json_with_array_payload_succeeds() {
    assert!(Response::encode_json("test.array", &json!([1, 2, 3])).is_ok());
}

#[test]
fn encode_json_with_null_payload_succeeds() {
    assert!(Response::encode_json("test.null", &json!(null)).is_ok());
}

#[test]
fn encode_json_with_nul_byte_topic_returns_error() {
    assert!(Response::encode_json("bad\0topic", &json!({})).is_err());
}

// =========================================================================
// Response::encode_serializable
// =========================================================================

#[test]
fn encode_serializable_with_struct_succeeds() {
    let payload = TestPayload {
        x: 42,
        label: "hello".to_string(),
    };
    assert!(Response::encode_serializable("test.serial", &payload).is_ok());
}

#[test]
fn encode_serializable_with_vec_succeeds() {
    let payload = vec![1u32, 2, 3, 4];
    assert!(Response::encode_serializable("test.vec", &payload).is_ok());
}

// =========================================================================
// Response::encode_error
// =========================================================================

#[test]
fn encode_error_with_os_error_succeeds() {
    let err = std::io::Error::from_raw_os_error(libc::ENOENT);
    assert!(Response::encode_error("test.error", err).is_ok());
}

#[test]
fn encode_error_without_os_error_returns_logic_error() {
    // io::Error::new does not carry a raw OS error code.
    let err = std::io::Error::other("custom error");
    assert!(matches!(
        Response::encode_error("test.error", err),
        Err(FluxError::Logic(_))
    ));
}

#[test]
fn encode_error_with_nul_byte_topic_returns_error() {
    let err = std::io::Error::from_raw_os_error(libc::ENOENT);
    assert!(Response::encode_error("bad\0topic", err).is_err());
}

// =========================================================================
// Response::encode_raw_error
// =========================================================================

#[test]
fn encode_raw_error_with_valid_inputs_succeeds() {
    assert!(Response::encode_raw_error("test.error", libc::EPERM, "permission denied").is_ok());
}

#[test]
fn encode_raw_error_with_nul_byte_topic_returns_error() {
    assert!(Response::encode_raw_error("bad\0topic", libc::ENOENT, "msg").is_err());
}

#[test]
fn encode_raw_error_with_nul_byte_errmsg_returns_error() {
    assert!(Response::encode_raw_error("test.error", libc::ENOENT, "bad\0msg").is_err());
}

// =========================================================================
// Response::decode (raw)
// =========================================================================

#[test]
fn decode_round_trips_topic() {
    let resp = Response::encode("service.method", b"data").unwrap();
    let decoded = resp.decode().unwrap();
    assert_eq!(decoded.topic, "service.method");
}

#[test]
fn decode_round_trips_payload() {
    let resp = Response::encode("test.topic", b"hello world").unwrap();
    let decoded = resp.decode().unwrap();
    assert_eq!(decoded.payload.unwrap(), b"hello world");
}

#[test]
fn decode_round_trips_binary_payload() {
    let payload: Vec<u8> = (0u8..16).collect();
    let resp = Response::encode("test.binary", &payload).unwrap();
    let decoded = resp.decode().unwrap();
    assert_eq!(decoded.payload.unwrap(), payload.as_slice());
}

#[test]
fn decode_empty_payload_returns_none() {
    // With the check_ptr(data) guard removed, a zero-length payload now
    // correctly returns payload: None rather than an error.
    let resp = Response::encode("test.empty", b"").unwrap();
    let decoded = resp.decode().unwrap();
    assert!(
        decoded.payload.is_none(),
        "Expected None payload for an empty encode, got {:?}",
        decoded.payload
    );
}

#[test]
fn decode_error_response_returns_request_response_error() {
    // An error-encoded response causes flux_response_decode_raw to return
    // -1, triggering the flux_response_decode_error fallback branch.
    let resp = Response::encode_raw_error("test.error", libc::EPERM, "permission denied").unwrap();
    assert!(matches!(
        resp.decode(),
        Err(FluxError::RequestResponseError(_, _))
    ));
}

#[test]
fn decode_error_response_message_contains_errmsg() {
    let resp = Response::encode_raw_error("test.error", libc::ENOENT, "file not found").unwrap();
    match resp.decode() {
        Err(FluxError::RequestResponseError(_, msg)) => {
            assert!(msg.contains("file not found"), "Expected errmsg in: {msg}");
        }
        other => panic!("Expected RequestResponseError, got {:?}", other),
    }
}

// =========================================================================
// Response::decode_json
// =========================================================================

#[test]
fn decode_json_round_trips_object_value() {
    let payload = json!({"x": 1, "y": 2});
    let resp = Response::encode_json("test.json", &payload).unwrap();
    let decoded = resp.decode_json().unwrap();
    assert_eq!(decoded.topic, "test.json");
    assert_eq!(decoded.payload.unwrap(), payload);
}

#[test]
fn decode_json_round_trips_array_value() {
    let payload = json!([10, 20, 30]);
    let resp = Response::encode_json("test.array", &payload).unwrap();
    let decoded = resp.decode_json().unwrap();
    assert_eq!(decoded.payload.unwrap(), payload);
}

#[test]
fn decode_json_topic_matches_encoded_topic() {
    let resp = Response::encode_json("my.service.method", &json!(42)).unwrap();
    let decoded = resp.decode_json().unwrap();
    assert_eq!(decoded.topic, "my.service.method");
}

#[test]
fn decode_json_error_response_returns_error() {
    let resp = Response::encode_raw_error("test.error", libc::ENOENT, "file not found").unwrap();
    assert!(resp.decode_json().is_err());
}

// =========================================================================
// Response::decode_deserializable
// =========================================================================

#[test]
fn decode_deserializable_round_trips_struct() {
    let payload = TestPayload {
        x: 99,
        label: "flux".to_string(),
    };
    let resp = Response::encode_serializable("test.deser", &payload).unwrap();
    let decoded = resp.decode_deserializable::<TestPayload>().unwrap();
    assert_eq!(decoded.topic, "test.deser");
    assert_eq!(decoded.payload.unwrap(), payload);
}

#[test]
fn decode_deserializable_wrong_type_returns_error() {
    // Encoded as a flat integer; deserializing as TestPayload should fail.
    let resp = Response::encode_serializable("test.mismatch", &42u32).unwrap();
    assert!(resp.decode_deserializable::<TestPayload>().is_err());
}

#[test]
fn decode_deserializable_error_response_returns_error() {
    let resp = Response::encode_raw_error("test.error", libc::EPERM, "permission denied").unwrap();
    assert!(resp.decode_deserializable::<TestPayload>().is_err());
}

// =========================================================================
// Response::derive
// =========================================================================

#[test]
fn derive_with_no_error_succeeds() {
    let req = make_request();
    assert!(Response::derive(&req, None).is_ok());
}

#[test]
fn derive_with_os_error_succeeds() {
    let req = make_request();
    let err = std::io::Error::from_raw_os_error(libc::ENOENT);
    assert!(Response::derive(&req, Some(err)).is_ok());
}

#[test]
fn derive_with_non_os_error_returns_logic_error() {
    let req = make_request();
    let err = std::io::Error::other("custom");
    assert!(matches!(
        Response::derive(&req, Some(err)),
        Err(FluxError::Logic(_))
    ));
}

#[test]
fn derive_preserves_topic_from_request() {
    let req = Request::encode("service.method", b"data").unwrap();
    let resp = Response::derive(&req, None).unwrap();
    let decoded = resp.decode().unwrap();
    assert_eq!(decoded.topic, "service.method");
}

// =========================================================================
// Response::derive_raw_error
// =========================================================================

#[test]
fn derive_raw_error_with_none_errnum_succeeds() {
    let req = make_request();
    assert!(Response::derive_raw_error(&req, None).is_ok());
}

#[test]
fn derive_raw_error_with_some_errnum_succeeds() {
    let req = make_request();
    assert!(Response::derive_raw_error(&req, Some(libc::EPERM)).is_ok());
}

#[test]
fn derive_raw_error_with_errnum_produces_error_on_decode() {
    // A non-zero errnum causes flux_response_decode_raw to return -1.
    let req = make_request();
    let resp = Response::derive_raw_error(&req, Some(libc::ENOENT)).unwrap();
    assert!(matches!(
        resp.decode(),
        Err(FluxError::RequestResponseError(_, _)) | Err(FluxError::System(_, _))
    ));
}

// =========================================================================
// From<Message> for Response
// =========================================================================

#[test]
fn from_message_wraps_message_preserving_topic() {
    use crate::msg::{Message, MessageType};
    let mut msg = Message::new(MessageType::RESPONSE).unwrap();
    msg.set_topic("wrapped.topic").unwrap();
    let resp = Response::from(msg);
    // msg is now pub(crate) — inspect directly without round-tripping.
    assert_eq!(resp.msg.get_topic().unwrap(), "wrapped.topic");
}

#[test]
fn from_message_wraps_message_preserving_payload() {
    use crate::msg::{Message, MessageType};
    let mut msg = Message::new(MessageType::RESPONSE).unwrap();
    msg.set_topic("test.topic").unwrap();
    msg.set_payload(b"inner payload").unwrap();
    let resp = Response::from(msg);
    assert_eq!(resp.msg.get_payload().unwrap(), b"inner payload");
}

// =========================================================================
// From<Response> for Message
// =========================================================================

#[test]
fn from_response_unwraps_to_message_with_correct_topic() {
    let resp = make_encoded_response();
    let msg: Message = resp.into();
    assert_eq!(msg.get_topic().unwrap(), "test.topic");
}

#[test]
fn from_response_unwraps_to_message_with_correct_payload() {
    let resp = make_encoded_response();
    let msg: Message = resp.into();
    assert_eq!(msg.get_payload().unwrap(), b"hello");
}
