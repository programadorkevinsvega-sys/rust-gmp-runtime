# F5 stealing: batch size and thief competition

## Objective

Follow up the isolated scheduler matrix with three targeted checks:

1. Record each routine's initial Processor, ordered transfer chain, and final
   selecting Processor.
2. Compare steal batches of 1, 4, 8, 16, and 32.
3. Compare 1, 2, 3, 7, and 15 thieves competing for one loaded local queue.

The production scheduler policy was not changed. The configurable batch and
identity trace are available only with `scheduler-benchmark`. Trace runs take
extra work while holding the scheduler lock, so their elapsed times are
diagnostic and excluded from performance comparisons.

## Environment and design

Rust 1.85.0, Linux x86_64, kernel `7.0.0-42-generic`, Intel Core i5-12450H
(12 logical CPUs), `powersave` governor. Base revision:
`b82fe1df54596880c00e9242cc9010d746ae8db9`; the worktree contained other
changes, so these results describe the tested worktree, not a clean revision.

The timed matrix used 4,096 routines and 3 samples per process. Each
thief-count × batch-size combination ran in three independent fresh processes:
75 process runs and 225 measured samples total. There were 16 refill rounds of
256 tasks from P0's local queue. Process summaries are the median of each
process's three sample times; the table reports the median of those three
process medians and their range. Each measured interval includes synchronized
selection, transfers, task drops, and test-harness barriers; it does not execute
closures.

The separate trace pass used batch 16, two samples per thief count, and one
fresh process for each of the five thief counts. It recorded 40,960
per-routine histories in total. The harness validates continuity of ownership,
exactly one final selection per routine, and an empty queue after each sample.

## Batch and thief-count results

Median selection/transfer time in milliseconds; process range is across the
three independent process medians.

| Thieves | Batch 1 | Batch 4 | Batch 8 | Batch 16 | Batch 32 |
|---:|---:|---:|---:|---:|---:|
| 1 | 13.011 (10.600–13.327) | 10.527 (10.413–10.762) | 10.368 (9.360–11.369) | 11.632 (10.101–11.852) | 10.514 (10.204–10.689) |
| 2 | 10.535 (9.050–10.962) | 12.441 (10.285–12.693) | 9.610 (8.976–10.081) | 10.448 (10.106–10.733) | 11.176 (9.143–12.000) |
| 3 | 12.905 (12.868–13.441) | 12.074 (11.158–12.185) | 11.884 (11.527–13.169) | 11.696 (11.461–12.759) | 13.314 (12.133–13.631) |
| 7 | 14.534 (13.983–14.699) | 14.245 (13.997–14.582) | 14.464 (14.126–14.627) | 13.970 (13.914–14.631) | 14.096 (13.065–14.542) |
| 15 | 15.203 (14.906–16.951) | 14.899 (14.690–15.399) | 15.562 (14.534–16.819) | 15.242 (14.678–15.398) | 15.302 (15.069–15.784) |

Normalized movement and failed-probe counts below are per unique routine.
Each cell is `transfers / failed attempts`; failed attempts are expected
empty-queue probes at the end of each round, not an estimate of mutex failures.

| Thieves | Batch 1 | Batch 4 | Batch 8 | Batch 16 | Batch 32 |
|---:|---:|---:|---:|---:|---:|
| 1 | 1.0000 / 0.0039 | 1.0000 / 0.0039 | 1.0000 / 0.0039 | 1.0000 / 0.0039 | 1.0000 / 0.0039 |
| 2 | 1.0000 / 0.0078 | 1.2339 / 0.0078 | 1.2491 / 0.0078 | 1.2311 / 0.0078 | 1.2567 / 0.0078 |
| 3 | 1.0000 / 0.0117 | 1.3389 / 0.0117 | 1.4067 / 0.0117 | 1.4437 / 0.0117 | 1.4986 / 0.0117 |
| 7 | 1.0000 / 0.0273 | 1.5121 / 0.0273 | 1.7622 / 0.0273 | 1.9215 / 0.0273 | 1.9906 / 0.0273 |
| 15 | 1.0000 / 0.0586 | 1.5767 / 0.0586 | 1.9403 / 0.0586 | 2.2227 / 0.0586 | 2.3876 / 0.0586 |

Larger batches reduce the number of steal attempts per routine, but do not
produce a consistent time improvement in this experiment. With more thieves,
larger batches also increase repeated transfers. Batch 1 prevents multi-hop
transfers in this setup, but this does not establish that it is the best
production policy: the matrix does not execute useful work or measure
application throughput, fairness, lock wait, or actual scheduler idle time.

## Identity trace

For batch 16, routines with more than one transfer and the maximum observed
transfer count were:

| Thieves | Routines traced | Routines transferred more than once | Maximum transfers | Final selections per P (P1, P2, …) |
|---:|---:|---:|---:|---|
| 1 | 8,192 | 0 | 1 | 8,192 |
| 2 | 8,192 | 2,003 | 3 | 4,187; 4,005 |
| 3 | 8,192 | 3,557 | 4 | 2,564; 2,749; 2,879 |
| 7 | 8,192 | 5,137 | 5 | 1,477; 1,088; 1,173; 1,158; 1,075; 1,106; 1,115 |
| 15 | 8,192 | 6,226 | 5 | 517; 513; 618; 535; 537; 559; 495; 577; 573; 527; 538; 536; 570; 545; 552 |

The task-level trace confirms that, in this deliberately skewed experiment,
some routines moved through multiple thieves before selection. All traced
identities retained a continuous owner, were selected once, and ended on the
Processor reported in their trace. More thieves increased multi-transfer
histories, but this isolated queue experiment does not show whether avoiding
those transfers would improve the real scheduler.

The diagnostic pass also records each thief's wall time draining work and
waiting at the experiment's refill barriers. Barrier-wait values are
harness-induced waits, not production scheduler idle time or a direct measure
of OS thread parking. The normal scheduler matrix does not yet measure
per-Processor idle duration.

| Thieves | Median barrier wait per thief (ms) | Observed range (ms) |
|---:|---:|---:|
| 1 | 0.815 | 0.523–1.106 |
| 2 | 0.715 | 0.300–1.117 |
| 3 | 0.718 | 0.545–0.985 |
| 7 | 0.792 | 0.643–1.629 |
| 15 | 1.885 | 1.456–2.094 |

These are waits at barriers inserted to refill the synthetic queue, not time
spent idle in the production scheduler; they must not be used to claim that
workers are inefficiently parked.

## Conclusion and limits

The results support the hypothesis that a routine can be stolen and later
stolen again when several Ps compete for a skewed queue. They do not identify
whether elapsed-time growth is caused by mutex contention, repeated movement,
barrier coordination, or OS scheduling. The process ranges overlap for many
batch comparisons, and there is no consistent winner across thief counts.

Do not change the default steal batch or replace the mutex based on these
measurements. The next useful evidence would instrument lock wait and
production per-P idle/active time separately, then repeat representative
settings with executing workloads and the public injection/refill path.

Raw timed matrix:
[20261114T2155-scheduler-steal-process-isolated.log](./runs/20261114T2155-scheduler-steal-process-isolated.log).
Raw identity traces:
[20261114T2155-scheduler-steal-traces-isolated.log](./runs/20261114T2155-scheduler-steal-traces-isolated.log).
