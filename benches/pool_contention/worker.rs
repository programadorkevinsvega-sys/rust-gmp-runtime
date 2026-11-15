use std::error::Error;
use std::sync::{Arc, Barrier};
#[cfg(feature = "allocator-benchmark")]
use std::time::Instant;

use rust_gmp_runtime::stack::{
    GoInspiredOptions, GoInspiredStackManager, ProcessorStackCache, StackConfig,
};

use super::{CACHE_BYTES, PoolPath, SLOTS_PER_SPAN};

pub(super) struct WorkerResult {
    pub(super) acquisition_ns: u128,
    pub(super) return_ns: u128,
    pub(super) operations: u64,
    pub(super) cache: Option<ProcessorStackCache>,
}

pub(super) fn new_manager(
    config: StackConfig,
    options: GoInspiredOptions,
) -> Result<GoInspiredStackManager, rust_gmp_runtime::stack::StackError> {
    #[cfg(feature = "allocator-benchmark")]
    {
        GoInspiredStackManager::new_benchmarked(config, options)
    }
    #[cfg(not(feature = "allocator-benchmark"))]
    {
        GoInspiredStackManager::with_options(config, options)
    }
}

pub(super) fn warm_global_pool(manager: &GoInspiredStackManager) -> Result<(), Box<dyn Error>> {
    let leases = (0..SLOTS_PER_SPAN)
        .map(|_| manager.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    drop(leases);
    Ok(())
}

pub(super) fn warm_worker_caches(
    manager: &GoInspiredStackManager,
    workers: usize,
) -> Result<Vec<Option<ProcessorStackCache>>, Box<dyn Error>> {
    let stride = manager
        .usable_size()
        .checked_add(manager.page_size())
        .ok_or("stack stride overflow")?;
    let slots_per_cache = CACHE_BYTES / stride;
    if slots_per_cache == 0 {
        return Err("cache is too small for one stack slot".into());
    }
    let mut caches = Vec::with_capacity(workers);
    for _ in 0..workers {
        let mut cache = manager.processor_cache()?;
        let mut leases = Vec::with_capacity(slots_per_cache);
        for _ in 0..slots_per_cache {
            leases.push(manager.allocate_cached(&mut cache)?);
        }
        for mut lease in leases {
            lease.release_to_cache(&mut cache)?;
        }
        caches.push(Some(cache));
    }
    Ok(caches)
}

pub(super) fn run_worker(
    manager: &GoInspiredStackManager,
    path: PoolPath,
    operations: usize,
    mut cache: Option<ProcessorStackCache>,
    start: Arc<Barrier>,
) -> Result<WorkerResult, String> {
    start.wait();
    #[cfg(feature = "allocator-benchmark")]
    let mut acquisition_ns = 0_u128;
    #[cfg(not(feature = "allocator-benchmark"))]
    let acquisition_ns = 0_u128;
    #[cfg(feature = "allocator-benchmark")]
    let mut return_ns = 0_u128;
    #[cfg(not(feature = "allocator-benchmark"))]
    let return_ns = 0_u128;
    let mut completed_operations = 0_u64;
    for _ in 0..operations {
        #[cfg(feature = "allocator-benchmark")]
        let acquisition_started = Instant::now();
        let mut lease = match path {
            PoolPath::Global => manager.allocate(),
            PoolPath::WorkerCache => manager.allocate_cached(
                cache
                    .as_mut()
                    .ok_or_else(|| "worker cache missing".to_owned())?,
            ),
        }
        .map_err(|error| error.to_string())?;
        #[cfg(feature = "allocator-benchmark")]
        {
            acquisition_ns =
                acquisition_ns.saturating_add(acquisition_started.elapsed().as_nanos());
        }
        #[cfg(feature = "allocator-benchmark")]
        let return_started = Instant::now();
        match path {
            PoolPath::Global => drop(lease),
            PoolPath::WorkerCache => lease
                .release_to_cache(
                    cache
                        .as_mut()
                        .ok_or_else(|| "worker cache missing".to_owned())?,
                )
                .map_err(|error| error.to_string())?,
        }
        completed_operations = completed_operations
            .checked_add(1)
            .ok_or_else(|| "worker operation count overflow".to_owned())?;
        #[cfg(feature = "allocator-benchmark")]
        {
            return_ns = return_ns.saturating_add(return_started.elapsed().as_nanos());
        }
    }
    Ok(WorkerResult {
        acquisition_ns,
        return_ns,
        operations: completed_operations,
        cache,
    })
}
