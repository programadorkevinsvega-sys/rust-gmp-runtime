# Assembly candidates and F5 scheduler diagnostics

## Assembly: retain the measured context-switch backend

The x86_64 context switch is the only production assembly path identified in
this review that needs to manipulate the native stack and transfer control
across ABI contexts. An interleaved comparison ran 20 samples of one million
roundtrips with both stacks prefaulted:

| Backend | Median ns/transfer | Range ns/transfer |
|---|---:|---:|
| Previous backend | 12.828 | 12.211–15.769 |
| Current backend | 11.711 | 11.053–12.722 |
| Paired current minus previous | -1.375 | -3.797–-0.297 |

The current/previous median ratio is an 8.7% local reduction. Every paired
sample favored the current backend in this run. This remains a context-switch
microbenchmark, not a claim of an 8.7% end-to-end runtime improvement.

## Atomic dispatch candidate: no handwritten assembly

The optimized x86_64 build for `MachineManager::dispatch` lowers
`AtomicUsize::fetch_add(1, Relaxed)` to `lock xaddq`. The dynamic
`% workers.len()` path lowers to `divl`. Replacing `fetch_add` with handwritten
`lock xadd` would therefore repeat the instruction Rust already emits and add
an unsafe, architecture-specific implementation without reducing work.

A separate 30-sample component experiment compared dynamic modulo with a
precomputed mask for four workers. Both paths use the same Rust atomic
`fetch_add`; the mask is prepared outside the timed loop:

| Selection arithmetic | Median ns/batch of 128 | MAD | p95 |
|---|---:|---:|---:|
| Dynamic worker-count modulo | 1,063 | 8 | 1,224 |
| Precomputed power-of-two mask | 788 | 7 | 872 |

The mask saves 275 ns per batch, or about 2.15 ns per selected task, in this
isolated component. The real dispatch benchmark in the same run measured
no-op end-to-end medians of 610,322 ns, 513,689 ns, and 660,794 ns per batch
with 1, 2, and 4 workers. Even eliminating the entire measured modulo
component would therefore be below 0.06% of those end-to-end times. The
component does not prove that the mask fast path improves real dispatch, and
the benchmark had no production A/B implementation. Neither the fast path nor
handwritten atomic assembly is adopted.

The other atomic operations inspected use Rust `Atomic*` APIs and compile to
normal target instructions or compare/exchange loops. No independent
measurement shows that their generated sequences are a bottleneck. Syscalls
and platform operations remain behind standard-library or platform
abstractions; copying them into inline assembly would increase portability and
safety risk without evidence of a benefit.

## Diagnostic-only scheduler measurements

The `scheduler-diagnostics` feature adds no instrumentation to normal builds.
It collects queue-lock wait/hold durations, queue-selection durations,
per-Processor task activity and no-work time, condition-variable wait duration,
steal attempts, and transfer counts. The queue lock is never used to update
diagnostic counters: measurements are accumulated in cache-padded per-Processor
atomic shards after releasing it. Publication wait/hold is attributed to the
round-robin origin Processor; worker selection and parking are attributed to
the Processor that performs them. Totals are summed from those per-Processor
shards, with no global diagnostic counter.

The benchmark runs the real `MachineManager` with one fresh process per
workers/publishers combination, 4,096 preallocated routines, and synchronized
no-op publishers.

These are diagnostic runs, not performance runs. `Instant` calls remain in the
measured scheduler path when this feature is enabled, and per-Processor relaxed
atomics add diagnostic overhead. They are cache-padded and sharded rather than
global, and all counter updates happen outside the scheduler queue lock; this
removes the previous instrumentation's shared-lock writes, but does not make
the measurements non-perturbing. Lock totals cover `publish` and worker
`wait_next` acquisitions, not every administrative lock use or condition
variable reacquisition. Publication times are attributed to the Processor
chosen by round-robin before dispatch; they do not mean the worker on that
Processor acquired the lock. `no_work_ns` includes initial lock wait, queue
selection, and parking; `parked_ns` is a subset. A live diagnostics snapshot
loads counters independently and is not transactionally consistent. Unpark
latency is intentionally omitted: `Condvar` does not identify which waiter a
notification wakes, so FIFO-matched timestamps would not be a valid per-worker
measurement. `active_ns` sums across Ps and can exceed wall time.

All 16 configurations completed and validated exactly 4,096 publications,
selections, completions, and per-P selections:

| Workers | Publishers | Mutex wait (us) | Mutex hold (us) | Selection (us) | Steal attempts / failed | Transfers in / out |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 472.1 | 637.9 | 178.1 | 2 / 2 | 0 / 0 |
| 1 | 2 | 881.5 | 770.2 | 185.0 | 1 / 1 | 0 / 0 |
| 1 | 4 | 1,965.8 | 823.7 | 197.6 | 1 / 1 | 0 / 0 |
| 1 | 8 | 6,410.8 | 855.9 | 198.6 | 1 / 1 | 0 / 0 |
| 2 | 1 | 547.0 | 978.3 | 389.9 | 3 / 2 | 1 / 1 |
| 2 | 2 | 1,284.0 | 1,066.0 | 413.9 | 4 / 2 | 7 / 7 |
| 2 | 4 | 2,562.0 | 1,136.1 | 389.8 | 3 / 2 | 4 / 4 |
| 2 | 8 | 7,714.6 | 1,173.8 | 368.2 | 3 / 2 | 4 / 4 |
| 4 | 1 | 726.5 | 926.7 | 396.2 | 12 / 4 | 19 / 19 |
| 4 | 2 | 1,523.1 | 1,062.9 | 433.0 | 7 / 4 | 9 / 9 |
| 4 | 4 | 2,876.8 | 1,248.5 | 435.7 | 11 / 4 | 13 / 13 |
| 4 | 8 | 9,310.3 | 1,369.4 | 424.2 | 7 / 4 | 7 / 7 |
| 8 | 1 | 1,070.7 | 1,080.6 | 466.7 | 30 / 8 | 54 / 54 |
| 8 | 2 | 1,793.7 | 1,234.0 | 497.1 | 22 / 8 | 33 / 33 |
| 8 | 4 | 3,661.1 | 1,285.9 | 489.4 | 22 / 8 | 31 / 31 |
| 8 | 8 | 7,217.4 | 1,287.0 | 464.2 | 29 / 8 | 49 / 49 |

Wait totals rise with publisher count in this single diagnostic matrix, but
instrumentation and OS scheduling remain confounders; these are not estimates
of uncontended production costs or evidence identifying a root cause. Selection
totals remain below the per-Processor no-work totals. The workload is still a
synchronized no-op injection/refill path, not an application workload or a
forced queue-imbalance experiment. Transfer in/out totals match in every
configuration, and no configuration lost or duplicated a task.

Environment: Rust 1.85.0, Linux x86_64, kernel `7.0.0-42-generic`,
Intel Core i5-12450H, 12 logical CPUs, `powersave` governor. Base revision:
`b82fe1df54596880c00e9242cc9010d746ae8db9`; the worktree contained other
changes.

Raw dispatch and selection experiment:
[20261114T2157-assembly-candidate-dispatch.log](./runs/20261114T2157-assembly-candidate-dispatch.log).
Raw sharded-counter diagnostic matrix:
[20261114T2212-scheduler-diagnostic-sharded-matrix.log](./runs/20261114T2212-scheduler-diagnostic-sharded-matrix.log).
