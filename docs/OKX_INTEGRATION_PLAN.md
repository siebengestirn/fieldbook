# OKX Integration Plan

- **Status:** H0 reference data **implemented**. H1 (REST discovery + telemetry-only
  websocket), H2 (selected book feed) and H3 (capacity expansion) are **planned**,
  reusing the same staged gates as
  [Hyperliquid](HYPERLIQUID_INTEGRATION_PLAN.md).
- **Branch:** `feat/okx-reference-data` (H0). Later stages use separate branches.
- **Scope:** add a second non-prediction-market connector behind the existing
  collector seams, one stage at a time, with independently falsifiable evidence.
- **Reading rule:** every statement below is either **Current** (verifiable in
  this tree) or **Planned**. H0 is implemented and offline-verified; H1-H3 are
  decisions to implement later. No live capture, live certification, latency
  performance or capacity/throughput result is claimed anywhere in this plan.
  Where a wire detail is not pinned by a checked-in fixture it is marked
  **verify at H1** or explicitly deferred.

The collector currently ingests Kalshi and Polymarket prediction-market books
plus Hyperliquid perpetual/spot reference data and selected books. OKX is a
different reference-data shape: an `instruments` document keyed by `instId` and
scoped by `instType` (for example SPOT and SWAP), with decimal `tickSz`/`lotSz`
strings and an explicit `state`. H0 stages that difference behind the same seams
so each increment can be rejected independently.

## 1. Existing adapter seams (Current)

These are the only sanctioned extension points; H0 uses the first four.

| Seam | Exact location | H0 use |
|---|---|---|
| `ExchangeKind` / settings | `src/config.rs` | `ExchangeKind::Okx`, `OkxSettings`, lowercase serde/Display, `COLLECTOR_EXCHANGE=okx`. |
| `Exchange` trait | `src/exchange/mod.rs` | `name`, `environment`, `capabilities`, `discover_markets`, `discover_markets_with_stats`, `session_factory`; `lifecycle_session_factory` defaults to `None`. |
| `ExchangeCapabilities` | `src/exchange/mod.rs` | `capabilities_for(Okx)` maps to `has_sequence_clock: false`. |
| `DiscoveryResult` / `DiscoveryStats` | `src/exchange/mod.rs` | One offline page; selected/outcome counts reconcile with decoded records. |
| `MarketDataSession` / `SessionFactory` | `src/session.rs` | H0 installs an inert session stub with `HeartbeatSpec::disabled()`; no socket is opened. |
| `MarketInfo` / `NumericSpec` | `src/model.rs` | Exact scales, units, tick size and lot/step validated by `validate()`. |
| Composition | `src/coordinator.rs`, `src/exchange/mod.rs` | `build_exchange` arm; `OkxExchange` + `OkxSessionFactory` behind `Arc<dyn Exchange>`. |
| Admission | `src/capacity.rs`, `src/probe.rs` | OKX allocates zero lifecycle connections; the probe's default target is the planned H1 REST endpoint. No H0 connection is dialled. |

Dependency direction is fixed: adapters depend on the normalized model and
protocol vocabulary, the collector core depends on capabilities and normalized
events (not on a websocket library), and the coordinator owns composition. H0
adds no state-machine branch and no new runtime dependency.

## 2. Staged deliverables, gates and non-goals

| Stage | Deliverable | Evidence gate (all required to advance) | Stage non-goals |
|---|---|---|---|
| **H0 — reference data (IMPLEMENTED)** | `ExchangeKind::Okx` + `OkxSettings` + env/Display/serde; capability mapping (false); coordinator, capacity and probe arms; `src/exchange/okx/{mod,codec,discovery,session}.rs`; offline `instruments` fixture with SPOT and SWAP instruments, a shared underlying, additive unknown fields and one non-live instrument. | `parse_instruments_document` produces deterministic, validated `MarketInfo`; identities are unique and product-type-qualified; `tickSz`/`lotSz` become exact scales and excessive precision is rejected, never rounded; non-live instruments are excluded; record/outcome counts reconcile with `DiscoveryStats`; the session is inert with a disabled heartbeat; no socket is opened. | No subscriptions, no book events, no websocket, no live claim, no REST dial, no telemetry, no lifecycle, no capacity result. |
| **H1 — REST discovery + telemetry (PLANNED)** | Replace the offline fixture with `/api/v5/public/instruments` discovery scoped by `instType`; add a telemetry-only websocket session for public tickers/contexts with fixture-pinned wire shapes. | Golden REST/telemetry fixtures with additive unknown fields and malformed numerics; exact-or-rejected numeric decoding; typed `ProtocolControl` journal observations with exact raw bytes; zero new `Snapshot`/`LevelChange` events and zero new anchors. | No book coverage, no depth derivation, no synthetic books, no heartbeat satisfaction. Venue heartbeat and snapshot mechanics are **verify at H1** against fixtures. |
| **H2 — selected books (PLANNED)** | Subscribe to a configurable finite list of OKX instruments; map genuine full-book snapshots to `ExchangeEvent::Snapshot`, keep updates/deltas out unless a real delta channel is fixture-pinned and normalized; exact subscription reconciliation. | Per-symbol `coverage` certificate for the exact selected set; valid `completeness`/`verify`/`journal verify`; exact receive times and receive-fence recovery; zero unproven depth. | Full universe, unspecified product types, checksum-based gap claims without a pinned fixture, latency/throughput results. |
| **H3 — capacity expansion (PLANNED)** | Deterministic OKX loopback wire mock and seed-derived workload profiles reusing the shared stress ledger, dispositions, three-set hashes and manifest schema. | Exact-instrument coverage cells with reconciled ledger/dispositions, bounded queues, journal/SBE integrity and recovery under reconnect; manifests marked `evidence_class: synthetic-loopback`. | Live certification, latency percentiles, throughput/capacity results, treating synthetic runs as live evidence. No `SCENARIO_VERSION` change is implied by H0. |

Cross-stage non-goals: no trading, positions, strategy or pre-trade risk; no
change to the SBE challenge schema; no silent rounding of precision; no claim
that ADR-0001's instrument/listing model is implemented; no new external
dependency without the existing dependency-governance gate.

### 2.1 H0 implementation notes (Current)

- `OkxSettings.inst_types` (default `["SPOT", "SWAP"]`) is the product-type
  scope. An instrument whose `instType` is unconfigured or unsupported is
  filtered out, never silently admitted. `reference_fixture` is the offline
  source; when absent H0 discovery fails closed. `rest_url` is recorded as the
  planned H1 endpoint and is the probe's default target, but H0 never dials it.
- `src/exchange/okx/codec.rs` parses each live, in-scope record. `tickSz` and
  `lotSz` decimal strings are converted to exact integer steps at canonical
  SBE scale 6; more than six fractional digits, a non-decimal string, or a zero
  is rejected. No value is truncated or rounded. The optional `minSz` becomes
  the lot size and must be a multiple of the quantity step under
  `NumericSpec::validate()`. Additive unknown fields are ignored.
- `state == "live"` is required; a missing or non-live state is excluded. The
  checked-in fixture has six records: five live instruments (SPOT BTC/USDT,
  SPOT ETH/USDT, SWAP BTC/USDT-SWAP, SWAP ETH/USDT-SWAP, SWAP SOL-USDT-SWAP)
  and one suspended SPOT instrument that must be excluded.
- `src/exchange/okx/discovery.rs` reads the configured fixture as one page and
  reports `selected_markets == outcome_symbols` with zero malformed records and
  zero retries. Discovery is a pure offline function; no `reqwest` client is
  constructed in H0.
- `src/exchange/okx/session.rs` is an inert stub: heartbeat `Disabled`, no
  subscriptions applied, `connect`/`next_frame` fail closed as `Fatal`. It is
  wired through `OkxExchange::session_factory` so H1 can replace it without a
  state-machine change.

## 3. What H0 reference data can and cannot prove

A decoded `MarketInfo` is metadata, not market evidence.

- **Reference data is not depth.** A validated tick size, lot size or identity
  does not prove a book exists, is two-sided, or has ever been anchored. It
  cannot satisfy `coverage`, `completeness` or `verify`.
- **An instrument list is not a subscription.** Discovering SPOT and SWAP
  identities does not subscribe to any channel and does not prove venue
  availability beyond the offline fixture.
- **A fixture is not a live venue.** The checked-in document is a deterministic
  codec checkpoint. It pins identity, scoping, exact scales and the state
  filter; it makes no claim about a live OKX endpoint or feed.
- A later credible book claim still requires the conjunction of journal wire
  evidence, `verify` reconstruction, `completeness` anchoring, exact `coverage`
  and recovery evidence — none of which H0 provides.

## 4. Identity mapping (finalized at H0)

`native_symbol` is the key of `CollectorCore.markets` and the subscription unit;
`symbol` is the namespaced SBE serialization identity. Neither is an ADR-0001
instrument identity.

**4.1 Product-type scope.** Identity is `(okx, instType, base, quote)`. The
namespace is the lowercased `instType`, so `okx:spot:BTC/USDT` and
`okx:swap:BTC/USDT` are distinct even though they share an underlying. Bare
`instId` text is never reused as a key: a multi-venue or multi-product capture
would then be ambiguous. SPOT ids decode from `BASE-QUOTE` and SWAP ids from
`BASE-QUOTE-SWAP`; other shapes are rejected.

**4.2 Exchange metadata.** `exchange = "okx"`, `symbol == native_symbol`,
`outcome = None`, `exchange_market_id = None`, `close_time_us = None`,
`price_ranges` empty. `title` is the venue `instId` (advisory).

**4.3 Precision and specs.** Price and quantity scales are frozen at the
canonical SBE scale 6. `tickSz` sets `tick_size_us`; `lotSz` sets
`quantity_step`; `minSz` sets `lot_size` (falling back to `lotSz`). SPOT uses
`PriceUnit::Currency(quoteCcy)` and `QuantityUnit::Currency(base)`; SWAP uses
`PriceUnit::Currency(settleCcy)` and `QuantityUnit::Contracts`. Any value not
exactly representable at scale 6 is rejected with evidence, never rounded.
This is the same precision rule as Hyperliquid H0 (§4.3 there).

**4.4 Output symbol compatibility.** The `<namespace>:<...>` SBE format is kept.
H0 freezes `okx:<instType>:<BASE>/<QUOTE>`; a later change would invalidate
catalog checksums and comparison against earlier evidence.

## 5. Capability declaration for OKX (Current)

`ExchangeCapabilities` has one field, `has_sequence_clock`. OKX H0 declares
`false`: H0 uses no venue sequence and the receive-fence-only ordering
authority remains. H1-H3 must keep `false` unless a cross-replica comparable
clock is verified against a checked-in fixture; an advisory feed timestamp
belongs in `EventMeta.exchange_ts_us`, never in gap detection.

| Dimension | H0 declaration | Planned direction |
|---|---|---|
| Ordering | Receive-fence only; no venue clock | Unchanged until a fixture proves a comparable sequence/clock. |
| Snapshot support | None in H0 | H2 pins snapshot vs delta wire shapes with fixtures before mapping either. |
| Heartbeat | `HeartbeatMechanism::Disabled` | H1 verifies the venue mechanism; no heartbeat is invented. |
| Replay | No venue history; journal is the sole evidence source | Unchanged. |
| Backpressure | Unchanged bounded writer/journal | Unchanged. |
| Lifecycle plane | `lifecycle_session_factory() = None`; zero lifecycle connections | Unchanged for public feeds. |
| Subscription scope | No subscriptions in H0 | H2 reconciles an exact, finite selected set. |

## 6. Test strategy (Current H0, planned H1-H3)

- **Fixture.** `fixtures/okx/reference.json` is an `instruments`-style document
  with two SPOT and three SWAP live instruments, shared BTC/ETH underlyings,
  additive unknown fields of several JSON types, and one suspended instrument
  that must be excluded.
- **Identity uniqueness.** A test asserts the five live identities are distinct
  and that the BTC spot and swap symbols coexist.
- **Numeric spec validation.** Tests assert exact tick/quantity steps and units
  for spot and swap, and that every market passes `MarketInfo::validate()` and
  `NumericSpec::validate()`.
- **Precision rejection.** Tests reject tick/lot values with more than six
  decimals, non-decimal strings and zeros rather than rounding them.
- **State filter.** Tests assert the suspended instrument is excluded and that
  an all-non-live document fails closed.
- **Offline discovery stats.** Tests assert `selected_markets ==
  outcome_symbols == 5`, `pages == 1`, zero malformed/retries, a deterministic
  sort, the `max_markets` cap, and failure without a configured fixture.
- **Capability / no lifecycle.** Tests assert `name() == "okx"`, no sequence
  clock, matching `capabilities()`, `kind() == ExchangeKind::Okx`, a `None`
  lifecycle factory, and an inert session with disabled heartbeat.
- **Planned (H1-H3).** REST/telemetry golden fixtures, exact numeric tables,
  journal v2 observation round-trips, per-symbol coverage/completeness/verify
  on a loopback mock, typed recovery drills, and deterministic capacity cells.
  `SCENARIO_VERSION` is not bumped by H0 and would only change with H3 evidence.

## 7. Implemented / current versus planned

| Item | Status |
|---|---|
| `Exchange`, `SessionFactory`, `MarketDataSession`, `ExchangeCapabilities`, `MarketInfo`, `NumericSpec`, exact numeric conversion, catalog, recovery, completeness/coverage/verify, stress lab | **Current** — cited in §1 and the architecture doc. |
| OKX H0 settings, ExchangeKind, capability/coordinator/capacity/probe wiring, adapter module, offline codec/discovery, no-network session, reference fixture | **Current** — deterministic offline reference data only; no socket, no book event, no live claim. |
| OKX H1 REST discovery + telemetry | **Planned**. |
| OKX H2 selected books / H3 capacity cells | **Planned**. |
| ADR-0001 instrument/listing reference model | **Accepted design, not implemented**; H0 does not claim it. |

## 8. Reused staged gates

H1-H3 use the Hyperliquid gates verbatim, adapted to OKX product types:
fixture-pinned decoding and zero book events for H1; exact finite selected-set
coverage with `verify`/`completeness`/`journal verify` and a recovery drill for
H2; seed-derived loopback profiles reusing the shared ledger, three-set hashes
and manifest schema for H3. Synthetic results never satisfy a live tier and
carry no latency or capacity claim. Reference → stats → selected books →
capacity, each with its own gate; live evidence only from a permitted
connection with access restrictions respected.

## 9. Key decisions

1. Product type is part of OKX identity; bare `instId` never keys a book.
2. `tickSz`/`lotSz` are exact or rejected at SBE scale 6; no rounding.
3. Only `state == "live"` instruments enter the universe.
4. `has_sequence_clock` stays `false`; H0 opens no socket and claims no feed.
5. H0 fails closed without an offline fixture; live REST discovery is H1.
6. Each later stage has a falsifiable evidence gate; no live or performance
   claim from H0 fixtures or synthetic runs.
7. Venue additions remain template-driven behind the existing seams; no
   state-machine changes and no `SCENARIO_VERSION` bump in H0.
