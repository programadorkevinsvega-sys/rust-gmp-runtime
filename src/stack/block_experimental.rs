//! Benchmark-only arena allocator; it is not used by the production manager.

#[path = "block_experimental/allocation.rs"]
mod allocation;
#[path = "block_experimental/api.rs"]
mod api;
#[path = "block_experimental/arena.rs"]
mod arena;
#[path = "block_experimental/budget.rs"]
mod budget;
#[path = "block_experimental/failure.rs"]
mod failure;
mod lifecycle;
mod manager;
mod memory;
mod release;
mod state;
mod timing;

pub use manager::{BlockStack, BlockStackManager};
pub use state::BlockStackMetrics;
pub use timing::{BlockStackTimingHandle, BlockStackTimingSnapshot};

#[cfg(test)]
#[path = "block_experimental/tests.rs"]
mod tests;
