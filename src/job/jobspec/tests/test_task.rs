use serde_json::json;

use crate::job::jobspec::task::{Task, TaskCount, TaskCountPerResource};

// =========================================================================
// Helpers
// =========================================================================

fn make_task_count(value: serde_json::Value) -> Result<TaskCount, serde_json::Error> {
    serde_json::from_value::<TaskCount>(value)
}

fn make_task(value: serde_json::Value) -> Result<Task, serde_json::Error> {
    serde_json::from_value::<Task>(value)
}

fn minimal_task_json() -> serde_json::Value {
    json!({
        "command": ["echo", "hello"],
        "slot": "default",
        "count": {"per_slot": 1}
    })
}

// =========================================================================
// TaskCountPerResource serde
// =========================================================================

#[test]
fn task_count_per_resource_deserializes_type_key() {
    let v: TaskCountPerResource =
        serde_json::from_value(json!({"type": "core", "count": 2})).unwrap();
    assert_eq!(v.resource_type, "core");
    assert_eq!(v.count, 2);
}

#[test]
fn task_count_per_resource_missing_type_returns_error() {
    assert!(serde_json::from_value::<TaskCountPerResource>(json!({"count": 2})).is_err());
}

#[test]
fn task_count_per_resource_missing_count_returns_error() {
    assert!(serde_json::from_value::<TaskCountPerResource>(json!({"type": "core"})).is_err());
}

#[test]
fn task_count_per_resource_serializes_with_type_key_not_resource_type() {
    let v = TaskCountPerResource {
        resource_type: "gpu".to_string(),
        count: 4,
    };
    let json = serde_json::to_value(&v).unwrap();
    assert!(
        json.get("type").is_some(),
        "Expected 'type' key in serialized output"
    );
    assert!(
        json.get("resource_type").is_none(),
        "Expected no 'resource_type' key"
    );
}

#[test]
fn task_count_per_resource_struct_round_trips() {
    let original = TaskCountPerResource {
        resource_type: "core".to_string(),
        count: 8,
    };
    let json = serde_json::to_value(&original).unwrap();
    let decoded: TaskCountPerResource = serde_json::from_value(json).unwrap();
    assert_eq!(decoded, original);
}

// =========================================================================
// TaskCountPerResource Display (no_debug — only Display generated)
// =========================================================================

#[test]
fn task_count_per_resource_display_is_non_empty() {
    let v = TaskCountPerResource {
        resource_type: "core".to_string(),
        count: 2,
    };
    assert!(!v.to_string().is_empty());
}

#[test]
fn task_count_per_resource_display_contains_type_value() {
    let v = TaskCountPerResource {
        resource_type: "gpu".to_string(),
        count: 1,
    };
    assert!(v.to_string().contains("gpu"));
}

// =========================================================================
// TaskCount deserialization — valid cases
// =========================================================================

#[test]
fn task_count_per_slot_valid_value_succeeds() {
    assert!(make_task_count(json!({"per_slot": 1})).is_ok());
}

#[test]
fn task_count_per_slot_stores_correct_value() {
    let tc = make_task_count(json!({"per_slot": 4})).unwrap();
    assert!(matches!(tc, TaskCount::PerSlot(4)));
}

#[test]
fn task_count_total_valid_value_succeeds() {
    assert!(make_task_count(json!({"total": 1})).is_ok());
}

#[test]
fn task_count_total_stores_correct_value() {
    let tc = make_task_count(json!({"total": 16})).unwrap();
    assert!(matches!(tc, TaskCount::Total(16)));
}

#[test]
fn task_count_per_resource_succeeds() {
    assert!(make_task_count(json!({"per_resource": {"type": "node", "count": 1}})).is_ok());
}

#[test]
fn task_count_per_resource_stores_correct_fields() {
    let tc = make_task_count(json!({"per_resource": {"type": "core", "count": 2}})).unwrap();
    match tc {
        TaskCount::PerResource(ref r) => {
            assert_eq!(r.resource_type, "core");
            assert_eq!(r.count, 2);
        }
        _ => panic!("Expected PerResource variant"),
    }
}

#[test]
fn task_count_unknown_key_produces_extension_variant() {
    let tc = make_task_count(json!({"custom_key": 42})).unwrap();
    assert!(matches!(tc, TaskCount::Extension(_, _)));
}

#[test]
fn task_count_extension_stores_key_and_value() {
    let tc = make_task_count(json!({"my_extension": {"foo": "bar"}})).unwrap();
    match tc {
        TaskCount::Extension(key, val) => {
            assert_eq!(key, "my_extension");
            assert_eq!(val, json!({"foo": "bar"}));
        }
        _ => panic!("Expected Extension variant"),
    }
}

// =========================================================================
// TaskCount deserialization — validation errors
// =========================================================================

#[test]
fn task_count_per_slot_zero_returns_error() {
    assert!(make_task_count(json!({"per_slot": 0})).is_err());
}

#[test]
fn task_count_total_zero_returns_error() {
    assert!(make_task_count(json!({"total": 0})).is_err());
}

#[test]
fn task_count_empty_map_returns_error() {
    assert!(make_task_count(json!({})).is_err());
}

#[test]
fn task_count_multiple_keys_returns_error() {
    assert!(make_task_count(json!({"per_slot": 1, "total": 2})).is_err());
}

#[test]
fn task_count_non_map_value_returns_error() {
    // TaskCount expects a map, not a scalar
    assert!(make_task_count(json!(42)).is_err());
}

#[test]
fn task_count_array_value_returns_error() {
    assert!(make_task_count(json!([1, 2, 3])).is_err());
}

// =========================================================================
// TaskCount serialization
// =========================================================================

#[test]
fn task_count_per_slot_serializes_correctly() {
    let tc = TaskCount::PerSlot(3);
    let json = serde_json::to_value(&tc).unwrap();
    assert_eq!(json, json!({"per_slot": 3}));
}

#[test]
fn task_count_total_serializes_correctly() {
    let tc = TaskCount::Total(8);
    let json = serde_json::to_value(&tc).unwrap();
    assert_eq!(json, json!({"total": 8}));
}

#[test]
fn task_count_per_resource_serializes_correctly() {
    let tc = TaskCount::PerResource(TaskCountPerResource {
        resource_type: "node".to_string(),
        count: 1,
    });
    let json = serde_json::to_value(&tc).unwrap();
    assert!(json.get("per_resource").is_some());
}

#[test]
fn task_count_extension_serializes_with_custom_key() {
    let tc = TaskCount::Extension("my_key".to_string(), json!(99));
    let json = serde_json::to_value(&tc).unwrap();
    assert_eq!(json, json!({"my_key": 99}));
}

#[test]
fn task_count_serialized_map_has_exactly_one_key() {
    let cases = vec![
        TaskCount::PerSlot(1),
        TaskCount::Total(2),
        TaskCount::Extension("ext".to_string(), json!(true)),
    ];
    for tc in cases {
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(
            json.as_object().unwrap().len(),
            1,
            "Expected exactly one key in serialized TaskCount"
        );
    }
}

#[test]
fn task_count_per_resource_round_trips() {
    let tc = TaskCount::PerResource(TaskCountPerResource {
        resource_type: "gpu".to_string(),
        count: 2,
    });
    let json = serde_json::to_value(&tc).unwrap();
    let decoded: TaskCount = serde_json::from_value(json).unwrap();
    assert_eq!(decoded, tc);
}

// =========================================================================
// TaskCount Display (no_debug — only Display generated)
// =========================================================================

#[test]
fn task_count_display_is_non_empty() {
    let tc = TaskCount::PerSlot(5);
    assert!(!tc.to_string().is_empty());
}

#[test]
fn task_count_display_contains_key_name() {
    let tc = TaskCount::PerSlot(3);
    assert!(tc.to_string().contains("per_slot"));
}

// =========================================================================
// Task — basic construction and serde
// =========================================================================

#[test]
fn task_minimal_deserialization_succeeds() {
    assert!(make_task(minimal_task_json()).is_ok());
}

#[test]
fn task_stores_command() {
    let t = make_task(minimal_task_json()).unwrap();
    assert_eq!(t.command, vec!["echo", "hello"]);
}

#[test]
fn task_stores_slot() {
    let t = make_task(minimal_task_json()).unwrap();
    assert_eq!(t.slot, "default");
}

#[test]
fn task_stores_count() {
    let t = make_task(minimal_task_json()).unwrap();
    assert!(matches!(t.count, TaskCount::PerSlot(1)));
}

#[test]
fn task_optional_fields_are_none_when_absent() {
    let t = make_task(minimal_task_json()).unwrap();
    assert!(t.attributes.is_none());
    assert!(t.distribution.is_none());
}

#[test]
fn task_stores_attributes_when_provided() {
    let json = json!({
        "command": ["echo"],
        "slot": "default",
        "count": {"per_slot": 1},
        "attributes": {"foo": "bar", "num": 42}
    });
    let t = make_task(json).unwrap();
    assert!(t.attributes.is_some());
    let attrs = t.attributes.unwrap();
    assert_eq!(attrs.get("foo").unwrap(), &json!("bar"));
    assert_eq!(attrs.get("num").unwrap(), &json!(42));
}

#[test]
fn task_stores_distribution_when_provided() {
    let json = json!({
        "command": ["echo"],
        "slot": "default",
        "count": {"per_slot": 1},
        "distribution": "cyclic"
    });
    let t = make_task(json).unwrap();
    assert_eq!(t.distribution.as_deref(), Some("cyclic"));
}

#[test]
fn task_missing_command_returns_error() {
    let json = json!({
        "slot": "default",
        "count": {"per_slot": 1}
    });
    assert!(make_task(json).is_err());
}

#[test]
fn task_missing_slot_returns_error() {
    let json = json!({
        "command": ["echo"],
        "count": {"per_slot": 1}
    });
    assert!(make_task(json).is_err());
}

#[test]
fn task_missing_count_returns_error() {
    let json = json!({
        "command": ["echo"],
        "slot": "default"
    });
    assert!(make_task(json).is_err());
}

#[test]
fn task_invalid_count_propagates_error() {
    let json = json!({
        "command": ["echo"],
        "slot": "default",
        "count": {}
    });
    assert!(make_task(json).is_err());
}

// =========================================================================
// Task serialization
// =========================================================================

#[test]
fn task_round_trips_minimal() {
    let original = make_task(minimal_task_json()).unwrap();
    let json = serde_json::to_value(&original).unwrap();
    let decoded = make_task(json).unwrap();
    assert_eq!(decoded.command, original.command);
    assert_eq!(decoded.slot, original.slot);
    assert_eq!(decoded.count, original.count);
}

#[test]
fn task_round_trips_all_fields() {
    let json = json!({
        "command": ["bash", "-c", "echo hello"],
        "slot": "gpu",
        "count": {"per_resource": {"type": "gpu", "count": 2}},
        "attributes": {"gpu_type": "a100"},
        "distribution": "block"
    });
    let original = make_task(json).unwrap();
    let encoded = serde_json::to_value(&original).unwrap();
    let decoded = make_task(encoded).unwrap();
    assert_eq!(decoded.command, original.command);
    assert_eq!(decoded.slot, original.slot);
    assert_eq!(decoded.count, original.count);
    assert_eq!(decoded.attributes, original.attributes);
    assert_eq!(decoded.distribution, original.distribution);
}

#[test]
fn task_serialization_omits_none_optional_fields() {
    let t = make_task(minimal_task_json()).unwrap();
    let json = serde_json::to_value(&t).unwrap();
    assert!(json.get("attributes").is_none());
    assert!(json.get("distribution").is_none());
}

#[test]
fn task_serialization_includes_present_optional_fields() {
    let json = json!({
        "command": ["echo"],
        "slot": "default",
        "count": {"per_slot": 1},
        "attributes": {"key": "value"},
        "distribution": "cyclic"
    });
    let t = make_task(json).unwrap();
    let encoded = serde_json::to_value(&t).unwrap();
    assert!(encoded.get("attributes").is_some());
    assert!(encoded.get("distribution").is_some());
}

// =========================================================================
// Task Display (with Debug and Display)
// =========================================================================

#[test]
fn task_display_is_non_empty() {
    let t = make_task(minimal_task_json()).unwrap();
    assert!(!t.to_string().is_empty());
}

#[test]
fn task_debug_format_starts_with_type_name() {
    let t = make_task(minimal_task_json()).unwrap();
    assert!(format!("{t:?}").starts_with("Task"));
}

#[test]
fn task_debug_format_contains_command() {
    let t = make_task(minimal_task_json()).unwrap();
    let debug_str = format!("{t:?}");
    assert!(debug_str.contains("echo"));
}
