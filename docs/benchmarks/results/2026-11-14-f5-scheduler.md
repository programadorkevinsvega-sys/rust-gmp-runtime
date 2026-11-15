# F5 scheduler characterization

Follow-up with 30 samples and a wider worker matrix:
[expanded characterization](./2026-11-14-f5-scheduler-expanded.md).
The latest follow-up separates uninstrumented throughput, burst latency, and
parked-worker wake:
[separated-mode characterization](./2026-11-14-f5-scheduler-separated-modes.md).
The process-isolated matrix, rotated independent runs, and controlled
imbalanced-stealing experiment are documented in the
[isolated matrix report](./2026-11-14-f5-scheduler-isolated-matrix.md).

## Scope and method

One local characterization of the F5.3/F5.4 scheduler, not a policy
comparison. Run:

```text
cargo bench --bench scheduler
```

The harness used Rust 1.85.0, Linux x86_64 (kernel 7.0.0-42-generic), an Intel
Core i5-12450H, and the `powersave` CPU governor. The repository base revision
was `12444b8`; the scheduler changes were uncommitted during the run.

The benchmark ran 12 samples of 256 pre-created routines per worker/workload
pair, after 32 warm-up routines per pair, with 1, 2, and 4 workers. Workloads
were no-op and a 64-step xorshift64* CPU loop. It rotated the six configurations
between samples. End-to-end includes submission and joins; stack/routine
creation is outside the timed interval. Publication-to-start latency starts
immediately before each `dispatch` call and ends at closure entry. Its
instrumentation (`Instant` and a per-task atomic write) adds overhead to both
workloads. Each batch checked exact publication, selection, completion and
per-Processor counts, plus empty queues at the end.

## Results

Times are nanoseconds per 256-task batch. Throughput is the median of
per-sample task throughput. Scheduler latency columns are medians across the
12 samples of each sample's per-task p50/p95. With only 12 samples, the
nearest-rank p95 of batch time is the maximum observed value.

| Workers | Workload | Median end-to-end | p95 end-to-end | Median throughput (tasks/s) | Median scheduling p50 | Median scheduling p95 |
|---:|---|---:|---:|---:|---:|---:|
| 1 | no-op | 1,766,503 | 2,561,570 | 143,726.8 | 803,730 | 1,533,666 |
| 1 | CPU64 | 1,426,540 | 1,924,539 | 166,407.6 | 708,822 | 1,268,105 |
| 2 | no-op | 1,254,770 | 3,322,233 | 196,358.9 | 620,848 | 1,071,350 |
| 2 | CPU64 | 1,187,689 | 1,912,432 | 209,822.0 | 591,712 | 1,041,802 |
| 4 | no-op | 1,704,209 | 2,349,774 | 149,065.3 | 768,279 | 1,483,755 |
| 4 | CPU64 | 1,564,097 | 2,399,942 | 156,249.8 | 686,109 | 1,357,321 |

Across each manager's full measured run, peak injection occupancy was 241, 234,
and 241 tasks for 1, 2, and 4 workers respectively; peak global occupancy was
zero and each local queue peaked at 16. These are cumulative high-water marks,
not per-sample peaks. Work-steal counts and per-P activity are preserved in the
raw output.

The noisy workload ordering and the different best medians do not establish an
optimal worker count or general scalability. This run only shows that the
implementation parks idle workers, wakes enough workers to execute a blocked
four-task batch concurrently, and drains measured batches without loss or
duplication. Results are specific to this host, governor, batch size, and
instrumented workloads.

Raw output: [20261114T2140-scheduler-f5.log](./runs/20261114T2140-scheduler-f5.log).
