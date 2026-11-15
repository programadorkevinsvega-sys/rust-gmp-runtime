//! Measures the synthetic CPU workloads without routine dispatch.

#[path = "machine_dispatch_stabilized/workload.rs"]
mod workload;

use std::time::Instant;

use workload::Workload;

const SAMPLE_COUNT: usize = 30;
const CALLS_PER_SAMPLE: usize = 128;
const WARMUP_CALLS: usize = 64;

fn main() {
    println!(
        "workload_cpu target={}-{} samples={SAMPLE_COUNT} calls_per_sample={CALLS_PER_SAMPLE} warmup_calls={WARMUP_CALLS} ordering=rotating dispatch=none",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for workload in Workload::ALL {
        for _ in 0..WARMUP_CALLS {
            workload.execute();
        }
    }

    let mut samples: [Vec<u128>; 4] = std::array::from_fn(|_| Vec::with_capacity(SAMPLE_COUNT));
    for sample in 0..SAMPLE_COUNT {
        for offset in 0..Workload::ALL.len() {
            let workload = Workload::ALL[(sample + offset) % Workload::ALL.len()];
            let started = Instant::now();
            for call in 0..CALLS_PER_SAMPLE {
                let seed = std::hint::black_box(call as u64 + 1);
                workload.execute_with_seed(seed);
            }
            let elapsed = started.elapsed().as_nanos();
            samples[workload.index()].push(elapsed);
            println!(
                "workload_cpu sample={sample} workload={} calls={CALLS_PER_SAMPLE} elapsed_ns={elapsed} ns_per_call={}",
                workload.label(),
                elapsed / CALLS_PER_SAMPLE as u128
            );
        }
    }

    for workload in Workload::ALL {
        report(workload, &mut samples[workload.index()]);
    }
}

fn report(workload: Workload, samples: &mut [u128]) {
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let mut deviations = samples
        .iter()
        .map(|sample| sample.abs_diff(median))
        .collect::<Vec<_>>();
    deviations.sort_unstable();
    let p95 = samples[(samples.len() * 95).div_ceil(100) - 1];
    println!(
        "workload_cpu summary workload={} samples={} median_ns={} median_ns_per_call={} mad_ns={} p95_ns={} max_ns={} range_ns={}",
        workload.label(),
        samples.len(),
        median,
        median / CALLS_PER_SAMPLE as u128,
        deviations[deviations.len() / 2],
        p95,
        samples[samples.len() - 1],
        samples[samples.len() - 1] - samples[0]
    );
}
