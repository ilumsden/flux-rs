use serde::Serialize;

use crate::error::FluxError;
use crate::utils::{impl_serde_repr_str, parse_fsd, SignalCode};

// =========================================================================
// Helpers
// =========================================================================

// Minimal local types for exercising each arm of impl_serde_repr_str!.
// They must implement Serialize so the macro's serde_json::to_string call
// succeeds.  A newtype over u32 serializes as a bare JSON integer, making
// the expected output strings easy to reason about.

#[derive(Serialize)]
struct TestRepr(u32);
impl_serde_repr_str!(TestRepr);

// no_debug: only Display is generated; Debug must NOT be derived separately.
#[derive(Serialize)]
struct TestReprNoDebug(u32);
impl_serde_repr_str!(no_debug TestReprNoDebug);

// no_display: only Debug is generated; Display must NOT be derived separately.
#[derive(Serialize)]
struct TestReprNoDisplay(u32);
impl_serde_repr_str!(no_display TestReprNoDisplay);

// =========================================================================
// SignalCode::new
// =========================================================================

#[test]
fn new_positive_returns_some() {
    assert!(SignalCode::new(1).is_some());
}

#[test]
fn new_large_positive_returns_some() {
    assert!(SignalCode::new(i32::MAX).is_some());
}

#[test]
fn new_zero_returns_none() {
    assert!(SignalCode::new(0).is_none());
}

#[test]
fn new_negative_returns_none() {
    assert!(SignalCode::new(-1).is_none());
}

// =========================================================================
// SignalCode::as_raw
// =========================================================================

#[test]
fn as_raw_returns_inner_value() {
    let sig = SignalCode::new(9).unwrap();
    assert_eq!(sig.as_raw(), 9);
}

// =========================================================================
// SignalCode Deref / DerefMut
// =========================================================================

#[test]
fn deref_gives_inner_value() {
    let sig = SignalCode::new(15).unwrap();
    assert_eq!(*sig, 15);
}

// =========================================================================
// SignalCode derived traits (Copy, PartialEq, Eq)
// =========================================================================

#[test]
fn copy_produces_independent_value() {
    let sig = SignalCode::new(7).unwrap();
    // Copy: binding `copy` does not move `sig`
    let copy = sig;
    assert_eq!(sig.as_raw(), copy.as_raw());
}

#[test]
fn partial_eq_same_value_is_equal() {
    let a = SignalCode::new(9).unwrap();
    let b = SignalCode::new(9).unwrap();
    assert_eq!(a, b);
}

#[test]
fn partial_eq_different_values_is_not_equal() {
    let a = SignalCode::new(9).unwrap();
    let b = SignalCode::new(15).unwrap();
    assert_ne!(a, b);
}

// =========================================================================
// parse_fsd — infinity keywords
// =========================================================================

#[test]
fn parse_fsd_inf_lowercase_returns_infinity() {
    assert_eq!(parse_fsd("inf").unwrap(), f64::INFINITY);
}

#[test]
fn parse_fsd_infinity_lowercase_returns_infinity() {
    assert_eq!(parse_fsd("infinity").unwrap(), f64::INFINITY);
}

#[test]
fn parse_fsd_inf_uppercase_returns_infinity() {
    assert_eq!(parse_fsd("INF").unwrap(), f64::INFINITY);
}

#[test]
fn parse_fsd_infinity_uppercase_returns_infinity() {
    assert_eq!(parse_fsd("INFINITY").unwrap(), f64::INFINITY);
}

// =========================================================================
// parse_fsd — unit suffixes
// =========================================================================

#[test]
fn parse_fsd_seconds_suffix() {
    assert_eq!(parse_fsd("1s").unwrap(), 1.0);
}

#[test]
fn parse_fsd_fractional_seconds_suffix() {
    assert_eq!(parse_fsd("1.5s").unwrap(), 1.5);
}

#[test]
fn parse_fsd_milliseconds_suffix() {
    let result = parse_fsd("500ms").unwrap();
    assert!((result - 0.5).abs() < f64::EPSILON);
}

#[test]
fn parse_fsd_minutes_suffix() {
    assert_eq!(parse_fsd("2m").unwrap(), 120.0);
}

#[test]
fn parse_fsd_hours_suffix() {
    assert_eq!(parse_fsd("1h").unwrap(), 3600.0);
}

#[test]
fn parse_fsd_days_suffix() {
    assert_eq!(parse_fsd("1d").unwrap(), 86400.0);
}

#[test]
fn parse_fsd_no_suffix_treated_as_seconds() {
    assert_eq!(parse_fsd("5").unwrap(), 5.0);
}

#[test]
fn parse_fsd_zero_seconds() {
    assert_eq!(parse_fsd("0s").unwrap(), 0.0);
}

// =========================================================================
// parse_fsd — error cases
// =========================================================================

#[test]
fn parse_fsd_negative_seconds_returns_logic_error() {
    assert!(matches!(parse_fsd("-1s"), Err(FluxError::Logic(_))));
}

#[test]
fn parse_fsd_negative_milliseconds_returns_logic_error() {
    assert!(matches!(parse_fsd("-500ms"), Err(FluxError::Logic(_))));
}

#[test]
fn parse_fsd_nan_producing_input_returns_logic_error() {
    // "nan" parses as f64::NAN; appending 's' strips the suffix and
    // passes "nan" to the float parser.
    assert!(matches!(parse_fsd("nans"), Err(FluxError::Logic(_))));
}

#[test]
fn parse_fsd_invalid_string_returns_duration_parse_error() {
    assert!(matches!(
        parse_fsd("not_a_duration"),
        Err(FluxError::DurationParseError(_))
    ));
}

#[test]
fn parse_fsd_empty_string_returns_error() {
    assert!(parse_fsd("").is_err());
}

#[test]
fn parse_fsd_suffix_only_returns_error() {
    // "s" strips to "", which fails f64 parsing.
    assert!(parse_fsd("s").is_err());
}

// =========================================================================
// impl_serde_repr_str! — full variant (both Debug and Display)
// =========================================================================

#[test]
fn serde_repr_str_display_format() {
    // serde_json serializes a newtype tuple struct as its inner value.
    let val = TestRepr(42);
    assert_eq!(val.to_string(), "42");
}

#[test]
fn serde_repr_str_debug_format() {
    // The Debug impl wraps the serialized value as "TypeName(serialized)".
    let val = TestRepr(42);
    assert_eq!(format!("{:?}", val), "TestRepr(42)");
}

// =========================================================================
// impl_serde_repr_str! — no_debug variant (Display only)
// =========================================================================

#[test]
fn serde_repr_str_no_debug_display_format() {
    let val = TestReprNoDebug(7);
    assert_eq!(val.to_string(), "7");
}

// =========================================================================
// impl_serde_repr_str! — no_display variant (Debug only)
// =========================================================================

#[test]
fn serde_repr_str_no_display_debug_format() {
    let val = TestReprNoDisplay(99);
    assert_eq!(format!("{:?}", val), "TestReprNoDisplay(99)");
}
