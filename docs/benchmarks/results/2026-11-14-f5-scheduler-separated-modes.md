# F5 scheduler: separated throughput, burst latency, and parked wake

## Scope and method

Follow-up to the initial and expanded F5 characterizations. This run measures
three distinct cases rather than treating dispatch-to-entry as one generic
scheduling-latency result.

Command:

```text
RUST_GMP_SCHEDULER_SAMPLES=30 RUST_GMP_SCHEDULER_WORKERS=1,2,4,8,12 cargo +1.85.0 bench --bench scheduler
```

Environment: Rust 1.85.0, Linux x86_64 (kernel 7.0.0-42-generic), Intel Core
i5-12450H (12 logical CPUs), `powersave` governor. The repository base revision
was `b82fe1df54596880c00e9242cc9010d746ae8db9`; the working tree contained
uncommitted changes, so this is not a clean revision-only result. Five managers
(27 worker threads total) remained alive during the run; managers not serving
the current case were parked.

Each mode/workload/worker combination ran 30 samples of 256 pre-created Gs,
after 32 warm-up Gs. The no-op and 64-step xorshift64* workloads are synthetic.
The benchmark verified publication, selection, completion and per-P
accounting deltas for every batch, and checked that queues were empty.

### Measurement modes

- **Uninstrumented throughput:** no per-task `Instant`, latency atomic or
  per-entry counter in the closure. End-to-end timing and scheduler accounting
  remain, as do the scheduler's normal internal counters.
- **Instrumented burst latency:** each entry records time from immediately
  before its `dispatch` call to closure entry. Per-sample p50/p95 are calculated
  over its 256 tasks; the table reports the median of those per-sample
  percentiles. Instrumentation overhead is included.
- **Parked wake:** after verifying all workers for a manager are parked, one G
  is dispatched and its dispatch-to-entry time is recorded. This includes
  reservation, publication, notification, OS wake/scheduling, selection and
  closure entry; it does not isolate the wake syscall/condition variable from
  those other stages.

Submission time and completion-drain time are reported separately. No timestamp
was added inside the scheduler's publication, wake-return or selection path:
the benchmark therefore does **not** claim pure publication, wakeup or
selection costs. Adding stage timestamps there would perturb the runtime hot
path and requires a separately designed instrumentation mode.

Medians below use the conventional median (average of the middle two values
for 30 samples); MAD is median absolute deviation. The report does not treat
the observed p95s as SLOs.

## Uninstrumented throughput

Median throughput in tasks/s (MAD in parentheses):

| Workers | No-op | CPU64 |
|---:|---:|---:|
| 1 | 221,170.6 (11,384.0) | 216,545.0 (16,941.4) |
| 2 | 241,550.5 (21,645.5) | 253,752.2 (19,964.6) |
| 4 | 183,491.6 (22,419.6) | 195,167.5 (20,073.4) |
| 8 | 164,123.9 (10,059.8) | 175,034.1 (11,973.2) |
| 12 | 168,797.5 (6,637.9) | 169,724.0 (8,811.5) |

Two workers have the highest median in this run. Four workers did not improve
on one worker, and eight/twelve workers did not recover the difference. This
measurement does not identify whether queue coordination, notification,
context/lifecycle overhead, OS scheduling, or another factor dominates.

## Instrumented burst latency

Values are nanoseconds. Each cell reports the median of per-sample per-task
percentiles; the MAD shown is across the 30 batches, not across individual
tasks.

| Workers | Workload | Median batch p50 (MAD) | Median batch p95 (MAD) |
|---:|---|---:|---:|
| 1 | no-op | 555,232 (26,938) | 1,030,218 (57,644) |
| 1 | CPU64 | 561,122 (33,186) | 1,017,964 (37,400) |
| 2 | no-op | 488,222 (39,352) | 885,907 (94,860) |
| 2 | CPU64 | 466,644 (38,560) | 871,223 (67,895) |
| 4 | no-op | 598,120 (63,678) | 1,101,236 (90,021) |
| 4 | CPU64 | 588,748 (61,488) | 1,105,416 (121,986) |
| 8 | no-op | 633,178 (50,106) | 1,245,488 (87,181) |
| 8 | CPU64 | 643,995 (35,180) | 1,248,948 (93,422) |
| 12 | no-op | 600,512 (18,962) | 1,223,550 (47,961) |
| 12 | CPU64 | 615,166 (53,254) | 1,233,690 (97,839) |

These are dispatch-to-entry distributions, not the cost of selecting a G. A
task's elapsed time may include queue waiting, publication and lock contention,
worker wake/OS scheduling, selection and instrumentation. The per-batch p50 and
p95 are summarized across batches; they are not pooled percentiles over all
7,680 task latencies per configuration.

## Parked-worker wake case

One dispatch-to-entry sample per repetition, with all workers parked before
publication. Nanoseconds; MAD and p95 are calculated across 30 samples.

| Workers | Median | MAD | p95 |
|---:|---:|---:|---:|
| 1 | 3,765.5 | 206.5 | 6,572 |
| 2 | 3,539.5 | 214.0 | 6,843 |
| 4 | 6,929.5 | 2,252.5 | 11,894 |
| 8 | 4,414.0 | 968.0 | 13,449 |
| 12 | 5,192.5 | 1,085.5 | 11,512 |

This isolates a controlled **parked-start scenario**, not pure wake-up time.
The higher four-worker median and dispersion are observations from this run,
not evidence of a worker-count law.

## Queue routing and work-stealing evidence

`dispatch` assigns a round-robin origin Machine ID, then publishes to the
injection queue. A worker first checks its own local queue, refills from the
global or injection queue in batches of at most 16, and attempts a bounded
steal only after those shared queues are empty. The direct saturated-local test
fills one P's actual local queue, then calls the same blocking `wait_next` path
used by workers for an idle second P. It verifies unique selected IDs, all
256 tasks accounted for, and a nonzero steal counter.

The measured 256-task batches also observed steals: in uninstrumented batches,
stealing occurred in 54/60 two-worker batches and all 60 batches at 4, 8 and
12 workers; every P had nonzero median activity. This confirms steals and
cross-P work selection occur for this workload on this run. It does not prove
fairness for arbitrary workloads or starvation freedom. Existing stress,
parking/wakeup and shutdown-drain tests cover additional cases; these tests
provide evidence, not a formal concurrency proof.

## Conclusion and limits

This run distinguishes uninstrumented throughput from instrumented latency and
shows a parked-worker case. It does not isolate the individual time of
publication, condition-variable wake, OS rescheduling, queue selection or
closure entry. The dispatch-to-entry figures are not SLOs and must not be used
as pure scheduler-selection latency.

Results are one run on one host, with one batch size and two small synthetic
workloads. All configurations were managers in one process, with inactive
workers parked. The observed two-worker peak is not a production policy
recommendation, and no general scalability conclusion follows. Repeated runs
on other hosts and workloads remain necessary.

Raw per-sample output:
[20261114T2128-scheduler-f5-modes.log](./runs/20261114T2128-scheduler-f5-modes.log).
