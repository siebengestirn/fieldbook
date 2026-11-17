# M5 workload programme: capacity characterization

This document defines the M5 capacity and workload programme. It was introduced
with the M5.7 capacity scaffold; the workload implementations described here are
now integrated into `main`. See [`PROGRAMME_STATUS.md`](PROGRAMME_STATUS.md)
for exact per-milestone status and evidence. It describes
what the harness is arranged to measure, not what has been measured. The
authoritative stress-lab design remains
[`stress-lab-design.md`](stress-lab-design.md); the frozen hot/quiet workload
and its v5 certification evidence remain in
[`m5-workload-spec.md`](m5-workload-spec.md). All cells described here are
**synthetic/loopback** evidence: the loopback Kalshi mock feeds the production
collector path (adapter → discovery → capacity planner → coordinator → books →
writer → journal), and the runner certifies the result fail-closed before any
manifest body is published.

> **Discovery success is not capacity certification.** A complete discovery
> round — including the 548,942-token full-universe result recorded in the
> live-evidence directory — proves only that the venue's metadata endpoints
> were paginated and filtered. It does not prove that the collector can hold
> 4,000 / 10,000 / 25,000 / 50,000 live books, sustain their delta stream,
> keep coverage exact, or persist every event. Only a certified cell of this
> programme makes a capacity claim, and even that claim is bounded to the
> selected instrument count, workload model, and observation window.

## Capacity progression

A stage fixes the eligible-instrument count. The existing deterministic
generator (`SyntheticUniverse::generate`) produces that many eligible markets
plus MVE/closed decoys; the existing scheduler (`publish_events`) assigns the
logical deltas. No new generator is introduced by this programme.

| Stage | Instruments | Default CI | Workload models | Manifest evidence |
|---|---|---|---|---|
| `4k` | 4,000 | Yes — `smoke_cell_anchors_applies_and_certifies`, `mixed_hot_quiet_4000_cell_reconciles_and_certifies`, and the 200-instrument selection test | all three | in-memory acceptance; no 50k artifact |
| `10k` | 10,000 | No — opt-in | all three | `STRESS_CAPACITY_STAGE=10k` |
| `25k` | 25,000 | No — opt-in | all three | `STRESS_CAPACITY_STAGE=25k` |
| `50k` | 50,000 | **Never in CI** — opt-in only | all three | `STRESS_CAPACITY_STAGE=50k` |

The selected stage plus model is translated into the ordinary stress-runner
configuration (`tests/stress_support/capacity.rs`): shards are derived by the
production capacity planner, replicas and the lifecycle plane are unchanged,
and the journal is enabled. Scenario schema `v16` records the stage, workload
model and expected event count in every manifest. Earlier v3-v15 manifests are
immutable historical evidence and are not regenerated.

## Workload models

| Model | Event budget | Scheduler configuration |
|---|---|---|
| `fixed_per_instrument` | `instruments × 1` | Uniform spread; total offered load scales linearly with the stage. |
| `fixed_aggregate` | 400 | Uniform spread; total offered load is independent of the stage. |
| `skewed_hot_quiet` | 400 | Existing exact-quota hot/quiet assignment: 5% of instruments designated hot (`hot_fraction_ppm = 50_000`), 80% of deltas routed to them (`hot_event_share_pct = 80`). At 4,000 instruments this pins 200 hot instruments and 320/80 hot/cold events. |

The per-instrument model is a characterization model: at 50k it offers 50,000
deltas and can saturate the loopback mock's bounded outbound queue, in which
case the harness records `mock_throughput_limited` and refuses to certify. The
opt-in short correctness cells therefore default to `skewed_hot_quiet`, whose
bounded 400-event budget keeps every stage a correctness check rather than a
throughput test. `STRESS_WORKLOAD_MODEL` selects `fixed_per_instrument`,
`fixed_aggregate` or `skewed_hot_quiet` explicitly.

## Measurement requirements

Every capacity cell must satisfy the same fail-closed gates as the 4k smoke
cell, computed inside `run_cell` before a success manifest can exist:

1. **Anchoring and coverage.** Every eligible instrument is snapshot-anchored;
   expected, acknowledged and anchored symbol sets are byte-identical (canonical
   SHA-256); MVE/closed decoys never appear in the capture.
2. **Delivery barrier.** A quiescent writer baseline, then a metrics-driven
   drain over production counters: every published frame leaves the mock, is
   socket-written, reaches the collector, is decoded and is accounted for by
   the core and writer before shutdown. No fixed sleep certifies delivery.
3. **Two-sided event ledger.** `generated = transmitted = received = decoded =
   applied = written = SBE persisted = journaled`, with both shortfall and
   excess computed at each boundary. Missing evidence stays `unavailable`,
   never zero, and fails certification.
4. **Verification.** SBE `verify`, completeness, and journal finalization must
   all pass. The manifest records `verify_valid`, `completeness_valid`,
   `coverage_valid`, `journal_complete` and the journal record count.
5. **Resources.** The manifest records harness RSS, CPU count, RAM, file
   descriptor limit and free disk where the host can supply them. Anything
   unmeasurable (notably CPU seconds, which needs no libc dependency here) is
   recorded through the `unavailable(reason)` object rather than as `0`.
6. **Manifest provenance.** Schema version, scenario version, generator/PRNG
   versions, binary hashes, worktree provenance, host details and an explicit
   acceptance verdict are published index-first with a body hash, so an
   interrupted publication is recoverable and a corrupt index blocks the cell.

No latency percentile, CPU-seconds, queue-wait or per-stage throughput number
is claimed by this programme: the corresponding manifest fields remain
`unavailable` with an explicit reason until real instrumentation exists. A
successful cell establishes correctness at a stage, not a sustainable rate.

## Deterministic gates in default CI

- `tests/full_universe_stress.rs::capacity_stage_and_workload_model_selection_is_deterministic`
  pins all four stages and all three model selections, including event budgets,
  shard derivation, hot/quiet knobs and seed independence.
- `tests/full_universe_stress.rs::capacity_manifest_records_stage_model_and_unavailable_reasons`
  pins the `capacity` manifest object, including the explicit `unavailable`
  reason for unmeasured CPU time, without running a long stage cell.
- `tests/full_universe_stress.rs::capacity_workload_models_reconcile_at_small_scale`
  executes all three models at 200 instruments through the real runner and
  requires exact 200-symbol coverage, exact event counts and a reconciled
  ledger.
- `smoke_cell_anchors_applies_and_certifies` remains the non-ignored 4k
  correctness gate. The 10k, 25k and 50k stage tests are `#[ignore]`d and are
  never invoked by `cargo test --all-targets`.

## Reproduce an opt-in stage cell

```bash
# Example: 10k, skewed hot/quiet, published manifest. 25k/50k are identical
# with STRESS_CAPACITY_STAGE=25k / 50k; 50k is never run in CI.
STRESS_CAPACITY_STAGE=10k \
STRESS_WORKLOAD_MODEL=skewed_hot_quiet \
STRESS_MANIFEST_OUT=/tmp/m5-capacity-10k \
  cargo test --release --locked --test full_universe_stress -- \
  --ignored capacity_stage_10k_short_correctness --test-threads=1 --nocapture
```

`STRESS_CAPACITY_STAGE` must name the test's stage, and `STRESS_MANIFEST_OUT`
must point at a manifest directory. `STRESS_MANIFEST_OUT` is never checked into
`measurements/stress`: stage artifacts are produced on demand, not silently
overwriting prior evidence. Each published body includes a `capacity` object
recording stage, instrument count, workload model, expected events, coverage,
verification, resources and explicit unavailable reasons.

## Limits and non-goals

- This scaffold does not itself implement burst, lifecycle churn, disconnect
  sweeps, backpressure characterization or stage-timing instrumentation; those
  are separate M5.2-M5.6 work streams carried in the same integrated v16 schema.
  Existing failure fixtures for bounded queues and shutdown remain
  failure-injection tests, not capacity results.
- The M5 capacity scaffold itself introduces no new venue and no framework
  rewrite: its cells reuse the Kalshi-shaped loopback mock, the synthetic
  generator and the existing scheduling/certification path. A separate H3
  integration below adds a Hyperliquid loopback mock behind the same ledger,
  coverage-hash, manifest and scenario-version seams; it does not change this
  scaffold or add a framework.
- A short correctness cell does not establish sustained throughput, a maximum
  sustainable rate, or live-venue behavior. Discovery pagination results are
  explicitly not capacity evidence.

## Hyperliquid H3 capacity cells (integrated at `scenario_version = 16`)

The H3 increment adds a venue-specific synthetic cell that reuses the same
fail-closed gates rather than a new harness. `tests/stress_support/hyperliquid.rs`
serves REST `meta` discovery and the `allMids`, `fastAssetCtxs`, `l2Book`,
`trades` websocket channels plus a scripted reconnect from one seed-derived
explicit schedule. Three declared workload profiles change only frame counts:
`balanced`, `telemetry_heavy` (mid fan-out and fast-context rate) and
`book_refresh_burst` (`l2Book` refresh cadence and trade bursts).

`tests/hyperliquid_capacity.rs` runs the exact ten selected perpetuals through
the real adapter, worker, journal and `CollectorCore` and requires exact
three-set coverage (expected == acknowledged == anchored), a reconciled
`EventLedger` and terminal dispositions, a bounded writer queue, journal/SBE
verification, zero `LevelChange`, and a fresh book re-anchored under reconnect.
The ignored `reference_hyperliquid_manifest_is_versioned` generator publishes a
v16 manifest under `STRESS_MANIFEST_OUT` using the shared manifest schema; cell
ids are content-addressed by version, profile and seed so existing evidence is
never overwritten.

**These are synthetic loopback cells. They are not live certification, and they
make no latency, throughput or capacity performance claim.** They establish
correctness at ten instruments only; no universe growth, live tier, order entry
or production capacity is implied. See
[`HYPERLIQUID_INTEGRATION_PLAN.md`](HYPERLIQUID_INTEGRATION_PLAN.md).
