use super::*;

pub fn run_synchronized_refill_sample(
    allocator: Arc<Allocator>,
    workers: usize,
    refill_target: usize,
    waves: usize,
) -> Result<RefillWaveSample, Box<dyn Error>> {
    let participants = workers
        .checked_add(1)
        .ok_or("refill-wave barrier participant count overflow")?;
    let ready = Arc::new(Barrier::new(participants));
    let start = Arc::new(Barrier::new(participants));
    let finished_wave = Arc::new(Barrier::new(participants));
    let aborted = Arc::new(AtomicBool::new(false));
    let first_error = Arc::new(std::sync::Mutex::new(None::<String>));
    let worker_threads = (0..workers)
        .map(|_| {
            let allocator = Arc::clone(&allocator);
            let ready = Arc::clone(&ready);
            let start = Arc::clone(&start);
            let finished_wave = Arc::clone(&finished_wave);
            let aborted = Arc::clone(&aborted);
            let first_error = Arc::clone(&first_error);
            thread::spawn(move || {
                let setup = (|| {
                    let cache = allocator
                        .worker_cache()
                        .map_err(|error| error.to_string())?;
                    let mut leases = Vec::new();
                    leases
                        .try_reserve_exact(refill_target)
                        .map_err(|error| error.to_string())?;
                    let mut wave_latencies_ns = Vec::new();
                    wave_latencies_ns
                        .try_reserve_exact(waves)
                        .map_err(|error| error.to_string())?;
                    Ok::<_, String>((cache, leases, wave_latencies_ns))
                })();
                ready.wait();
                let mut cache_and_leases = match setup {
                    Ok(setup) => Some(setup),
                    Err(error) => {
                        record_worker_error(&first_error, &aborted, error);
                        None
                    }
                };
                let mut wave_latencies_ns = match cache_and_leases.as_mut() {
                    Some((_, _, latencies)) => std::mem::take(latencies),
                    None => Vec::new(),
                };
                let mut operations = 0usize;
                for _ in 0..waves {
                    start.wait();
                    if !aborted.load(Ordering::Acquire) {
                        let wave_started = Instant::now();
                        let result = (|| {
                            let (cache, leases, _) = cache_and_leases
                                .as_mut()
                                .ok_or_else(|| "refill-wave worker setup failed".to_owned())?;
                            for _ in 0..refill_target {
                                leases.push(
                                    allocator
                                        .allocate_for_worker(cache)
                                        .map_err(|error| error.to_string())?,
                                );
                            }
                            operations = operations
                                .checked_add(refill_target)
                                .ok_or_else(|| "refill-wave operation count overflow".to_owned())?;
                            while let Some(lease) = leases.pop() {
                                allocator
                                    .release_for_worker(lease, cache)
                                    .map_err(|error| error.to_string())?;
                            }
                            allocator
                                .flush_worker_cache(cache)
                                .map_err(|error| error.to_string())?;
                            Ok::<_, String>(())
                        })();
                        wave_latencies_ns.push(
                            u64::try_from(wave_started.elapsed().as_nanos()).unwrap_or(u64::MAX),
                        );
                        if let Err(error) = result {
                            record_worker_error(&first_error, &aborted, error);
                        }
                    }
                    finished_wave.wait();
                }
                Ok::<_, String>((operations, wave_latencies_ns))
            })
        })
        .collect::<Vec<_>>();

    ready.wait();
    let started = Instant::now();
    for _ in 0..waves {
        start.wait();
        finished_wave.wait();
    }
    let elapsed_ns = started.elapsed().as_nanos();
    let mut operations = 0usize;
    let mut wave_latencies_ns = Vec::new();
    let mut worker_error = None;
    for worker in worker_threads {
        match worker.join() {
            Ok(Ok((worker_operations, mut latencies))) => {
                operations = operations
                    .checked_add(worker_operations)
                    .ok_or("refill-wave operation count overflow")?;
                wave_latencies_ns.append(&mut latencies);
            }
            Ok(Err(error)) => worker_error = Some(error),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    let recorded_error = match first_error.lock() {
        Ok(mut error) => error.take(),
        Err(poisoned) => Some(format!(
            "refill-wave error recorder mutex was poisoned: {poisoned}"
        )),
    };
    let worker_error = worker_error.or(recorded_error);
    if let Some(error) = worker_error {
        return Err(error.into());
    }
    Ok(RefillWaveSample {
        elapsed_ns,
        operations,
        wave_latencies_ns,
    })
}
pub(super) fn record_worker_error(
    first_error: &std::sync::Mutex<Option<String>>,
    aborted: &AtomicBool,
    error: String,
) {
    aborted.store(true, Ordering::Release);
    match first_error.lock() {
        Ok(mut first_error) => {
            if first_error.is_none() {
                *first_error = Some(error);
            }
        }
        Err(poisoned) => {
            let mut first_error = poisoned.into_inner();
            if first_error.is_none() {
                *first_error = Some(format!(
                    "{error}; refill-wave error recorder mutex was poisoned"
                ));
            }
        }
    }
}
