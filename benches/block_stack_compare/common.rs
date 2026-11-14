#[path = "allocator.rs"]
mod allocator;
#[path = "handle.rs"]
mod handle;
#[path = "stats.rs"]
mod stats;
#[path = "timing.rs"]
mod timing;

pub use allocator::{Allocator, WorkerCache};
pub use stats::Stats;

use rust_gmp_runtime::stack::VmaSnapshot;
use std::error::Error;

pub fn env_usize(name: &str, default: usize) -> Result<usize, Box<dyn Error>> {
    match std::env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}

pub fn env_usize_list(name: &str, default: &[usize]) -> Result<Vec<usize>, Box<dyn Error>> {
    let value = match std::env::var(name) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => {
            return Ok(default.to_vec());
        }
        Err(error) => return Err(error.into()),
    };
    value
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn supported_capacity(snapshot: VmaSnapshot, count: usize) -> Result<bool, Box<dyn Error>> {
    let count = u64::try_from(count)?;
    let needed = count
        .checked_mul(2)
        .and_then(|required| required.checked_add(1_024))
        .and_then(|required| snapshot.current.checked_add(required))
        .ok_or("VMA capacity calculation overflow")?;
    Ok(needed <= snapshot.limit)
}

pub fn current_vma_count() -> Result<u64, Box<dyn Error>> {
    stats::current_vma_count()
}

pub fn distribution(latencies: &mut [u64]) -> Option<(u64, u64, u64, u64)> {
    if latencies.is_empty() {
        return None;
    }
    latencies.sort_unstable();
    let p50 = percentile_index(latencies.len(), 50);
    let p95 = percentile_index(latencies.len(), 95);
    let p99 = percentile_index(latencies.len(), 99);
    Some((
        latencies[p50],
        latencies[p95],
        latencies[p99],
        latencies[latencies.len() - 1],
    ))
}

fn percentile_index(count: usize, percentile: usize) -> usize {
    count
        .checked_mul(percentile)
        .map(|scaled| scaled.div_ceil(100).saturating_sub(1))
        .unwrap_or(count - 1)
}
