use super::*;

pub(super) const DEFAULT_SAMPLE_COUNT: usize = 30;
pub(super) const TASK_COUNT: usize = 256;
pub(super) const WARMUP_TASK_COUNT: usize = 32;
pub(super) const STACK_SIZE: usize = 8 * 1024;
pub(super) const DEFAULT_WORKER_COUNTS: [usize; 5] = [1, 2, 4, 8, 12];

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

pub(super) struct WorkerSet {
    pub(super) count: usize,
    pub(super) manager: MachineManager,
}

pub(super) struct Sample {
    pub(super) submit_ns: u128,
    pub(super) drain_ns: u128,
    pub(super) end_to_end_ns: u128,
    pub(super) throughput_per_second: f64,
    pub(super) schedule_p50_ns: Option<u64>,
    pub(super) schedule_p95_ns: Option<u64>,
    pub(super) stolen_tasks: u64,
    pub(super) park_events: u64,
    pub(super) wake_notifications: u64,
    pub(super) tasks_by_processor: Vec<u64>,
    pub(super) peak_injection_queue_len: usize,
    pub(super) peak_global_queue_len: usize,
    pub(super) peak_local_queue_lengths: Vec<usize>,
    pub(super) worker_state_transitions: u64,
}

#[derive(Default)]
pub(super) struct Summary {
    pub(super) submit_ns: Vec<u128>,
    pub(super) drain_ns: Vec<u128>,
    pub(super) end_to_end_ns: Vec<u128>,
    pub(super) throughput_per_second: Vec<f64>,
    pub(super) schedule_p50_ns: Vec<u64>,
    pub(super) schedule_p95_ns: Vec<u64>,
}
