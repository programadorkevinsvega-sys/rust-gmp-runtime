use super::*;

pub fn run_demand_refill_sample(
    allocator: Arc<Allocator>,
    workers: usize,
    epochs: usize,
    demand_schedule: &[usize],
    mode: DemandRefillMode,
) -> Result<LifetimeSample, Box<dyn Error>> {
    if demand_schedule.is_empty() {
        return Err("demand schedule must not be empty".into());
    }
    let participants = workers
        .checked_add(1)
        .ok_or("demand-phase barrier participant count overflow")?;
    let ready = Arc::new(Barrier::new(participants));
    let start = Arc::new(Barrier::new(participants));
    let worker_threads = (0..workers)
        .map(|_| {
            let allocator = Arc::clone(&allocator);
            let ready = Arc::clone(&ready);
            let start = Arc::clone(&start);
            let demand_schedule = demand_schedule.to_vec();
            thread::spawn(move || {
                let setup = (|| {
                    let mut cache = allocator
                        .worker_cache()
                        .map_err(|error| error.to_string())?;
                    let max_demand = demand_schedule.iter().copied().max().unwrap_or(0);
                    let mut leases = Vec::new();
                    leases
                        .try_reserve_exact(max_demand)
                        .map_err(|error| error.to_string())?;
                    let mut latencies_ns = Vec::new();
                    latencies_ns
                        .try_reserve_exact(epochs)
                        .map_err(|error| error.to_string())?;
                    let (minimum, maximum) = match mode {
                        DemandRefillMode::Fixed(target) => {
                            allocator
                                .set_worker_cache_refill_target(&mut cache, target)
                                .map_err(|error| error.to_string())?;
                            (target, target)
                        }
                        DemandRefillMode::Adaptive { baseline, capacity } => (baseline, capacity),
                    };
                    Ok::<_, String>((cache, leases, latencies_ns, minimum, maximum))
                })();
                ready.wait();
                start.wait();
                let (mut cache, mut leases, mut latencies_ns, minimum, maximum) = setup?;
                let mut target = minimum;
                let mut max_refill_target = target;
                let mut exhausted_refills = 0usize;
                let mut idle_epochs = 0usize;
                let mut low_demand_epochs = 0usize;
                let mut target_changes = 0usize;
                let mut operations = 0usize;
                for epoch in 0..epochs {
                    let demand = demand_schedule[epoch % demand_schedule.len()];
                    let phase_started = Instant::now();
                    if demand == 0 {
                        allocator
                            .flush_worker_cache(&mut cache)
                            .map_err(|error| error.to_string())?;
                        idle_epochs = idle_epochs.saturating_add(1);
                        low_demand_epochs = 0;
                        exhausted_refills = 0;
                        if matches!(mode, DemandRefillMode::Adaptive { .. }) && idle_epochs >= 2 {
                            let next = (target / 2).max(minimum);
                            if next != target {
                                target = next;
                                max_refill_target = max_refill_target.max(target);
                                target_changes = target_changes.saturating_add(1);
                            }
                            idle_epochs = 0;
                        }
                    } else {
                        idle_epochs = 0;
                        if matches!(mode, DemandRefillMode::Adaptive { .. }) {
                            if demand < target / 2 {
                                low_demand_epochs = low_demand_epochs.saturating_add(1);
                                if low_demand_epochs >= 2 {
                                    let next = (target / 2).max(minimum);
                                    if next != target {
                                        target = next;
                                        max_refill_target = max_refill_target.max(target);
                                        target_changes = target_changes.saturating_add(1);
                                    }
                                    low_demand_epochs = 0;
                                }
                            } else {
                                low_demand_epochs = 0;
                            }
                        }
                        for _ in 0..demand {
                            if allocator
                                .worker_cache_is_empty(&cache)
                                .map_err(|error| error.to_string())?
                            {
                                if matches!(mode, DemandRefillMode::Adaptive { .. }) {
                                    if exhausted_refills >= 2 {
                                        let next = target.saturating_mul(2).min(maximum);
                                        if next != target {
                                            target = next;
                                            max_refill_target = max_refill_target.max(target);
                                            target_changes = target_changes.saturating_add(1);
                                        }
                                        exhausted_refills = 0;
                                    }
                                    exhausted_refills = exhausted_refills.saturating_add(1);
                                }
                                allocator
                                    .set_worker_cache_refill_target(&mut cache, target)
                                    .map_err(|error| error.to_string())?;
                            }
                            leases.push(
                                allocator
                                    .allocate_for_worker(&mut cache)
                                    .map_err(|error| error.to_string())?,
                            );
                            operations = operations
                                .checked_add(1)
                                .ok_or_else(|| "demand operation count overflow".to_owned())?;
                        }
                        while let Some(lease) = leases.pop() {
                            allocator
                                .release_for_worker(lease, &mut cache)
                                .map_err(|error| error.to_string())?;
                        }
                    }
                    latencies_ns.push(
                        u64::try_from(phase_started.elapsed().as_nanos()).unwrap_or(u64::MAX),
                    );
                }
                Ok::<_, String>((
                    operations,
                    latencies_ns,
                    cache,
                    target_changes,
                    max_refill_target,
                ))
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
    let mut target_changes = 0usize;
    let mut max_refill_target = 0usize;
    for worker in worker_threads {
        match worker.join() {
            Ok(Ok((worker_operations, mut latencies, cache, changes, max_target))) => {
                operations = operations
                    .checked_add(worker_operations)
                    .ok_or("demand operation count overflow")?;
                latencies_ns.append(&mut latencies);
                worker_caches.push(cache);
                target_changes = target_changes.saturating_add(changes);
                max_refill_target = max_refill_target.max(max_target);
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
        target_changes,
        max_refill_target,
    })
}
