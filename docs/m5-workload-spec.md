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
| `hot_quiet` | **Implemented in this increment** | Exactly 5% of eligible instruments are designated hot by a derived PRNG stream; they receive 80% of published deltas. Remaining 95% retain equal weight over the residual 20%. |
| `burst` | Planned | Short deterministic spikes in publish rate. |
| `lifecycle` | Planned | Add/remove/settle universe churn through the mock's lifecycle channel. |
| `disconnect` | Planned | Deterministic replica/source closure and re-anchoring sweep. |
| `backpressure` | Existing failure fixture | Bounded mock/writer queues with explicit saturation classification. |
| `mixed` | Planned | Combined hot/quiet, lifecycle, disconnect and backpressure in one cell. |

## Frozen first workload

| Parameter | Value |
|---|---|
| Seed | `23` |
| Eligible instruments | 4,000 (`decoy_fraction_ppm = 20_000`) |
| Hot instruments | 200 (5% of eligible, `hot_fraction_ppm = 50_000`) |
| Hot event share | 80% (`hot_event_share_pct = 80`) |
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

Current state: generated, transmitted, applied, persisted-SBE and journaled are
recorded and reconciled by `EventLedger`. Received and decoded counters exist as
production metrics but are **not yet scraped into the ledger**; this is an
explicit M5 gap, not a claimed measurement. Injected drops, replica duplicates
and over-application are recorded separately and must never be silently folded
into `unexplained_drops` as a successful result.

## Acceptance gates for this increment

| Gate | Proof |
|---|---|
| Correctness | `EventLedger::reconciled()` is true; `unexplained_drops = 0`; `persistence_gap = 0` |
| Coverage | expected, acknowledged and anchored symbol sets are identical at 4,000 |
| Continuity | no decoy appears in the capture; journal finalizes completely |
| Determinism | hot-role unit test pins the 200/4,000 split and rejects a different seed |
| Evidence | `mixed_hot_quiet_4000_cell_reconciles_and_certifies` passes offline; `reference_mixed_hot_quiet_manifest_is_reproducible` emits a versioned manifest when run explicitly with `STRESS_MANIFEST_OUT` |

Performance gates (p50/p95/p99/p99.9, CPU, RSS, disk, queue wait) are **not**
claimed by this increment. They require the instrumentation work identified in
the stress-lab audit before any number can be called evidence.

## Reproduce

```bash
cargo test --test full_universe_stress mixed_hot_quiet_4000_cell_reconciles_and_certifies
cargo test --test full_universe_stress stress_support::universe::tests::hot_quiet_roles_are_exact_deterministic_and_free_of_decoys

STRESS_MANIFEST_OUT=measurements/stress/seed-0023-4000-hotquiet.json \
  cargo test --test full_universe_stress \
  reference_mixed_hot_quiet_manifest_is_reproducible -- --ignored --nocapture
```
