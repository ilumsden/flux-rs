use std::time::Duration;

use crate::duration::FluxDuration;
use crate::error::FluxError;

// =========================================================================
// From / Into conversions
// =========================================================================

#[test]
fn from_f64_yields_secs_variant() {
    let d = FluxDuration::from(2.5_f64);
    assert!(matches!(d, FluxDuration::Secs(s) if s == 2.5));
}

#[test]
fn from_u64_yields_secs_variant() {
    let d = FluxDuration::from(3_u64);
    assert!(matches!(d, FluxDuration::Secs(s) if s == 3.0));
}

#[test]
fn from_i32_yields_secs_variant() {
    let d = FluxDuration::from(7_i32);
    assert!(matches!(d, FluxDuration::Secs(s) if s == 7.0));
}

#[test]
fn from_str_yields_fsd_variant() {
    let d = FluxDuration::from("1.5s");
    assert!(matches!(d, FluxDuration::Fsd("1.5s")));
}

#[test]
fn from_duration_yields_standard_variant() {
    let dur = Duration::from_secs(10);
    let d = FluxDuration::from(dur);
    assert!(matches!(d, FluxDuration::Standard(inner) if inner == Duration::from_secs(10)));
}

// =========================================================================
// TryFrom<FluxDuration> for f64
// =========================================================================

// --- Secs variant ---

#[test]
fn secs_positive_converts_to_f64() {
    let result: crate::error::Result<f64> = FluxDuration::Secs(4.5).try_into();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 4.5);
}

#[test]
fn secs_zero_converts_to_f64() {
    // Zero is a valid duration (unlimited / best-effort in Flux)
    let result: crate::error::Result<f64> = FluxDuration::Secs(0.0).try_into();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0.0);
}

#[test]
fn secs_negative_returns_logic_error() {
    let result: crate::error::Result<f64> = FluxDuration::Secs(-1.0).try_into();
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn secs_nan_returns_logic_error() {
    let result: crate::error::Result<f64> = FluxDuration::Secs(f64::NAN).try_into();
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn secs_infinite_returns_logic_error() {
    let result: crate::error::Result<f64> = FluxDuration::Secs(f64::INFINITY).try_into();
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

#[test]
fn secs_neg_infinite_returns_logic_error() {
    let result: crate::error::Result<f64> = FluxDuration::Secs(f64::NEG_INFINITY).try_into();
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

// --- Standard variant ---

#[test]
fn standard_duration_converts_to_f64() {
    let dur = Duration::from_millis(1500);
    let result: crate::error::Result<f64> = FluxDuration::Standard(dur).try_into();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1.5);
}

#[test]
fn standard_duration_zero_converts_to_f64() {
    let result: crate::error::Result<f64> = FluxDuration::Standard(Duration::ZERO).try_into();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0.0);
}

// --- Fsd variant ---

#[test]
fn fsd_valid_string_converts_to_f64() {
    // "1s" is a minimal valid RFC 23 Flux Standard Duration string.
    // Adjust if parse_fsd expects a different canonical form.
    let result: crate::error::Result<f64> = FluxDuration::Fsd("1s").try_into();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1.0);
}

#[test]
fn fsd_invalid_string_returns_error() {
    let result: crate::error::Result<f64> = FluxDuration::Fsd("not_a_duration").try_into();
    assert!(result.is_err());
}

// =========================================================================
// Display
// =========================================================================

#[test]
fn secs_display_format() {
    let d = FluxDuration::Secs(1.5);
    assert_eq!(d.to_string(), "1.5s");
}

#[test]
fn secs_display_whole_number_format() {
    let d = FluxDuration::Secs(2.0);
    assert_eq!(d.to_string(), "2s");
}

#[test]
fn fsd_display_passes_through_str() {
    let d = FluxDuration::Fsd("1h30m");
    assert_eq!(d.to_string(), "1h30m");
}

#[test]
fn standard_display_format() {
    let d = FluxDuration::Standard(Duration::from_secs(2));
    assert_eq!(d.to_string(), "2s");
}

#[test]
fn standard_display_fractional_format() {
    let d = FluxDuration::Standard(Duration::from_millis(500));
    assert_eq!(d.to_string(), "0.5s");
}
