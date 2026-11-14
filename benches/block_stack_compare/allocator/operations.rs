use super::*;

impl Allocator {
    pub fn allocate(&self) -> Result<Handle, Box<dyn Error>> {
        match self {
            Self::Fixed { manager, .. } => Ok(Handle::Fixed(manager.allocate()?)),
            Self::Block { manager, .. } => Ok(Handle::Block(manager.allocate()?)),
            Self::GoInspired { manager, .. } => Ok(Handle::GoInspired(manager.allocate()?)),
        }
    }

    pub fn worker_cache(&self) -> Result<WorkerCache, StackError> {
        match self {
            Self::GoInspired { manager, .. } => {
                Ok(WorkerCache::GoInspired(manager.processor_cache()?))
            }
            Self::Fixed { .. } | Self::Block { .. } => Ok(WorkerCache::None),
        }
    }

    pub fn allocate_for_worker(&self, cache: &mut WorkerCache) -> Result<Handle, Box<dyn Error>> {
        match (self, cache) {
            (Self::GoInspired { manager, .. }, WorkerCache::GoInspired(cache)) => {
                Ok(Handle::GoInspired(manager.allocate_cached(cache)?))
            }
            (_, WorkerCache::None) => self.allocate(),
            _ => Err("allocator and worker cache do not match".into()),
        }
    }

    pub fn release_for_worker(
        &self,
        handle: Handle,
        cache: &mut WorkerCache,
    ) -> Result<(), Box<dyn Error>> {
        match (handle, cache) {
            (Handle::GoInspired(mut stack), WorkerCache::GoInspired(cache)) => {
                stack.release_to_cache(cache)?;
            }
            (Handle::GoInspired(stack), WorkerCache::None) => drop(stack),
            (handle, WorkerCache::None) => drop(handle),
            (Handle::Fixed(stack), WorkerCache::GoInspired(_)) => drop(stack),
            (Handle::Block(stack), WorkerCache::GoInspired(_)) => drop(stack),
        }
        Ok(())
    }

    pub fn flush_worker_cache(&self, cache: &mut WorkerCache) -> Result<(), StackError> {
        match (self, cache) {
            (Self::GoInspired { .. }, WorkerCache::GoInspired(cache)) => {
                cache.flush_cache()?;
            }
            (Self::Fixed { .. } | Self::Block { .. }, WorkerCache::None) => {}
            _ => {
                return Err(StackError::MetricsInvariant {
                    operation: "flush a mismatched allocator worker cache",
                    observed: "allocator and cache do not match".to_owned(),
                });
            }
        }
        Ok(())
    }

    pub fn flush_all_caches_and_release_empty_spans(
        &self,
        caches: &mut [WorkerCache],
    ) -> Result<usize, Box<dyn Error>> {
        let Self::GoInspired { manager, .. } = self else {
            return Err("span release is only supported by the Go-inspired allocator".into());
        };
        let mut processor_caches: Vec<&mut ProcessorStackCache> = Vec::with_capacity(caches.len());
        for cache in caches {
            match cache {
                WorkerCache::GoInspired(cache) => processor_caches.push(cache),
                WorkerCache::None => {
                    return Err("Go-inspired allocator received a non-Go worker cache".into());
                }
            }
        }
        Ok(manager.flush_all_caches_and_release_empty_spans(&mut processor_caches)?)
    }

    pub fn page_size(&self) -> usize {
        match self {
            Self::Fixed { manager, .. } => manager.page_size(),
            Self::Block { manager, .. } => manager.page_size(),
            Self::GoInspired { manager, .. } => manager.page_size(),
        }
    }

    pub fn vma_snapshot(&self) -> Result<VmaSnapshot, Box<dyn Error>> {
        match self {
            Self::Fixed { manager, .. } => Ok(manager.vma_snapshot()?),
            Self::Block { manager, .. } => Ok(manager.vma_snapshot()?),
            Self::GoInspired { manager, .. } => Ok(manager.vma_snapshot()?),
        }
    }

    pub fn stats(&self) -> Result<Stats, Box<dyn Error>> {
        match self {
            Self::Fixed { manager, .. } => {
                Ok(stats::fixed(manager.metrics()?, manager.page_size())?)
            }
            Self::Block { manager, .. } => {
                Ok(stats::block(manager.metrics()?, manager.page_size())?)
            }
            Self::GoInspired { manager, .. } => {
                Ok(stats::go_inspired(manager.metrics()?, manager.page_size())?)
            }
        }
    }
}
