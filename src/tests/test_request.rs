use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::msg::{Message, MessageType};
use crate::request::Request;

// =========================================================================
// Helpers
// =========================================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TestPayload {
    x: u32,
    label: String,
}

/// A minimal pre-encoded request used by conversion tests.
fn make_encoded_request() -> Request {
    Request::encode("test.topic", b"hello").expect("Failed to encode request")
}

// =========================================================================
// Request::encode (raw bytes)
// =========================================================================

#[test]
fn encode_with_valid_topic_and_payload_succeeds() {
    assert!(Request::encode("test.topic", b"payload data").is_ok());
}

#[test]
fn encode_with_nul_byte_topic_returns_error() {
    assert!(Request::encode("bad\0topic", b"payload").is_err());
}

#[test]
fn encode_with_empty_payload_succeeds() {
    // An empty byte slice is a valid (zero-length) payload.
    assert!(Request::encode("test.empty", b"").is_ok());
}

#[test]
fn encode_with_binary_payload_succeeds() {
    let payload: Vec<u8> = (0u8..=255).collect();
    assert!(Request::encode("test.binary", &payload).is_ok());
}

// =========================================================================
// Request::encode_json
// =========================================================================

#[test]
fn encode_json_with_object_payload_succeeds() {
    let payload = json!({"key": "value", "num": 42});
    assert!(Request::encode_json("test.json", &payload).is_ok());
}

#[test]
fn encode_json_with_array_payload_succeeds() {
    assert!(Request::encode_json("test.array", &json!([1, 2, 3])).is_ok());
}

#[test]
fn encode_json_with_null_payload_succeeds() {
    assert!(Request::encode_json("test.null", &json!(null)).is_ok());
}

#[test]
fn encode_json_with_nul_byte_topic_returns_error() {
    assert!(Request::encode_json("bad\0topic", &json!({})).is_err());
}

// =========================================================================
// Request::encode_serializable
// =========================================================================

#[test]
fn encode_serializable_with_struct_succeeds() {
    let payload = TestPayload {
        x: 42,
        label: "hello".to_string(),
    };
    assert!(Request::encode_serializable("test.serial", &payload).is_ok());
}

#[test]
fn encode_serializable_with_vec_succeeds() {
    let payload = vec![1u32, 2, 3, 4];
    assert!(Request::encode_serializable("test.vec", &payload).is_ok());
}

// =========================================================================
// Request::decode (raw)
// =========================================================================

#[test]
fn decode_round_trips_topic() {
    let req = Request::encode("service.method", b"data").unwrap();
    let decoded = req.decode().unwrap();
    assert_eq!(decoded.topic, "service.method");
}

#[test]
fn decode_round_trips_payload() {
    let req = Request::encode("test.topic", b"hello world").unwrap();
    let decoded = req.decode().unwrap();
    assert_eq!(decoded.payload.unwrap(), b"hello world");
}

#[test]
fn decode_round_trips_binary_payload() {
    let payload: Vec<u8> = (0u8..16).collect();
    let req = Request::encode("test.binary", &payload).unwrap();
    let decoded = req.decode().unwrap();
    assert_eq!(decoded.payload.unwrap(), payload.as_slice());
}

#[test]
fn decode_empty_payload_returns_none() {
    // With the check_ptr(data_ptr) guard removed, a zero-length payload
    // now correctly returns payload: None rather than an error.
    let req = Request::encode("test.empty", b"").unwrap();
    let decoded = req.decode().unwrap();
    assert!(
        decoded.payload.is_none(),
        "Expected None payload for an empty encode, got {:?}",
        decoded.payload
    );
}

// =========================================================================
// Request::decode_json
// =========================================================================

#[test]
fn decode_json_round_trips_object_value() {
    let payload = json!({"x": 1, "y": 2});
    let req = Request::encode_json("test.json", &payload).unwrap();
    let decoded = req.decode_json().unwrap();
    assert_eq!(decoded.topic, "test.json");
    assert_eq!(decoded.payload.unwrap(), payload);
}

#[test]
fn decode_json_round_trips_array_value() {
    let payload = json!([10, 20, 30]);
    let req = Request::encode_json("test.array", &payload).unwrap();
    let decoded = req.decode_json().unwrap();
    assert_eq!(decoded.payload.unwrap(), payload);
}

#[test]
fn decode_json_topic_matches_encoded_topic() {
    let req = Request::encode_json("my.service.method", &json!(42)).unwrap();
    let decoded = req.decode_json().unwrap();
    assert_eq!(decoded.topic, "my.service.method");
}

// =========================================================================
// Request::decode_deserializable
// =========================================================================

#[test]
fn decode_deserializable_round_trips_struct() {
    let payload = TestPayload {
        x: 99,
        label: "flux".to_string(),
    };
    let req = Request::encode_serializable("test.deser", &payload).unwrap();
    let decoded = req.decode_deserializable::<TestPayload>().unwrap();
    assert_eq!(decoded.topic, "test.deser");
    assert_eq!(decoded.payload.unwrap(), payload);
}

#[test]
fn decode_deserializable_wrong_type_returns_error() {
    // Encoded as a flat integer; deserializing as TestPayload should fail.
    let req = Request::encode_serializable("test.mismatch", &42u32).unwrap();
    assert!(req.decode_deserializable::<TestPayload>().is_err());
}

#[test]
fn decode_deserializable_topic_matches_encoded_topic() {
    let payload = TestPayload {
        x: 1,
        label: "label".to_string(),
    };
    let req = Request::encode_serializable("topic.check", &payload).unwrap();
    let decoded = req.decode_deserializable::<TestPayload>().unwrap();
    assert_eq!(decoded.topic, "topic.check");
}

// =========================================================================
// From<Message> for Request
// =========================================================================

#[test]
fn from_message_wraps_message_preserving_topic() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_topic("wrapped.topic").unwrap();
    let req = Request::from(msg);
    assert_eq!(req.msg.get_topic().unwrap(), "wrapped.topic");
}

#[test]
fn from_message_wraps_message_preserving_payload() {
    let mut msg = Message::new(MessageType::REQUEST).unwrap();
    msg.set_topic("test.topic").unwrap();
    msg.set_payload(b"inner payload").unwrap();
    let req = Request::from(msg);
    assert_eq!(req.msg.get_payload().unwrap(), b"inner payload");
}

// =========================================================================
// From<Request> for Message
// =========================================================================

#[test]
fn from_request_unwraps_to_message_with_correct_topic() {
    let req = make_encoded_request();
    let msg = Message::from(req);
    assert_eq!(msg.get_topic().unwrap(), "test.topic");
}

#[test]
fn from_request_unwraps_to_message_with_correct_payload() {
    let req = make_encoded_request();
    let msg = Message::from(req);
    assert_eq!(msg.get_payload().unwrap(), b"hello");
}

// =========================================================================
// Round-trip: encode → From<Request> for Message → From<Message> for Request → decode
// =========================================================================

#[test]
fn round_trip_through_message_conversion_preserves_topic() {
    let req = Request::encode("round.trip", b"data").unwrap();
    let msg = Message::from(req);
    let req2 = Request::from(msg);
    let decoded = req2.decode().unwrap();
    assert_eq!(decoded.topic, "round.trip");
}

#[test]
fn round_trip_through_message_conversion_preserves_payload() {
    let req = Request::encode("round.trip", b"data").unwrap();
    let msg = Message::from(req);
    let req2 = Request::from(msg);
    let decoded = req2.decode().unwrap();
    assert_eq!(decoded.payload.unwrap(), b"data");
}
