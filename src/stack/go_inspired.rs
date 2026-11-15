//! Span-backed stack engine with explicit exclusive local caches.

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
#[cfg(feature = "allocator-benchmark")]
mod timing;
mod trim;

pub use api::GoInspiredStackManager;
pub use cache::ProcessorStackCache;
pub use layout::GoInspiredOptions;
pub use lease::GoInspiredStack;
pub use metrics::GoInspiredStackMetrics;
#[cfg(feature = "allocator-benchmark")]
pub use timing::GoInspiredTimingSnapshot;

#[cfg(test)]
#[path = "go_inspired/tests.rs"]
mod tests;
