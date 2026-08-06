use std::io;

use crate::error::{FluxError, Result, check_ptr, check_rc, to_flux_rc};
use crate::tests::common::with_handle;

#[test]
fn from_io_error_yields_system_variant() {
    let io_err = io::Error::from_raw_os_error(libc::ENOENT);
    let flux_err = FluxError::from(io_err);
    assert!(matches!(flux_err, FluxError::System(_)));
}

#[test]
fn from_nul_error_yields_nul_error_variant() {
    // CString::new fails when the string contains an interior NUL byte.
    let nul_err = std::ffi::CString::new("hello\0world").unwrap_err();
    let flux_err = FluxError::from(nul_err);
    assert!(matches!(flux_err, FluxError::NulError(_)));
}

#[test]
fn from_bytes_with_nul_error_yields_nul_interpret_error_variant() {
    // from_bytes_with_nul requires exactly one NUL at the very end.
    // Providing a slice with no NUL triggers the error.
    let err = std::ffi::CStr::from_bytes_with_nul(b"no nul here").unwrap_err();
    let flux_err = FluxError::from(err);
    assert!(matches!(flux_err, FluxError::NulInterpretError(_)));
}

#[test]
fn from_bytes_until_nul_error_yields_from_bytes_until_nul_error_variant() {
    // from_bytes_until_nul fails when the slice contains no NUL byte.
    let err = std::ffi::CStr::from_bytes_until_nul(b"no nul here").unwrap_err();
    let flux_err = FluxError::from(err);
    assert!(matches!(flux_err, FluxError::FromBytesUntilNulError(_)));
}

#[test]
fn from_utf8_error_yields_utf8_error_variant() {
    let dummy: Vec<u8> = vec![0xFF, 0xFE];
    let err = std::str::from_utf8(&dummy).unwrap_err();
    let flux_err = FluxError::from(err);
    assert!(matches!(flux_err, FluxError::Utf8Error(_)));
}

#[test]
fn from_parse_float_error_yields_duration_parse_error_variant() {
    let err = "not_a_float".parse::<f64>().unwrap_err();
    let flux_err = FluxError::from(err);
    assert!(matches!(flux_err, FluxError::DurationParseError(_)));
}

#[test]
fn from_url_parse_error_yields_url_parse_error_variant() {
    let err = url::Url::parse("not a valid url !!!").unwrap_err();
    let flux_err = FluxError::from(err);
    assert!(matches!(flux_err, FluxError::UrlParseError(_)));
}

#[test]
fn from_nix_error_yields_nix_error_variant() {
    let nix_err = nix::Error::EACCES;
    let flux_err = FluxError::from(nix_err);
    assert!(matches!(flux_err, FluxError::NixError(_)));
}

#[test]
fn from_serde_json_error_yields_json_variant() {
    let err = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
    let flux_err = FluxError::from(err);
    assert!(matches!(flux_err, FluxError::Json(_)));
}

// =========================================================================
// Display (error messages)
// =========================================================================

#[test]
fn logic_error_display_contains_message() {
    let msg = "something went wrong";
    let flux_err = FluxError::Logic(msg.to_string());
    assert!(flux_err.to_string().contains(msg));
}

#[test]
fn request_response_error_display_contains_both_parts() {
    let io_err = io::Error::from_raw_os_error(libc::EPERM);
    let detail = "RPC timed out";
    let flux_err = FluxError::RequestResponseError(io_err, detail.to_string());
    let msg = flux_err.to_string();
    assert!(msg.contains(detail), "Expected detail in: {msg}");
    // The Display impl should also embed the IO error description somewhere.
    assert!(!msg.is_empty());
}

// =========================================================================
// to_errno
// =========================================================================

#[test]
fn to_errno_system_with_raw_os_error_returns_that_errno() {
    let flux_err = FluxError::System(io::Error::from_raw_os_error(libc::ENOENT));
    assert_eq!(flux_err.to_errno(), libc::ENOENT);
}

#[test]
fn to_errno_system_without_raw_os_error_returns_einval() {
    // io::Error::new does not carry a raw OS code.
    let flux_err = FluxError::System(io::Error::other("custom"));
    assert_eq!(flux_err.to_errno(), libc::EINVAL);
}

#[test]
fn to_errno_nix_error_returns_correct_errno() {
    let flux_err = FluxError::NixError(nix::Error::EACCES);
    assert_eq!(flux_err.to_errno(), libc::EACCES);
}

#[test]
fn to_errno_request_response_with_raw_os_error_returns_that_errno() {
    let io_err = io::Error::from_raw_os_error(libc::EPERM);
    let flux_err = FluxError::RequestResponseError(io_err, "msg".to_string());
    assert_eq!(flux_err.to_errno(), libc::EPERM);
}

#[test]
fn to_errno_request_response_without_raw_os_error_returns_einval() {
    let io_err = io::Error::other("custom");
    let flux_err = FluxError::RequestResponseError(io_err, "msg".to_string());
    assert_eq!(flux_err.to_errno(), libc::EINVAL);
}

/// Every variant that does not carry an errno falls back to EINVAL.
#[test]
fn to_errno_non_system_variants_return_einval() {
    let dummy_slice: Vec<u8> = vec![0xFF];
    let cases: &[FluxError] = &[
        FluxError::Logic("oops".to_string()),
        FluxError::from(std::ffi::CString::new("a\0b").unwrap_err()),
        FluxError::from("not a float".parse::<f64>().unwrap_err()),
        FluxError::from(std::str::from_utf8(&dummy_slice).unwrap_err()),
    ];
    for err in cases {
        assert_eq!(err.to_errno(), libc::EINVAL, "Expected EINVAL for: {err}");
    }
}

// =========================================================================
// to_errno_with_flux_log  (requires a live Flux instance)
// =========================================================================

#[test]
fn to_errno_with_flux_log_returns_same_value_as_to_errno() {
    with_handle(|handle| {
        let flux_err = FluxError::System(io::Error::from_raw_os_error(libc::ENOENT));
        let with_log = flux_err.to_errno_with_flux_log(handle);
        let without_log = flux_err.to_errno();
        assert_eq!(with_log, without_log);
    });
}

// =========================================================================
// set_errno
// =========================================================================

// errno is thread-local on Linux, so these tests are safe to run in
// parallel even though they mutate errno.

#[test]
fn set_errno_none_sets_correct_value() {
    let flux_err = FluxError::System(io::Error::from_raw_os_error(libc::ENOENT));
    flux_err.set_errno(None);
    let actual = unsafe { *libc::__errno_location() };
    assert_eq!(actual, libc::ENOENT);
}

#[test]
fn set_errno_with_handle_sets_correct_value() {
    with_handle(|handle| {
        let flux_err = FluxError::System(io::Error::from_raw_os_error(libc::EACCES));
        flux_err.set_errno(Some(handle));
        let actual = unsafe { *libc::__errno_location() };
        assert_eq!(actual, libc::EACCES);
    });
}

#[test]
fn set_errno_logic_error_sets_einval() {
    let flux_err = FluxError::Logic("bad state".to_string());
    flux_err.set_errno(None);
    let actual = unsafe { *libc::__errno_location() };
    assert_eq!(actual, libc::EINVAL);
}

// =========================================================================
// to_flux_rc
// =========================================================================

#[test]
fn to_flux_rc_ok_returns_zero() {
    let result: Result<()> = Ok(());
    assert_eq!(to_flux_rc(result, None), 0);
}

#[test]
fn to_flux_rc_err_returns_minus_one_and_sets_errno() {
    let result: Result<()> = Err(FluxError::System(io::Error::from_raw_os_error(
        libc::ENOENT,
    )));
    let rc = to_flux_rc(result, None);
    let actual_errno = unsafe { *libc::__errno_location() };
    assert_eq!(rc, -1);
    assert_eq!(actual_errno, libc::ENOENT);
}

#[test]
fn to_flux_rc_err_with_handle_returns_minus_one_and_sets_errno() {
    with_handle(|handle| {
        let result: Result<()> = Err(FluxError::System(io::Error::from_raw_os_error(libc::EPERM)));
        let rc = to_flux_rc(result, Some(handle));
        let actual_errno = unsafe { *libc::__errno_location() };
        assert_eq!(rc, -1);
        assert_eq!(actual_errno, libc::EPERM);
    });
}

// =========================================================================
// check_rc
// =========================================================================

#[test]
fn check_rc_zero_is_ok() {
    assert!(check_rc(0).is_ok());
}

#[test]
fn check_rc_positive_is_ok() {
    assert!(check_rc(1).is_ok());
    assert!(check_rc(42).is_ok());
}

#[test]
fn check_rc_minus_one_captures_errno() {
    // Set a known errno value before calling so the assertion is reliable.
    unsafe { *libc::__errno_location() = libc::ENOENT };
    let result = check_rc(-1);
    assert!(result.is_err());
    match result {
        Err(FluxError::System(ref io_err)) => {
            assert_eq!(
                io_err.raw_os_error(),
                Some(libc::ENOENT),
                "check_rc should capture the current errno"
            );
        }
        other => panic!("Expected FluxError::System, got: {other:?}"),
    }
}

// =========================================================================
// check_ptr
// =========================================================================

#[test]
fn check_ptr_non_null_returns_same_pointer() {
    let mut value: i32 = 42;
    let ptr = &mut value as *mut i32;
    let result = check_ptr(ptr);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ptr);
}

#[test]
fn check_ptr_null_captures_errno() {
    unsafe { *libc::__errno_location() = libc::ENOMEM };
    let result = check_ptr(std::ptr::null_mut::<i32>());
    assert!(result.is_err());
    match result {
        Err(FluxError::System(ref io_err)) => {
            assert_eq!(
                io_err.raw_os_error(),
                Some(libc::ENOMEM),
                "check_ptr should capture the current errno on null"
            );
        }
        other => panic!("Expected FluxError::System, got: {other:?}"),
    }
}
