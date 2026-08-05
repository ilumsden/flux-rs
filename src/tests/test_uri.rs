use std::sync::Mutex;

use crate::error::FluxError;
use crate::uri::{BaseUri, JobUri, UriResolverUri};

// ENV_MUTEX serializes all tests that mutate FLUX_URI_RESOLVE_LOCAL.
// Environment variables are process-wide, and Rust runs tests on multiple
// threads, so unguarded set_var / remove_var calls would race.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

// =========================================================================
// normalize_slashes (private; tested indirectly via JobUri::new)
// =========================================================================

#[test]
fn normalize_slashes_collapses_consecutive_slashes_in_path() {
    let uri = JobUri::new("local:///run//flux///local", None).unwrap();
    assert_eq!(uri.base.path, "/run/flux/local");
}

#[test]
fn normalize_slashes_preserves_already_normalized_path() {
    let uri = JobUri::new("local:///run/flux/local", None).unwrap();
    assert_eq!(uri.base.path, "/run/flux/local");
}

// =========================================================================
// BaseUri::new
// =========================================================================

#[test]
fn base_uri_new_parses_scheme() {
    let uri = BaseUri::new("local:///run/flux/local").unwrap();
    assert_eq!(uri.scheme, "local");
}

#[test]
fn base_uri_new_parses_netloc() {
    let uri = BaseUri::new("ssh://hostname.example.com/path").unwrap();
    assert_eq!(uri.netloc, "hostname.example.com");
}

#[test]
fn base_uri_new_parses_path() {
    let uri = BaseUri::new("local:///run/flux/local").unwrap();
    assert_eq!(uri.path, "/run/flux/local");
}

#[test]
fn base_uri_new_parses_query_string() {
    let uri = BaseUri::new("local:///run/flux/local?key=value").unwrap();
    assert_eq!(uri.query, "key=value");
}

#[test]
fn base_uri_new_empty_query_yields_empty_string() {
    let uri = BaseUri::new("local:///run/flux/local").unwrap();
    assert_eq!(uri.query, "");
}

#[test]
fn base_uri_new_populates_query_dict_single_value() {
    let uri = BaseUri::new("local:///run/flux/local?key=value").unwrap();
    assert_eq!(
        uri.query_dict.get("key").unwrap(),
        &vec!["value".to_string()]
    );
}

#[test]
fn base_uri_new_populates_query_dict_multiple_values_for_same_key() {
    let uri = BaseUri::new("local:///run/flux/local?key=a&key=b").unwrap();
    let values = uri.query_dict.get("key").unwrap();
    assert_eq!(values.len(), 2);
    assert!(values.contains(&"a".to_string()));
    assert!(values.contains(&"b".to_string()));
}

#[test]
fn base_uri_new_parses_fragment() {
    let uri = BaseUri::new("local:///run/flux/local#section").unwrap();
    assert_eq!(uri.fragment, "section");
}

#[test]
fn base_uri_new_empty_fragment_yields_empty_string() {
    let uri = BaseUri::new("local:///run/flux/local").unwrap();
    assert_eq!(uri.fragment, "");
}

#[test]
fn base_uri_new_stores_original_raw_uri() {
    let raw = "local:///run/flux/local";
    let uri = BaseUri::new(raw).unwrap();
    assert_eq!(uri.uri, raw);
}

#[test]
fn base_uri_new_invalid_uri_returns_error() {
    assert!(BaseUri::new("not a valid url !!!").is_err());
}

// =========================================================================
// BaseUri serde
// =========================================================================

#[test]
fn base_uri_round_trips_through_json() {
    let uri = BaseUri::new("local:///run/flux/local?key=val#frag").unwrap();
    let json = serde_json::to_string(&uri).unwrap();
    let decoded: BaseUri = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.uri, uri.uri);
    assert_eq!(decoded.scheme, uri.scheme);
    assert_eq!(decoded.path, uri.path);
    assert_eq!(decoded.query, uri.query);
    assert_eq!(decoded.fragment, uri.fragment);
}

// =========================================================================
// JobUri::new
// =========================================================================

#[test]
fn job_uri_new_with_local_scheme_succeeds() {
    assert!(JobUri::new("local:///run/flux/local", None).is_ok());
}

#[test]
fn job_uri_new_with_ssh_scheme_succeeds() {
    assert!(JobUri::new("ssh://hostname.example.com/path", None).is_ok());
}

#[test]
fn job_uri_new_invalid_uri_returns_error() {
    assert!(JobUri::new("not a valid url !!!", None).is_err());
}

#[test]
fn job_uri_new_stores_remote_hostname() {
    let uri = JobUri::new("local:///run/flux/local", Some("myhost".to_string())).unwrap();
    assert_eq!(uri.remote_hostname.as_deref(), Some("myhost"));
}

#[test]
fn job_uri_new_none_remote_hostname_is_none() {
    let uri = JobUri::new("local:///run/flux/local", None).unwrap();
    assert!(uri.remote_hostname.is_none());
}

// =========================================================================
// JobUri::as_remote
// =========================================================================

#[test]
fn as_remote_ssh_scheme_returns_original_uri() {
    let raw = "ssh://hostname.example.com/path";
    let uri = JobUri::new(raw, None).unwrap();
    assert_eq!(uri.as_remote().unwrap(), raw);
}

#[test]
fn as_remote_local_scheme_with_explicit_hostname_produces_ssh_uri() {
    let uri = JobUri::new("local:///run/flux/local", Some("myhost".to_string())).unwrap();
    assert_eq!(uri.as_remote().unwrap(), "ssh://myhost/run/flux/local");
}

#[test]
fn as_remote_local_scheme_without_hostname_uses_system_hostname() {
    // get_system_hostname() is called — the result is non-deterministic
    // but must always produce a valid ssh:// URI ending in the original path.
    let uri = JobUri::new("local:///run/flux/local", None).unwrap();
    let remote = uri.as_remote().unwrap();
    assert!(
        remote.starts_with("ssh://"),
        "Expected ssh:// prefix, got: {remote}"
    );
    assert!(
        remote.ends_with("/run/flux/local"),
        "Expected path suffix, got: {remote}"
    );
}

#[test]
fn as_remote_unknown_scheme_returns_logic_error() {
    let uri = JobUri::new("custom:///some/path", None).unwrap();
    assert!(matches!(uri.as_remote(), Err(FluxError::Logic(_))));
}

#[test]
fn as_remote_result_is_cached_on_second_call() {
    let uri = JobUri::new("local:///run/flux/local", Some("myhost".to_string())).unwrap();
    let first = uri.as_remote().unwrap();
    let second = uri.as_remote().unwrap();
    assert_eq!(first, second);
}

// =========================================================================
// JobUri::as_local
// =========================================================================

#[test]
fn as_local_local_scheme_returns_original_uri() {
    let raw = "local:///run/flux/local";
    let uri = JobUri::new(raw, None).unwrap();
    assert_eq!(uri.as_local().unwrap(), raw);
}

#[test]
fn as_local_ssh_scheme_produces_local_uri() {
    // format!("local://{}", path) where path = "/path/to/socket"
    // → "local:///path/to/socket" (local:// + /path/to/socket)
    let uri = JobUri::new("ssh://hostname.example.com/path/to/socket", None).unwrap();
    assert_eq!(uri.as_local().unwrap(), "local:///path/to/socket");
}

#[test]
fn as_local_unknown_scheme_returns_logic_error() {
    let uri = JobUri::new("custom:///some/path", None).unwrap();
    assert!(matches!(uri.as_local(), Err(FluxError::Logic(_))));
}

#[test]
fn as_local_result_is_cached_on_second_call() {
    let uri = JobUri::new("ssh://hostname.example.com/path", None).unwrap();
    let first = uri.as_local().unwrap();
    let second = uri.as_local().unwrap();
    assert_eq!(first, second);
}

// =========================================================================
// JobUri Display
//
// All tests here must hold ENV_MUTEX for their full duration to prevent
// parallel tests from observing an inconsistent FLUX_URI_RESOLVE_LOCAL state.
// std::env::set_var / remove_var require unsafe since Rust 1.81 in
// multi-threaded contexts.
// =========================================================================

#[test]
fn display_without_env_var_shows_base_uri() {
    let _guard = ENV_MUTEX.lock().unwrap();
    unsafe { std::env::remove_var("FLUX_URI_RESOLVE_LOCAL") };
    let raw = "local:///run/flux/local";
    let uri = JobUri::new(raw, None).unwrap();
    assert_eq!(uri.to_string(), raw);
}

#[test]
fn display_with_env_var_set_local_scheme_shows_original_uri() {
    let _guard = ENV_MUTEX.lock().unwrap();
    unsafe { std::env::set_var("FLUX_URI_RESOLVE_LOCAL", "1") };
    let raw = "local:///run/flux/local";
    let uri = JobUri::new(raw, None).unwrap();
    let display = uri.to_string();
    unsafe { std::env::remove_var("FLUX_URI_RESOLVE_LOCAL") };
    // local scheme → as_local() returns base.uri unchanged
    assert_eq!(display, raw);
}

#[test]
fn display_with_env_var_set_ssh_scheme_shows_local_form() {
    let _guard = ENV_MUTEX.lock().unwrap();
    unsafe { std::env::set_var("FLUX_URI_RESOLVE_LOCAL", "1") };
    let uri = JobUri::new("ssh://hostname.example.com/path/to/socket", None).unwrap();
    let display = uri.to_string();
    unsafe { std::env::remove_var("FLUX_URI_RESOLVE_LOCAL") };
    assert_eq!(display, "local:///path/to/socket");
}

#[test]
fn display_with_env_var_set_falls_back_to_base_uri_on_as_local_error() {
    let _guard = ENV_MUTEX.lock().unwrap();
    unsafe { std::env::set_var("FLUX_URI_RESOLVE_LOCAL", "1") };
    // custom scheme → as_local() returns Err → Display falls back to base.uri
    let uri = JobUri::new("custom:///some/path", None).unwrap();
    let raw_base_uri = uri.base.uri.clone();
    let display = uri.to_string();
    unsafe { std::env::remove_var("FLUX_URI_RESOLVE_LOCAL") };
    assert_eq!(display, raw_base_uri);
}

// =========================================================================
// JobUri Deref / DerefMut
// =========================================================================

#[test]
fn job_uri_deref_exposes_base_scheme() {
    let uri = JobUri::new("local:///run/flux/local", None).unwrap();
    assert_eq!(uri.scheme, "local");
}

#[test]
fn job_uri_deref_exposes_base_path() {
    let uri = JobUri::new("local:///run/flux/local", None).unwrap();
    assert_eq!(uri.path, "/run/flux/local");
}

#[test]
fn job_uri_deref_mut_allows_path_mutation() {
    let mut uri = JobUri::new("local:///run/flux/local", None).unwrap();
    uri.path = "/new/path".to_string();
    assert_eq!(uri.base.path, "/new/path");
}

// =========================================================================
// JobUri serde
// =========================================================================

#[test]
fn job_uri_serialized_json_omits_cache_fields() {
    let uri = JobUri::new("local:///run/flux/local", Some("myhost".to_string())).unwrap();
    // Populate caches before serializing.
    let _ = uri.as_remote().unwrap();
    let _ = uri.as_local().unwrap();
    let json = serde_json::to_string(&uri).unwrap();
    assert!(
        !json.contains("remote_uri"),
        "remote_uri should be skipped in JSON: {json}"
    );
    assert!(
        !json.contains("local_uri"),
        "local_uri should be skipped in JSON: {json}"
    );
}

#[test]
fn job_uri_deserializes_with_empty_caches() {
    let uri = JobUri::new("local:///run/flux/local", Some("myhost".to_string())).unwrap();
    let _ = uri.as_remote().unwrap(); // populate cache before serializing
    let json = serde_json::to_string(&uri).unwrap();
    let decoded: JobUri = serde_json::from_str(&json).unwrap();
    assert!(
        decoded.remote_uri.borrow().is_none(),
        "Deserialized remote_uri cache should be None"
    );
    assert!(
        decoded.local_uri.borrow().is_none(),
        "Deserialized local_uri cache should be None"
    );
}

#[test]
fn job_uri_round_trips_preserves_key_fields() {
    let uri = JobUri::new(
        "ssh://hostname.example.com/path",
        Some("myhost".to_string()),
    )
    .unwrap();
    let json = serde_json::to_string(&uri).unwrap();
    let decoded: JobUri = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.base.scheme, uri.base.scheme);
    assert_eq!(decoded.base.path, uri.base.path);
    assert_eq!(decoded.remote_hostname, uri.remote_hostname);
}

// =========================================================================
// UriResolverUri::new
//
// UriResolverUri is designed for HPC resolver-style URI strings that are
// URI-like but not strictly URL-compliant (e.g., "pdsh:node1,node2").
// The FXX trick inserts "FXX" after the first ":" to make the string
// parseable by url::Url, then strips "FXX" from the resulting path.
// =========================================================================

#[test]
fn uri_resolver_uri_new_parses_scheme() {
    // "pdsh:node1" → "pdsh:FXXnode1" → scheme=pdsh, path=FXXnode1 → path=node1
    let uri = UriResolverUri::new("pdsh:node1").unwrap();
    assert_eq!(uri.scheme, "pdsh");
}

#[test]
fn uri_resolver_uri_path_has_no_fxx_prefix_after_stripping() {
    let uri = UriResolverUri::new("pdsh:node1").unwrap();
    assert!(
        !uri.path.contains("FXX"),
        "Path should not contain FXX after stripping, got: {}",
        uri.path
    );
}

#[test]
fn uri_resolver_uri_path_contains_expected_content() {
    let uri = UriResolverUri::new("pdsh:node1").unwrap();
    assert!(
        uri.path.contains("node1"),
        "Expected path to contain 'node1', got: {}",
        uri.path
    );
}

#[test]
fn uri_resolver_uri_new_invalid_uri_returns_error() {
    // No colon means no scheme → unchanged string → Url::parse fails.
    assert!(UriResolverUri::new("not a valid url !!!").is_err());
}

// =========================================================================
// UriResolverUri Deref / DerefMut
// =========================================================================

#[test]
fn uri_resolver_deref_exposes_base_scheme() {
    let uri = UriResolverUri::new("pdsh:node1").unwrap();
    assert_eq!(uri.scheme, "pdsh");
}

#[test]
fn uri_resolver_deref_mut_allows_path_mutation() {
    let mut uri = UriResolverUri::new("pdsh:node1").unwrap();
    uri.path = "/modified".to_string();
    assert_eq!(uri.base.path, "/modified");
}
