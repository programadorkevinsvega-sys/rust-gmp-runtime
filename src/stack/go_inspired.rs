//! Benchmark-only stack pools with explicit Processor-owned local caches.

mod admission;
mod api;
mod cache;
mod layout;
mod lease;
mod manager;
mod memory;
mod metrics;
mod pool;
mod slot;
mod span;
mod timing;
mod trim;

pub use api::GoInspiredStackManager;
pub use cache::ProcessorStackCache;
pub use layout::GoInspiredOptions;
pub use lease::GoInspiredStack;
pub use metrics::GoInspiredStackMetrics;
pub use timing::GoInspiredTimingSnapshot;

#[cfg(test)]
#[path = "go_inspired/tests.rs"]
mod tests;
