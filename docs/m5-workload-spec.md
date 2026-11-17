# M5 workload specification (frozen workload, versioned evidence increments)

This document freezes the M5 workload before results are collected. It
describes what the harness produces, not what has been measured. M5.2 adds
deterministic burst schedules without changing the frozen 4,000-instrument
exact/weighted workload, its seed, its hot-set definition or its event budget.
The authoritative design for the stress laboratory remains
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
| `burst` | **Implemented in M5.2** | Four deterministic profiles (`step`, `impulse`, `repeated_impulses`, `ramp`) that partition the same 400 logical deltas into publish phases. No sleeps, timing instrumentation or performance claims. |
| `lifecycle` | Planned | Add/remove/settle universe churn through the mock's lifecycle channel. |
| `disconnect` | Planned | Deterministic replica/source closure and re-anchoring sweep. |
| `backpressure` | Existing failure fixture | Bounded mock/writer queues with explicit saturation classification. |
| `mixed` | Planned | Combined hot/quiet, lifecycle, disconnect and backpressure in one cell. |

## M5.2 burst profiles

A burst profile changes only the **phase decomposition** of the logical-event
budget. The seed, universe, hot set, role selection, topology, journal and
400-event budget are identical to the frozen M5.1 exact workload. The schedule
is defined by exact event counts; no sleep, wall-clock rate control, timing
measurement or throughput claim is part of this increment.

| Profile | Phase event counts (400 total) | Declared rate multipliers | Meaning |
|---|---|---|---|
| `step` | 100 / 200 / 100 (baseline / burst / recovery) | 1 / 2 / 1 | One sustained doubling of the declared rate. |
| `impulse` | 40 / 280 / 80 (baseline / impulse / recovery) | 1 / 5 / 1 | One concentrated spike followed by a quiet tail. |
| `repeated_impulses` | 80 / 80 / 80 / 80 / 80 | 1 / 3 / 3 / 3 / 1 | Several equally sized impulses separated by quiet phases. |
| `ramp` | 40 / 80 / 80 / 80 / 120 | 1 / 1 / 2 / 3 / 1 | Rising declared multipliers, then a larger recovery phase. |

Counts are derived deterministically by integer weights and always sum to the
configured event budget; an indivisible remainder is assigned to the
highest-weight phase (`tests/stress_support/runner.rs` unit tests pin the
partition). The `target_rate_per_tick` and `rate_multiplier` fields are
**declared schedule labels** recorded so a manifest reader can reproduce the
intended shape. They are not measured rates, not latency figures, and not
performance evidence. The manifest states this under
`workload_schedule.rate_semantics`; all performance fields remain
`unavailable`.

Publishing is phased: each phase publishes its exact count, then the existing
drain barrier waits until every production counter reaches the **cumulative**
expected count (plus the mock's decoded-frame catch-up) before the next phase
is published. This is implemented in `run_cell_inner` with `await_drain`; the
existing one-shot workload is the single `steady` phase with `burst = None`.
The final event ledger, disposition accounting, coverage and SBE verification
still run once over the whole cell, so no check is weakened.

The manifest records the explicit schedule under `workload_schedule`:

- `workload_id` — `{profile}-seed-{seed:04x}-{symbols}`;
- `profile` — `steady | step | impulse | repeated_impulses | ramp`;
- `seed`, `instrument_count`, `eligible_count`;
- `hot_set` — `fraction_ppm`, `member_count`, `selection`, `event_share_pct` and
  the canonical `sha256` of the sorted hot set;
- `phases` — per-phase `name`, `events`, `target_rate_per_tick`,
  `rate_multiplier`;
- `expected_total_events` — sum of all phase counts;
- `fault_schedule` — empty for M5.2 burst cells (no fault injection is
  scheduled);
- `delivery.phases` — actual per-phase `published_events`,
  `cumulative_expected` and `drain_completed`.

Scenario `scenario_version` is bumped to **6**; burst cell IDs carry the
profile label so two profiles never overwrite one another in the index. The
v5 disposition-certified manifests remain immutable historical evidence.

## Frozen first workload

| Parameter | Value |
|---|---|
| Seed | `23` for weighted, `29` for exact-quota reference |
| Eligible instruments | 4,000 (`decoy_fraction_ppm = 20_000`) |
| Hot instruments | 200 (5% of eligible, `hot_fraction_ppm = 50_000`) |
| Hot event share | 80% (`hot_event_share_pct = 80`); exact mode pins 320/400 |
| Selection mode | `hot_selection = weighted` or `exact_quota` (`scenario_version = 6`; burst profiles are v6-only) |
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
- `transmit_decode_shortfall` / `transmit_decode_excess`
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

## Terminal delta disposition (v5)

The v5 ledger certifies that every decoded delta has exactly one terminal
outcome. `DeltaDisposition` defines eleven mutually exclusive variants, and
`CollectorStats` carries a counter for each. The acceptance path verifies:

```
decoded deltas = emitted + suppressed replica + no active source
               + unknown symbol + non-live + unassigned source
               + invalid book + shadow clock regression
               + apply rejected + zero noop + SBE conversion failed
```

Transport counters are owned by the session worker and include malformed
frames; core-ingress counters are sampled separately under distinct metric
names. The full contract is in
[`EVENT_ACCOUNTING_CONTRACT.md`](EVENT_ACCOUNTING_CONTRACT.md).

## Acceptance gates for this increment

| Gate | Proof |
|---|---|
| Correctness | `EventLedger::reconciled()` is true; `unexplained_drops = 0`; `persistence_gap = 0` |
| Workload semantics | `hot_set ∩ cold_set = ∅`, `hot_set ∪ cold_set = eligible_set`; exact mode pins 320 hot / 80 cold events |
| Coverage | expected, acknowledged and anchored symbol sets are identical at 4,000 |
| Continuity | no decoy appears in the capture; journal finalizes completely |
| Determinism | hot-role unit test pins the 200/4,000 split and rejects a different seed |
| Evidence | `mixed_hot_quiet_4000_cell_reconciles_and_certifies` passes offline; `reference_mixed_hot_quiet_manifest_is_reproducible` emits a versioned manifest when run explicitly with `STRESS_MANIFEST_OUT` |
| M5.2 burst phases | `burst_step_cell_reconciles_all_phases` and `burst_impulse_cell_reconciles_all_phases` prove acceptance, exact per-phase counts (100/200/100 and 40/280/80), cumulative drain completion, reconciled ledger, exact 4,000 coverage, SBE/completeness/journal verification, empty fault schedule and a manifest carrying the full workload schedule |
| M5.2 determinism | `burst_profiles_partition_the_budget_exactly_and_deterministically` and `burst_profile_counts_are_pinned` pin the four partitions; `schedule_records_id_seed_hot_definition_and_empty_faults` pins the recorded schedule fields |

The earlier `seed-0017-4000-hotquiet.json` manifest was generated by the
pre-correction sampler (residual draws could include hot instruments, giving
about 81% hot events in expectation). It is retained unchanged as historical
evidence, explicitly superseded by `scenario_version = 5`. The corrected v3
reference artifacts are retained as historical evidence. The certified v4
artifacts are `seed-0007-4000-uniform-v4.json`,
`seed-0017-4000-hotquiet-v4.json`, `seed-001d-4000-hotquiet-v4.json`, and
`failed-v4-seed-000b-200-coverage_set_mismatch.json`, v4 tied to `3f22f47`; the v5 disposition-certified artifacts
`seed-0007-4000-uniform-v5.json`, `seed-0017-4000-hotquiet-v5.json`,
`seed-001d-4000-hotquiet-v5.json` and
`failed-v5-seed-000b-200-coverage_set_mismatch.json` are tied to `5bb54e6`. Do not use the v2 manifest for comparative
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
cargo test --test full_universe_stress burst_step_cell_reconciles_all_phases
cargo test --test full_universe_stress burst_impulse_cell_reconciles_all_phases
cargo test --test full_universe_stress stress_support::runner::tests::burst_profiles_partition_the_budget_exactly_and_deterministically

STRESS_MANIFEST_OUT=/tmp/stress-v6 \
  cargo test --release --locked --test full_universe_stress -- \
  --ignored reference_burst_step_manifest_is_reproducible --test-threads=1 --nocapture

STRESS_MANIFEST_OUT=/tmp/stress-v3 \
  cargo test --release --locked --test full_universe_stress -- \
  --ignored reference_exact_hot_quiet_manifest_is_reproducible --test-threads=1 --nocapture
```
