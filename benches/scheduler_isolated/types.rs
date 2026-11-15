use super::*;

pub(super) const DEFAULT_TASK_COUNTS: [usize; 4] = [32, 256, 1_024, 4_096];
pub(super) const DEFAULT_SAMPLE_COUNT: usize = 3;
pub(super) const STACK_SIZE: usize = 8 * 1024;
pub(super) const MAX_TASK_COUNT: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Workload {
    Noop,
    Cpu64,
}

impl Workload {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::Cpu64 => "cpu64",
        }
    }

    pub(super) fn run(self, seed: u64) {
        if self == Self::Cpu64 {
            let mut value = seed;
            for _ in 0..64 {
                value ^= value >> 12;
                value ^= value << 25;
                value ^= value >> 27;
                value = value.wrapping_mul(0x2545_f491_4f6c_dd1d);
            }
            black_box(value);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Scenario {
    Burst,
    Flow,
}

impl Scenario {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Burst => "burst",
            Self::Flow => "flow",
        }
    }
}

pub(super) struct Summary {
    pub(super) end_to_end_ns: Vec<u128>,
    pub(super) throughput_per_second: Vec<f64>,
    pub(super) stolen_tasks: u64,
    pub(super) steal_attempts: u64,
    pub(super) failed_steal_attempts: u64,
    pub(super) tasks_by_processor: Vec<u64>,
    pub(super) max_injection_queue_len: usize,
    pub(super) max_global_queue_len: usize,
    pub(super) max_local_queue_lengths: Vec<usize>,
}

pub(super) struct Sample {
    pub(super) submit_ns: u128,
    pub(super) drain_ns: u128,
    pub(super) end_to_end_ns: u128,
    pub(super) throughput_per_second: f64,
    pub(super) stolen_tasks: u64,
    pub(super) tasks_by_processor: Vec<u64>,
    pub(super) peak_injection_queue_len: usize,
    pub(super) peak_global_queue_len: usize,
    pub(super) peak_local_queue_lengths: Vec<usize>,
    pub(super) steal_attempts: u64,
    pub(super) failed_steal_attempts: u64,
}
