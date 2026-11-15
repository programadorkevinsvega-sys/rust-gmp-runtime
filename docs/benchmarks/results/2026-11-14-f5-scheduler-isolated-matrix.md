# F5 scheduler: isolated-process matrix and controlled stealing

## Why this follow-up

The earlier expanded matrix kept managers for 1, 2, 4, 6, 8, 10, 12, and 16
workers alive together (59 native worker threads). Even parked managers could
confound the 16-worker result through thread resources and process-wide
scheduling. This follow-up runs one worker configuration per fresh benchmark
process and limits the matrix to 1, 2, 4, 8, 12, and 16 workers.

## Environment and repetition design

Environment: Rust 1.85.0, Linux x86_64 (kernel 7.0.0-42-generic), Intel Core
i5-12450H (12 logical CPUs), `powersave` governor. Repository base revision:
`b82fe1df54596880c00e9242cc9010d746ae8db9`; working-tree changes were present,
so results do not describe a clean revision-only build.

For each worker count, the benchmark was launched in three independent fresh
processes, with worker-count order rotated between process repetitions. Each
process created one `MachineManager` with exactly that worker count and ran
three samples for each batch size (32/256/1,024/4,096), workload (no-op/CPU64),
and scenario (burst/flow), then three parked-wake samples.
Routines and stacks were created outside the timed interval. Every batch
verified publication, selection, completion, per-P selection totals, and empty
queues. The process matrix therefore contains 18 independent process runs, 864 timed
throughput batches, and 54 isolated parked-wake samples.

The tables show the median of the three per-process medians; each process
median summarizes its three batch samples. They are descriptive summaries, not
formal confidence intervals. Raw rows preserve the within-process variation.
The low sample counts per independent run and one-host design still limit
generalization.

## Burst and flow throughput

Median tasks/second. `CPU64` is a synthetic 64-step xorshift64* loop, not an
application workload.

### No-op

| Workers | Burst 32 | Burst 256 | Burst 1,024 | Burst 4,096 | Flow 32 | Flow 256 | Flow 1,024 | Flow 4,096 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 129,446 | 202,459 | 221,697 | 233,914 | 135,688 | 224,257 | 213,193 | 245,518 |
| 2 | 138,098 | 199,332 | 246,576 | 241,516 | 140,212 | 251,296 | 268,404 | 241,947 |
| 4 | 113,733 | 122,825 | 171,924 | 174,795 | 160,384 | 170,355 | 201,212 | 190,878 |
| 8 | 146,264 | 170,375 | 170,748 | 151,469 | 150,418 | 166,069 | 179,370 | 152,874 |
| 12 | 111,164 | 153,579 | 157,702 | 148,859 | 137,988 | 148,982 | 168,512 | 158,809 |
| 16 | 119,257 | 149,990 | 153,772 | 146,492 | 134,113 | 159,534 | 169,802 | 153,269 |

### CPU64

| Workers | Burst 32 | Burst 256 | Burst 1,024 | Burst 4,096 | Flow 32 | Flow 256 | Flow 1,024 | Flow 4,096 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 138,853 | 233,365 | 207,729 | 227,941 | 145,839 | 237,082 | 226,340 | 241,058 |
| 2 | 125,039 | 283,249 | 245,121 | 244,770 | 214,407 | 236,277 | 241,139 | 246,236 |
| 4 | 172,587 | 192,059 | 189,317 | 183,073 | 164,419 | 205,362 | 177,574 | 189,264 |
| 8 | 138,008 | 173,617 | 166,344 | 150,752 | 155,076 | 174,201 | 171,569 | 162,764 |
| 12 | 128,722 | 155,558 | 160,595 | 155,080 | 143,309 | 172,830 | 160,230 | 151,024 |
| 16 | 124,176 | 162,288 | 154,540 | 145,507 | 136,667 | 154,729 | 165,379 | 150,694 |

The best median varies with batch size, scenario, and workload. CPU64 is often
faster than no-op, which is not evidence that doing CPU work is cheaper; it
reinforces that these short batches include scheduler, wakeup, lifecycle, and
OS timing interactions. Four workers did not consistently improve over one,
but this isolated run does not identify which subsystem explains that result.

### Isolated parked wake

The manager was confirmed idle with all workers parked before each of 54
dispatches (nine samples per worker count across the three processes). This is
dispatch-to-entry, not a pure condition-variable or OS wake cost.

| Workers | Median ns | MAD ns | Observed range ns |
|---:|---:|---:|---:|
| 1 | 6,716 | 3,445 | 2,927–51,467 |
| 2 | 6,915 | 2,901 | 3,389–53,087 |
| 4 | 8,641 | 3,522 | 4,797–125,893 |
| 8 | 8,342 | 4,105 | 4,237–35,729 |
| 12 | 14,718 | 4,876 | 5,757–57,531 |
| 16 | 7,049 | 1,565 | 5,484–43,871 |

The broad ranges show why these short wake measurements should not be treated
as stable latency guarantees.

Burst publishes the entire batch while dispatched closures wait on a gate, then
releases them. Thus all work is published before its measured workload begins,
although worker threads may already have selected and be waiting inside the
closures. Flow allows earlier closures to run while the caller continues to
publish. Both measurements include submission and join; neither is pure
publication or selection cost.

## Controlled imbalanced-steal experiment

The normal public dispatch API publishes into the shared injection queue, so
it cannot deliberately concentrate tasks on one local P. A separate
feature-gated scheduler-only benchmark preloads one local queue, leaves its P
without a worker, and starts the other P workers as thieves. The local queue is
refilled in 256-task windows for 256, 1,024, or 4,096 tasks total. Routine/stack
creation and worker-thread startup are outside the measured interval; the
interval includes synchronized selection, local-queue transfer, task drop, and
barrier coordination. It does not execute closures or measure full
MachineManager throughput. One Processor has no thief, so its result is not
applicable.

Each configuration ran in three independent processes with five samples per
process. Times below are medians of the process medians; ranges are across
those three process medians. `transfers/sample` and `failed attempts/sample`
are the median per sample. A routine can be transferred more than once as
thieves redistribute work, so successful task transfers can exceed the number
of unique routines.

| Processors (thieves) | Tasks | Median elapsed ns (selection + barriers) | Range across process medians ns | Transfers/sample | Failed attempts/sample |
|---:|---:|---:|---:|---:|---:|
| 2 (1) | 256 | 673,846 | 659,276–758,758 | 256 | 1 |
| 2 (1) | 1,024 | 3,021,242 | 2,616,331–3,128,903 | 1,024 | 4 |
| 2 (1) | 4,096 | 10,055,187 | 9,827,727–11,027,141 | 4,096 | 16 |
| 4 (3) | 256 | 953,316 | 916,619–976,066 | 367 | 3 |
| 4 (3) | 1,024 | 3,271,754 | 2,956,273–3,305,744 | 1,514 | 12 |
| 4 (3) | 4,096 | 12,577,712 | 12,570,360–14,169,930 | 5,893 | 48 |
| 8 (7) | 256 | 870,439 | 820,198–884,142 | 500 | 7 |
| 8 (7) | 1,024 | 3,371,184 | 3,293,180–3,527,637 | 1,965 | 28 |
| 8 (7) | 4,096 | 13,581,016 | 13,544,992–13,719,496 | 7,754 | 112 |
| 12 (11) | 256 | 843,678 | 837,468–881,490 | 549 | 11 |
| 12 (11) | 1,024 | 3,407,508 | 3,318,715–3,715,987 | 2,144 | 44 |
| 12 (11) | 4,096 | 14,450,689 | 14,357,292–14,641,667 | 8,601 | 176 |
| 16 (15) | 256 | 896,542 | 891,362–916,173 | 551 | 15 |
| 16 (15) | 1,024 | 3,633,261 | 3,498,084–4,147,730 | 2,289 | 60 |
| 16 (15) | 4,096 | 14,860,930 | 14,798,009–15,451,899 | 9,062 | 240 |

The experiment confirms that idle Ps actually steal from the loaded local
queue, and the accounting verifies every task was selected once and every
queue drained. More thieves produced more transfers and failed attempts in
this deliberately skewed setup; the elapsed times are selection-only
diagnostics with barrier overhead, not scheduler throughput or evidence of
fairness for real applications.

## Interpretation and limits

Running one configuration per fresh process removes the previous 59-thread
cross-manager confound. The separated design still has limits: three independent
process runs per configuration, one host and governor, small synthetic work,
fixed task sizes, and sequential case order. It does not identify lock wait
time, OS wake cost, time spent in injection transfer, or per-P idle duration.
The controlled steal experiment measures queue selection and transfer with
direct local-queue injection, not the public publication path. No mutex or
lock-free policy change is justified by these data alone.

Raw throughput and parked-wake output:
[20261114T2139-scheduler-isolated-processes.log](./runs/20261114T2139-scheduler-isolated-processes.log).
Raw imbalanced-steal output:
[20261114T2139-scheduler-imbalanced-steal.log](./runs/20261114T2139-scheduler-imbalanced-steal.log).
