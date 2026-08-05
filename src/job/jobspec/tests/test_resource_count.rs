use serde_json::json;

use crate::job::jobspec::resource_count::{
    ResourceCount, ResourceCountDict, ResourceCountOperator,
};

// =========================================================================
// Helpers
// =========================================================================

/// Build a ResourceCountDict from a JSON value, going through the full
/// TryFrom<RawResourceCountDict> validation path.
fn make_dict(value: serde_json::Value) -> Result<ResourceCountDict, serde_json::Error> {
    serde_json::from_value::<ResourceCountDict>(value)
}

// =========================================================================
// ResourceCountOperator serde
// =========================================================================

#[test]
fn operator_plus_serializes_correctly() {
    let op = ResourceCountOperator::Plus;
    let json = serde_json::to_string(&op).unwrap();
    assert_eq!(json, "\"+\"");
}

#[test]
fn operator_multiply_serializes_correctly() {
    let op = ResourceCountOperator::Multipy;
    let json = serde_json::to_string(&op).unwrap();
    assert_eq!(json, "\"*\"");
}

#[test]
fn operator_exponentiate_serializes_correctly() {
    let op = ResourceCountOperator::Exponentiate;
    let json = serde_json::to_string(&op).unwrap();
    assert_eq!(json, "\"^\"");
}

#[test]
fn operator_plus_deserializes_correctly() {
    let op: ResourceCountOperator = serde_json::from_str("\"+\"").unwrap();
    assert_eq!(op, ResourceCountOperator::Plus);
}

#[test]
fn operator_multiply_deserializes_correctly() {
    let op: ResourceCountOperator = serde_json::from_str("\"*\"").unwrap();
    assert_eq!(op, ResourceCountOperator::Multipy);
}

#[test]
fn operator_exponentiate_deserializes_correctly() {
    let op: ResourceCountOperator = serde_json::from_str("\"^\"").unwrap();
    assert_eq!(op, ResourceCountOperator::Exponentiate);
}

#[test]
fn operator_invalid_string_returns_error() {
    assert!(serde_json::from_str::<ResourceCountOperator>("\"invalid\"").is_err());
}

#[test]
fn operator_round_trips_plus() {
    let op = ResourceCountOperator::Plus;
    let json = serde_json::to_string(&op).unwrap();
    let decoded: ResourceCountOperator = serde_json::from_str(&json).unwrap();
    assert_eq!(op, decoded);
}

#[test]
fn operator_round_trips_multiply() {
    let op = ResourceCountOperator::Multipy;
    let json = serde_json::to_string(&op).unwrap();
    let decoded: ResourceCountOperator = serde_json::from_str(&json).unwrap();
    assert_eq!(op, decoded);
}

#[test]
fn operator_round_trips_exponentiate() {
    let op = ResourceCountOperator::Exponentiate;
    let json = serde_json::to_string(&op).unwrap();
    let decoded: ResourceCountOperator = serde_json::from_str(&json).unwrap();
    assert_eq!(op, decoded);
}

// =========================================================================
// ResourceCountOperator Display (impl_serde_repr_str! no_debug)
// =========================================================================

#[test]
fn operator_display_plus() {
    assert_eq!(ResourceCountOperator::Plus.to_string(), "\"+\"");
}

#[test]
fn operator_display_multiply() {
    assert_eq!(ResourceCountOperator::Multipy.to_string(), "\"*\"");
}

#[test]
fn operator_display_exponentiate() {
    assert_eq!(ResourceCountOperator::Exponentiate.to_string(), "\"^\"");
}

// =========================================================================
// ResourceCountDict — valid construction
// =========================================================================

#[test]
fn dict_min_only_succeeds() {
    assert!(make_dict(json!({"min": 1})).is_ok());
}

#[test]
fn dict_min_stores_correct_value() {
    let d = make_dict(json!({"min": 4})).unwrap();
    assert_eq!(d.min, 4);
}

#[test]
fn dict_min_zero_succeeds() {
    assert!(make_dict(json!({"min": 0})).is_ok());
}

#[test]
fn dict_min_and_max_equal_succeeds() {
    assert!(make_dict(json!({"min": 4, "max": 4})).is_ok());
}

#[test]
fn dict_max_greater_than_min_succeeds() {
    assert!(make_dict(json!({"min": 1, "max": 8})).is_ok());
}

#[test]
fn dict_stores_max_when_provided() {
    let d = make_dict(json!({"min": 1, "max": 8})).unwrap();
    assert_eq!(d.max, Some(8));
}

#[test]
fn dict_max_is_none_when_absent() {
    let d = make_dict(json!({"min": 1})).unwrap();
    assert!(d.max.is_none());
}

#[test]
fn dict_operator_is_none_when_absent() {
    let d = make_dict(json!({"min": 1})).unwrap();
    assert!(d.operator.is_none());
}

#[test]
fn dict_operand_is_none_when_absent() {
    let d = make_dict(json!({"min": 1})).unwrap();
    assert!(d.operand.is_none());
}

#[test]
fn dict_with_plus_operator_succeeds() {
    assert!(make_dict(json!({"min": 1, "operator": "+"})).is_ok());
}

#[test]
fn dict_with_multiply_operator_and_valid_operand_succeeds() {
    assert!(make_dict(json!({"min": 1, "operator": "*", "operand": 2})).is_ok());
}

#[test]
fn dict_with_exponentiate_operator_and_valid_inputs_succeeds() {
    assert!(make_dict(json!({"min": 2, "operator": "^", "operand": 2})).is_ok());
}

#[test]
fn dict_stores_operator_when_provided() {
    let d = make_dict(json!({"min": 1, "operator": "+"})).unwrap();
    assert_eq!(d.operator, Some(ResourceCountOperator::Plus));
}

#[test]
fn dict_stores_operand_when_provided() {
    let d = make_dict(json!({"min": 1, "operator": "*", "operand": 3})).unwrap();
    assert_eq!(d.operand, Some(3));
}

// =========================================================================
// ResourceCountDict — validation errors
// =========================================================================

#[test]
fn dict_max_less_than_min_returns_error() {
    assert!(make_dict(json!({"min": 4, "max": 2})).is_err());
}

#[test]
fn dict_exponentiate_with_min_less_than_2_returns_error() {
    assert!(make_dict(json!({"min": 1, "operator": "^"})).is_err());
}

#[test]
fn dict_exponentiate_with_min_zero_returns_error() {
    assert!(make_dict(json!({"min": 0, "operator": "^"})).is_err());
}

#[test]
fn dict_exponentiate_with_operand_less_than_2_returns_error() {
    assert!(make_dict(json!({"min": 2, "operator": "^", "operand": 1})).is_err());
}

#[test]
fn dict_exponentiate_with_operand_zero_returns_error() {
    assert!(make_dict(json!({"min": 2, "operator": "^", "operand": 0})).is_err());
}

#[test]
fn dict_multiply_with_operand_less_than_2_returns_error() {
    assert!(make_dict(json!({"min": 1, "operator": "*", "operand": 1})).is_err());
}

#[test]
fn dict_multiply_with_operand_zero_returns_error() {
    assert!(make_dict(json!({"min": 1, "operator": "*", "operand": 0})).is_err());
}

#[test]
fn dict_missing_min_returns_error() {
    assert!(make_dict(json!({"max": 4})).is_err());
}

// =========================================================================
// ResourceCountDict serde round-trip
// =========================================================================

#[test]
fn dict_round_trips_min_only() {
    let d = make_dict(json!({"min": 4})).unwrap();
    let json = serde_json::to_value(&d).unwrap();
    let decoded = make_dict(json).unwrap();
    assert_eq!(d.min, decoded.min);
    assert_eq!(d.max, decoded.max);
}

#[test]
fn dict_round_trips_all_fields() {
    let d = make_dict(json!({
        "min": 2,
        "max": 16,
        "operator": "^",
        "operand": 2
    }))
    .unwrap();
    let json = serde_json::to_value(&d).unwrap();
    let decoded = make_dict(json).unwrap();
    assert_eq!(d.min, decoded.min);
    assert_eq!(d.max, decoded.max);
    assert_eq!(d.operator, decoded.operator);
    assert_eq!(d.operand, decoded.operand);
}

#[test]
fn dict_serialized_json_omits_none_fields() {
    let d = make_dict(json!({"min": 1})).unwrap();
    let json = serde_json::to_value(&d).unwrap();
    assert!(json.get("max").is_none());
    assert!(json.get("operator").is_none());
    assert!(json.get("operand").is_none());
}

// =========================================================================
// ResourceCountDict Debug and Display (impl_serde_repr_str!)
// =========================================================================

#[test]
fn dict_display_contains_min_value() {
    let d = make_dict(json!({"min": 7})).unwrap();
    assert!(d.to_string().contains("7"));
}

#[test]
fn dict_debug_starts_with_type_name() {
    let d = make_dict(json!({"min": 1})).unwrap();
    assert!(format!("{:?}", d).starts_with("ResourceCountDict("));
}

// =========================================================================
// ResourceCount — untagged deserialization order
//
// Declaration order: Integer(usize), Idset(Idset), Dict(ResourceCountDict)
// - JSON number   → Integer
// - JSON string   → Idset (parsed as idset notation)
// - JSON object   → Dict
// =========================================================================

#[test]
fn resource_count_integer_from_json_number() {
    let rc: ResourceCount = serde_json::from_value(json!(4)).unwrap();
    assert!(matches!(rc, ResourceCount::Integer(_)));
}

#[test]
fn resource_count_integer_stores_correct_value() {
    let rc: ResourceCount = serde_json::from_value(json!(4)).unwrap();
    match rc {
        ResourceCount::Integer(n) => assert_eq!(n, 4),
        _ => panic!("Expected Integer variant"),
    }
}

#[test]
fn resource_count_idset_from_json_string() {
    let rc: ResourceCount = serde_json::from_value(json!("1-3")).unwrap();
    assert!(matches!(rc, ResourceCount::Idset(_)));
}

#[test]
fn resource_count_idset_contains_correct_ids() {
    let rc: ResourceCount = serde_json::from_value(json!("1,2,3")).unwrap();
    match rc {
        ResourceCount::Idset(idset) => {
            assert_eq!(idset.len(), 3);
            assert!(idset.contains(1));
            assert!(idset.contains(2));
            assert!(idset.contains(3));
        }
        _ => panic!("Expected Idset variant"),
    }
}

#[test]
fn resource_count_dict_from_json_object() {
    let rc: ResourceCount = serde_json::from_value(json!({"min": 1})).unwrap();
    assert!(matches!(rc, ResourceCount::Dict(_)));
}

#[test]
fn resource_count_dict_stores_correct_min() {
    let rc: ResourceCount = serde_json::from_value(json!({"min": 8})).unwrap();
    match rc {
        ResourceCount::Dict(d) => assert_eq!(d.min, 8),
        _ => panic!("Expected Dict variant"),
    }
}

#[test]
fn resource_count_invalid_idset_string_returns_error() {
    // A string that is not valid idset notation fails all three variants.
    let result = serde_json::from_value::<ResourceCount>(json!("not_an_idset!@#"));
    assert!(result.is_err());
}

#[test]
fn resource_count_dict_with_invalid_max_returns_error() {
    // Validation fires through Dict's TryFrom.
    let result = serde_json::from_value::<ResourceCount>(json!({"min": 4, "max": 1}));
    assert!(result.is_err());
}

// =========================================================================
// ResourceCount Debug and Display (impl_serde_repr_str!)
// =========================================================================

#[test]
fn resource_count_integer_display_is_json_number() {
    let rc = ResourceCount::Integer(42);
    assert_eq!(rc.to_string(), "42");
}

#[test]
fn resource_count_integer_debug_starts_with_type_name() {
    let rc = ResourceCount::Integer(42);
    assert!(format!("{:?}", rc).starts_with("ResourceCount("));
}

#[test]
fn resource_count_dict_display_contains_min() {
    let rc: ResourceCount = serde_json::from_value(json!({"min": 3})).unwrap();
    assert!(rc.to_string().contains("3"));
}

#[test]
fn resource_count_idset_display_is_non_empty_string() {
    let rc: ResourceCount = serde_json::from_value(json!("0-3")).unwrap();
    assert!(!rc.to_string().is_empty());
}
