# Kraken Integration Plan

- **Status:** H0 reference data implemented; H1-H3 remain planned. This document defines the staging contract for the next exchange after Hyperliquid.
- **Branch:** `feat/kraken-reference-data`.
- **Scope:** add Kraken behind the existing adapter seams on the normalized collector, one evidence-gated stage at a time.
- **Reading rule:** every statement below is either **Current** (verifiable in this tree) or **Planned** (a decision to implement later). H0 is claimed as implemented from offline fixtures only. No Kraken live capture, coverage, latency, throughput or capacity result is claimed anywhere in this document.

The collector today ingests Kalshi and Polymarket prediction-market books and Hyperliquid perpetual/spot reference data with H1 telemetry, H2 selected books and H3 synthetic capacity cells. Kraken is another asset class (spot crypto) with a different reference-data shape: a single `AssetPairs` document carrying legacy asset codes, websocket names, decimal-string tick sizes and lot decimals, plus a pair status field. The plan stages that difference so each increment is independently falsifiable without changing the normalized core.

## 1. Existing adapter seams (Current)

These are the only sanctioned extension points; H0 uses a subset of them.

| Seam | Exact location | Contract |
|---|---|---|
| `Exchange` trait | `src/exchange/mod.rs` | `name()`, `environment()`, `capabilities()`, `discover_markets(max)`, `discover_markets_with_stats(max)`, `session_factory(self: Arc<Self>)`; optional `lifecycle_session_factory()` defaults to `None`. |
| `ExchangeCapabilities` | `src/exchange/mod.rs` | One field `has_sequence_clock: bool`; `capabilities_for(ExchangeKind)` maps Kalshi/Polymarket/Hyperliquid/Kraken to `false`. |
| `DiscoveryResult` / `DiscoveryStats` | `src/exchange/mod.rs` | `markets: Vec<MarketInfo>` plus page/selected/outcome/malformed/retry/duration timestamps. |
| `MarketDataSession` / `SessionFactory` | `src/session.rs` | `heartbeat_spec()`, `connect()`, `apply_subscriptions`, `next_frame()`, `send_keepalive()`, `request_snapshot()`, `close()`; `SessionFactory::create_session(SourceId)`. |
| `HeartbeatSpec` / `HeartbeatRequest` | `src/session.rs` | Presets `ten_second_application_text()`, `ten_second_websocket_control()`, `disabled()`. |
| Normalized events | `src/model.rs` | `ExchangeEvent::{Snapshot,LevelChange,Lifecycle,StreamReset}`, `MarketInfo`, `NumericSpec`, `Price`/`Quantity`, `EventMeta`, `Control`, `OutputMessage`, `SourceId`. |
| Numeric pipeline | `src/fixed.rs`, `src/util/numeric.rs` | `SCALE = 1_000_000`, `DECIMALS = 6`, `MAX_SCALE = 18`; exact rescale or reject inexact values. |
| Composition | `src/coordinator.rs`, `src/config.rs` | `build_exchange(settings, require_auth)` matches every `ExchangeKind`; `ExchangeKind`, `FileConfig`/`Settings`, `COLLECTOR_EXCHANGE` env, per-venue settings. |
| Capacity / probe | `src/capacity.rs`, `src/probe.rs` | Kraken charges no lifecycle connections; probe default target is the configured `asset_pairs_url`. |
| Adapter layout | `src/exchange/kraken/{mod,codec,discovery,session}.rs` | `codec` decodes reference data, `discovery` builds `MarketInfo`, `session` is the H0 no-network seam. |

Dependency direction is fixed: adapters depend on the normalized model and protocol vocabulary, `CollectorCore` depends on capabilities and normalized events, and the coordinator owns composition. A new venue must be added behind these seams, not by branching the state machine.

## 2. Staged deliverables, gates, and non-goals

The staged gates mirror the Hyperliquid plan exactly (see
[`docs/HYPERLIQUID_INTEGRATION_PLAN.md`](HYPERLIQUID_INTEGRATION_PLAN.md)); only the venue-specific wire details differ.

| Stage | Planned deliverable | Evidence gate (all required to advance) | Stage non-goals |
|---|---|---|---|
| **H0 — reference data — IMPLEMENTED** | `KrakenSettings` and `ExchangeKind::Kraken` in `config.rs`; env selector, `build_exchange`/`capabilities_for` arms, capacity/probe handling; `src/exchange/kraken/{mod,codec,discovery,session}.rs`; offline-fixture and read-only REST discovery of `AssetPairs`; golden fixture. | `parse_asset_pairs` produces a deterministic catalog; every `MarketInfo` passes `MarketInfo::validate()` and `NumericSpec::validate()`; namespaced identity `kraken:spot:BASE/QUOTE` is unique; `tick_size` and `lot_decimals` are converted exactly or rejected, never rounded; only `status == "online"` pairs are emitted; record counts reconcile with `DiscoveryStats`; no socket is opened. | No subscriptions, no book events, no telemetry, no live claim, no ADR-0001 reference-model migration, no spot book. |
| **H1 — low-complexity feeds — PLANNED** | Subscribe to venue-wide statistics/ticker channels classified as telemetry only; journal typed observation evidence with exact raw bytes; heartbeat policy `Disabled` unless a checked-in fixture proves a venue heartbeat. | Golden fixtures for ticker/statistics payloads with unknown additive fields and malformed numeric values; codec table tests; journal v2 round-trip; **zero** new `Snapshot`/`LevelChange` events and **zero** new coverage anchors proved by a dedicated integration test; numeric strings parsed exactly or rejected. | No claim of book coverage, no depth derivation, no synthetic books, no latency/throughput claim. |
| **H2 — selected books — PLANNED** | A configurable finite list of selected spot pairs (never the full universe); full snapshots decoded to `ExchangeEvent::Snapshot` with exact receive time and advisory feed timestamp; exact subscription reconciliation; recovery via `Control::{RequestSnapshot,Recover}` fences. | Per-symbol `coverage` certificate for the exact selected set (`expected == acknowledged == anchored`); `completeness` valid (no unanchored deltas); `verify` and `journal verify` pass; one reconnect drill re-anchors with a fresh snapshot in a new `connection_epoch`; zero `LevelChange`. | Full universe, cross-venue consolidation, order entry, "all pairs covered" claims, latency/throughput/capacity results. |
| **H3 — capacity expansion — PLANNED** | Kraken loopback wire mock serving `AssetPairs` discovery, subscribe/unsubscribe, statistics, book snapshots and a scripted reconnect; seed-derived workload profiles; reuse of the shared stress ledger, dispositions, three-set hashes and manifest schema. | A dedicated capacity test proves exact selected-pair coverage, a reconciled ledger/dispositions, bounded writer queue, journal/SBE integrity, zero `LevelChange` and recovery under reconnect for multiple profiles; a reference generator publishes a manifest under `STRESS_MANIFEST_OUT` without overwriting existing evidence. Any `SCENARIO_VERSION` change is a separate, explicitly reviewed decision. | Production or live certification, order-entry guarantees, full-universe subscription, latency percentiles, throughput/capacity results, treating synthetic runs as live evidence. |

Cross-stage non-goals: no trading, positions, strategy or pre-trade risk; no change to the SBE challenge schema; no silent rounding of precision; no claim that ADR-0001's instrument/listing model is implemented; no new external dependency without the existing dependency-governance gate.

### 2.1 H0 implementation notes (Current)

- `src/config.rs` adds `ExchangeKind::Kraken`, `KrakenSettings` (`asset_pairs_url`, `reference_fixture`, `pairs`, `request_timeout_secs`), `Display` output `kraken`, the `COLLECTOR_EXCHANGE=kraken` selector, `build_exchange`/`capabilities_for` arms, capacity (no lifecycle sockets) and probe default target.
- `src/exchange/kraken/codec.rs` decodes the `error`/`result` `AssetPairs` envelope. Identity prefers `wsname` (`XBT/USD`) and otherwise resolves normalized legacy `base`/`quote`, yielding `kraken:spot:XBT/USD`, `kraken:spot:XBT/EUR`, `kraken:spot:XRP/USD`, and so on. `altname` becomes the human-readable title and the venue map key is kept as `exchange_market_id`.
- **Exact scales:** `tick_size` is a decimal string converted to an exact raw step at SBE scale 6; `lot_decimals` produces `10^(6 - lot_decimals)` for both `quantity_step` and `lot_size`. More than six fractional digits is rejected, never rounded. Price/quantity units are the normalized quote/base currencies.
- **Active filter:** only `status == "online"` is emitted. Restricted Kraken states (`cancel_only`, `limit_only`, `post_only`, `maintenance`) and a missing status are excluded fail-closed.
- **Legacy symbols:** an explicit normalization map handles Kraken's `X*`/`Z*` legacy codes (for example `XXBT`→`XBT`, `ZUSD`→`USD`, `XXRP`→`XRP`); unknown codes pass through unchanged rather than being guessed.
- `fixtures/kraken/reference.json` is an `AssetPairs`-style document with five active pairs (including two sharing base `XBT`, one `wsname`-less pair resolved from legacy `base`/`quote`, and additive unknown fields such as fees, leverage, margin and a future field) plus one `cancel_only` pair that is excluded.
- `src/exchange/kraken/session.rs` is an H0 no-network stub: `heartbeat_spec()` is `disabled()`, `connect`/`apply_subscriptions`/`next_frame`/`send_keepalive`/`request_snapshot` are inert, and no socket is created. There is no websocket, book, telemetry or live evidence in H0.

## 3. Verification (Current)

All H0 claims are offline and deterministic:

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`.
- `scripts/check_matrix_tests.py`, `scripts/check_architecture.py --check`, `scripts/check_generated_paths.py`.
- Exact test references are registered in
  [`docs/requirement-test-matrix.md`](requirement-test-matrix.md) and the
  responsibility map in [`scripts/check_architecture.py`](../scripts/check_architecture.py).

No `SCENARIO_VERSION` bump is included. No performance claim is made.
