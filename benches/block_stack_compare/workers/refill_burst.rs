use super::*;

pub fn run_refill_burst_sample(
    allocator: Arc<Allocator>,
    workers: usize,
    burst_size: usize,
    bursts_per_worker: usize,
) -> Result<RefillBurstSample, Box<dyn Error>> {
    let participants = workers
        .checked_add(1)
        .ok_or("refill-burst barrier participant count overflow")?;
    let ready = Arc::new(Barrier::new(participants));
    let start = Arc::new(Barrier::new(participants));
    let worker_threads = (0..workers)
        .map(|_| {
            let allocator = Arc::clone(&allocator);
            let ready = Arc::clone(&ready);
            let start = Arc::clone(&start);
            thread::spawn(move || {
                let setup = (|| {
                    let cache = allocator
                        .worker_cache()
                        .map_err(|error| error.to_string())?;
                    let mut leases = Vec::new();
                    leases
                        .try_reserve_exact(burst_size)
                        .map_err(|error| error.to_string())?;
                    let mut latencies = Vec::new();
                    latencies
                        .try_reserve_exact(bursts_per_worker)
                        .map_err(|error| error.to_string())?;
                    Ok::<_, String>((cache, leases, latencies))
                })();
                ready.wait();
                start.wait();
                let (mut cache, mut leases, mut latencies) = match setup {
                    Ok(setup) => setup,
                    Err(error) => return Err(error),
                };
                for _ in 0..bursts_per_worker {
                    let burst_started = Instant::now();
                    for _ in 0..burst_size {
                        leases.push(
                            allocator
                                .allocate_for_worker(&mut cache)
                                .map_err(|error| error.to_string())?,
                        );
                    }
                    while let Some(lease) = leases.pop() {
                        allocator
                            .release_for_worker(lease, &mut cache)
                            .map_err(|error| error.to_string())?;
                    }
                    latencies.push(
                        u64::try_from(burst_started.elapsed().as_nanos()).unwrap_or(u64::MAX),
                    );
                }
                Ok((latencies, cache))
            })
        })
        .collect::<Vec<_>>();

    ready.wait();
    let started = Instant::now();
    start.wait();
    let mut burst_latencies_ns = Vec::new();
    let mut worker_caches = Vec::new();
    let mut worker_error = None;
    for worker in worker_threads {
        match worker.join() {
            Ok(Ok((mut latencies, cache))) => {
                burst_latencies_ns.append(&mut latencies);
                worker_caches.push(cache);
            }
            Ok(Err(error)) => worker_error = Some(error),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    if let Some(error) = worker_error {
        return Err(error.into());
    }
    let elapsed_ns = started.elapsed().as_nanos();
    let operations = workers
        .checked_mul(burst_size)
        .and_then(|count| count.checked_mul(bursts_per_worker))
        .ok_or("refill-burst operation count overflow")?;
    Ok(RefillBurstSample {
        elapsed_ns,
        operations,
        burst_latencies_ns,
        worker_caches,
    })
}
