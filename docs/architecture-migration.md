# Market-data architecture migration

**Status:** Adopted in part (M1.5 active on `feat/feed-continuity-contracts`)  
**Base:** `6610edc` (`main`; PR #42 merged as `abd7971`, PR #44 as `6610edc`)  
**Origin:** carried from `architecture/market-data-contracts` (`dda6f81`); that branch is retired after this reconciliation.  
**Scope:** Architecture contracts and migration sequence. No production code or venue adapter changes are authorized by this document.

## Reconciliation with the failed full-universe run (2026-11-14)

The preserved 60-minute live run (`measurements/live/2026-11-14-universe-capture-failed/`)
reframes this migration's first priority. The collector persisted 28.2M SBE
messages with zero writer errors and a finalized journal, and the SBE
verification gate refused the capture: 12.34% of snapshots fail delta
reconstruction after 756 websocket connection errors drove 10,051,317
failover snapshots. Final anchored coverage was 152,441 of 385,604 expected.

What this proves: a completed process is not a correct dataset. What it
cannot prove: whether the resets originated at the venue or the host (the
live path sampled no RSS or memory-pressure telemetry).

The migration order below is therefore amended: **M1.5 (connection health,
subscription state, and recovery correctness) precedes all capacity and
venue-expansion work.** The regression pin exposed an unsafe coupling between instrument quietness
and source-level recovery: an exhausted per-symbol probe budget could upgrade
a pending recovery to a whole-source reconnect, invalidating unrelated books.
The current patch removes that escalation from the stale-probe path. It records
a symbol continuity incident and reissues the symbol-scoped refresh while
source transport and heartbeat state remain independently supervised. The
live capture still does not prove that this was the cause of the historical
connection errors; that requires host telemetry and a staged live experiment.
This is a narrow branch-level correction, not the final recovery architecture:
`PendingRecovery` is still keyed by source and merges the `reconnect` flag, so
independent instrument/source queues and per-instrument generation fences
remain required before claiming complete scope isolation (see ADR-0002 and
M3.1–M3.3). On Polymarket a quiet instrument is expected — the venue does not
promise a periodic per-token event. Connection health, instrument activity,
and book continuity remain distinct facts.

## Objective

Build a venue-neutral market-data and research platform whose data, event, recovery, persistence, and experiment claims can be reproduced from retained evidence.

Initial venue workstreams have equal engineering standing and independent certification:

- Kalshi: event-contract markets.
- Polymarket: outcome-token markets.
- Hyperliquid: perpetual and spot instruments.

A venue's access or certification status must not block another venue's development or evidence.

## Current baseline and explicit non-claims

The repository is a read-only market-data collector. It does not submit orders, hold positions, or implement strategy or pre-trade risk.

The accepted ADR 0001 defines a persistent instrument identity independent of ticker, listing, venue, feed, connection, and serialization symbol. Its scope explicitly says documentation/design only; the Rust model has not implemented the migration.

Current code still has:
- `MarketInfo` combining venue, native symbol, output symbol, grouping and instrument metadata.
- Collector books keyed by venue-native symbol.
- `MarketDataSession::apply_subscriptions(&BTreeSet<String>)`, which represents symbols rather than data products.
- `ExchangeCapabilities::has_sequence_clock`, which does not describe update model, sequence scope, recovery, subscription topology, limits, or event products.
- `ExchangeEvent` variants for snapshots, level changes, lifecycle and stream reset, but no normalized Trade, BBO, or asset-context event.

PR #42's synthetic/loopback evidence is not live-venue evidence, capacity evidence, or a production readiness certificate. The checked-in 4,000-symbol fixture is a correctness smoke, not proof of 548,942-token live collection or a 24-hour soak.

The verification record documents a Polymarket discovery run of 548,942 unique tokens across 2,746 pages in 9m52s, a 10-token/90-second live capture, and a 100,000-symbol synthetic scale run with 696,448 KB RSS. These are separate evidence classes and must remain separate. They do not prove full-universe live collection.

## Five engineering principles

1. **Correctness:** each state transition has explicit preconditions, postconditions, invariants and failure outcomes.
2. **Reproducibility:** each experiment can be reconstructed from immutable configuration, code identity, data fingerprints, seeds, toolchain and retained inputs.
3. **Determinism:** identical normalized inputs and configuration produce identical outputs, modulo explicitly excluded environmental metadata.
4. **Observability:** a failure is diagnosable from retained evidence; unobservable stages fail closed rather than being assigned zero.
5. **Risk containment:** research cannot submit orders; execution cannot bypass independent risk admission and kill controls.

Every PR must state the problem, design choice, invariants, failure modes, tests, reproducible commands, evidence class, limitations and migration impact.

## Architecture boundaries

```
Venue adapters (Kalshi | Polymarket | Hyperliquid)
                |
      Discovery + reference data
                |
       Instrument identity
                |
 Typed data-product subscriptions
                |
 Raw-frame journal (source evidence)
                |
 Versioned normalized market events
        /       |       |       \
      Books   Trades   BBO   Asset context
        \       |       |       /
          Replay + normalized store
                    |
             Research datasets
                    |
             Hypothesis registry
                    |
            Feature/experiment lab
                    |
            Statistical validation
                    |
             Strategy registry
                    |
              Event backtest
                    |
           Approved strategy artifact
                    |
         Independent risk engine
                    |
         Broker-neutral OMS
                    |
             Venue adapters
```

Keep one venue collector per process for now. Normalize at durable boundaries; do not force all venue protocols into one state machine. Keep legacy challenge SBE as a projection, not as the canonical representation of every market event.

Research and execution must have separate processes, permissions, credentials, and APIs. Research artifacts cannot contain live-order capabilities. Execution accepts only signed/versioned strategy artifacts that pass policy and independent risk admission.

## Contract A — instrument and reference identity

Implement only the identity boundaries required for live market data first; do not attempt the entire reference-data ontology in one PR.

Required concepts:
- `InstrumentId`: platform-assigned persistent identity.
- `VenueId` and `VenueOperatorId`: distinct identities.
- `ListingId`: time-bounded admission of an instrument to a venue.
- `FeedId` and `DataProviderId`: source identities.
- `VenueInstrumentMapping`: venue-native identifier scoped by venue/feed/protocol and effective time.
- `FeedInstrumentMapping`: provider/feed identifier that need not imply a public listing.
- Parent relationships such as Kalshi event, Polymarket condition, and Hyperliquid DEX are metadata/relationships, never substitutes for instrument identity.
- `SerializationSymbol`: compatibility projection only.

Identity must not be inferred from display ticker. Identifier resolution must be namespaced, scoped, effective-dated and provenance-bearing. Ambiguous resolution is an explicit unresolved state.

Acceptance tests:
- Same ticker on two venues produces distinct mappings.
- Symbol reuse across time preserves two effective intervals.
- Kalshi event grouping does not collapse distinct market instruments.
- Polymarket condition does not collapse distinct outcome tokens.
- Hyperliquid spot pair, perpetual, DEX-scoped instrument and builder-deployed instrument remain distinct.
- Serialization symbol changes do not mutate canonical instrument identity.
- Unknown/ambiguous mappings fail closed and preserve raw evidence.

## Contract B — typed data-product subscriptions

Replace symbol-only subscription semantics with a typed set of data products. Initial conceptual variants:

- OrderBookL2(InstrumentId)
- Trades(InstrumentId)
- BestBidOffer(InstrumentId)
- AssetContext(InstrumentId)
- AllMids(DexId or venue scope)
- AllAssetContexts(venue scope)
- VenueLifecycle(scope)

A subscription has a stable key and explicit lifecycle:
`Desired -> Sent -> Acknowledged -> Active -> Removing -> Removed`, with `Failed` and `UnknownAfterDisconnect` outcomes.

Acknowledgement means the venue/session confirmed the request according to that protocol. Local write success is not venue acknowledgement. Reconnect creates a new connection epoch; prior active state cannot silently carry over.

Capability metadata must describe:
- Product types and update model (snapshot, delta, partial update).
- Sequence or ordering domain and reset behavior.
- Recovery procedure and whether an authoritative snapshot exists.
- Per-connection, per-IP and shared-egress budgets.
- Subscription topology and rate limits.
- Precision and schema version.
- Market status/lifecycle coverage.

Do not represent a venue-wide/global feed as one subscription per instrument. Hyperliquid's documented per-IP limits must be enforced by a shared egress budget, not per-process estimates.

## Contract C — normalized event semantics

Keep event types semantically distinct:
- BookSnapshot
- BookDelta
- Trade
- BestBidOffer
- AssetContext
- InstrumentStatus/Lifecycle
- StreamReset/Gap

Every event carries:
- Canonical InstrumentId where resolvable.
- Venue, provider, feed and source identity.
- Connection epoch and sequence domain/value where available.
- Exchange timestamp with source precision and semantics.
- Local receive timestamp from a monotonic clock plus wall-clock timestamp where needed.
- Raw-frame reference and decoder/schema version.
- Event-specific exact numeric fields.

Trade is not a book delta. BBO is not an L2 book. allMids is not an executable quote or L2 snapshot. An L2 full snapshot must enter as a snapshot; never disguise it as a delta. Partial asset-context updates must merge by instrument; they must not replace the whole state map.

Sequence numbers are meaningful only within their documented scope. Trade IDs must use the venue-defined composite identity rather than assuming one field is globally unique.

## Contract D — numeric and persistence semantics

- Preserve raw venue frames before normalization.
- Persist versioned normalized events with raw-frame references.
- Retain exact decimal coefficients and scales; never round during adapter normalization.
- Reject an inexact projection into legacy SBE.
- Keep SBE challenge output as a versioned compatibility projection.
- A failed projection must not discard the raw frame or normalized event.
- Journal offsets, hashes, source identity and event identity must support deterministic replay.
- Schema evolution must be explicit and backward-compatible or version-bumped.

The journal is evidence of what the collector received, not proof that the venue delivered every event. Venue completeness needs independent universe/reconciliation and feed-specific continuity evidence.

## Contract E — replay and experiment identity

An experiment manifest must identify:
- Immutable configuration bytes and SHA-256.
- Source commit and dirty-tree state.
- Binary hashes and build profile/toolchain.
- Dataset version, partition list and cryptographic fingerprints.
- Venue universe snapshot and effective-time policy.
- Random seeds and RNG algorithm version.
- Feature/strategy artifact hashes.
- Cost model, execution model and parameter search space.
- Train/validation/test time boundaries and embargo policy.
- All attempted hypotheses/configurations, including failures.
- Output artifacts and verification results.

Identical inputs/configuration must yield deterministic event ordering and result hashes where environmental measurements are excluded explicitly.

Manifest publication must be transactional: unique temp files, flush/sync where required, atomic rename, directory durability where supported, and concurrency protection for shared indexes. Index updates must not lose concurrent cells. A failure between manifest and index publication must leave a recoverable state that cannot be misread as a complete success.

## Contract F — evidence classes

Every result must declare exactly one evidence class:

1. `unit-fixture`: deterministic decoder/state tests.
2. `synthetic-loopback`: production code path against a synthetic venue.
3. `historical-replay`: recorded historical inputs with provenance.
4. `live-smoke`: selected live instruments and bounded duration.
5. `live-universe`: complete discovered eligible set anchored for a declared product.
6. `live-soak`: continuous operation for a declared duration with loss/recovery accounting.

Evidence classes are not interchangeable. A green synthetic test cannot imply a live result. A live-universe result is product-specific: Hyperliquid allMids coverage does not imply full L2 coverage.

Each venue and data product has its own certification manifest. One venue must never block another venue's evidence.

## Contract G — failure and recovery

All data products define:
- Connection/session state.
- Subscription state.
- Sequence/order domain.
- Duplicate policy.
- Gap detection capability and limits.
- Snapshot authority and re-anchoring rules.
- Queue/backpressure policy.
- Journal failure policy.
- Shutdown and restart behavior.

Known gaps are measured and classified. Unknown gaps are marked unknown; never fabricate exact loss counts. A recovered book is not considered continuous until its protocol-defined recovery fence and authoritative snapshot have passed.

## Contract H — statistical and economic validation

The future research lab must require:
- Hypothesis and falsification criterion before parameter search.
- Point-in-time universe membership and delisted/expired instruments.
- Corporate actions where applicable.
- No look-ahead or survivorship leakage.
- Chronological train/validation/test with purging/embargo where leakage is possible.
- Multiple-testing accounting and experiment registry.
- Selection-bias diagnostics, including Probability of Backtest Overfitting and Deflated Sharpe Ratio where assumptions/data support them.
- Walk-forward and out-of-sample evaluation.
- Brokerage, exchange fees, taxes, spread, slippage, market impact, funding/borrow and liquidity constraints appropriate to venue/instrument.
- Capacity and participation limits.
- Cost sensitivity and adverse scenarios.
- Rejection as a first-class outcome.

A backtest cannot be accepted without data provenance, execution assumptions, cost schedule and complete experiment history.

## Migration plan

### M1.5 — Feed continuity and recovery contracts (ACTIVE — this branch)

The failed full-universe run is the driver; see the reconciliation above.
`feat/feed-continuity-contracts` implements this phase as a correctness
project, not a reconnection feature. Three state domains, kept independent:

1. **Connection health** (per source): `Connecting / Healthy / Suspect /
   Reconnecting / Backoff / Stopped`, owned by the connection worker, driven
   by transport state, heartbeat acknowledgements and protocol errors —
   never by market-data timestamps.
2. **Instrument activity** (per instrument): `Unobserved / Active / Quiet`,
   derived from the last relevant market-data event; may trigger a
   targeted refresh under policy; must never control connection health.
3. **Book continuity** (per instrument+source): the existing
   `ContinuityState` (`Unseeded / Live / Stale / Resyncing / Terminal`),
   with precise meanings and recovery-fence enforcement.

Central invariant: *an instrument's lack of updates must not be interpreted
as evidence that its connection is unhealthy; a connection's health must
not be interpreted as proof that every instrument's book is continuous.*

Recovery requests become typed (`action`, `cause`, `generation`,
`instruments`, `deadline`, `attempt`); instrument-scoped refresh and
source-scoped reconnect are distinct in code, logs, metrics and manifests;
escalation from refresh to reconnect is an explicit policy decision, and
scheduled refresh is recorded separately from transport failure.

#### M3.1 — Typed recovery decisions (implemented)

`src/recovery.rs` owns the pure M3.1 policy. `RecoverySignals` is a snapshot
of observed conditions; `RecoveryCause` records why a decision was made;
`RecoveryAction` is the only authorized outcome (`RefreshInstruments` or
`ReconnectSource`); `RecoveryDecision` keeps every contributing cause, the
authorized action, and an explicit budget-exhaustion outcome. The decision
function is deterministic and has no I/O.

The policy rules are:

- `InstrumentQuiet`, `SnapshotTimeout`, and `ScheduledRefresh` authorize an
  in-place instrument refresh only — a quiet instrument can never escalate
  its source to a reconnect.
- `TransportClosed`, `HeartbeatFailure`, `DecodeFailure`,
  `ProtocolViolation`, and `OperatorRequested` authorize a source reconnect.
  A heartbeat failure is ignored when the venue heartbeat mechanism is
  `Disabled`, so a venue without heartbeats cannot produce a false failure.
- When both classes are present the stronger action wins, but all causes stay
  on the decision for audit.
- `attempts >= max_attempts` adds `RecoveryExhausted` and withholds the action
  instead of escalating without bound; repeated identical causes keep the
  same action and budget.

This module does not yet own the recovery queues or acknowledgement fences:
those remain M3.2 and M3.3. The transitional `PendingRecovery` structure in
`src/collector/state.rs` is still source-keyed and still merges a `reconnect`
boolean; the protected quiet-instrument test
`src/collector/state/tests.rs::quiet_instrument_on_healthy_source_never_escalates_to_source_reconnect`
remains the guard until that migration lands.

#### M3.2 — Independent recovery queues (implemented)

`RecoveryCoordinator` owns two disjoint, bounded queues.
`InstrumentRecoveryQueue` is keyed by `(SourceId, instrument, generation)` and
holds only `RefreshInstruments` requests; `SourceRecoveryQueue` is keyed by
`(SourceId, generation)` and holds only `ReconnectSource` requests. Insertion
validates the action against the owning queue, so a source reconnect cannot be
routed through the instrument queue. Repeated causes coalesce onto the exact
same key, preserving instrument identity and generation; a different
instrument never merges into another's request. Each `RecoveryRequest` carries
its action, sorted cause set, attempt count, attempt bound, optional deadline,
and terminal outcome (`Acknowledged` with epoch and receive fence, `Cancelled`,
or `Failed`). Due work is emitted in deterministic key order, and completed
requests are reaped before generations are reused.

This coordinator is not yet wired into `CollectorCore` or the worker command
path; that integration, together with per-instrument acknowledgement fences,
is M3.3. The 3,000-instrument test
`src/recovery.rs::three_thousand_quiet_instruments_fill_only_the_instrument_queue`
proves that 3,000 quiet instruments produce 3,000 instrument requests and zero
source requests.

#### M3.3 — Per-instrument acknowledgement fences (implemented)

`RecoveryFence` binds a `(connection_epoch, receive_seq)` fence to an exact
`RecoveryTarget` of `(source, instrument, generation)`. `WorkerEvent::RecoveryReady`
now enumerates the exact instruments an acknowledgement covers, and
`CollectorCore::handle_recovery_ready` stamps a fence only on those named
instruments whose outstanding request has a matching generation and whose
connection epoch is not stale. An unknown target, an unknown or stale
generation, or an older epoch fences nothing. A batch acknowledgement clears
the pending entry only when every named target was fenced; a partial batch
leaves the remaining instruments independently recoverable. Each stored fence
carries its target, so a later snapshot must beat both the fence and the
request generation before it can re-anchor that book.

Regression pins:
`src/collector/state/tests.rs::recovery_acknowledgement_fences_only_the_named_instrument`,
`src/collector/state/tests.rs::stale_generation_epoch_and_unknown_target_acks_are_rejected`,
`src/collector/state/tests.rs::batch_acknowledgement_enumerates_every_target`, and
`src/recovery.rs::a_fence_is_bound_to_its_exact_instrument_and_generation`.

#### M4 — Blast-radius evidence (implemented)

Deterministic core tests now prove the consequences of the M3 contracts on
emitted output, not only on state flags:

- `src/collector/state/tests.rs::blast_radius_source_loss_invalidates_all_books_then_reanchors_3000_instruments`
  — one source owns 3,000 instruments; its disconnect invalidates every book,
  only fenced snapshots re-anchor them, and all 3,000 emitted SBE snapshots
  carry their original receive timestamps.
- `src/collector/state/tests.rs::blast_radius_independent_shards_do_not_share_a_failure`
  — a failure on shard 0 leaves shard 1 valid and still producing deltas,
  with no recovery control sent to the healthy shard.
- `src/collector/state/tests.rs::blast_radius_backup_requires_a_fresh_snapshot_before_promotion`
  — on a clockless venue a pre-fence backup snapshot cannot become active;
  only a snapshot newer than the per-instrument acknowledgement fence does.

Stale-epoch data, stale-generation acknowledgements, and old-fence snapshots
are covered by the M3.3 regression pins above and by
`tests/session_recovery.rs::reconnect_recovery_advances_epoch_and_resets_sequence`.

Acceptance gates: A (state correctness), B (recovery scope), C (ordering —
epoch/receive-seq/fence), D (accounting in manifests), E (mixed
hot/quiet synthetic workload with injected failures), F (review package:
problem, model, policy, invariants, failure matrix, commands, evidence,
limitations). No full-universe live run is part of this phase's
acceptance; staged live experiments (1k → 5k → 10k → 50k → 60-minute soak
→ full universe) follow only after all gates pass.

### M0 — Baseline and evidence freeze
- Preserve PR #42 draft and its existing CI evidence.
- Label current stress manifests as synthetic/loopback and identify their source commit.
- Do not launch the full matrix until the manifest, ledger and runner contracts are audited.
- Record exact baseline commit and tests on a clean worktree.

Exit: baseline reproducible; evidence classes explicit; no claim of live full-universe coverage.

### M1 — Correctness contracts and PR #42 disposition
- Review the stress runner ledger against generated -> mock enqueue -> socket write -> collector receive -> decode -> applied -> writer accepted -> SBE persisted -> journaled stages.
- Use typed metric parsing: missing required metrics are unavailable, never zero.
- Verify all required stage counts and discrepancy categories before success.
- Make manifest/index publication concurrency-safe and recoverable.
- Bind manifests to the exact source commit and binary that generated them.
- Validate config and output paths before starting sockets.
- Add multi-process concurrency/crash-injection tests.

Exit: every success manifest is mechanically derivable from independent stage evidence; no race loses cells; artifacts cannot claim success with incomplete provenance.

### M2 — Instrument identity minimum viable migration
- Add newtypes and reference records alongside old structures.
- Add explicit mapping from current adapter `MarketInfo` into the new catalog.
- Migrate collector maps to InstrumentId plus source/feed scoping.
- Preserve output symbol compatibility through a projection.
- Retain old APIs only as a bounded migration seam; do not spread dual identity further.

Exit: Kalshi and Polymarket fixtures prove identity and grouping semantics; existing replay/SBE fixtures remain stable.

### M3 — Subscription and feed capability contracts
- Add typed data-product subscriptions and lifecycle.
- Add feed capability declarations for snapshot/delta, ordering, recovery, topology and limits.
- Keep one process per venue.
- Add fake-session tests for reconnect and uncertain subscription state.

Exit: no symbol-only subscription API remains on the core adapter boundary; product-specific evidence possible.

### M4 — Event model and normalized persistence
- Add typed Trade, BBO, AssetContext and Status events without changing existing semantics.
- Add versioned normalized journal alongside raw frames.
- Add replay equality tests from raw to normalized to core state.
- Preserve SBE as compatibility projection.

Exit: exact numeric round-trip; replay output hashes match; unsupported projections fail without data loss.

### M5 — Hyperliquid adapter, staged
- H0 reference data: perp metadata, DEX scoping and spot metadata stored separately; precision/specifications pinned.
- H1 allMids + fastAssetCtxs; partial updates merge correctly; global-feed evidence separate.
- H2 10–20 selected perps for l2Book snapshots and trades; measure feed rates, latency, precision, heartbeats, queue pressure and disconnect recovery.
- H3 expand products/instruments only after shared per-IP budget admission and measured capacity.
- Do not claim all-L2 coverage from allMids or asset contexts.

Exit: decoder fixtures, protocol tests, live-smoke manifest, reconnect evidence, exact precision round-trip and separate product coverage.

### M6 — Venue-specific certification and scale
- Polymarket discovery fingerprint and point-in-time eligibility definition.
- Hyperliquid universe and product-specific coverage.
- Kalshi authenticated discovery and coverage when permission exists.
- Synthetic capacity sweep first: 10k, 50k, 100k, 175k, 320k, 550k+ with measured resources.
- Validate shard count, connections, queue memory, RSS peak, CPU, descriptors, disk, subscriptions, snapshot rates and recovery.
- Then run live-universe on independently available venues.
- Test 24-hour soak only after bounded live-universe operation.

Exit: separate signed certification records for each venue/product. No forced dual-venue all-or-nothing result.

### M7 — Research platform
- Immutable historical datasets and point-in-time universe snapshots.
- Feature computation with lineage.
- Hypothesis registry and experiment tracking.
- Statistical validation gates.
- Event-driven backtest with venue-specific execution and fees.
- Accepted/rejected strategy artifacts.

Exit: independently reproducible OOS result with all attempted experiments and costs recorded.

### M8 — Execution boundary
- Separate execution process and deployment identity.
- Broker-neutral OMS.
- Independent risk engine with hard limits and kill switch.
- Venue adapters with idempotent client order IDs and reconciliation.
- Paper/shadow operations before any restricted live canary.

Exit: risk admission cannot be bypassed by research or strategy code; kill control independently verified.

## PR #42 disposition

Keep #42 draft. It is useful as an initial synthetic stress correctness increment and has successful CI, but it is not a complete capacity or certification system.

Before phase matrix execution, resolve the M1 findings:
- Event accounting does not yet evidence every stage named in the design.
- Required metrics must be typed and absence must fail closed.
- The manifest and shared index need concurrency and crash consistency tests.
- Current checked-in reference manifests identify source commit `5c9e45a`, not PR head `c0eaea2`; regenerate evidence only after the final runner code is frozen.
- The malformed-index fixture correctly asserts no artifact is written, but failure evidence/recovery semantics must be specified.
- Per-cell configuration and output path validation must be explicit.
- Writer queue accounting, mock socket completion and journal counts need separate semantics.

No merge, phase matrix, live access request, large-scale run, or production claim is authorized by this migration document.

## Definition of done

A feature is done only when:
1. Its invariants are written as executable tests.
2. Failure modes produce explicit states and retained evidence.
3. Exact inputs, version and provenance are recorded.
4. Replay reproduces the result.
5. The correct evidence class supports the claim.
6. Independent reviewers can reproduce the verification commands.
7. Known limits are stated.
8. The feature does not cross research/execution or venue certification boundaries.

The platform succeeds by rejecting unsupported claims and invalid hypotheses, not by maximizing model count, symbol count, or benchmark headlines.
