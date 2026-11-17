# Stage performance methodology

This document defines the *measured* stage boundaries, the histogram rules, the
provenance published with each sample, and the limits of the claims. It applies
to the deterministic synthetic loopback stress laboratory
([`stress-lab-design.md`](stress-lab-design.md)) and to the live run report's
optional `stage_timing` field. It is not a benchmark or a production
certification.

## Measured boundaries

All durations use `std::time::Instant` (monotonic). A timer is started at the
first line of the boundary and its `elapsed()` value is recorded at the last;
no wall-clock timestamp is used for a duration. Both endpoints are inside the
same process. The coordinator, writer thread and journal writer thread share
the same histogram counters, so a boundary has **one** distribution — there is
no per-worker percentile to combine.

| Boundary | Start | End | Measured when |
|---|---|---|---|
| `core_processing` | Just before `CollectorCore::handle_event` (core admission in the coordinator event loop and drain loop) | Return of that call (state transition complete, outputs produced) | At least one normalized event is applied |
| `writer_queue_wait` | `Instant` captured in `WriterHandle::send` when a reserved queue slot is filled (admission) | Writer thread dequeues the `WriterCommand::Message` | At least one message is admitted |
| `sbe_write` | Entry to `MessageSink::write` on the writer thread | Return from `sink.write` | At least one SBE message is written (successful or failed) |
| `flush` | Entry to `MessageSink::flush` for an explicit `Flush` command | Return from `sink.flush` | A flush was requested and completed |
| `journal_append` | Entry to `SegmentWriter::write_record` on the journal writer thread | Return from that call | Journal is enabled and at least one record is appended |

The writer's shutdown `finish()`/`sync()` path and journal segment rotation,
finalization and fsync are **not** included in `flush`/`journal_append`: those
are durability phases with different semantics, and folding them into a per-item
latency would mislabel a batched cost.

A boundary with no samples is never published as `0`. It is published as
`{"state": "unavailable", "reason": "..."}` — for example, `journal_append` is
`unavailable` with `journal append boundary not enabled for this run` when
`journal.directory` is not configured.

## Histogram rules

Implementation: [`src/histogram.rs`](../src/histogram.rs).

- Fixed boundaries: buckets 0..33 have inclusive upper bounds `1, 2, 4, ...,
  2^33` nanoseconds; bucket 34 is an open-ended overflow bucket represented by
  the observed maximum. Bucket `i` covers `(2^(i-1), 2^i]`, bucket 0 covers
  `[0, 1]`.
- Sampling is allocation-free and lock-free: one `AtomicU64` per bucket plus
  total/min/max, all updated with relaxed ordering. `record()` takes a
  `Duration`, never allocates, and is safe to call from any thread.
- A report carries an explicit `sample_count`, `min_ns`, `max_ns`, `p50_ns`,
  `p90_ns`, `p95_ns`, `p99_ns`, `p999_ns`, an optional `p9999_ns`, and the raw
  count of **every** bucket (including empty ones).
- Percentiles are nearest-rank estimates computed from the raw counts:
  `rank = ceil(p * sample_count)` (at least 1), reported as the inclusive upper
  bound of the bucket containing that rank. They are bucket-resolution
  estimates, not exact order statistics.
- `p9999_ns` is omitted unless `sample_count >= 10_000`; four nines from fewer
  samples would imply resolution the data does not have.
- Workers are combined only by summing raw bucket counts and recomputing the
  percentile from the merged distribution (`HistogramReport::merge`). Averaging
  worker percentiles is never performed and would be invalid.
- Snapshots are read at report/manifest time, not on the hot path. Serialized
  field names are part of the evidence contract.

## Timestamp resolution versus accuracy

Durations are **stored** in nanoseconds. That is a unit, not an accuracy claim.
`Instant::now()` has a platform-dependent step, measured per host by the stress
harness as `clock_resolution_ns` (with an explicit reason when it cannot be
measured). The clock source is published as `clock_source`. Observed samples
also include scheduling delays, function-call overhead, and the `Instant`
reads themselves. Consequently:

- A reported value is never claimed to be nanosecond-accurate. Percentiles are
  accurate only to the bucket that contains them.
- Short intervals near or below the clock step are quantized; a non-zero
  sample proves the boundary was crossed, not the exact elapsed time.
- Samples from different hosts or builds are only comparable when the
  environment provenance below matches; no normalization is applied.

## Published provenance

The stress manifest places the report under `performance.stage_timing`:

- `boundaries` — the five `StageHistogram` values (measured with sample counts,
  or `unavailable` with a reason).
- `duration_unit` — `nanoseconds`.
- `percentile_method` — `nearest_rank_from_merged_bucket_counts`.
- `p9999_min_samples` — `10000`.
- `resolution_vs_accuracy` — the caveat above, inlined so the artifact is
  self-describing.
- `provenance` — `clock_source`, `clock_resolution_ns` (or `unavailable` with
  `clock_resolution_reason`), `build_profile`, `target_triple`, and
  `scenario_version` (16 for the combined integrated schema).

The same manifest already records `run_id`, `source_commit`,
`generator_version`, `prng_algorithm_version`, `configuration_sha256`,
`collector_binary_sha256`, `stress_harness_binary_sha256`, host OS/kernel/CPU,
`rustc_version`, and the full `scenario` object. Together these identify the
build and environment for every histogram. A live run carries the same
distribution in `RunReport.stage_timing` (an optional, backward-compatible
field).

## What is not claimed

- **No throughput, CPU, RSS or disk performance claim.** The existing
  `msgs_per_sec_*`, `cpu_seconds` and related fields remain `unavailable` with
  reasons. Stage latency is not throughput.
- **No network/wire latency.** These boundaries start after decoding; they do
  not measure exchange, TCP, or TLS delay. Source `latency_ewma_us` is a
  separate receive-time observation and is not used here.
- **No exact percentiles.** Values are bucket upper bounds, and resolution is
  bounded by the host clock step and instrumentation overhead.
- **No cross-workload comparison.** The workload cells are measured separately;
  no combined-run comparison, sustained capacity result, live venue performance
  result, or framework performance claim is made.
- **No averaging of worker percentiles.** Only raw counts are merged.
- **No production certification.** The evidence class is
  `synthetic-loopback`; artifacts are generated by deterministic tests, and
  previously published manifests are retained unchanged as historical evidence.

## Reproduce

```bash
cargo test --lib histogram::tests
cargo test --test full_universe_stress stage_timing_cell_records_real_writer_write_latency
python3 scripts/check_matrix_tests.py
python3 scripts/check_architecture.py --check
```
