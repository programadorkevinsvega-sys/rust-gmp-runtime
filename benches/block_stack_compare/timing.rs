use rust_gmp_runtime::stack::{
    BlockStackTimingHandle, GoInspiredStackManager, StackBenchmarkMetrics,
};
use std::sync::Arc;

use super::allocator::Allocator;

pub enum BenchmarkTimer {
    Fixed(Arc<StackBenchmarkMetrics>),
    Block(BlockStackTimingHandle),
    GoInspired(GoInspiredStackManager),
}

impl BenchmarkTimer {
    pub fn snapshot_and_reset(&self) -> String {
        match self {
            Self::Fixed(timing) => format!("{:?}", timing.snapshot_and_reset()),
            Self::Block(timing) => format!("{:?}", timing.snapshot_and_reset()),
            Self::GoInspired(manager) => format!("{:?}", manager.take_timing_snapshot()),
        }
    }
}

impl Allocator {
    pub fn take_timing(&self) -> String {
        match self {
            Self::Fixed {
                timing: Some(metrics),
                ..
            } => format!("{:?}", metrics.snapshot_and_reset()),
            Self::Fixed { timing: None, .. } => "disabled".to_owned(),
            Self::Block {
                timing: Some(timing),
                ..
            } => format!("{:?}", timing.snapshot_and_reset()),
            Self::Block { timing: None, .. } => "disabled".to_owned(),
            Self::GoInspired {
                manager,
                instrumented: true,
            } => format!("{:?}", manager.take_timing_snapshot()),
            Self::GoInspired {
                instrumented: false,
                ..
            } => "disabled".to_owned(),
        }
    }

    pub fn timer_handle(&self) -> Option<BenchmarkTimer> {
        match self {
            Self::Fixed {
                timing: Some(timing),
                ..
            } => Some(BenchmarkTimer::Fixed(Arc::clone(timing))),
            Self::Fixed { timing: None, .. } => None,
            Self::Block {
                timing: Some(timing),
                ..
            } => Some(BenchmarkTimer::Block(timing.clone())),
            Self::Block { timing: None, .. } => None,
            Self::GoInspired {
                manager,
                instrumented: true,
            } => Some(BenchmarkTimer::GoInspired(manager.clone())),
            Self::GoInspired {
                instrumented: false,
                ..
            } => None,
        }
    }
}
