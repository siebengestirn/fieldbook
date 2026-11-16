# M5 workload specification (frozen for the first evidence increment)

This document freezes the first M5 workload before results are collected. It
describes what the harness produces, not what has been measured. The
authoritative design for the stress laboratory remains
[`stress-lab-design.md`](stress-lab-design.md); the canonical starting point is
[`baseline-post-m4.md`](baseline-post-m4.md).

## Invariants

- Synthetic generation passes through the **production** ingestion path: loopback
  Kalshi REST discovery, real `KalshiExchange`, real coordinator, real SBE writer
  and real journal. The harness never calls `CollectorCore` directly.
- One root seed drives distinct derived streams for universe generation, book
  seeding, and publishing. Changing workload shape must not perturb the other
  streams.
- Workload roles are assigned by the harness, never inferred from payload bytes
  by the collector.
- Acceptance is fail-closed. A cell may only publish `success` when anchoring,
  drain, exact coverage, SBE verification, completeness, journal finalization,
  decoy exclusion and the two-sided event ledger all pass inside the runner.
- Latency, CPU, and per-stage timing fields stay `unavailable` with an explicit
  reason until real instrumentation exists. Synthetic mock send timestamps are
  not used as measured latency.

## Profiles

| Profile | Status | Definition |
|---|---|---|
| `uniform` | Existing baseline | All eligible instruments have equal selection weight. |
| `zipf_hot_pair` | Existing legacy fixture-only profile | First 10% of markets receive 80% of events; retained for compatibility, not an M5 characterization profile. |
| `hot_quiet` weighted | **Implemented, corrected in M5.1** | Exactly 5% of eligible instruments are designated hot by a derived PRNG stream. Hot and cold indices are disjoint and exhaustive; 80% of draws select from the hot set and 20% from the cold set, so no hot instrument can be chosen through the cold branch. The realized share varies within a finite run. |
| `hot_quiet` exact | **Implemented in M5.1** | Same roles, but event assignment is an exact shuffled quota: 320 of 400 deltas are hot and 80 are cold, reproducible under the seed. Used as the frozen reference profile. |
| `burst` | Planned | Short deterministic spikes in publish rate. |
| `lifecycle` | Planned | Add/remove/settle universe churn through the mock's lifecycle channel. |
| `disconnect` | Planned | Deterministic replica/source closure and re-anchoring sweep. |
| `backpressure` | Existing failure fixture | Bounded mock/writer queues with explicit saturation classification. |
| `mixed` | Planned | Combined hot/quiet, lifecycle, disconnect and backpressure in one cell. |

## Frozen first workload

| Parameter | Value |
|---|---|
| Seed | `23` for weighted, `29` for exact-quota reference |
| Eligible instruments | 4,000 (`decoy_fraction_ppm = 20_000`) |
| Hot instruments | 200 (5% of eligible, `hot_fraction_ppm = 50_000`) |
| Hot event share | 80% (`hot_event_share_pct = 80`); exact mode pins 320/400 |
| Selection mode | `hot_selection = weighted` or `exact_quota` (`scenario_version = 4`) |
| Logical deltas | 400 |
| Topology | 2 shards × 2 replicas + 1 lifecycle connection |
| Journal | enabled |
| Timeouts | anchor 45 s, drain 10 s (runner cell limits) |

The hot set is exact, deterministic under the seed, disjoint from MVE/closed
decoys, and reproducible by
`SyntheticUniverse::generate(seed, symbols, decoys).hot_tickers(50_000)`.
Bumping the generator or scenario schema version is required before changing the
semantics of this table.

## Stage accounting

The ledger must distinguish, at minimum:

```
generated -> transmitted -> received -> decoded -> applied
          -> writer accepted -> SBE persisted -> journaled
```

The certified accounting model uses monotonic counters sampled at baseline
and final, with both directional discrepancies computed for every comparable
stage pair:

- `shortfall = max(0, expected - observed)`
- `excess = max(0, observed - expected)`

Missing evidence stays `None` and blocks certification; it is never zero.

Independently sourced counters:

- `enqueued_deltas` — mock outbound-queue admission counter.
- `transmitted_deltas` — mock websocket send completion counter.
- `received_frames` — `collector_data_frames_received_total`, accumulated in
  the collector core across reconnects.
- `delta_events_decoded` — `collector_delta_events_decoded_total`, one count
  per decoded `LevelChange` event, also core-accumulated.
- `writer_delta_admissions` — `collector_writer_delta_admissions_total`, the
  queue-admission boundary.
- `writer_delta_writes` — `collector_writer_delta_messages_total`, successful
  SBE delta writes. This is distinct from total writer messages because the
  output also contains snapshots.
- `sbe_deltas` — independent decode of the persisted capture.
- `journaled_records` — independent journal verification.

Certified adjacent gaps:

- `enqueue_transmit_shortfall` / `enqueue_transmit_excess`
- `decode_transmit_shortfall` / `decode_transmit_excess`
- `writer_admission_shortfall` / `writer_admission_excess`
- `writer_write_shortfall` / `writer_write_excess`

Replica fan-out appears on both transmit and decode sides, so no dedup
arithmetic is needed for that comparison. Duplicate suppression remains a
separately recorded downstream signal. A nonzero decode error count, missing
receive/decode/writer evidence, or any reversed counter discrepancy fails the
actual `CellAcceptance` path through `ledger_acceptance_reasons`, not only the
`EventLedger::reconciled()` helper.

Historical note: `failed-v3-seed-000b-200-coverage_set_mismatch.json` recorded
zero received frames because the earlier sampler read per-source status gauges
that reset across reconnects. The artifact is retained unchanged as superseded
evidence; a diagnostic regression test pins it. Current certification uses
core-accumulated monotonic counters.

Latency, CPU, peak RSS and queue-wait instrumentation remain unavailable and
block **performance** certification; they are never substituted with zero.

## Acceptance gates for this increment

| Gate | Proof |
|---|---|
| Correctness | `EventLedger::reconciled()` is true; `unexplained_drops = 0`; `persistence_gap = 0` |
| Workload semantics | `hot_set ∩ cold_set = ∅`, `hot_set ∪ cold_set = eligible_set`; exact mode pins 320 hot / 80 cold events |
| Coverage | expected, acknowledged and anchored symbol sets are identical at 4,000 |
| Continuity | no decoy appears in the capture; journal finalizes completely |
| Determinism | hot-role unit test pins the 200/4,000 split and rejects a different seed |
| Evidence | `mixed_hot_quiet_4000_cell_reconciles_and_certifies` passes offline; `reference_mixed_hot_quiet_manifest_is_reproducible` emits a versioned manifest when run explicitly with `STRESS_MANIFEST_OUT` |

The earlier `seed-0017-4000-hotquiet.json` manifest was generated by the
pre-correction sampler (residual draws could include hot instruments, giving
about 81% hot events in expectation). It is retained unchanged as historical
evidence, explicitly superseded by `scenario_version = 4`. The corrected v3
reference artifacts are `seed-0007-4000-uniform-v3.json`,
`seed-0017-4000-hotquiet-v3.json`, and `seed-001d-4000-hotquiet-v3.json`,
together with `failed-v3-seed-000b-200-coverage_set_mismatch.json`. All are
tied to source commit `5fa9dda`. Do not use the v2 manifest for comparative
workload claims.

Performance gates (p50/p95/p99/p99.9, CPU, RSS, disk, queue wait) are **not**
claimed by this increment. They require the instrumentation work identified in
the stress-lab audit before any number can be called evidence.

## Reproduce

```bash
cargo test --test full_universe_stress mixed_hot_quiet_4000_cell_reconciles_and_certifies
cargo test --test full_universe_stress mixed_hot_quiet_exact_4000_cell_assigns_320_hot_events
cargo test --test full_universe_stress stress_support::runner::tests::hot_cold_partition_is_exhaustive_and_disjoint
cargo test --test full_universe_stress stress_support::runner::tests::exact_event_targets_have_an_exact_quota_and_are_reproducible
cargo test --test full_universe_stress stress_support::universe::tests::hot_quiet_roles_are_exact_deterministic_and_free_of_decoys

STRESS_MANIFEST_OUT=/tmp/stress-v3 \
  cargo test --release --locked --test full_universe_stress -- \
  --ignored reference_exact_hot_quiet_manifest_is_reproducible --test-threads=1 --nocapture
```
