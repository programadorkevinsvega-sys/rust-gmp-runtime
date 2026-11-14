use std::error::Error;
use std::fmt;

use super::StackError;

impl fmt::Display for StackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { requested_bytes } => {
                write!(
                    formatter,
                    "stack size must be nonzero; got {requested_bytes} bytes"
                )
            }
            Self::InvalidCacheRefillTarget {
                requested_slots,
                capacity_slots,
            } => write!(
                formatter,
                "cache refill target must be between 1 and {capacity_slots} slots; got {requested_slots}"
            ),
            Self::SizeOverflow => formatter.write_str("stack reservation size overflowed"),
            Self::UnsupportedPlatform => {
                formatter.write_str("guarded stacks are currently supported only on Linux x86_64")
            }
            Self::SystemCall { operation, source } => {
                write!(formatter, "{operation} failed: {source}")
            }
            Self::InvalidSystemValue { operation, value } => {
                write!(formatter, "{operation} returned invalid value {value}")
            }
            Self::MetricsPoisoned { message } => {
                write!(formatter, "stack metrics lock was poisoned: {message}")
            }
            Self::MetricsOverflow => formatter.write_str("stack metrics counter overflowed"),
            Self::MetricsInvariant {
                operation,
                observed,
            } => write!(
                formatter,
                "stack metrics transition {operation} failed at observed value {observed}"
            ),
            Self::IntegerConversion { operation, source } => {
                write!(formatter, "could not represent {operation}: {source}")
            }
            Self::ProcessMetricsParse { path, source } => {
                write!(
                    formatter,
                    "could not parse process metrics in {path}: {source}"
                )
            }
            Self::ProcessMetrics { path, source } => {
                write!(
                    formatter,
                    "could not read process metrics from {path}: {source}"
                )
            }
            Self::VmaProcfs { path, source } => {
                write!(
                    formatter,
                    "could not read VMA diagnostics from {path}: {source}"
                )
            }
            Self::VmaLimitParse { source } => {
                write!(formatter, "could not parse the Linux VMA limit: {source}")
            }
            Self::MalformedVmaLimit => {
                formatter.write_str("Linux VMA limit was empty, zero, or malformed")
            }
            Self::MalformedVmaMaps { line } => {
                write!(
                    formatter,
                    "Linux process maps contained a malformed line {line}"
                )
            }
            Self::VmaPreflightRejected {
                limit,
                current,
                required,
                safety_margin,
            } => write!(
                formatter,
                "VMA preflight rejected reservation: limit {limit}, estimated current {current}, required {required}, safety margin {safety_margin}"
            ),
            Self::VmaPreflightDisabled => {
                formatter.write_str("VMA preflight is disabled for this stack manager")
            }
            Self::AdmissionPoisoned { message } => {
                write!(
                    formatter,
                    "stack allocation admission lock was poisoned: {message}"
                )
            }
            Self::MalformedProcessMetrics { path } => {
                write!(formatter, "process metrics in {path} had an invalid format")
            }
            Self::CleanupFailure { operation, source } => {
                write!(formatter, "{operation} cleanup failed: {source}")
            }
            Self::CleanupAfterFailure { original, cleanup } => {
                write!(formatter, "{original}; additionally, {cleanup}")
            }
        }
    }
}

impl Error for StackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SystemCall { source, .. }
            | Self::CleanupFailure { source, .. }
            | Self::ProcessMetrics { source, .. }
            | Self::VmaProcfs { source, .. } => Some(source),
            Self::VmaLimitParse { source } => Some(source),
            Self::IntegerConversion { source, .. } => Some(source),
            Self::ProcessMetricsParse { source, .. } => Some(source),
            Self::CleanupAfterFailure { original, .. } => Some(original),
            Self::InvalidSize { .. }
            | Self::InvalidCacheRefillTarget { .. }
            | Self::SizeOverflow
            | Self::UnsupportedPlatform
            | Self::InvalidSystemValue { .. }
            | Self::MetricsPoisoned { .. }
            | Self::MetricsOverflow
            | Self::MetricsInvariant { .. }
            | Self::MalformedProcessMetrics { .. }
            | Self::MalformedVmaLimit
            | Self::MalformedVmaMaps { .. }
            | Self::VmaPreflightRejected { .. }
            | Self::VmaPreflightDisabled
            | Self::AdmissionPoisoned { .. } => None,
        }
    }
}
