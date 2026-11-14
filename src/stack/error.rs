use std::io;

mod display;

/// Errors reported while reserving, protecting, measuring, or releasing stacks.
#[derive(Debug)]
pub enum StackError {
    /// The requested usable stack size was zero.
    InvalidSize {
        /// Rejected size in bytes.
        requested_bytes: usize,
    },
    /// A local cache refill target was zero or exceeded its configured capacity.
    InvalidCacheRefillTarget {
        /// Rejected number of slots requested per refill.
        requested_slots: usize,
        /// Maximum number of slots allowed in one refill.
        capacity_slots: usize,
    },
    /// A size or accounting calculation overflowed.
    SizeOverflow,
    /// The current target has no validated guarded-stack backend.
    UnsupportedPlatform,
    /// A native system call returned an error.
    SystemCall {
        /// Name of the failed operation.
        operation: &'static str,
        /// Error reported by the operating system.
        source: io::Error,
    },
    /// A system call returned a value outside its documented result range.
    InvalidSystemValue {
        /// Name of the operation with the unexpected value.
        operation: &'static str,
        /// Returned value.
        value: i64,
    },
    /// The stack-metrics mutex was poisoned.
    MetricsPoisoned {
        /// Details from the poisoned mutex.
        message: String,
    },
    /// A stack-metrics counter overflowed or became inconsistent.
    MetricsOverflow,
    /// A stack-metrics transition could not be applied to its observed state.
    MetricsInvariant {
        /// Counter or state transition that could not be applied.
        operation: &'static str,
        /// Value observed when the transition failed.
        observed: String,
    },
    /// Converting a system size into the metrics representation failed.
    IntegerConversion {
        /// Quantity that could not be represented.
        operation: &'static str,
        /// Standard-library conversion error.
        source: std::num::TryFromIntError,
    },
    /// A process-metrics field could not be parsed.
    ProcessMetricsParse {
        /// Procfs path containing the invalid field.
        path: &'static str,
        /// Standard-library parse error.
        source: std::num::ParseIntError,
    },
    /// A process metric could not be read.
    ProcessMetrics {
        /// Procfs path used for the observation.
        path: &'static str,
        /// I/O error returned by the operating system.
        source: io::Error,
    },
    /// Linux VMA procfs data could not be read.
    VmaProcfs {
        /// Procfs path used for the diagnostic.
        path: &'static str,
        /// I/O error reported by the operating system.
        source: io::Error,
    },
    /// The kernel VMA limit contained an invalid integer.
    VmaLimitParse {
        /// Standard-library parse error.
        source: std::num::ParseIntError,
    },
    /// The VMA limit was empty, zero, or otherwise malformed.
    MalformedVmaLimit,
    /// A `/proc/self/maps` line was empty or malformed.
    MalformedVmaMaps {
        /// One-based malformed line, or zero for an empty file.
        line: u64,
    },
    /// The estimated VMA cost and margin exceed observed capacity.
    VmaPreflightRejected {
        /// Current kernel limit.
        limit: u64,
        /// Conservative current VMA estimate used for admission.
        current: u64,
        /// Estimated additional VMA cost.
        required: u64,
        /// Configured capacity retained as headroom.
        safety_margin: u64,
    },
    /// An explicit capacity preflight was requested while disabled.
    VmaPreflightDisabled,
    /// The allocation admission mutex was poisoned.
    AdmissionPoisoned {
        /// Details from the poisoned mutex.
        message: String,
    },
    /// A process-metrics file did not contain the expected fields.
    MalformedProcessMetrics {
        /// Procfs path containing malformed data.
        path: &'static str,
    },
    /// Releasing a stack mapping failed.
    CleanupFailure {
        /// Name of the failed cleanup operation.
        operation: &'static str,
        /// Error reported by the operating system.
        source: io::Error,
    },
    /// The requested operation and a subsequent cleanup/accounting operation failed.
    CleanupAfterFailure {
        /// Original failure.
        original: Box<StackError>,
        /// Secondary cleanup or accounting failure.
        cleanup: Box<StackError>,
    },
}
