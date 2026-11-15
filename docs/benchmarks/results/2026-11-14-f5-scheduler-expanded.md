# F5 scheduler: expanded worker-count characterization

Follow-up separating uninstrumented throughput, burst latency, and parked-worker
wake measurements:
[separated-mode characterization](./2026-11-14-f5-scheduler-separated-modes.md).
The later [isolated-process matrix](./2026-11-14-f5-scheduler-isolated-matrix.md)
addresses the simultaneous-manager confound.

## Scope and method

Follow-up to the initial 12-sample F5 scheduler run. This expands the matrix;
it does not change the scheduler policy or compare against another runtime.

Command:

```text
RUST_GMP_SCHEDULER_SAMPLES=30 RUST_GMP_SCHEDULER_WORKERS=1,2,4,6,8,10,12,16 cargo +1.85.0 bench --bench scheduler
```

The run used Rust 1.85.0, Linux x86_64 (kernel 7.0.0-42-generic), an Intel
Core i5-12450H with 12 logical CPUs, and the `powersave` governor. The repository
revision was `b82fe1df54596880c00e9242cc9010d746ae8db9`; the benchmark and other
working-tree changes were present during the run, so this is not a clean
revision-only result.

For each of eight worker counts and two workloads, the harness warmed up with
32 routines, then recorded 30 batches of 256 pre-created routines. Workloads
were no-op and a 64-step xorshift64* loop. All eight managers (59 worker
threads total) were started before measurement; inactive managers remained
parked. The sixteenth-worker condition therefore ran alongside parked threads
from the other configurations. Each batch verified exactly-once execution,
published/selected/completed counts, per-Processor counts, and empty queues.
The run completed all 480 batches (122,880 measured routines) without a
counter mismatch.

The tables below were recomputed from the per-sample raw output using the
conventional median (averaging the middle pair for 30 samples) and median
absolute deviation (MAD). Paired deltas compare each worker-count sample with
the 1-worker sample from the same rotated measurement round. The intervals are
95% percentile bootstrap intervals for the median paired difference, using
20,000 resamples of the 30 rounds. They are exploratory: no adjustment is made
for multiple comparisons, and the method cannot remove host noise or time
trends.

## Throughput and paired differences

Throughput is measured tasks per second over the batch end-to-end interval.
The paired difference is the median change versus one worker; the interval is
for that difference, in tasks/s.

### No-op

| Workers | Median throughput | MAD | Paired difference vs. 1 worker | 95% bootstrap interval |
|---:|---:|---:|---:|---:|
| 1 | 216,518.5 | 10,581.3 | — | — |
| 2 | 252,511.9 | 18,579.8 | +49,851.6 | [+29,715.9, +53,725.9] |
| 4 | 181,121.5 | 16,551.8 | -33,698.2 | [-42,839.4, -19,521.3] |
| 6 | 167,749.1 | 9,731.0 | -47,156.6 | [-49,858.2, -35,328.9] |
| 8 | 169,657.5 | 7,222.9 | -48,277.2 | [-50,519.2, -42,086.7] |
| 10 | 169,840.5 | 5,353.6 | -44,636.6 | [-52,170.7, -29,625.2] |
| 12 | 169,046.1 | 6,436.2 | -39,333.8 | [-53,151.8, -32,655.3] |
| 16 | 162,042.5 | 8,264.6 | -57,034.3 | [-63,018.5, -38,279.7] |

### CPU64

| Workers | Median throughput | MAD | Paired difference vs. 1 worker | 95% bootstrap interval |
|---:|---:|---:|---:|---:|
| 1 | 218,652.7 | 14,081.3 | — | — |
| 2 | 259,975.6 | 17,059.2 | +49,440.2 | [+24,168.2, +58,997.6] |
| 4 | 188,140.2 | 10,608.4 | -28,550.0 | [-40,722.6, -14,422.6] |
| 6 | 164,248.4 | 13,807.3 | -43,591.3 | [-52,784.1, -38,175.1] |
| 8 | 173,587.4 | 8,626.8 | -50,232.1 | [-54,880.4, -30,335.2] |
| 10 | 167,829.8 | 8,180.0 | -50,375.8 | [-55,734.8, -40,929.9] |
| 12 | 167,743.0 | 7,088.6 | -51,301.8 | [-57,636.2, -38,728.2] |
| 16 | 157,956.9 | 8,175.9 | -61,808.1 | [-72,304.0, -49,752.5] |

## Interpretation and limitations

Within this run, the paired differences consistently favor two workers over
one, while four through sixteen workers have lower median throughput than one
for both tested workloads. This is stronger local evidence than the original
three-point, 12-sample characterization, and it suggests investigating
coordination/contention before assuming that more workers help this
implementation.

It still does **not** establish a universal optimum or general scalability:

- The result is one host, one governor, one toolchain, one run, two synthetic
  workloads, one batch size, and one scheduler configuration.
- The workers are all represented by separate managers; up to 59 OS worker
  threads exist during the run. Inactive ones park, but this arrangement is not
  the same as repeatedly running one manager at each size.
- The workload and latency instrumentation include runtime and measurement
  overhead. CPU64 does not model arbitrary application work.
- Bootstrap intervals describe variation among these 30 paired rounds only;
  they do not account for multiple comparisons, environmental changes, or
  reproducibility across runs and machines.

Therefore, do not hard-code two workers as a scheduling policy from these
measurements. Repeat the matrix in fresh runs and vary batch size and workload
before drawing a policy conclusion. The result is evidence of this
implementation's behavior on this host, not evidence that the benchmark's
performance conclusion generalizes.

Raw per-sample output:
[20261114T2149-scheduler-expanded.log](./runs/20261114T2149-scheduler-expanded.log).
