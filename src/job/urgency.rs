use std::ops::{Deref, DerefMut};

use flux_sys::core::{
    job_urgency_FLUX_JOB_URGENCY_DEFAULT, job_urgency_FLUX_JOB_URGENCY_EXPEDITE,
    job_urgency_FLUX_JOB_URGENCY_HOLD, job_urgency_FLUX_JOB_URGENCY_MAX,
    job_urgency_FLUX_JOB_URGENCY_MIN,
};
use serde::{Deserialize, Serialize};

use crate::error::{FluxError, Result};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct JobUrgency(u8);

impl JobUrgency {
    pub const HOLD: JobUrgency = JobUrgency(job_urgency_FLUX_JOB_URGENCY_HOLD as u8);
    pub const DEFAULT: JobUrgency = JobUrgency(job_urgency_FLUX_JOB_URGENCY_DEFAULT as u8);
    pub const EXPEDITE: JobUrgency = JobUrgency(job_urgency_FLUX_JOB_URGENCY_EXPEDITE as u8);
    pub const MIN: JobUrgency = JobUrgency(job_urgency_FLUX_JOB_URGENCY_MIN as u8);
    pub const MAX: JobUrgency = JobUrgency(job_urgency_FLUX_JOB_URGENCY_MAX as u8);

    pub fn new(val: u8) -> Result<Self> {
        if val <= 31 {
            Ok(Self(val))
        } else {
            Err(FluxError::Logic(
                "Job urgency must be between 0 and 31, inclusive".to_string(),
            ))
        }
    }

    pub const fn as_u8(self) -> u8 {
        self.0
    }
}

impl Deref for JobUrgency {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for JobUrgency {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

macro_rules! define_converters {
    ($($integer_type:ty),+) => {
        $(
        impl TryFrom<$integer_type> for JobUrgency {
            type Error = FluxError;

            fn try_from(value: $integer_type) -> Result<Self> {
                Self::new(value as u8)
            }
        }

        impl From<JobUrgency> for $integer_type {
            fn from(value: JobUrgency) -> Self {
                value.0 as $integer_type
            }
        }
        )+
    };
}

define_converters!(u8, u16, u32, u64, i8, i16, i32, i64);
