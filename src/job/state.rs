use std::ffi::{CStr, CString};
use std::fmt::Display;

use bitflags::bitflags;
use flux_sys::core::{
    FLUX_JOB_STATE_ACTIVE, FLUX_JOB_STATE_PENDING, FLUX_JOB_STATE_RUNNING, flux_job_state_t,
    flux_job_state_t_FLUX_JOB_STATE_CLEANUP, flux_job_state_t_FLUX_JOB_STATE_DEPEND,
    flux_job_state_t_FLUX_JOB_STATE_INACTIVE, flux_job_state_t_FLUX_JOB_STATE_NEW,
    flux_job_state_t_FLUX_JOB_STATE_PRIORITY, flux_job_state_t_FLUX_JOB_STATE_RUN,
    flux_job_state_t_FLUX_JOB_STATE_SCHED, flux_job_statetostr, flux_job_strtostate,
};
use serde::{Deserialize, Serialize};

use crate::error::{Result, check_ptr, check_rc};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobStateFormat {
    LowerCaseShort,
    UpperCaseShort,
    LowerCaseLong,
    UpperCaseLong,
    Emoji,
}

impl JobStateFormat {
    pub const fn as_str(&self) -> &str {
        match self {
            JobStateFormat::LowerCaseShort => "s",
            JobStateFormat::UpperCaseShort | JobStateFormat::Emoji => "S",
            JobStateFormat::LowerCaseLong => "l",
            JobStateFormat::UpperCaseLong => "L",
        }
    }

    pub const fn as_c_str(&self) -> &CStr {
        match self {
            JobStateFormat::LowerCaseShort => c"s",
            JobStateFormat::UpperCaseShort | JobStateFormat::Emoji => c"S",
            JobStateFormat::LowerCaseLong => c"l",
            JobStateFormat::UpperCaseLong => c"L",
        }
    }
}

bitflags! {
    #[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct JobState: u32 {
        const NONE = 0;
        const CLEANUP = flux_job_state_t_FLUX_JOB_STATE_CLEANUP;
        const DEPEND = flux_job_state_t_FLUX_JOB_STATE_DEPEND;
        const INACTIVE = flux_job_state_t_FLUX_JOB_STATE_INACTIVE;
        const NEW = flux_job_state_t_FLUX_JOB_STATE_NEW;
        const PRIORITY = flux_job_state_t_FLUX_JOB_STATE_PRIORITY;
        const RUN = flux_job_state_t_FLUX_JOB_STATE_RUN;
        const SCHED = flux_job_state_t_FLUX_JOB_STATE_SCHED;
        const ACTIVE = FLUX_JOB_STATE_ACTIVE;
        const RUNNING = FLUX_JOB_STATE_RUNNING;
        const PENDING = FLUX_JOB_STATE_PENDING;
    }
}

impl JobState {
    pub fn encode(&self, fmt: JobStateFormat) -> Result<String> {
        let fmt_c_str = fmt.as_c_str();
        // Note: do not free this string since flux_job_statetostr just returns a string literal
        let c_str =
            unsafe { flux_job_statetostr(self.bits() as flux_job_state_t, fmt_c_str.as_ptr()) };
        check_ptr(c_str as *mut i8)?;
        let mut owned_str = unsafe { CStr::from_ptr(c_str).to_str()?.to_owned() };
        if matches!(fmt, JobStateFormat::Emoji) {
            owned_str = match owned_str.as_str() {
                "N" => String::from("\u{1F381}"), // wrapped gift
                "D" => String::from("\u{1F6D1}"), // stop sign
                "P" => String::from("\u{1F6A6}"), // vertical traffic light
                "S" => String::from("\u{1F4C5}"), // calendar
                "R" => String::from("\u{1F3C3}"), // person running
                "C" => String::from("\u{1F5D1}"), // wastebasket
                "I" => String::from("\u{1F480}"), // skull
                _ => owned_str,                   // Fallback if the state is unknown
            };
        }
        Ok(owned_str)
    }

    pub fn decode(encoded_state: &str) -> Result<Self> {
        let c_encoded_state = CString::new(encoded_state)?;
        let mut c_state: flux_job_state_t = 0;
        let rc = unsafe {
            flux_job_strtostate(
                c_encoded_state.as_ptr(),
                &mut c_state as *mut flux_job_state_t,
            )
        };
        check_rc(rc)?;
        Ok(Self::from_bits_retain(c_state))
    }
}

impl Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let job_state_str = match self.encode(JobStateFormat::UpperCaseLong) {
            Ok(s) => s,
            Err(e) => format!("UNKNOWN (failed to convert C string to Rust string: {e})"),
        };
        write!(f, "{}", job_state_str)
    }
}
