use std::fmt::Display;
use std::time::Duration;

use crate::error::{FluxError, Result};
use crate::utils::parse_fsd;

/// An enum representing job durations for Jobspecs.
pub enum FluxDuration<'a> {
    /// The number of seconds for the duration.
    Secs(f64),
    /// A Flux Standard Duration (RFC 23).
    Fsd(&'a str),
    /// A Rust std::time::Duration.
    Standard(Duration),
}

impl From<f64> for FluxDuration<'_> {
    fn from(v: f64) -> Self {
        Self::Secs(v)
    }
}

impl From<u64> for FluxDuration<'_> {
    fn from(v: u64) -> Self {
        Self::Secs(v as f64)
    }
}

impl From<i32> for FluxDuration<'_> {
    fn from(v: i32) -> Self {
        Self::Secs(v as f64)
    }
}

impl<'a> From<&'a str> for FluxDuration<'a> {
    fn from(v: &'a str) -> Self {
        Self::Fsd(v)
    }
}

impl From<Duration> for FluxDuration<'_> {
    fn from(v: Duration) -> Self {
        Self::Standard(v)
    }
}

impl<'a> TryFrom<FluxDuration<'a>> for f64 {
    type Error = FluxError;

    fn try_from(value: FluxDuration<'a>) -> Result<Self> {
        let seconds = match value {
            FluxDuration::Secs(s) => s,
            FluxDuration::Standard(d) => d.as_secs_f64(),
            // parse_fsd handles validation of the number of seconds already, so we just return directly
            FluxDuration::Fsd(s) => return parse_fsd(s),
        };
        if seconds < 0.0 || seconds.is_nan() || seconds.is_infinite() {
            return Err(FluxError::Logic("The provided Flux Standard Duration produced a negative, NaN, or Infinite number of seconds".to_string()));
        }
        Ok(seconds)
    }
}

impl<'a> Display for FluxDuration<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FluxDuration::Secs(s) => write!(f, "{}s", s),
            FluxDuration::Fsd(fsd) => write!(f, "{}", *fsd),
            FluxDuration::Standard(dur) => write!(f, "{}s", dur.as_secs_f64()),
        }
    }
}
