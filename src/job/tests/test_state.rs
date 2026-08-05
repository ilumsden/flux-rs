use crate::job::state::{JobState, JobStateFormat};

// =========================================================================
// JobStateFormat
// =========================================================================

#[test]
fn format_as_str_returns_expected_strings() {
    assert_eq!(JobStateFormat::LowerCaseShort.as_str(), "s");
    assert_eq!(JobStateFormat::UpperCaseShort.as_str(), "S");
    assert_eq!(JobStateFormat::LowerCaseLong.as_str(), "l");
    assert_eq!(JobStateFormat::UpperCaseLong.as_str(), "L");
    assert_eq!(JobStateFormat::Emoji.as_str(), "S");
}

#[test]
fn format_as_c_str_returns_expected_c_strings() {
    assert_eq!(JobStateFormat::LowerCaseShort.as_c_str(), c"s");
    assert_eq!(JobStateFormat::UpperCaseShort.as_c_str(), c"S");
    assert_eq!(JobStateFormat::LowerCaseLong.as_c_str(), c"l");
    assert_eq!(JobStateFormat::UpperCaseLong.as_c_str(), c"L");
    assert_eq!(JobStateFormat::Emoji.as_c_str(), c"S");
}

// =========================================================================
// JobState::encode
// =========================================================================

#[test]
fn encode_upper_case_long_returns_non_empty_string() {
    let state = JobState::NEW;
    let encoded = state.encode(JobStateFormat::UpperCaseLong).unwrap();
    assert_eq!(encoded, "NEW");
}

#[test]
fn encode_lower_case_long_returns_lowercase() {
    let state = JobState::RUN;
    let encoded = state.encode(JobStateFormat::LowerCaseLong).unwrap();
    assert_eq!(encoded, "run");
}

#[test]
fn encode_upper_case_short_returns_single_char() {
    let state = JobState::INACTIVE;
    let encoded = state.encode(JobStateFormat::UpperCaseShort).unwrap();
    assert_eq!(encoded, "I");
}

#[test]
fn encode_lower_case_short_returns_single_char() {
    let state = JobState::PRIORITY;
    let encoded = state.encode(JobStateFormat::LowerCaseShort).unwrap();
    assert_eq!(encoded, "p");
}

#[test]
fn encode_emoji_maps_known_states_to_emojis() {
    assert_eq!(
        JobState::NEW.encode(JobStateFormat::Emoji).unwrap(),
        "\u{1F381}"
    );
    assert_eq!(
        JobState::DEPEND.encode(JobStateFormat::Emoji).unwrap(),
        "\u{1F6D1}"
    );
    assert_eq!(
        JobState::PRIORITY.encode(JobStateFormat::Emoji).unwrap(),
        "\u{1F6A6}"
    );
    assert_eq!(
        JobState::SCHED.encode(JobStateFormat::Emoji).unwrap(),
        "\u{1F4C5}"
    );
    assert_eq!(
        JobState::RUN.encode(JobStateFormat::Emoji).unwrap(),
        "\u{1F3C3}"
    );
    assert_eq!(
        JobState::CLEANUP.encode(JobStateFormat::Emoji).unwrap(),
        "\u{1F5D1}"
    );
    assert_eq!(
        JobState::INACTIVE.encode(JobStateFormat::Emoji).unwrap(),
        "\u{1F480}"
    );
}

// =========================================================================
// JobState::decode
// =========================================================================

#[test]
fn decode_valid_long_name_succeeds() {
    let state = JobState::decode("NEW").unwrap();
    assert_eq!(state, JobState::NEW);
}

#[test]
fn decode_valid_lowercase_name_succeeds() {
    let state = JobState::decode("run").unwrap();
    assert_eq!(state, JobState::RUN);
}

#[test]
fn decode_valid_short_char_succeeds() {
    let state = JobState::decode("I").unwrap();
    assert_eq!(state, JobState::INACTIVE);
}

#[test]
fn decode_invalid_string_returns_error() {
    assert!(JobState::decode("NOT_A_REAL_JOB_STATE").is_err());
}

#[test]
fn decode_nul_byte_returns_error() {
    assert!(JobState::decode("NEW\0RUN").is_err());
}

// =========================================================================
// Round-trip & Display
// =========================================================================

#[test]
fn encode_and_decode_round_trips() {
    let states = [
        JobState::NEW,
        JobState::DEPEND,
        JobState::PRIORITY,
        JobState::SCHED,
        JobState::RUN,
        JobState::CLEANUP,
        JobState::INACTIVE,
    ];
    for state in states {
        let encoded = state.encode(JobStateFormat::UpperCaseLong).unwrap();
        let decoded = JobState::decode(&encoded).unwrap();
        assert_eq!(state, decoded);
    }
}

#[test]
fn display_uses_uppercase_long_format() {
    let state = JobState::NEW;
    assert_eq!(state.to_string(), "NEW");
}
