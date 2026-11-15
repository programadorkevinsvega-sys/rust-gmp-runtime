use std::error::Error;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::super::{SAMPLE_COUNT, TASK_COUNT};

pub(super) fn measure() -> Result<(), Box<dyn Error>> {
    let mut command_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut completion_samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let command = transfer_to_worker(TASK_COUNT)?;
        let completion = transfer_to_caller(TASK_COUNT)?;
        println!(
            "dispatch_channel sample={sample} messages={TASK_COUNT} command_delivery_send_and_drain_ns={} completion_delivery_send_and_drain_ns={}",
            command.as_nanos(),
            completion.as_nanos()
        );
        command_samples.push(command);
        completion_samples.push(completion);
    }
    report("command_delivery_send_and_drain", &mut command_samples);
    report(
        "completion_delivery_send_and_drain",
        &mut completion_samples,
    );
    Ok(())
}

fn transfer_to_worker(count: usize) -> Result<Duration, Box<dyn Error>> {
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        for _ in 0..count {
            receiver.recv().map_err(std::io::Error::other)?;
        }
        Ok::<(), std::io::Error>(())
    });
    let started = Instant::now();
    for message in 0..count {
        sender.send(message).map_err(std::io::Error::other)?;
    }
    worker
        .join()
        .map_err(|payload| std::io::Error::other(panic_message(payload)))??;
    Ok(started.elapsed())
}

fn transfer_to_caller(count: usize) -> Result<Duration, Box<dyn Error>> {
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        for message in 0..count {
            sender.send(message).map_err(std::io::Error::other)?;
        }
        Ok::<(), std::io::Error>(())
    });
    let started = Instant::now();
    for _ in 0..count {
        receiver.recv().map_err(std::io::Error::other)?;
    }
    worker
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
        "dispatch_channel summary component={label} samples={} messages_per_sample={TASK_COUNT} median_ns={median} mad_ns={} p95_ns={} max_observed_ns={} range_ns={}",
        values.len(),
        deviations[deviations.len() / 2],
        values[p95],
        values[values.len() - 1],
        values[values.len() - 1] - values[0]
    );
}
