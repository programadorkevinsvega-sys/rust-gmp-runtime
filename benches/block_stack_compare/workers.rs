use super::common::{Allocator, WorkerCache};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread::{self, JoinHandle};
use std::time::Instant;

pub const SNAPSHOTS_PER_SAMPLE: usize = 5;
const SNAPSHOT_LATENCY_SAMPLE_INTERVAL: usize = 8;

#[derive(Debug)]
pub struct SnapshotReport {
    pub durations_ns: Vec<u64>,
    pub vma_counts: Vec<u64>,
}

pub struct ControlledSample {
    pub elapsed_ns: u128,
    pub operations: usize,
    pub latencies_ns: Vec<u64>,
    pub snapshot: Option<SnapshotReport>,
}

pub struct RefillBurstSample {
    pub elapsed_ns: u128,
    pub operations: usize,
    pub burst_latencies_ns: Vec<u64>,
    pub worker_caches: Vec<WorkerCache>,
}

pub struct RefillWaveSample {
    pub elapsed_ns: u128,
    pub operations: usize,
    pub wave_latencies_ns: Vec<u64>,
}

pub struct LifetimeSample {
    pub elapsed_ns: u128,
    pub operations: usize,
    pub latencies_ns: Vec<u64>,
    pub worker_caches: Vec<WorkerCache>,
    pub target_changes: usize,
    pub max_refill_target: usize,
}

#[derive(Clone, Copy)]
pub enum DemandRefillMode {
    Fixed(usize),
    Adaptive { baseline: usize, capacity: usize },
}

#[path = "workers/controlled.rs"]
pub(super) mod controlled;
#[path = "workers/controlled_workers.rs"]
pub(super) mod controlled_workers;
#[path = "workers/demand_refill.rs"]
pub(super) mod demand_refill;
#[path = "workers/lifetime.rs"]
pub(super) mod lifetime;
#[path = "workers/refill_burst.rs"]
pub(super) mod refill_burst;
#[path = "workers/synchronized_refill.rs"]
pub(super) mod synchronized_refill;

use controlled_workers::{WorkerBarriers, spawn_snapshotter, spawn_workers};
