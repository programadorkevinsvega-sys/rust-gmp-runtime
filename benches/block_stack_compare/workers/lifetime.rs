use super::*;

pub fn run_lifetime_sample(
    allocator: Arc<Allocator>,
    workers: usize,
    operations_per_worker: usize,
    lifetimes: &[usize],
    cache_retention_operations: usize,
) -> Result<LifetimeSample, Box<dyn Error>> {
    if lifetimes.is_empty() || lifetimes.contains(&0) {
        return Err("lifetime schedule must contain positive durations".into());
    }
    let participants = workers
        .checked_add(1)
        .ok_or("lifecycle barrier participant count overflow")?;
    let ready = Arc::new(Barrier::new(participants));
    let start = Arc::new(Barrier::new(participants));
    let worker_threads = (0..workers)
        .map(|_| {
            let allocator = Arc::clone(&allocator);
            let ready = Arc::clone(&ready);
            let start = Arc::clone(&start);
            let lifetimes = lifetimes.to_vec();
            thread::spawn(move || {
                let setup = (|| {
                    let cache = allocator
                        .worker_cache()
                        .map_err(|error| error.to_string())?;
                    let mut latencies_ns = Vec::new();
                    latencies_ns
                        .try_reserve_exact(operations_per_worker)
                        .map_err(|error| error.to_string())?;
                    let mut held = Vec::new();
                    held.try_reserve_exact(lifetimes.iter().copied().max().unwrap_or(1))
                        .map_err(|error| error.to_string())?;
                    Ok::<_, String>((cache, latencies_ns, held))
                })();
                ready.wait();
                start.wait();
                let (mut cache, mut latencies_ns, mut held) = setup?;
                let mut operation = 0usize;
                for current in 0..operations_per_worker {
                    let operation_started = Instant::now();
                    let lease = allocator
                        .allocate_for_worker(&mut cache)
                        .map_err(|error| error.to_string())?;
                    held.push((current, lifetimes[current % lifetimes.len()], lease));
                    let mut index = 0;
                    while index < held.len() {
                        if current
                            .checked_sub(held[index].0)
                            .ok_or_else(|| "stack lifetime clock moved backwards".to_owned())?
                            .saturating_add(1)
                            >= held[index].1
                        {
                            let (_, _, lease) = held.swap_remove(index);
                            allocator
                                .release_for_worker(lease, &mut cache)
                                .map_err(|error| error.to_string())?;
                        } else {
                            index += 1;
                        }
                    }
                    if cache_retention_operations != 0
                        && (current + 1) % cache_retention_operations == 0
                    {
                        allocator
                            .flush_worker_cache(&mut cache)
                            .map_err(|error| error.to_string())?;
                    }
                    latencies_ns.push(
                        u64::try_from(operation_started.elapsed().as_nanos()).unwrap_or(u64::MAX),
                    );
                    operation = operation
                        .checked_add(1)
                        .ok_or_else(|| "lifecycle operation count overflow".to_owned())?;
                }
                while let Some((_, _, lease)) = held.pop() {
                    allocator
                        .release_for_worker(lease, &mut cache)
                        .map_err(|error| error.to_string())?;
                }
                Ok::<_, String>((operation, latencies_ns, cache))
            })
        })
        .collect::<Vec<_>>();

    ready.wait();
    let started = Instant::now();
    start.wait();
    let mut operations = 0usize;
    let mut latencies_ns = Vec::new();
    let mut worker_caches = Vec::new();
    let mut worker_error = None;
    for worker in worker_threads {
        match worker.join() {
            Ok(Ok((worker_operations, mut latencies, cache))) => {
                operations = operations
                    .checked_add(worker_operations)
                    .ok_or("lifecycle operation count overflow")?;
                latencies_ns.append(&mut latencies);
                worker_caches.push(cache);
            }
            Ok(Err(error)) => worker_error = Some(error),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    if let Some(error) = worker_error {
        return Err(error.into());
    }
    Ok(LifetimeSample {
        elapsed_ns: started.elapsed().as_nanos(),
        operations,
        latencies_ns,
        worker_caches,
        target_changes: 0,
        max_refill_target: 0,
    })
}
