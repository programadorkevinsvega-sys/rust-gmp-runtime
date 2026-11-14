use rust_gmp_runtime::stack::{
    BlockStackManager, GoInspiredOptions, GoInspiredStackManager, GoInspiredTimingSnapshot,
    ProcessorStackCache, StackBenchmarkMetrics, StackConfig, StackError, StackManager, VmaSnapshot,
};
use std::error::Error;
use std::sync::Arc;

use super::handle::Handle;
use super::stats::{self, Stats};

pub enum Allocator {
    Fixed {
        manager: StackManager,
        timing: Option<Arc<StackBenchmarkMetrics>>,
    },
    Block {
        manager: BlockStackManager,
        timing: Option<rust_gmp_runtime::stack::BlockStackTimingHandle>,
    },
    GoInspired {
        manager: GoInspiredStackManager,
        instrumented: bool,
    },
}

pub enum WorkerCache {
    None,
    GoInspired(ProcessorStackCache),
}

impl Allocator {
    pub fn is_instrumented(&self) -> bool {
        match self {
            Self::Fixed { timing, .. } => timing.is_some(),
            Self::Block { timing, .. } => timing.is_some(),
            Self::GoInspired { instrumented, .. } => *instrumented,
        }
    }

    pub fn reset_refill_peak(&self) -> Result<(), StackError> {
        match self {
            Self::GoInspired { manager, .. } => manager.reset_refill_peak(),
            Self::Fixed { .. } | Self::Block { .. } => Ok(()),
        }
    }

    pub fn take_go_inspired_timing(&self) -> Option<GoInspiredTimingSnapshot> {
        match self {
            Self::GoInspired {
                manager,
                instrumented: true,
            } => Some(manager.take_timing_snapshot()),
            Self::Fixed { .. } | Self::Block { .. } | Self::GoInspired { .. } => None,
        }
    }

    pub fn cache_refill_target(&self) -> Option<usize> {
        match self {
            Self::GoInspired { manager, .. } => Some(manager.cache_refill_target()),
            Self::Fixed { .. } | Self::Block { .. } => None,
        }
    }

    pub fn cache_refill_capacity(&self) -> Option<usize> {
        match self {
            Self::GoInspired { manager, .. } => Some(manager.cache_refill_capacity()),
            Self::Fixed { .. } | Self::Block { .. } => None,
        }
    }

    pub fn set_worker_cache_refill_target(
        &self,
        cache: &mut WorkerCache,
        target: usize,
    ) -> Result<(), StackError> {
        match (self, cache) {
            (Self::GoInspired { .. }, WorkerCache::GoInspired(cache)) => {
                cache.set_refill_target(target)
            }
            _ => Err(StackError::MetricsInvariant {
                operation: "set a worker cache refill target",
                observed: "allocator and cache do not match".to_owned(),
            }),
        }
    }

    pub fn worker_cache_is_empty(&self, cache: &WorkerCache) -> Result<bool, StackError> {
        match (self, cache) {
            (Self::GoInspired { .. }, WorkerCache::GoInspired(cache)) => Ok(cache.is_empty()),
            _ => Err(StackError::MetricsInvariant {
                operation: "inspect a worker cache",
                observed: "allocator and cache do not match".to_owned(),
            }),
        }
    }
}

#[path = "allocator/create.rs"]
mod create;
#[path = "allocator/operations.rs"]
mod operations;
