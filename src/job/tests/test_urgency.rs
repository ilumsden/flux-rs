use crate::job::urgency::JobUrgency;

// =========================================================================
// Constants
// =========================================================================

#[test]
fn constants_are_within_valid_range() {
    assert!(JobUrgency::HOLD.as_u8() <= 31);
    assert!(JobUrgency::DEFAULT.as_u8() <= 31);
    assert!(JobUrgency::EXPEDITE.as_u8() <= 31);
    assert!(JobUrgency::MIN.as_u8() <= 31);
    assert!(JobUrgency::MAX.as_u8() <= 31);
}

#[test]
fn min_and_max_bounds() {
    assert_eq!(JobUrgency::MIN.as_u8(), 0);
    assert_eq!(JobUrgency::MAX.as_u8(), 31);
}

// =========================================================================
// JobUrgency::new & as_u8
// =========================================================================

#[test]
fn new_valid_range_succeeds() {
    for val in 0..=31 {
        let urgency = JobUrgency::new(val).unwrap();
        assert_eq!(urgency.as_u8(), val);
    }
}

#[test]
fn new_above_31_returns_error() {
    assert!(JobUrgency::new(32).is_err());
    assert!(JobUrgency::new(255).is_err());
}

// =========================================================================
// Deref & DerefMut
// =========================================================================

#[test]
fn deref_exposes_inner_u8() {
    let urgency = JobUrgency::DEFAULT;
    assert_eq!(*urgency, urgency.as_u8());
}

#[test]
fn deref_mut_allows_mutation() {
    let mut urgency = JobUrgency::DEFAULT;
    *urgency = 10;
    assert_eq!(urgency.as_u8(), 10);
}

// =========================================================================
// Conversions (TryFrom / From)
// =========================================================================

#[test]
fn try_from_unsigned_integers_valid_succeeds() {
    assert_eq!(
        JobUrgency::try_from(10u8).unwrap(),
        JobUrgency::new(10).unwrap()
    );
    assert_eq!(
        JobUrgency::try_from(10u16).unwrap(),
        JobUrgency::new(10).unwrap()
    );
    assert_eq!(
        JobUrgency::try_from(10u32).unwrap(),
        JobUrgency::new(10).unwrap()
    );
    assert_eq!(
        JobUrgency::try_from(10u64).unwrap(),
        JobUrgency::new(10).unwrap()
    );
}

#[test]
fn try_from_signed_integers_valid_succeeds() {
    assert_eq!(
        JobUrgency::try_from(10i8).unwrap(),
        JobUrgency::new(10).unwrap()
    );
    assert_eq!(
        JobUrgency::try_from(10i16).unwrap(),
        JobUrgency::new(10).unwrap()
    );
    assert_eq!(
        JobUrgency::try_from(10i32).unwrap(),
        JobUrgency::new(10).unwrap()
    );
    assert_eq!(
        JobUrgency::try_from(10i64).unwrap(),
        JobUrgency::new(10).unwrap()
    );
}

#[test]
fn from_job_urgency_to_primitive_integers() {
    let urgency = JobUrgency::EXPEDITE;
    let val_u8: u8 = urgency.into();
    let val_u32: u32 = urgency.into();
    let val_i64: i64 = urgency.into();

    assert_eq!(val_u8, urgency.as_u8());
    assert_eq!(val_u32, urgency.as_u8() as u32);
    assert_eq!(val_i64, urgency.as_u8() as i64);
}
