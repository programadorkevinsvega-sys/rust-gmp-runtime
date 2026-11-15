use std::error::Error;
use std::time::{Duration, Instant};

use super::super::{SAMPLE_COUNT, TASK_COUNT};

pub(super) fn measure() -> Result<(), Box<dyn Error>> {
    let mut direct_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut unwind_samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let direct_started = Instant::now();
        for _ in 0..TASK_COUNT {
            let entry = std::hint::black_box(direct_entry as fn() -> usize);
            std::hint::black_box(entry());
        }
        let direct = direct_started.elapsed();

        let unwind_started = Instant::now();
        for _ in 0..TASK_COUNT {
            match std::panic::catch_unwind(|| {
                let entry = std::hint::black_box(direct_entry as fn() -> usize);
                entry()
            }) {
                Ok(value) => {
                    std::hint::black_box(value);
                }
                Err(payload) => {
                    drop(payload);
                    return Err("non-panicking catch_unwind baseline panicked".into());
                }
            }
        }
        let unwind = unwind_started.elapsed();
        println!(
            "dispatch_context sample={sample} calls={TASK_COUNT} direct_call_ns={} catch_unwind_body_ns={}",
            direct.as_nanos(),
            unwind.as_nanos()
        );
        direct_samples.push(direct);
        unwind_samples.push(unwind);
    }
    report("direct_call", &mut direct_samples);
    report("catch_unwind_body", &mut unwind_samples);
    Ok(())
}

#[inline(never)]
fn direct_entry() -> usize {
    std::hint::black_box(1)
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
        "dispatch_context summary component={label} samples={} calls_per_sample={TASK_COUNT} median_ns={median} mad_ns={} p95_ns={} max_observed_ns={} range_ns={}",
        values.len(),
        deviations[deviations.len() / 2],
        values[p95],
        values[values.len() - 1],
        values[values.len() - 1] - values[0]
    );
}
