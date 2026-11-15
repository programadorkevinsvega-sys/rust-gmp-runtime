# F5 scheduler cause attribution

## Question and method

This experiment separates three candidate explanations for weak scaling:

1. Contention when publishers enter the shared scheduler queue.
2. Cost or benefit from work stealing.
3. Too little useful work per routine to amortize scheduling and worker overhead.

The performance harness is `scheduler_attribution`, gated by
`scheduler-benchmark`. It runs the public `MachineManager::dispatch` path with
one manager per process, pre-created routines, publisher threads ready behind a
start barrier, and no diagnostic timing in the scheduler. Every sample verifies
exactly-once publication, selection, and completion, plus empty queues.

Five fresh processes were run for each of five configurations. Each process
measured seven batches of 2,048 routines at three closure-work sizes: 0, 64, and
4,096 xorshift iterations. The work sizes were interleaved in a rotated order
between samples. Configurations varied publishers (1/4/8 with four workers),
stealing (on/off with four workers and four publishers), and worker count
(one/four with four publishers). Thread creation, routine creation, and a
warm-up batch are outside the timed samples.

Reported medians are the median of the five process medians. Ranges are the
minimum and maximum of those process medians, not confidence intervals.
`submit` is wall time until all publishers return their handles; `publisher
dispatch sum` is the sum of each publisher thread's dispatch-loop duration and
may exceed wall time. The raw output also identifies each publisher's duration,
per-P selected-task counts, and steal activity.

## Uninstrumented end-to-end results

Times are milliseconds. `work` is deterministic xorshift iterations per
closure; 0 is no-op.

| Workers | Publishers | Stealing | Work | Submit wall | End-to-end median (process range) | Publisher dispatch sum | Steals / failed attempts |
|---:|---:|:---:|---:|---:|---:|---:|---:|
| 4 | 1 | on | 0 | 0.750 | 10.556 (9.943–10.638) | 0.658 | 92 / 28 |
| 4 | 1 | on | 64 | 0.720 | 10.461 (9.776–10.963) | 0.619 | 103 / 28 |
| 4 | 1 | on | 4,096 | 0.697 | 11.274 (10.277–11.724) | 0.631 | 85 / 28 |
| 4 | 4 | on | 0 | 0.693 | 10.781 (10.287–11.037) | 2.341 | 121 / 28 |
| 4 | 4 | on | 64 | 0.688 | 11.180 (10.761–11.711) | 2.339 | 99 / 28 |
| 4 | 4 | on | 4,096 | 0.653 | 11.912 (10.963–12.127) | 2.212 | 103 / 28 |
| 4 | 8 | on | 0 | 0.800 | 10.813 (10.426–11.702) | 5.197 | 95 / 28 |
| 4 | 8 | on | 64 | 0.787 | 11.237 (10.464–11.502) | 5.258 | 111 / 28 |
| 4 | 8 | on | 4,096 | 0.775 | 11.688 (11.265–12.031) | 5.141 | 92 / 28 |
| 4 | 4 | off | 0 | 0.708 | 11.014 (10.776–11.480) | 2.356 | 0 / 0 |
| 4 | 4 | off | 64 | 0.715 | 11.059 (10.500–11.399) | 2.268 | 0 / 0 |
| 4 | 4 | off | 4,096 | 0.670 | 12.153 (11.376–12.238) | 2.312 | 0 / 0 |
| 1 | 4 | on | 0 | 0.614 | 8.894 (8.858–9.126) | 1.848 | 0 / 7 |
| 1 | 4 | on | 64 | 0.603 | 9.164 (9.094–10.424) | 1.899 | 0 / 7 |
| 1 | 4 | on | 4,096 | 0.559 | 27.222 (26.971–27.291) | 1.727 | 0 / 7 |

### Interpretation

- **Work granularity changes the scaling result.** With four publishers fixed,
  four workers are about 21–22% slower than one for no-op and 64-iteration
  batches in this harness. At 4,096 iterations per routine, four workers
  complete the batch about 2.3 times
  faster. This is strong evidence that the short synthetic tasks do not expose
  enough useful parallel work to amortize the runtime/worker costs here. It is
  not a claim about all application workloads.
- **Concurrent publishers increase publisher-side work, but do not show a
  clear end-to-end penalty in this sample.** For four workers and no-op work,
  going from one to eight publishers raises median `publisher dispatch sum`
  from 0.658 ms to 5.197 ms, while end-to-end changes from 10.556 ms to
  10.813 ms. Process ranges overlap. The sum is concurrent thread-time, not
  elapsed wall time.
- **Disabling stealing does not produce a stable end-to-end improvement.**
  At four workers/four publishers, the on/off differences are about +2.2%,
  -1.1%, and +2.0% for off versus on at work sizes 0, 64, and 4,096
  respectively, with overlapping process ranges. Stealing is observable, but
  these runs do not establish it as the dominant penalty or prove a multi-hop
  stealing penalty.

## Where the measured penalty lands

The uninstrumented phase split gives the clearest wall-time location. With four
publishers and no-op work, one worker spends a median 0.614 ms in submission and
8.260 ms draining after submission (8.894 ms total). Four workers spend
0.693 ms submitting and 10.052 ms draining (10.781 ms total). Thus the
approximately 21% regression from adding workers is in the post-submit drain,
not primarily in the wall time to publish the batch. At 4,096 iterations per
routine, four workers instead reduce end-to-end time from 27.222 ms to
11.912 ms.

The separate diagnostics locate the dominant measured per-G phase more
narrowly, while remaining approximate because they are instrumented. For
2,048 no-op tasks and four workers, the median aggregate `run_once_with_permit`
active duration is 7.8–8.2 ms across the tested publisher counts, or about
3.6–4.0 us per G. Queue selection is about 0.21 ms total, or about 0.10 us per
G. That makes routine execution/context lifecycle the largest directly
measured per-G phase; it is roughly 35–40 times the selection duration.
Publication lock wait increases substantially with publishers (0.157, 1.411,
and 4.478 ms aggregate for 1/4/8 publishers), but this is summed concurrent
thread time. In the uninstrumented runs, changing from one to eight publishers
only moves four-worker no-op end-to-end from 10.556 to 10.813 ms, with
overlapping process ranges. Therefore the publication lock has contention, but
these data do not show it as the dominant wall-time penalty.

This identifies the likely penalty class for the tested tiny tasks:
fixed per-G execution/context-lifecycle and completion/scheduling overhead is
too large relative to useful work, so extra worker coordination is not
amortized. The existing timer surrounds `run_once_with_permit` as one region;
it does not separate context setup, closure invocation, completion bookkeeping,
or join handling. The benchmark therefore cannot claim which of those internal
operations is individually dominant.

## Separate lock diagnostics

To identify which callers wait on the shared lock, a separate
`scheduler-diagnostics` run used four workers, 2,048 no-op tasks, and 1/4/8
publishers in five fresh processes per setting. It reports per-Processor
publisher-lock and worker-loop lock wait/hold separately. These timings remain
diagnostic: `Instant` calls perturb scheduler paths, and the values are sums of
thread time, not elapsed wall time.

| Publishers | Publish lock wait (median; process range, us) | Worker-loop lock wait (median; process range, us) | Selection (median, us) | Drain wall (median, ms) |
|---:|---:|---:|---:|---:|
| 1 | 156.8 (105.1–306.4) | 181.3 (134.9–269.2) | 207.6 | 9.119 |
| 4 | 1,410.8 (1,070.7–1,735.4) | 338.0 (312.6–584.9) | 214.8 | 11.908 |
| 8 | 4,477.5 (4,053.4–5,038.6) | 389.9 (284.2–512.5) | 207.4 | 10.377 |

The diagnostic run establishes that **concurrent publishers spend increasing
aggregate time waiting to acquire the queue mutex** in this workload. Worker
loop wait grows much less, and selection time stays roughly constant. This
identifies where lock contention is observed and which caller class accounts
for it. It does **not** make mutex wait a wall-time fraction or prove that this
wait causes the end-to-end throughput change: waiting threads overlap, and
instrumentation changes the measured system.

## Conclusions and remaining limit

For these tested workloads:

- Shared-lock contention is present and grows specifically on the publication
  side as publisher concurrency rises.
- Useful-work granularity is the clearest explanation for why more workers
  fail to help the tiny no-op/64-step jobs; substantial synthetic work benefits
  from four workers.
- Stealing on/off produces no stable throughput difference in the public
  injection/refill workload. The controlled intervention is not a test of
  multi-hop frequency under an intentionally imbalanced real workload.
- No single scheduler component is proven to dominate end-to-end cost across
  workloads. In particular, do not claim that the mutex or stealing explains
  all worker-scaling degradation.
- The performance harness is built with `scheduler-benchmark`, whose runtime
  stealing switch adds a branch to selection. All comparisons use that same
  feature build; absolute times are not production baselines.

If publication becomes the target for optimization, first repeat this
comparison on another host and with application-representative jobs. Then test
a production-equivalent alternative to the shared publication path behind a
benchmark-only switch; keep exactly-once and shutdown behavior checks enabled.
Do not replace the mutex with a lock-free queue based on aggregate wait alone.

Environment: Rust 1.85.0, Linux x86_64, kernel `7.0.0-42-generic`,
Intel Core i5-12450H, 12 logical CPUs, `powersave` governor. One host and one
governor limit generalization.

Raw uninstrumented intervention matrix:
[20261114T2248-scheduler-cause-attribution-final.log](./runs/20261114T2248-scheduler-cause-attribution-final.log).

Raw sharded lock diagnostics:
[20261114T2248-scheduler-lock-attribution.log](./runs/20261114T2248-scheduler-lock-attribution.log).
