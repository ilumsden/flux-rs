use serde_json::json;

use crate::job::jobspec::resource_vertex::ResourceVertex;

// =========================================================================
// Helpers
// =========================================================================

/// Build a ResourceVertex from a JSON value, going through the full
/// TryFrom<RawResourceVertex> validation path.
fn make_vertex(value: serde_json::Value) -> Result<ResourceVertex, serde_json::Error> {
    serde_json::from_value::<ResourceVertex>(value)
}

// =========================================================================
// ResourceVertex — valid construction
// =========================================================================

#[test]
fn vertex_node_with_integer_count_succeeds() {
    assert!(make_vertex(json!({"type": "node", "count": 1})).is_ok());
}

#[test]
fn vertex_core_with_integer_count_succeeds() {
    assert!(make_vertex(json!({"type": "core", "count": 4})).is_ok());
}

#[test]
fn vertex_slot_with_label_succeeds() {
    assert!(make_vertex(json!({"type": "slot", "count": 1, "label": "task"})).is_ok());
}

#[test]
fn vertex_stores_resource_type() {
    let v = make_vertex(json!({"type": "node", "count": 1})).unwrap();
    assert_eq!(v.resource_type, "node");
}

#[test]
fn vertex_optional_fields_are_none_when_absent() {
    let v = make_vertex(json!({"type": "node", "count": 1})).unwrap();
    assert!(v.unit.is_none());
    assert!(v.exclusive.is_none());
    assert!(v.with.is_none());
    assert!(v.label.is_none());
    assert!(v.id.is_none());
}

#[test]
fn vertex_stores_unit_when_provided() {
    let v = make_vertex(json!({"type": "node", "count": 1, "unit": ""})).unwrap();
    assert_eq!(v.unit.as_deref(), Some(""));
}

#[test]
fn vertex_stores_exclusive_true() {
    let v = make_vertex(json!({"type": "node", "count": 1, "exclusive": true})).unwrap();
    assert_eq!(v.exclusive, Some(true));
}

#[test]
fn vertex_stores_exclusive_false() {
    let v = make_vertex(json!({"type": "node", "count": 1, "exclusive": false})).unwrap();
    assert_eq!(v.exclusive, Some(false));
}

#[test]
fn vertex_stores_label_when_provided() {
    let v = make_vertex(json!({"type": "node", "count": 1, "label": "worker"})).unwrap();
    assert_eq!(v.label.as_deref(), Some("worker"));
}

#[test]
fn vertex_stores_id_when_provided() {
    let v = make_vertex(json!({"type": "node", "count": 1, "id": "rank0"})).unwrap();
    assert_eq!(v.id.as_deref(), Some("rank0"));
}

// =========================================================================
// ResourceVertex — with field
// =========================================================================

#[test]
fn vertex_with_single_child_succeeds() {
    assert!(
        make_vertex(json!({
            "type": "node",
            "count": 1,
            "with": [{"type": "core", "count": 4}]
        }))
        .is_ok()
    );
}

#[test]
fn vertex_with_multiple_children_succeeds() {
    assert!(
        make_vertex(json!({
            "type": "node",
            "count": 1,
            "with": [
                {"type": "core", "count": 4},
                {"type": "gpu", "count": 1}
            ]
        }))
        .is_ok()
    );
}

#[test]
fn vertex_with_stores_correct_child_count() {
    let v = make_vertex(json!({
        "type": "node",
        "count": 1,
        "with": [
            {"type": "core", "count": 4},
            {"type": "gpu", "count": 1}
        ]
    }))
    .unwrap();
    assert_eq!(v.with.as_ref().unwrap().len(), 2);
}

#[test]
fn vertex_with_stores_correct_child_type() {
    let v = make_vertex(json!({
        "type": "node",
        "count": 1,
        "with": [{"type": "core", "count": 4}]
    }))
    .unwrap();
    assert_eq!(v.with.as_ref().unwrap()[0].resource_type, "core");
}

#[test]
fn vertex_with_deeply_nested_children_succeeds() {
    assert!(
        make_vertex(json!({
            "type": "node",
            "count": 1,
            "with": [{
                "type": "slot",
                "count": 1,
                "label": "task",
                "with": [{"type": "core", "count": 1}]
            }]
        }))
        .is_ok()
    );
}

// =========================================================================
// ResourceVertex — validation errors
// =========================================================================

#[test]
fn vertex_empty_with_returns_error() {
    assert!(
        make_vertex(json!({
            "type": "node",
            "count": 1,
            "with": []
        }))
        .is_err()
    );
}

#[test]
fn vertex_slot_without_label_returns_error() {
    assert!(make_vertex(json!({"type": "slot", "count": 1})).is_err());
}

#[test]
fn vertex_slot_with_null_label_returns_error() {
    // Explicit null is equivalent to absent for Option fields
    assert!(make_vertex(json!({"type": "slot", "count": 1, "label": null})).is_err());
}

#[test]
fn vertex_non_slot_without_label_succeeds() {
    // Only "slot" requires a label — other types do not
    assert!(make_vertex(json!({"type": "node", "count": 1})).is_ok());
}

#[test]
fn vertex_missing_type_returns_error() {
    assert!(make_vertex(json!({"count": 1})).is_err());
}

#[test]
fn vertex_missing_count_returns_error() {
    assert!(make_vertex(json!({"type": "node"})).is_err());
}

// =========================================================================
// ResourceVertex — count variants
// =========================================================================

#[test]
fn vertex_with_dict_count_succeeds() {
    assert!(
        make_vertex(json!({
            "type": "node",
            "count": {"min": 1, "max": 8}
        }))
        .is_ok()
    );
}

#[test]
fn vertex_with_idset_count_succeeds() {
    assert!(
        make_vertex(json!({
            "type": "node",
            "count": "0-3"
        }))
        .is_ok()
    );
}

// =========================================================================
// ResourceVertex serde round-trip
// =========================================================================

#[test]
fn vertex_round_trips_minimal() {
    let original = make_vertex(json!({"type": "core", "count": 4})).unwrap();
    let json = serde_json::to_value(&original).unwrap();
    let decoded = make_vertex(json).unwrap();
    assert_eq!(decoded.resource_type, original.resource_type);
}

#[test]
fn vertex_round_trips_all_fields() {
    let original = make_vertex(json!({
        "type": "node",
        "count": 2,
        "unit": "nodes",
        "exclusive": true,
        "label": "worker",
        "id": "n0",
        "with": [{"type": "core", "count": 4}]
    }))
    .unwrap();
    let json = serde_json::to_value(&original).unwrap();
    let decoded = make_vertex(json).unwrap();
    assert_eq!(decoded.resource_type, original.resource_type);
    assert_eq!(decoded.unit, original.unit);
    assert_eq!(decoded.exclusive, original.exclusive);
    assert_eq!(decoded.label, original.label);
    assert_eq!(decoded.id, original.id);
    assert_eq!(decoded.with.as_ref().unwrap().len(), 1);
}

#[test]
fn vertex_serialization_omits_none_fields() {
    let v = make_vertex(json!({"type": "node", "count": 1})).unwrap();
    let json = serde_json::to_value(&v).unwrap();
    assert!(json.get("unit").is_none());
    assert!(json.get("exclusive").is_none());
    assert!(json.get("with").is_none());
    assert!(json.get("label").is_none());
    assert!(json.get("id").is_none());
}

#[test]
fn vertex_serialization_uses_type_key_not_resource_type() {
    // The field is renamed to "type" in serde, not "resource_type"
    let v = make_vertex(json!({"type": "node", "count": 1})).unwrap();
    let json = serde_json::to_value(&v).unwrap();
    assert!(json.get("type").is_some());
    assert!(json.get("resource_type").is_none());
}

// =========================================================================
// impl_serde_repr_str! — Display and Debug
// =========================================================================

#[test]
fn display_produces_non_empty_string() {
    let v = make_vertex(json!({"type": "node", "count": 1})).unwrap();
    assert!(!v.to_string().is_empty());
}

#[test]
fn display_contains_type_field() {
    let v = make_vertex(json!({"type": "node", "count": 1})).unwrap();
    assert!(v.to_string().contains("node"));
}

#[test]
fn debug_format_starts_with_type_name() {
    let v = make_vertex(json!({"type": "node", "count": 1})).unwrap();
    assert!(format!("{v:?}").starts_with("ResourceVertex("));
}

#[test]
fn debug_format_contains_serialized_value() {
    let v = make_vertex(json!({"type": "core", "count": 2})).unwrap();
    let debug = format!("{v:?}");
    assert!(debug.contains("core"));
}
