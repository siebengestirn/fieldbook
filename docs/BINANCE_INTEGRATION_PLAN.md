# Binance Integration Plan

- **Status:** H0 reference data implemented. H1-H3 are **planned** gates with falsifiable evidence requirements, not implemented behavior.
- **Branch:** `feat/binance-reference-data` (H0, from `main` at `372da5b`).
- **Scope:** add Binance behind the existing exchange seams using the same staged gates as the Hyperliquid integration plan (`docs/HYPERLIQUID_INTEGRATION_PLAN.md`).
- **Reading rule:** every statement below is labelled **Current** (verifiable in this tree) or **Planned** (a future decision). H0 is offline-only. No websocket, book, telemetry, live evidence, latency or capacity result is claimed, and `SCENARIO_VERSION` is unchanged.

Binance differs from the current prediction-market adapters in asset class (spot and perpetual crypto), in reference-data shape (`exchangeInfo` symbol arrays with string-valued `PRICE_FILTER`/`LOT_SIZE` filters), and in quote scoping (one canonical quote asset, not a prediction-market outcome set). H0 stages that difference so each later increment can be independently falsified.

## 1. Existing adapter seams (Current)

The only sanctioned extension points are those used by Hyperliquid H0:

| Seam | Exact location | Contract |
|---|---|---|
| `Exchange` trait | `src/exchange/mod.rs` | `name()`, `environment()`, `capabilities()`, `discover_markets(max)`, `discover_markets_with_stats(max)`, `session_factory(self: Arc<Self>)`, `lifecycle_session_factory()` (default `None`). |
| `ExchangeCapabilities` | `src/exchange/mod.rs` | One field `has_sequence_clock`; `capabilities_for(ExchangeKind)` maps every configured venue to `false`. |
| `DiscoveryResult` / `DiscoveryStats` | `src/exchange/mod.rs` | `markets: Vec<MarketInfo>` plus page/selected/outcome/malformed/retry/duration telemetry. |
| `MarketDataSession` / `SessionFactory` | `src/session.rs` | `heartbeat_spec()`, `connect()`, `apply_subscriptions()`, `next_frame()`, `send_keepalive()`, `request_snapshot()`, `close()`; `create_session(SourceId)`. |
| Normalized model | `src/model.rs` | `MarketInfo`, `NumericSpec`, `Price`/`Quantity`/`QuantityDelta`, `ExchangeEvent::{Snapshot,LevelChange,Lifecycle,StreamReset}`. |
| Numeric pipeline | `src/fixed.rs`, `src/util/numeric.rs` | Canonical `SCALE = 1_000_000`, `DECIMALS = 6`, `MAX_SCALE = 18`; exact rescale or reject, never round. |
| Composition | `src/coordinator.rs`, `src/config.rs` | `build_exchange(settings, require_auth)`, `ExchangeKind`, `FileConfig`/`Settings`, `COLLECTOR_EXCHANGE`, per-venue settings. |
| Adapter layout | `src/exchange/binance/{mod,codec,discovery,session}.rs` | One module per venue; H0 decodes reference data and exposes a no-network session stub. |
| Capacity / probe | `src/capacity.rs`, `src/probe.rs` | Zero lifecycle sockets for venues without a lifecycle channel; documented default probe target. |

Dependency direction is unchanged: adapters depend on the normalized model, `CollectorCore` depends on capabilities and normalized events, and the coordinator owns composition. Binance adds no state-machine branch.

## 2. Staged deliverables and gates

| Stage | Deliverable | Evidence gate (all required to advance) | Stage non-goals |
|---|---|---|---|
| **H0 — reference data (IMPLEMENTED)** | `ExchangeKind::Binance`, `BinanceSettings`, env/Display/capability/coordinator/capacity/probe wiring; `src/exchange/binance/{mod,codec,discovery,session}.rs`; offline `exchangeInfo` fixture; validated `MarketInfo` with explicit identity, canonical-quote filter and exact steps from `PRICE_FILTER`/`LOT_SIZE`; no-network session stub with disabled heartbeat. | `discover` produces a deterministic catalog from an explicit fixture; every `MarketInfo` passes `validate()` and `NumericSpec::validate()`; identities unique across spot/perp and across shared base assets; `tickSize`/`stepSize` beyond SBE scale 6 are rejected, not rounded; record/outcome counts reconcile with `DiscoveryStats`; the fixture includes spot, perpetual, shared bases and additive unknown fields; no socket is ever opened. | No subscriptions, no book events, no telemetry, no live claim, no websocket, no performance claim. |
| **H1 — telemetry (PLANNED)** | Selected low-complexity push channels, decoded to typed `ProtocolControl` observations with exact raw bytes. Heartbeat stays `Disabled` until a checked-in fixture proves a venue heartbeat. | Golden fixtures with unknown additive fields and malformed numerics; table-driven codec tests; journal v2 round-trip; **zero** new `Snapshot`/`LevelChange` events and **zero** coverage anchors; numeric strings parsed exactly or rejected. | No book coverage, no depth derivation, no synthetic books, no latency claim. Wire details (channel names, update speeds, tick weights) are deliberately unspecified until fixture-gated. |
| **H2 — selected books (PLANNED)** | Small finite list of explicitly selected instruments; genuine full-book pushes become `ExchangeEvent::Snapshot`, trades stay telemetry; exact subscription reconciliation; recovery through the existing `Control::{RequestSnapshot,Recover}` fences. | Per-symbol `coverage` certificate for the exact selected set (`expected == acknowledged == anchored`); `completeness` valid; `verify` and `journal verify` pass; one reconnect drill re-anchors in a newer `connection_epoch`; zero `LevelChange`. | Full universe, all quote assets, order entry, "all symbols covered" claims, latency/throughput results. Snapshot versus delta wire shape must be pinned by a checked-in fixture before this gate. |
| **H3 — capacity expansion (PLANNED)** | Deterministic loopback wire mock and seed-derived workload profiles reusing the shared stress ledger, dispositions, three-set coverage hashes and manifest schema. | `tests/binance_capacity.rs` (planned) would certify exact coverage, reconciled ledger/dispositions, bounded queues, journal/SBE integrity and reconnect re-anchoring for declared profiles; an ignored generator would publish a versioned manifest without overwriting evidence. | Production or live certification, latency percentiles, throughput/capacity results, treating synthetic runs as live evidence. No `SCENARIO_VERSION` change is implied by H0. |

Cross-stage non-goals: no trading, positions, strategy or pre-trade risk; no change to the SBE challenge schema; no silent rounding of precision; no claim that ADR-0001's instrument/listing model is implemented; no new external dependency without the existing dependency-governance gate.

## 3. H0 implementation notes (Current)

### 3.1 Identity

`native_symbol` keys the collector book map and is mirrored by the namespaced SBE `symbol`. Neither is an ADR-0001 instrument identity.

- Spot: `binance:spot:<BASE>/<QUOTE>` (for example `binance:spot:BTC/USDT`).
- Perpetual: `binance:perp:<SYMBOL>` using the venue contract name (for example `binance:perp:BTCUSDT`).
- `outcome`, `exchange_market_id` and `close_time_us` are `None`; `contract_multiplier` and `tick_schedule` are empty.

A shared base asset (BTC in both spot and perpetual) is allowed and produces two distinct identities. A symbol quoting a non-canonical asset, a non-`TRADING` status, a spot symbol without `isSpotTradingAllowed`, and a futures contract whose `contractType` is not `PERPETUAL` are all excluded rather than merged under a bare base-asset name. Duplicate identities are rejected.

### 3.2 Numeric semantics

`PRICE_FILTER.tickSize` and `LOT_SIZE.stepSize` are decimal strings. `codec::parse_decimal` parses them without floating point, removes trailing fractional zeros to find the effective precision, and rejects any value needing more than the canonical SBE scale of 6 decimals. `exact_step` converts the accepted value to an exact micro-unit `Quantity`; that value becomes `NumericSpec.quantity_step` and `lot_size`, while `tickSize` becomes `MarketInfo.tick_size_us`. Both scales are frozen at 6. An absent or malformed filter is fatal. This is "exact or rejected", never silent rounding.

### 3.3 Offline discovery

`BinanceSettings.reference_fixture` must name an explicit file; with no fixture, discovery returns a clear error instead of falling back to the network. `discover_with_stats` reads the fixture, parses only the enabled product sections (`spot_enabled`, `perp_enabled`), filters to canonical quotes, sorts by `native_symbol`, applies `max_markets` when non-zero, and reports `pages` as one page per enabled product section. `malformed_records` and `retries` are always zero because a malformed record fails the whole round rather than being counted and skipped.

### 3.4 Session stub

`BinanceSessionFactory` satisfies the shared seam but returns a `BinanceSession` whose `connect`, `apply_subscriptions`, `next_frame` and `request_snapshot` return `SessionFailure::Fatal`. No socket, HTTP client, subscription or heartbeat request is created; `heartbeat_spec()` is `HeartbeatSpec::disabled()`. The lifecycle plane is absent: `lifecycle_session_factory()` returns `None` and capacity planning charges zero lifecycle sockets.

### 3.5 Fixture

`fixtures/binance/reference.json` contains `spot` and `perp` `exchangeInfo`-style sections. It includes a shared base asset (BTC) across both sections, additive unknown fields at document and symbol level, a non-canonical quote entry (`BTCUSDC`), a halted symbol, and a dated non-perpetual contract. These exercise the exclusion and additive-field rules.

## 4. Capability declaration (Current / Planned)

**Current:** `ExchangeCapabilities` carries only `has_sequence_clock`, and `capabilities_for(ExchangeKind::Binance)` returns `false`. The supervisor's `(connection_epoch, receive_seq)` fence is the only ordering authority; no venue clock is claimed.

**Planned** (explicit later changes, not made by H0): ordering and snapshot support, heartbeat mechanism, replay source, backpressure, lifecycle plane and subscription scope would be filled in when H1-H3 add wire evidence. No Binance sequence number, feed clock, heartbeat or snapshot opcode is asserted here.

## 5. Test strategy (Current)

- `src/exchange/binance/codec.rs`: exact steps from price/lot filters, numeric-spec validation for every market, precision rejection beyond scale 6, canonical-quote/status/contract-type exclusion, additive-field invariance, and fail-closed malformed documents.
- `src/exchange/binance/discovery.rs`: offline discovery stats reconciliation, `max_markets` cap, and no-network-fallback error.
- `src/exchange/binance/mod.rs`: identity uniqueness across spot/perp, clockless capability, absent lifecycle plane/zero lifecycle sockets, and fixture-backed offline discovery through the `Exchange` trait.
- `src/exchange/binance/session.rs`: disabled heartbeat and fatal-on-network stub.
- `src/config.rs`: Binance default settings and selector/Display mapping.

Exact file-qualified test names are registered in `docs/requirement-test-matrix.md`; source responsibilities and falsifiable anchors are registered in `scripts/check_architecture.py`.

No latency, throughput, capacity or live evidence test exists for Binance in H0. `SCENARIO_VERSION` is unchanged.

## 6. Implemented versus planned

| Item | Status |
|---|---|
| Binance H0 discovery, settings, identity map, exact numeric codec, offline fixture, no-network session stub | **Current** — behind the existing seams, no state-machine change. |
| Binance capability declaration (`has_sequence_clock = false`) | **Current**. |
| Binance H1 telemetry, H2 selected books, H3 synthetic capacity | **Planned** — gated as in §2; no implementation or evidence claim. |
| ADR-0001 instrument/listing reference model | **Accepted design, not implemented**; this plan does not depend on it. |
