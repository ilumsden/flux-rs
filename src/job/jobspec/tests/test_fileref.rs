use std::path::Path;

use serde_json::json;

use crate::job::jobspec::fileref::{Fileref, FilerefData, FilerefEncoding};

// =========================================================================
// Helpers
// =========================================================================

fn write_temp_file(name: &str, contents: &[u8]) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(format!("/tmp/{name}"));
    std::fs::write(&path, contents).expect("Failed to write temp file");
    path
}

fn remove_temp_file(path: &Path) {
    std::fs::remove_file(path).ok();
}

fn default_mode_value() -> u32 {
    use nix::sys::stat::{Mode, SFlag};
    let permissions = Mode::S_IRUSR | Mode::S_IWUSR;
    let file_type = SFlag::S_IFREG;
    permissions.bits() | file_type.bits()
}

// =========================================================================
// FilerefEncoding serde
// =========================================================================

#[test]
fn fileref_encoding_utf8_serializes_correctly() {
    let enc = FilerefEncoding::Utf8;
    let json = serde_json::to_string(&enc).unwrap();
    assert_eq!(json, "\"utf-8\"");
}

#[test]
fn fileref_encoding_base64_serializes_correctly() {
    let enc = FilerefEncoding::Base64;
    let json = serde_json::to_string(&enc).unwrap();
    assert_eq!(json, "\"base64\"");
}

#[test]
fn fileref_encoding_utf8_deserializes_correctly() {
    let enc: FilerefEncoding = serde_json::from_str("\"utf-8\"").unwrap();
    assert_eq!(enc, FilerefEncoding::Utf8);
}

#[test]
fn fileref_encoding_base64_deserializes_correctly() {
    let enc: FilerefEncoding = serde_json::from_str("\"base64\"").unwrap();
    assert_eq!(enc, FilerefEncoding::Base64);
}

#[test]
fn fileref_encoding_round_trips() {
    for enc in [FilerefEncoding::Utf8, FilerefEncoding::Base64] {
        let json = serde_json::to_string(&enc).unwrap();
        let decoded: FilerefEncoding = serde_json::from_str(&json).unwrap();
        assert_eq!(enc, decoded);
    }
}

// =========================================================================
// FilerefData serde — untagged deserialization order
//
// With the field order Encoded, Text, Json:
// - arrays of integers → Encoded(Vec<u8>)
// - JSON strings → Text(String)
// - JSON objects/numbers/booleans/null → Json(Value)
// =========================================================================

#[test]
fn fileref_data_encoded_serializes_to_array() {
    let data = FilerefData::Encoded(vec![1, 2, 3]);
    let json = serde_json::to_value(&data).unwrap();
    assert!(json.is_array());
}

#[test]
fn fileref_data_json_serializes_to_object() {
    let data = FilerefData::Json(json!({"key": "value"}));
    let json = serde_json::to_value(&data).unwrap();
    assert!(json.is_object());
}

#[test]
fn fileref_data_text_serializes_to_string() {
    let data = FilerefData::Text("hello".to_string());
    let json = serde_json::to_value(&data).unwrap();
    assert!(json.is_string());
}

#[test]
fn fileref_data_integer_array_deserializes_to_encoded() {
    let data: FilerefData = serde_json::from_value(json!([1, 2, 3])).unwrap();
    assert!(matches!(data, FilerefData::Encoded(_)));
}

#[test]
fn fileref_data_object_deserializes_to_json() {
    let data: FilerefData = serde_json::from_value(json!({"key": "val"})).unwrap();
    assert!(matches!(data, FilerefData::Json(_)));
}

#[test]
fn fileref_data_number_deserializes_to_json() {
    let data: FilerefData = serde_json::from_value(json!(42)).unwrap();
    assert!(matches!(data, FilerefData::Json(_)));
}

#[test]
fn fileref_data_string_deserializes_to_text() {
    // With field order Encoded, Text, Json — a JSON string matches
    // Text(String) before Json(Value) is tried.
    let data: FilerefData = serde_json::from_value(json!("hello")).unwrap();
    assert!(
        matches!(data, FilerefData::Text(_)),
        "Expected Text variant for a string value"
    );
}

#[test]
fn fileref_data_text_round_trips() {
    let original = FilerefData::Text("round trip".to_string());
    let json = serde_json::to_value(&original).unwrap();
    let decoded: FilerefData = serde_json::from_value(json).unwrap();
    assert!(matches!(decoded, FilerefData::Text(s) if s == "round trip"));
}

// =========================================================================
// Fileref::new
// =========================================================================

#[test]
fn new_stores_path() {
    let f = Fileref::new("/some/path", None, None, None, None, None, None);
    assert_eq!(f.path, std::path::PathBuf::from("/some/path"));
}

#[test]
fn new_with_no_mode_uses_default_with_reg_flag() {
    let f = Fileref::new("/some/path", None, None, None, None, None, None);
    assert_eq!(f.mode, default_mode_value());
}

#[test]
fn new_with_explicit_mode_ors_in_reg_flag() {
    use nix::sys::stat::{Mode, SFlag};
    let mode = Mode::S_IRWXU;
    let f = Fileref::new("/some/path", Some(mode), None, None, None, None, None);
    assert!(
        f.mode & SFlag::S_IFREG.bits() != 0,
        "S_IFREG bit must be set"
    );
    assert!(
        f.mode & Mode::S_IRWXU.bits() != 0,
        "S_IRWXU bits must be set"
    );
}

#[test]
fn new_stores_mtime_and_ctime() {
    let f = Fileref::new("/p", None, Some(100), Some(200), None, None, None);
    assert_eq!(f.mtime, Some(100));
    assert_eq!(f.ctime, Some(200));
}

#[test]
fn new_stores_size() {
    let f = Fileref::new("/p", None, None, None, Some(42), None, None);
    assert_eq!(f.size, Some(42));
}

#[test]
fn new_stores_encoding() {
    let f = Fileref::new(
        "/p",
        None,
        None,
        None,
        None,
        Some(FilerefEncoding::Utf8),
        None,
    );
    assert_eq!(f.encoding, Some(FilerefEncoding::Utf8));
}

#[test]
fn new_none_optional_fields_are_none() {
    let f = Fileref::new("/p", None, None, None, None, None, None);
    assert!(f.mtime.is_none());
    assert!(f.ctime.is_none());
    assert!(f.size.is_none());
    assert!(f.encoding.is_none());
    assert!(f.data.is_none());
}

// =========================================================================
// Fileref::create_directory_object
// =========================================================================

#[test]
fn create_directory_object_stores_path() {
    let f = Fileref::create_directory_object("/dir/path", None, None, None);
    assert_eq!(f.path, std::path::PathBuf::from("/dir/path"));
}

#[test]
fn create_directory_object_has_no_size_or_data() {
    let f = Fileref::create_directory_object("/dir", None, None, None);
    assert!(f.size.is_none());
    assert!(f.data.is_none());
}

#[test]
fn create_directory_object_stores_mtime_and_ctime() {
    let f = Fileref::create_directory_object("/dir", None, Some(10), Some(20));
    assert_eq!(f.mtime, Some(10));
    assert_eq!(f.ctime, Some(20));
}

// =========================================================================
// Fileref::create_symlink_object
// =========================================================================

#[test]
fn create_symlink_object_path_is_to_path() {
    let f = Fileref::create_symlink_object("/from", "/to", None, None, None);
    assert_eq!(f.path, std::path::PathBuf::from("/to"));
}

#[test]
fn create_symlink_object_data_contains_from_path() {
    let f = Fileref::create_symlink_object("/from/path", "/to/path", None, None, None);
    match &f.data {
        Some(FilerefData::Text(s)) => assert_eq!(s, "/from/path"),
        other => panic!(
            "Expected FilerefData::Text, got something else: data is_some={}",
            other.is_some()
        ),
    }
}

#[test]
fn create_symlink_object_stores_mtime_and_ctime() {
    let f = Fileref::create_symlink_object("/from", "/to", None, Some(10), Some(20));
    assert_eq!(f.mtime, Some(10));
    assert_eq!(f.ctime, Some(20));
}

#[test]
fn create_symlink_object_has_no_size() {
    let f = Fileref::create_symlink_object("/from", "/to", None, None, None);
    assert!(f.size.is_none());
}

// =========================================================================
// Fileref::create_empty_file_object
// =========================================================================

#[test]
fn create_empty_file_object_stores_path() {
    let f = Fileref::create_empty_file_object("/empty/file", None, None, None);
    assert_eq!(f.path, std::path::PathBuf::from("/empty/file"));
}

#[test]
fn create_empty_file_object_has_size_zero() {
    let f = Fileref::create_empty_file_object("/empty/file", None, None, None);
    assert_eq!(f.size, Some(0));
}

#[test]
fn create_empty_file_object_has_no_data() {
    let f = Fileref::create_empty_file_object("/empty/file", None, None, None);
    assert!(f.data.is_none());
}

#[test]
fn create_empty_file_object_stores_mtime_and_ctime() {
    let f = Fileref::create_empty_file_object("/empty", None, Some(5), Some(6));
    assert_eq!(f.mtime, Some(5));
    assert_eq!(f.ctime, Some(6));
}

// =========================================================================
// Fileref::create_json_content_object
// =========================================================================

#[test]
fn create_json_content_object_stores_path() {
    let f = Fileref::create_json_content_object("/json/file", json!({}), None, None, None);
    assert_eq!(f.path, std::path::PathBuf::from("/json/file"));
}

#[test]
fn create_json_content_object_data_is_json_variant() {
    let payload = json!({"key": "value"});
    let f = Fileref::create_json_content_object("/json/file", payload.clone(), None, None, None);
    match &f.data {
        Some(FilerefData::Json(v)) => assert_eq!(v, &payload),
        other => panic!(
            "Expected FilerefData::Json, data is_some={}",
            other.is_some()
        ),
    }
}

#[test]
fn create_json_content_object_stores_mtime_and_ctime() {
    let f = Fileref::create_json_content_object("/json/file", json!({}), None, Some(1), Some(2));
    assert_eq!(f.mtime, Some(1));
    assert_eq!(f.ctime, Some(2));
}

#[test]
fn create_json_content_object_has_no_encoding() {
    let f = Fileref::create_json_content_object("/json/file", json!({}), None, None, None);
    assert!(f.encoding.is_none());
}

// =========================================================================
// Fileref::create_text_content_object
// =========================================================================

#[test]
fn create_text_content_object_stores_path() {
    let f = Fileref::create_text_content_object("/text/file", "hello", None, None, None);
    assert_eq!(f.path, std::path::PathBuf::from("/text/file"));
}

#[test]
fn create_text_content_object_data_is_text_variant() {
    let f = Fileref::create_text_content_object("/text/file", "hello world", None, None, None);
    match &f.data {
        Some(FilerefData::Text(s)) => assert_eq!(s, "hello world"),
        other => panic!(
            "Expected FilerefData::Text, data is_some={}",
            other.is_some()
        ),
    }
}

#[test]
fn create_text_content_object_size_matches_data_len() {
    let content = "hello world";
    let f = Fileref::create_text_content_object("/text/file", content, None, None, None);
    assert_eq!(f.size, Some(content.len()));
}

#[test]
fn create_text_content_object_encoding_is_utf8() {
    let f = Fileref::create_text_content_object("/text/file", "hello", None, None, None);
    assert_eq!(f.encoding, Some(FilerefEncoding::Utf8));
}

#[test]
fn create_text_content_object_stores_mtime_and_ctime() {
    let f = Fileref::create_text_content_object("/text/file", "hi", None, Some(1), Some(2));
    assert_eq!(f.mtime, Some(1));
    assert_eq!(f.ctime, Some(2));
}

// =========================================================================
// Fileref::create_literal_binary_object
// =========================================================================

#[test]
fn create_literal_binary_object_stores_path() {
    let f = Fileref::create_literal_binary_object("/bin/file", vec![1u8, 2, 3], None, None, None);
    assert_eq!(f.path, std::path::PathBuf::from("/bin/file"));
}

#[test]
fn create_literal_binary_object_data_is_encoded_variant() {
    let bytes = vec![1u8, 2, 3, 4];
    let f = Fileref::create_literal_binary_object("/bin/file", bytes.clone(), None, None, None);
    match &f.data {
        Some(FilerefData::Encoded(v)) => assert_eq!(v, &bytes),
        other => panic!(
            "Expected FilerefData::Encoded, data is_some={}",
            other.is_some()
        ),
    }
}

#[test]
fn create_literal_binary_object_size_matches_data_len() {
    let bytes = vec![0u8; 16];
    let f = Fileref::create_literal_binary_object("/bin/file", bytes.clone(), None, None, None);
    assert_eq!(f.size, Some(16));
}

#[test]
fn create_literal_binary_object_encoding_is_base64() {
    let f = Fileref::create_literal_binary_object("/bin/file", vec![1u8], None, None, None);
    assert_eq!(f.encoding, Some(FilerefEncoding::Base64));
}

#[test]
fn create_literal_binary_object_stores_mtime_and_ctime() {
    let f = Fileref::create_literal_binary_object("/bin/file", vec![], None, Some(7), Some(8));
    assert_eq!(f.mtime, Some(7));
    assert_eq!(f.ctime, Some(8));
}

// =========================================================================
// Fileref::create_from_text_file
// =========================================================================

#[test]
fn create_from_text_file_succeeds_for_utf8_file() {
    let path = write_temp_file("test_fileref_text.txt", b"hello world");
    let result = Fileref::create_from_text_file("/fileref/path", &path);
    remove_temp_file(&path);
    assert!(result.is_ok());
}

#[test]
fn create_from_text_file_data_is_text_variant() {
    let path = write_temp_file("test_fileref_text_data.txt", b"hello world");
    let f = Fileref::create_from_text_file("/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    match &f.data {
        Some(FilerefData::Text(s)) => assert_eq!(s, "hello world"),
        other => panic!(
            "Expected FilerefData::Text, data is_some={}",
            other.is_some()
        ),
    }
}

#[test]
fn create_from_text_file_path_is_fileref_path() {
    let path = write_temp_file("test_fileref_text_path.txt", b"hello");
    let f = Fileref::create_from_text_file("/custom/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    assert_eq!(f.path, std::path::PathBuf::from("/custom/fileref/path"));
}

#[test]
fn create_from_text_file_nonexistent_returns_error() {
    let result = Fileref::create_from_text_file("/fileref/path", "/nonexistent/file/xyzzy");
    assert!(result.is_err());
}

#[test]
fn create_from_text_file_encoding_is_utf8() {
    let path = write_temp_file("test_fileref_text_enc.txt", b"data");
    let f = Fileref::create_from_text_file("/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    assert_eq!(f.encoding, Some(FilerefEncoding::Utf8));
}

#[test]
fn create_from_text_file_size_matches_content_len() {
    let content = b"hello world";
    let path = write_temp_file("test_fileref_text_size.txt", content);
    let f = Fileref::create_from_text_file("/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    assert_eq!(f.size, Some(content.len()));
}

// =========================================================================
// Fileref::create_from_binary_file
// =========================================================================

#[test]
fn create_from_binary_file_succeeds() {
    let path = write_temp_file("test_fileref_bin.bin", &[0u8, 1, 2, 3, 255]);
    let result = Fileref::create_from_binary_file("/fileref/path", &path);
    remove_temp_file(&path);
    assert!(result.is_ok());
}

#[test]
fn create_from_binary_file_data_is_encoded_variant() {
    let bytes = vec![0u8, 1, 2, 3, 255];
    let path = write_temp_file("test_fileref_bin_data.bin", &bytes);
    let f = Fileref::create_from_binary_file("/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    match &f.data {
        Some(FilerefData::Encoded(v)) => assert_eq!(v, &bytes),
        other => panic!(
            "Expected FilerefData::Encoded, data is_some={}",
            other.is_some()
        ),
    }
}

#[test]
fn create_from_binary_file_encoding_is_base64() {
    let path = write_temp_file("test_fileref_bin_enc.bin", &[0u8, 255]);
    let f = Fileref::create_from_binary_file("/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    assert_eq!(f.encoding, Some(FilerefEncoding::Base64));
}

#[test]
fn create_from_binary_file_size_matches_content_len() {
    let bytes = vec![0u8; 8];
    let path = write_temp_file("test_fileref_bin_size.bin", &bytes);
    let f = Fileref::create_from_binary_file("/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    assert_eq!(f.size, Some(8));
}

#[test]
fn create_from_binary_file_nonexistent_returns_error() {
    let result = Fileref::create_from_binary_file("/fileref/path", "/nonexistent/file/xyzzy");
    assert!(result.is_err());
}

// =========================================================================
// Fileref::create_from_file
// =========================================================================

#[test]
fn create_from_file_uses_text_for_utf8_content() {
    let path = write_temp_file("test_fileref_auto_text.txt", b"hello world");
    let f = Fileref::create_from_file("/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    assert!(matches!(f.data, Some(FilerefData::Text(_))));
}

#[test]
fn create_from_file_uses_binary_for_non_utf8_content() {
    let path = write_temp_file("test_fileref_auto_bin.bin", &[0u8, 1, 2, 255]);
    let f = Fileref::create_from_file("/fileref/path", &path).unwrap();
    remove_temp_file(&path);
    assert!(matches!(f.data, Some(FilerefData::Encoded(_))));
}

#[test]
fn create_from_file_nonexistent_returns_error() {
    let result = Fileref::create_from_file("/fileref/path", "/nonexistent/file/xyzzy");
    assert!(result.is_err());
}

// =========================================================================
// Fileref serde
// =========================================================================

#[test]
fn fileref_serializes_to_json_object() {
    let f = Fileref::new("/some/path", None, None, None, None, None, None);
    let json = serde_json::to_value(&f).unwrap();
    assert!(json.is_object());
}

#[test]
fn fileref_path_round_trips_through_json() {
    let f = Fileref::new("/some/path", None, None, None, None, None, None);
    let json = serde_json::to_string(&f).unwrap();
    let decoded: Fileref = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.path, f.path);
}

#[test]
fn fileref_mode_round_trips_through_json() {
    let f = Fileref::new("/some/path", None, None, None, None, None, None);
    let json = serde_json::to_string(&f).unwrap();
    let decoded: Fileref = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.mode, f.mode);
}

#[test]
fn fileref_deserializes_missing_mode_uses_default() {
    // When "mode" is absent from JSON, the default_mode function should fire.
    let json = r#"{"path": "/some/path"}"#;
    let decoded: Fileref = serde_json::from_str(json).unwrap();
    assert_eq!(decoded.mode, default_mode_value());
}

#[test]
fn fileref_optional_fields_omitted_when_none() {
    let f = Fileref::new("/p", None, None, None, None, None, None);
    let json = serde_json::to_value(&f).unwrap();
    assert!(!json.as_object().unwrap().contains_key("mtime"));
    assert!(!json.as_object().unwrap().contains_key("ctime"));
    assert!(!json.as_object().unwrap().contains_key("size"));
    assert!(!json.as_object().unwrap().contains_key("encoding"));
    assert!(!json.as_object().unwrap().contains_key("data"));
}

#[test]
fn fileref_mtime_and_ctime_round_trip_through_json() {
    let f = Fileref::new("/p", None, Some(100), Some(200), None, None, None);
    let json = serde_json::to_string(&f).unwrap();
    let decoded: Fileref = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.mtime, Some(100));
    assert_eq!(decoded.ctime, Some(200));
}

#[test]
fn fileref_encoding_round_trips_through_json() {
    let f = Fileref::new(
        "/p",
        None,
        None,
        None,
        None,
        Some(FilerefEncoding::Base64),
        None,
    );
    let json = serde_json::to_string(&f).unwrap();
    let decoded: Fileref = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.encoding, Some(FilerefEncoding::Base64));
}
