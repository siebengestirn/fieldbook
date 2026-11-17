# Coinbase Exchange Integration Plan

- **Status:** H0 reference data implemented; H1-H3 remain planned. This document
  defines the staging contract before those stages are implemented.
- **Branch:** `feat/coinbase-reference-data` (from `main` `372da5b`).
- **Scope:** add Coinbase Exchange behind the existing collector seams, using the
  same staged, falsifiable gates as the Hyperliquid integration plan.
- **Reading rule:** every statement below is labelled **Current** (verifiable in
  this tree) or **Planned** (a decision to implement later). H0 is claimed as
  implemented only for offline reference data. No Coinbase websocket, book,
  telemetry, live capture, coverage certificate, latency, throughput or capacity
  result is claimed. Where a wire detail is not pinned by a checked-in fixture,
  it is marked **verify at H1/H2** or explicitly deferred.

The collector currently ingests Kalshi and Polymarket prediction-market books and
Hyperliquid reference data with H1-H3 staged evidence. Coinbase Exchange is a
different reference-data shape (a public `products` catalog with base/quote
currencies and decimal increments) and a different feed family (product-scoped
ticker/heartbeat statistics and level2 books). The plan stages that difference
so each increment is independently falsifiable without changing the collector
state machine.

## 1. Existing adapter seams (Current)

These are the only sanctioned extension points; H0 uses the first five.

| Seam | Exact location | Contract |
|---|---|---|
| `Exchange` trait | `src/exchange/mod.rs` | `name()`, `environment()`, `capabilities()`, `discover_markets(max)`, `discover_markets_with_stats(max)`, `session_factory(self: Arc<Self>)`, `lifecycle_session_factory()` (default `None`). |
| `ExchangeCapabilities` | `src/exchange/mod.rs` | One field `has_sequence_clock: bool`; `capabilities_for(ExchangeKind)` now maps Coinbase → `false`. |
| `DiscoveryResult` / `DiscoveryStats` | `src/exchange/mod.rs` | `markets: Vec<MarketInfo>` plus page/selected/outcome/malformed/retry/duration telemetry. |
| `MarketDataSession` / `SessionFactory` | `src/session.rs` | H0 implements both with a no-network stub; `HeartbeatSpec::disabled()` is the only H0 heartbeat contract. |
| `NumericSpec` / `MarketInfo` | `src/model.rs` | Exact price/quantity scales, steps, units and validation; SBE scale is six decimals. |
| Composition | `src/config.rs`, `src/coordinator.rs`, `src/capacity.rs`, `src/probe.rs` | `ExchangeKind::Coinbase`, `CoinbaseSettings`, `COLLECTOR_EXCHANGE=coinbase`, `build_exchange` arm, zero lifecycle connections, probe target `coinbase.rest_url`. |
| Journal / recovery / evidence | `src/journal.rs`, `src/recovery.rs`, `src/verify.rs`, `src/coverage.rs`, `src/capacity.rs` | Unchanged by H0; later stages must reuse them rather than add venue branches to the core. |
| Test harness | `fixtures/coinbase/reference.json`, `docs/requirement-test-matrix.md`, `scripts/check_architecture.py` | H0 fixture and exact test references are checked in; later stages add their own fixtures and gates. |

Dependency direction is fixed: adapters depend on the normalized model and
protocol vocabulary; the collector core depends on capabilities and normalized
events, not on a websocket library; the coordinator owns composition. A new
venue is added behind these seams, not by branching the state machine.

## 2. Staged deliverables, gates and non-goals

| Stage | Planned deliverable | Evidence gate (all required to advance) | Stage non-goals |
|---|---|---|---|
| **H0 — reference data — IMPLEMENTED** | `ExchangeKind::Coinbase`, `CoinbaseSettings`, env parsing, Display, capability mapping (`false`), `build_exchange`/capacity/probe arms; `src/exchange/coinbase/{mod,codec,discovery,session}.rs`; offline `products` fixture → validated `MarketInfo` with `coinbase:spot:<BASE>/<QUOTE>` identity and exact steps from `quote_increment`/`base_increment`; active-status filter; no-network session with disabled heartbeat. | Deterministic discovery from `fixtures/coinbase/reference.json`; every `MarketInfo` and `NumericSpec` passes `validate()`; identity uniqueness; precision rejection; inactive products excluded; `DiscoveryStats` selected/outcome counts reconcile; no socket opened. | No subscriptions, no book events, no telemetry, no live claim, no performance/throughput/capacity claim, no `SCENARIO_VERSION` change. |
| **H1 — low-complexity feeds — PLANNED** | Coinbase websocket session for public product-scoped statistics and the venue heartbeat channel, with wire shapes pinned by checked-in fixtures. Statistics are telemetry-only observations; the heartbeat mechanism is chosen from fixture evidence, not assumed. | Golden fixtures with unknown additive fields and malformed numeric values; table-driven codec tests; journal v2 typed-observation round-trip with exact raw bytes; **zero** new `Snapshot`/`LevelChange` events and **zero** new coverage anchors; numeric strings parsed exactly or rejected. | No claim of book coverage, no depth derivation, no synthetic books, no heartbeat satisfied by statistics, no latency claim. |
| **H2 — selected books/trades — PLANNED** | An explicitly configured, finite set of spot products; `level2` full snapshots become `ExchangeEvent::Snapshot`, subsequent updates are validated before any delta is applied; trades remain telemetry. Exact subscription reconciliation and recovery via existing `Control::{RequestSnapshot,Recover}` fences. | Per-symbol `coverage` certificate for the exact selected set (`expected == acknowledged == anchored`); `completeness` valid; `verify` and `journal verify` pass; a reconnect drill re-anchors in a new `connection_epoch`; zero `LevelChange` unless a genuine delta wire shape is fixture-pinned. | Full universe, order entry, execution guarantees, venue-clock sequence claims, live certification, latency/throughput results. |
| **H3 — capacity expansion — PLANNED** | Coinbase loopback wire mock reusing the shared stress ledger, dispositions, three-set hashes and manifest schema. Workload profiles are seed-derived and change frame counts, never wall-clock timing. | Exact N-instrument coverage, reconciled ledger/dispositions, bounded writer queue, journal/SBE integrity and reconnect re-anchoring for each declared profile. Any manifest schema or `SCENARIO_VERSION` change is a separate, reviewed gate. | Production or live certification, order-entry guarantees, treating synthetic runs as live evidence, latency percentiles, throughput/capacity results. |

Cross-stage non-goals: no trading, positions, strategy or pre-trade risk; no
change to the SBE challenge schema; no silent rounding of precision; no claim
that ADR-0001's instrument/listing reference model is implemented; no new
external dependency without the existing dependency-governance gate.

### 2.1 H0 implementation notes (Current)

- `CoinbaseSettings` carries `rest_url`, `reference_fixture`,
  `request_timeout_secs` and `require_online`. `COLLECTOR_EXCHANGE=coinbase`
  selects the adapter; `coinbase.rest_url` is validated like other REST
  endpoints (TLS for production, plain HTTP only for loopback).
- `src/exchange/coinbase/codec.rs` decodes the `products` array. A product is
  active only when `status == "online"` and `trading_disabled == false` while
  `require_online` is set; a missing status is unconfirmed and excluded.
  Unknown additive fields are ignored. Inactive products are filtered, not
  counted as malformed.
- Increment strings are validated as plain decimals and parsed exactly at the
  canonical SBE scale (six decimals). An increment with more than six
  fractional digits (for example `0.00000001`) is rejected, never rounded.
  `quote_increment` becomes `tick_size_us`; `base_increment` becomes both
  `quantity_step` and `lot_size`. `price_unit` is the quote currency and
  `quantity_unit` is the base currency.
- `src/exchange/coinbase/discovery.rs` reads the configured offline fixture or
  fetches `/products`. H0 records one page, selected/outcome counts from the
  decoded markets, and zero malformed/retry counters: a bad record rejects the
  round rather than being silently dropped. Results are sorted by
  `native_symbol` and capped by `max_markets`.
- `src/exchange/coinbase/session.rs` is a no-network stub: `connect`,
  `apply_subscriptions`, `next_frame` (idle), `send_keepalive`,
  `request_snapshot` and `close` are total no-ops, `heartbeat_spec()` is
  `HeartbeatSpec::disabled()`, and `lifecycle_session_factory()` is `None` by
  default. It exists only to satisfy the composition seam.
- Fixture: `fixtures/coinbase/reference.json` contains five active products
  (including two sharing base `BTC` and one six-decimal quote increment) and
  two inactive products (`status: "offline"` and `trading_disabled: true`),
  with additive unknown fields on representative records.

## 3. Identity and precision decisions (H0 frozen)

`native_symbol` is the key of `CollectorCore.markets` and the subscription unit;
`symbol` is the namespaced SBE serialization identity. Neither is an ADR-0001
instrument identity.

- **Product identity.** Scope is `(coinbase, spot, base, quote)`, encoded as
  `coinbase:spot:<BASE>/<QUOTE>`. The explicit `coinbase` namespace prevents a
  bare `BTC/USD` from colliding across venues. A shared base with a different
  quote is a distinct market (`coinbase:spot:BTC/USD` and
  `coinbase:spot:BTC/USDT`); two records resolving to the same pair are an
  ambiguity error, never merged silently.
- **Outcome and market id.** `outcome` and `exchange_market_id` are `None`. Coinbase
  spot products are not prediction-market outcomes, and one product is one
  market, so `count_selected_markets` counts each `native_symbol` once.
- **Precision.** `price_scale` and `quantity_scale` are frozen at the canonical
  SBE scale of six, with the increments encoded as exact steps. Increment
  strings are parsed with the shared exact decimal parser; excess precision is
  rejected with evidence, and output conversion continues to use
  `util/numeric::{price_to_sbe,quantity_to_sbe}` from the same scale rules.
  Later stages may not silently round a value that cannot be represented.

## 4. Capability declaration (Current)

`capabilities_for(ExchangeKind::Coinbase)` and `CoinbaseExchange::capabilities()`
both return `ExchangeCapabilities::new(false)`. No Coinbase feed is verified in
H0, so no cross-replica comparable sequence or clock is claimed. The
supervisor's `(connection_epoch, receive_seq)` fence remains the ordering
authority. H1/H2 will fill in ordering, snapshot support, heartbeat mechanism,
replay, backpressure, lifecycle plane and subscription scope from fixture
evidence before any corresponding claim is made.

## 5. Test strategy and traceability

- **Identity uniqueness:** `exchange::coinbase::codec::tests::identity_is_unique_and_explicitly_namespaced`
  and `exchange::coinbase::codec::tests::duplicate_identity_and_malformed_fields_are_rejected`.
- **Numeric spec validation / precision rejection:**
  `exchange::coinbase::codec::tests::numeric_spec_is_exact_from_increments` and
  `exchange::coinbase::codec::tests::excess_precision_is_rejected_not_rounded`.
- **Active filter:** `exchange::coinbase::codec::tests::inactive_products_are_excluded`.
- **Offline discovery and capability/no-lifecycle:**
  `exchange::coinbase::tests::h0_discovery_is_offline_and_reconciles_stats`,
  `exchange::coinbase::tests::h0_reference_document_produces_valid_namespaced_markets`,
  `exchange::coinbase::tests::h0_uses_clockless_capabilities_and_no_lifecycle_plane`
  and `exchange::coinbase::session::tests::h0_session_is_no_network_with_disabled_heartbeat`.
- Exact file-qualified references are registered in
  `docs/requirement-test-matrix.md` (M2) and enforced by
  `scripts/check_matrix_tests.py`. Source responsibilities and falsifiable
  anchors are registered in `scripts/check_architecture.py`.
- Later stages will add golden fixtures with additive fields and malformed
  numerics, journal v2 observation round-trips, typed recovery drills, and a
  contained loopback wire mock — each with its own gate and no live or
  performance claim from synthetic evidence.

## 6. Implemented / current versus planned

| Item | Status |
|---|---|
| `Exchange`, `SessionFactory`, `MarketDataSession`, `SessionFrame`, typed protocol/journal vocabulary, `ExchangeCapabilities`, normalized `ExchangeEvent`, `NumericSpec`, exact numeric conversion, catalog, recovery policy, completeness/coverage/verify, stress lab | **Current** — existing tree, unchanged by Coinbase H0. |
| Coinbase H0 selector, settings, composition arm, capability mapping, capacity/probe handling | **Current**. |
| Coinbase H0 offline `products` discovery, identity map, exact increment specs, active filter, no-network session, fixture and traceability | **Current** — offline reference data only. |
| Coinbase H1 statistics/heartbeat feed, H2 selected books, H3 capacity expansion | **Planned** — not implemented; no websocket, book, telemetry, live evidence or performance claim. |

## 7. Follow-on order

Reuse the Hyperliquid order: config and identity → reference data → adapter
module → capability declaration → event mapping → evidence plumbing →
containment tests → staged rollout. Coinbase-specific questions to answer
before H1 (not asserted here): product/level versus advanced-trade feed wire
shapes, whether a venue sequence is cross-replica comparable, the heartbeat
channel contract, and level2 snapshot versus delta semantics. No endpoint,
symbol or capability beyond H0's `/products` reference data is claimed.
