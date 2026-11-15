use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::StackManager;

use super::{SAMPLE_COUNT, TASK_COUNT};

pub(super) fn measure_components(stack_manager: &StackManager) -> Result<(), Box<dyn Error>> {
    let processors = ProcessorPool::new(1)?;
    let processor = processors
        .iter()
        .next()
        .ok_or_else(|| std::io::Error::other("benchmark Processor missing"))?;
    let mut binding = processor.bind_current()?;
    let mut publish_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut queue_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut permit_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut completion_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut context_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut selection_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut channel_samples = Vec::with_capacity(SAMPLE_COUNT);

    for sample_index in 0..SAMPLE_COUNT {
        let routines = prepare_routines(stack_manager, TASK_COUNT)?;
        let started = Instant::now();
        for routine in &routines {
            routine.publish()?;
        }
        let publish_elapsed = started.elapsed();

        let queue_started = Instant::now();
        for routine in &routines {
            binding.enqueue(routine)?;
        }
        let mut queued = Vec::with_capacity(TASK_COUNT);
        while let Some(routine) = binding.dequeue()? {
            queued.push(routine);
        }
        if queued.len() != TASK_COUNT {
            return Err(format!(
                "expected {TASK_COUNT} queued routines, got {}",
                queued.len()
            )
            .into());
        }
        let queue_elapsed = queue_started.elapsed();
        drop(queued);

        let permit_started = Instant::now();
        for routine in &routines {
            routine.try_start()?.yield_to_scheduler()?;
        }
        let permit_elapsed = permit_started.elapsed();

        let terminal_routines = prepare_published_routines(stack_manager, TASK_COUNT)?;
        let completion_started = Instant::now();
        for routine in &terminal_routines {
            routine.try_start()?.complete()?;
        }
        let completion_elapsed = completion_started.elapsed();

        let context_routines = prepare_published_routines(stack_manager, TASK_COUNT)?;
        let context_started = Instant::now();
        for routine in &context_routines {
            routine.run_once(|| {})?;
        }
        let context_elapsed = context_started.elapsed();

        let selection_started = Instant::now();
        let selected = round_robin_selections(TASK_COUNT, 4);
        let selection_elapsed = selection_started.elapsed();
        if selected != TASK_COUNT {
            return Err(format!("expected {TASK_COUNT} worker selections, got {selected}").into());
        }

        let channel_elapsed = cross_thread_channel_roundtrip(TASK_COUNT)?;
        publish_samples.push(publish_elapsed);
        queue_samples.push(queue_elapsed);
        permit_samples.push(permit_elapsed);
        completion_samples.push(completion_elapsed);
        context_samples.push(context_elapsed);
        selection_samples.push(selection_elapsed);
        channel_samples.push(channel_elapsed);
        println!(
            "dispatch_stage sample={sample_index} routines={TASK_COUNT} publish_batch_ns={} processor_queue_roundtrip_ns={} permit_claim_yield_ns={} terminal_lifecycle_ns={} one_shot_context_ns={} round_robin_ns={} cross_thread_channel_roundtrip_ns={}",
            publish_elapsed.as_nanos(),
            queue_elapsed.as_nanos(),
            permit_elapsed.as_nanos(),
            completion_elapsed.as_nanos(),
            context_elapsed.as_nanos(),
            selection_elapsed.as_nanos(),
            channel_elapsed.as_nanos()
        );
    }

    report("publish", &mut publish_samples);
    report("processor_queue_roundtrip", &mut queue_samples);
    report("permit_claim_yield", &mut permit_samples);
    report("terminal_lifecycle", &mut completion_samples);
    report("one_shot_context", &mut context_samples);
    report("round_robin_selection", &mut selection_samples);
    report("cross_thread_channel_roundtrip", &mut channel_samples);
    Ok(())
}

fn round_robin_selections(count: usize, workers: usize) -> usize {
    let next_worker = AtomicUsize::new(0);
    let mut selections = 0;
    for _ in 0..count {
        std::hint::black_box(next_worker.fetch_add(1, Ordering::Relaxed) % workers);
        selections += 1;
    }
    selections
}

fn cross_thread_channel_roundtrip(count: usize) -> Result<Duration, Box<dyn Error>> {
    let (sender, receiver) = mpsc::channel();
    let receiver_thread = thread::spawn(move || {
        for _ in 0..count {
            receiver.recv().map_err(std::io::Error::other)?;
        }
        Ok::<(), std::io::Error>(())
    });
    let started = Instant::now();
    for value in 0..count {
        sender.send(value).map_err(std::io::Error::other)?;
    }
    receiver_thread
        .join()
        .map_err(|payload| std::io::Error::other(panic_message(payload)))??;
    Ok(started.elapsed())
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload")
        .to_owned()
}

fn prepare_routines(
    stack_manager: &StackManager,
    count: usize,
) -> Result<Vec<Routine>, Box<dyn Error>> {
    let mut routines = Vec::with_capacity(count);
    for _ in 0..count {
        routines.push(Routine::new(stack_manager, RoutineMetadata::new())?);
    }
    Ok(routines)
}

fn prepare_published_routines(
    stack_manager: &StackManager,
    count: usize,
) -> Result<Vec<Routine>, Box<dyn Error>> {
    let routines = prepare_routines(stack_manager, count)?;
    for routine in &routines {
        routine.publish()?;
    }
    Ok(routines)
}

fn report(label: &str, samples: &mut [Duration]) {
    let mut values = samples.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    values.sort_unstable();
    let median = values[values.len() / 2];
    let mut deviations = values
        .iter()
        .map(|value| value.abs_diff(median))
        .collect::<Vec<_>>();
    deviations.sort_unstable();
    let p95 = (values.len() * 95).div_ceil(100).saturating_sub(1);
    println!(
        "dispatch_stage summary component={label} samples={} routines_per_sample={TASK_COUNT} median_ns={median} mad_ns={} p95_ns={} max_observed_ns={} range_ns={}",
        values.len(),
        deviations[deviations.len() / 2],
        values[p95],
        values[values.len() - 1],
        values[values.len() - 1] - values[0]
    );
}
