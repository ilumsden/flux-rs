use flux_sys::core::{flux_jobid_t, FLUX_JOBID_ANY};

use crate::error::FluxError;
use crate::job::jobid::{JobId, JobIdEncodingType};

// =========================================================================
// JobIdEncodingType
// =========================================================================

#[test]
fn encoding_type_as_str_returns_correct_strings() {
    assert_eq!(JobIdEncodingType::Dec.as_str(), "dec");
    assert_eq!(JobIdEncodingType::F58.as_str(), "f58");
    assert_eq!(JobIdEncodingType::Emoji.as_str(), "emoji");
}

#[test]
fn encoding_type_as_c_str_returns_correct_c_strings() {
    assert_eq!(JobIdEncodingType::Hex.as_c_str(), c"hex");
    assert_eq!(JobIdEncodingType::Words.as_c_str(), c"words");
}

// =========================================================================
// JobId::parse
// =========================================================================

#[test]
fn parse_valid_string_succeeds() {
    // Create a valid string by encoding a known ID first
    let original_id = JobId::from(12345);
    let f58_str = original_id.f58().unwrap();

    let parsed_id = JobId::parse(&f58_str).unwrap();
    assert_eq!(parsed_id, original_id);
}

#[test]
fn parse_invalid_string_returns_error() {
    assert!(JobId::parse("not_a_valid_job_id!").is_err());
}

#[test]
fn parse_nul_byte_returns_error() {
    assert!(JobId::parse("bad\0id").is_err());
}

// =========================================================================
// JobId::encode / encode_with_max_size
// =========================================================================

#[test]
fn encode_with_max_size_too_small_returns_error() {
    let id = JobId::from(123456789);
    // Force the EOVERFLOW loop to hit the limit by providing a tiny max size
    let result = id.encode_with_max_size(JobIdEncodingType::F58, 2);
    assert!(matches!(result, Err(FluxError::Logic(_))));
}

// =========================================================================
// Encoding Helpers
// =========================================================================

#[test]
fn encode_dec_succeeds() {
    let id = JobId::from(42);
    assert!(!id.dec().unwrap().is_empty());
}

#[test]
fn encode_hex_succeeds() {
    let id = JobId::from(42);
    assert!(!id.hex().unwrap().is_empty());
}

#[test]
fn encode_kvs_succeeds() {
    let id = JobId::from(42);
    assert!(!id.kvs().unwrap().is_empty());
}

#[test]
fn encode_dothex_succeeds() {
    let id = JobId::from(42);
    assert!(!id.dothex().unwrap().is_empty());
}

#[test]
fn encode_words_succeeds() {
    let id = JobId::from(42);
    assert!(!id.words().unwrap().is_empty());
}

#[test]
fn encode_f58_succeeds() {
    let id = JobId::from(42);
    assert!(!id.f58().unwrap().is_empty());
}

#[test]
fn encode_f58plain_succeeds() {
    let id = JobId::from(42);
    assert!(!id.f58plain().unwrap().is_empty());
}

#[test]
fn encode_emoji_succeeds() {
    let id = JobId::from(42);
    assert!(!id.emoji().unwrap().is_empty());
}

// =========================================================================
// Traits (Deref, DerefMut, Display, From, Default)
// =========================================================================

#[test]
fn deref_returns_inner_value() {
    let id = JobId::from(99);
    assert_eq!(*id, 99);
}

#[test]
fn deref_mut_allows_mutation() {
    let mut id = JobId::from(99);
    *id = 100;
    assert_eq!(*id, 100);
}

#[test]
fn display_uses_f58_encoding() {
    let id = JobId::from(12345);
    let display_str = id.to_string();
    let f58_str = id.f58().unwrap();
    assert_eq!(display_str, f58_str);
}

#[test]
fn from_flux_jobid_t_stores_value() {
    let raw: flux_jobid_t = 777;
    let id = JobId::from(raw);
    assert_eq!(*id, 777);
}

#[test]
fn default_returns_flux_jobid_any() {
    let id = JobId::default();
    assert_eq!(*id, FLUX_JOBID_ANY as u64);
}
