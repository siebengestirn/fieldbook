# Hyperliquid Integration Plan

- **Status:** H0 reference data and H1 telemetry implemented; H2-H3 remain planned. This document defined the staging contract before implementation.
- **Branch:** `feat/hyperliquid-telemetry` (H1); the plan originated on `docs/hyperliquid-integration-plan`.
- **Scope:** plan the next exchange after the M series on the existing collector seams.
- **Reading rule:** every statement below is either labelled **Current** (verifiable in this
  tree) or **Planned** (a decision to implement later). Only H0/H1 are claimed as implemented;
  no H2/H3 capability, live capture, coverage or latency result is claimed. Where a wire detail
  is not pinned by a checked-in fixture, it is marked **verify at H0** or explicitly deferred.

The collector today ingests Kalshi and Polymarket prediction-market books only. Hyperliquid
is a different asset class (perpetual and spot crypto), a different reference-data shape
(DEX scope, token indices, non-.`[0,1]` prices), and a partly different feed model (full-book
snapshots and high-rate statistics rather than deltas). The plan stages that difference so
that each increment is independently falsifiable.

## 1. Existing adapter seams (Current)

Inventory taken from the code on this branch. These are the only sanctioned extension points.

| Seam | Exact location | Contract |
|---|---|---|
| `Exchange` trait | `src/exchange/mod.rs` | `name()`, `environment()`, `capabilities()`, `discover_markets(max)`, `discover_markets_with_stats(max)` (default provided), `session_factory(self: Arc<Self>)`, `lifecycle_session_factory()` (default `None`). |
| `ExchangeCapabilities` | `src/exchange/mod.rs` | One field: `has_sequence_clock: bool`; `ExchangeCapabilities::new(bool)`; `capabilities_for(ExchangeKind)` maps Kalshi→`false`, Polymarket→`false`. |
| `DiscoveryResult` / `DiscoveryStats` | `src/exchange/mod.rs` | `markets: Vec<MarketInfo>` plus page/selected/outcome/malformed/retry/duration timestamps. |
| `MarketDataSession` | `src/session.rs` | `heartbeat_spec()`, `connect()`, `apply_subscriptions(&BTreeSet<String>)`, `next_frame()`, `send_keepalive()`, `request_snapshot(&[String])`, `close()`. |
| `SessionFactory` | `src/session.rs` | `create_session(SourceId) -> Box<dyn MarketDataSession>`. |
| `SessionFrame` / `SessionFailure` | `src/session.rs` | Frame carries `events`, `raw: Option<RawFrame>`, `decode`, `signal`, `mechanism`, `observations`, optional `transport_failure`; failures are `Reconnect`, `Decode`, `Protocol`, `Fatal`. |
| `HeartbeatSpec` / `HeartbeatRequest` | `src/session.rs` | Presets `ten_second_application_text()`, `ten_second_websocket_control()`, `disabled()`, plus `validate()`. |
| Supervisor | `src/session.rs` | `run_worker` owns reconnect/backoff, heartbeat tracking, `(connection_epoch, receive_seq)` receive fence, `Control` handling, and emits `WorkerEvent::{Events,Status,Subscribed,RecoveryReady,Fatal,Stopped}`. |
| Protocol vocabulary | `src/protocol.rs` | `FrameKind`, `FrameDirection`, `ControlOpcode`, `FrameClassification`, `HeartbeatMechanism::{ApplicationText,WebSocketControl,Disabled}`, `SessionSignal::{MarketData,HeartbeatRequest,HeartbeatAck{correlation},PeerPing,PeerPong,ProtocolControl}`, `SendResult`, `TransportObservation`. |
| Normalized events | `src/model.rs` | `ExchangeEvent::{Snapshot,LevelChange,Lifecycle,StreamReset}`; `MarketInfo`; `NumericSpec`; `Price`/`Quantity`/`QuantityDelta`; `PriceBand`; `Ratio`; `EventMeta`; `Control::{SetSubscriptions,RequestSnapshot,Recover,Reconnect,Shutdown}`; `OutputMessage`; `SourceId`; `SourceStatus`; continuity/heartbeat state enums. |
| Numeric pipeline | `src/fixed.rs`, `src/util/numeric.rs` | `SCALE = 1_000_000`, `DECIMALS = 6`, `MAX_SCALE = 18`; `parse_scaled`, `rescale_exact`; `price_to_sbe`/`quantity_to_sbe`/`level_to_sbe` rescale exactly and **reject inexact** values. |
| Composition | `src/coordinator.rs`, `src/config.rs` | `build_exchange(settings, require_auth) -> Arc<dyn Exchange>` matches `ExchangeKind::{Kalshi,Polymarket}`; `ExchangeKind`, `FileConfig`/`Settings`, `COLLECTOR_EXCHANGE` env, per-venue settings. |
| Adapter layout | `src/exchange/kalshi/{mod,auth,codec,discovery,session}.rs`, `src/exchange/polymarket/{mod,codec,discovery,session}.rs` | One module per venue; `codec` decodes wire→`ExchangeEvent`, `discovery` builds `MarketInfo`, `session` implements both session traits. Kalshi additionally provides a credentialed lifecycle plane. |
| Journal | `src/journal.rs` | `JOURNAL_SCHEMA_VERSION = 2`, `DecodeOutcome::{Decoded,Ignored,Rejected}`, `RawFrame`, `JournalHandle`; `read_records()` is data-only, `read_observations()` includes typed data/control observations; checksummed segments and manifest. |
| Recovery | `src/recovery.rs` | `RecoveryAction::{RefreshInstruments,ReconnectSource}`, `RecoveryCause`, `RecoverySignals`, `RecoveryDecision`, pure `decide()`; bounded, fenced by `(source, instrument, generation)`. |
| Evidence | `src/verify.rs`, `src/completeness.rs`, `src/coverage.rs`, `src/capacity.rs`, `src/catalog.rs` | SBE reconstruction; per-symbol anchoring; three-set coverage; capacity `plan`/`count_selected_markets`; checksummed catalog checkpoints. |
| Test harness | `tests/e2e_mock_exchange.rs`, `tests/full_universe_stress.rs`, `fixtures/kalshi/`, `fixtures/polymarket/`, `fixtures/journal-v*-golden/`, `docs/stress-lab-design.md` | Loopback wire mocks, deterministic stress ledger, JSON protocol fixtures, golden journals. |

Dependency direction is fixed: adapters depend on the normalized model and protocol vocabulary,
`CollectorCore` depends on capabilities and normalized events (not on a websocket library), and
the coordinator owns composition. A new venue must be added behind these seams, not by branching
the state machine.

## 2. Staged deliverables, gates, and non-goals

| Stage | Planned deliverable | Evidence gate (all required to advance) | Stage non-goals |
|---|---|---|---|
| **H0 — reference data** | `HyperliquidSettings` and `ExchangeKind::Hyperliquid` in `config.rs`; `build_exchange`/`capabilities_for` arms; `src/exchange/hyperliquid/{mod,codec,discovery,session}.rs` skeleton; REST `/info` discovery for `meta` (every configured DEX) and `spotMeta`; emit validated `MarketInfo` with DEX/spot-qualified `native_symbol` and full `NumericSpec`; catalog checkpoint round-trip. | `discover` produces a deterministic catalog; every `MarketInfo` passes `MarketInfo::validate()` and `NumericSpec::validate()`; record/outcome counts reconcile with `DiscoveryStats`; golden `meta`/`spotMeta` fixtures include two DEXs sharing a coin name and spot token-index forms; no socket is opened. | No subscriptions, no book events, no live claim, no ADR-0001 reference-model migration, no spot book. |
| **H1 — low-complexity feeds — IMPLEMENTED** | Subscribe and decode `allMids` and `fastAssetCtxs`; classify as telemetry only; journal typed observation evidence with exact raw bytes; heartbeat policy is `Disabled` (no fixture proves a venue heartbeat, and telemetry must not satisfy one). `assetCtxs` is **not subscribed**: no golden fixture pins its wire shape, so adding it is deferred. | Golden fixtures for mid maps and context arrays with unknown additive fields and malformed numeric values; codec table tests; journal v2 round-trip; **zero** new `Snapshot`/`LevelChange` events and **zero** new coverage anchors proved by `tests/hyperliquid_telemetry.rs`; numeric strings parsed exactly or rejected. | No claim of book coverage, no depth derivation, no market-order/execution view, no synthetic books, no `assetCtxs` subscription. |
| **H2 — selected books/trades** | 10–20 explicitly selected perpetuals; `l2Book` full snapshots → `ExchangeEvent::Snapshot`; trades parsed but kept out of book deltas; exact subscription reconciliation; recovery via existing `Control::{RequestSnapshot,Recover}` and typed `RecoveryAction`/`RecoveryCause`. | Per-symbol `coverage` certificate for the exact selected set (`expected == acknowledged == anchored`); `completeness` valid (no unanchored deltas); `verify` and `journal verify` pass; one reconnect drill re-anchors before any promoted delta with a new `connection_epoch`; latency percentiles recorded with clock type. | Full universe, spot books, cross-DEX consolidation, order entry, "all coins covered" claims. |
| **H3 — capacity expansion** | Grow the selected universe; shard/replica and `max_symbols_per_socket` sizing; Hyperliquid loopback wire mock in `tests/stress_support`; reuse the stress ledger, coverage hashes, and manifest schema. | Stress manifests per `docs/stress-lab-design.md` §§4–8 with exact coverage, full event accounting, backpressure and recovery sweeps, three repetitions; `docs/operating-envelope.md` updated; live tiers emitted only by a permitted live run. | Production certification, order-entry/execution guarantees, treating synthetic runs as live evidence. |

Cross-stage non-goals: no trading, positions, strategy or pre-trade risk; no change to the SBE
challenge schema; no silent rounding of precision; no claim that ADR-0001's instrument/listing
model is implemented; no new external dependency without the existing dependency-governance gate.

### 2.1 H1 implementation notes (Current)

- `HyperliquidSettings` carries `ws_url` and a telemetry subscription list, both with defaults
  so H0 configurations still deserialize. The session subscribes to exactly `allMids` and
  `fastAssetCtxs`; `assetCtxs` is rejected by the subscription builder because no golden fixture
  pins its wire shape (§6). Unknown additive wire fields are ignored; recognised numeric strings
  are parsed at SBE scale 6 or the whole frame is rejected.
- `src/exchange/hyperliquid/telemetry.rs` decodes frames into typed mids/context observations
  and returns no `ExchangeEvent`. `src/exchange/hyperliquid/session.rs` wraps each accepted
  payload as a raw journal data frame with `SessionSignal::ProtocolControl`, zero events, and
  `HeartbeatMechanism::Disabled`. Telemetry advances only the data receive fence; it never
  satisfies a heartbeat, anchors a book, or advances instrument activity.
- Fixtures: `fixtures/hyperliquid/all_mids{,_malformed}.json` and
  `fast_asset_ctxs{,_malformed}.json`. The malformed fixtures carry a value with seven
  fractional digits and are rejected, not rounded. Additive fields (including high-precision
  funding/open-interest fields) are present in the valid fixtures and ignored.
- Idle/quiet-feed behaviour is unchanged: liveness remains the worker's
  `connection_idle_timeout_secs` plus frame receipt. No latency, capacity or performance claim
  is made by H1.

## 3. Product-level completeness: what each feed can and cannot prove

The existing evidence tools answer different questions and must not be substituted for one
another. `coverage.rs` defines **anchored** as "has at least one snapshot in the capture";
`completeness.rs` requires that no delta precedes that baseline; `verify` checks delta→snapshot
reconstruction; the journal records what the wire actually said. A mid price is none of these.

- **`allMids` does not prove book coverage.** It is a venue-wide map of coin→mid price. It
  proves the channel is alive, that coin names are recognized, and that price strings parse. It
  does not prove we subscribed to `l2Book` for any coin, that a two-sided book exists, that any
  level has depth, or that a single symbol is anchored. It typically also names coins we
  deliberately exclude (other DEXs, non-canonical spot pairs), so counting mids as coverage
  would *overstate* the universe. Planned H1 rule: never emit `Snapshot` or `LevelChange` from
  `allMids`, and never let it advance a book symbol's activity.
- **Asset context does not prove depth.** `fastAssetCtxs`/`assetCtxs` carry per-coin statistics
  (mark/oracle prices, funding, open interest, volume, and similar fields — **verify at H0**)
  at high frequency. There are no price levels, no cumulative quantity, and no top-of-book
  size. A rapidly updating context is not evidence that a book is fresh, continuous, or even
  subscribed. Planned rule: context is telemetry; it must not satisfy a heartbeat, an anchor,
  or a stale-probe budget.
- **`l2Book` is the only in-scope Hyperliquid feed that can anchor a book.** It pushes complete
  two-sided level arrays (a snapshot feed, not a delta feed — **verify at H0**). Each accepted
  push maps to `ExchangeEvent::Snapshot`, which seeds/re-anchors continuity. Because there is
  no delta stream, there is no `LevelChange` from Hyperliquid books; continuity relies on the
  supervisor's receive fence plus the existing freshness policy, not on gap detection.
- **Trades are prints, not level changes.** Mapping a trade into `LevelChange` would corrupt
  book state and SBE output. Planned H2 rule: decode trades as telemetry or behind a separate,
  explicitly designed trade event; never synthesize book deltas from them.
- A credible claim therefore requires the conjunction: journal evidence of exact wire frames,
  `verify` for reconstruction, `completeness` for anchoring, `coverage` for the exact expected
  set, and recovery evidence for continuity across incidents.

## 4. Identity mapping (Planned; finalized at H0)

`native_symbol` is the key of `CollectorCore.markets` and the subscription unit; `symbol` is the
namespaced SBE serialization identity. Neither is an ADR-0001 instrument identity.

**4.1 Perpetual (DEX + coin).** Identity scope is `(hyperliquid, perp, dex, coin)`. The DEX is
part of the key because the same coin text may exist on more than one DEX; a bare `BTC` would
collide in the book map. Planned `native_symbol = "hyperliquid:perp:<dex>:<coin>"`. Set
`exchange_market_id = None` for perps (there is no parent grouping): `count_selected_markets`
then falls back to `native_symbol` and counts each perpetual once. Only explicitly configured
DEXs are discovered; an unconfigured DEX is out of universe, not silently added.

**4.2 Spot (base/quote).** Identity scope is `(hyperliquid, spot, base, quote)` with token
indices retained as mapping metadata. Wire coins may appear as `<BASE>/<QUOTE>` or as token
indices such as `@<n>` — **verify at H0** against `spotMeta`. Discovery must resolve token index
→ token name and normalize to base/quote. Planned `native_symbol = "hyperliquid:spot:<BASE>/<QUOTE>"`.
Where a base/quote pair has canonical and non-canonical variants, default to canonical only;
non-canonical pairs are excluded or suffixed with the index — never merged silently. `outcome`
is `None` (this is not a prediction market), and `exchange_market_id` is `None`.

**4.3 Precision and instrument specs.** Map `szDecimals` to `quantity_scale` plus `quantity_step`
(`10^-szDecimals`) and any published minimum to `lot_size`; map the venue price-tick rule to
`price_scale` and one or more `PriceBand`s in `NumericSpec.tick_schedule` (origin and phase
**verify at H0**). Set `price_unit = PriceUnit::Currency("USDC")` for perpetuals and the quote
currency for spot; choose `QuantityUnit::Contracts` versus `Currency(base)`/`Lots` in H0 and
freeze it. Every wire value is parsed with `fixed::parse_scaled` into `i64` and validated by
`NumericSpec::validate()` (scales ≤ `MAX_SCALE = 18`, lot a multiple of step). Output conversion
uses `util/numeric::{price_to_sbe,quantity_to_sbe}`; values not exactly representable at the
canonical `DECIMALS = 6` scale are **rejected with evidence**, never rounded. Perps whose tick
cannot be represented at SBE scale are excluded from H2 selection and recorded as a precision
non-goal.

**4.4 Output symbol compatibility.** Keep the `<namespace>:<...>` SBE format. Proposed
`symbol` mirrors `native_symbol`: `hyperliquid:perp:<dex>:<coin>` and
`hyperliquid:spot:<BASE>/<QUOTE>`. Bare coin text must not be reused: a multi-venue capture
would then be ambiguous. The output symbol remains a serialization mapping, not an instrument
identity (ADR-0001). Freeze the format in H0 — later changes invalidate catalog checksums and
break comparison against earlier evidence.

## 5. Capability declaration for Hyperliquid (Planned)

**Current:** `ExchangeCapabilities` has only `has_sequence_clock`, and `capabilities_for` is a
two-arm match. Hyperliquid's planned value is `false`: public feeds publish no cross-replica
comparable sequence number (a feed `time` field, **verify at H0**, is advisory and belongs in
`EventMeta.exchange_ts_us`, never in gap detection). The supervisor's
`(connection_epoch, receive_seq)` fence remains the ordering authority.

**Planned extension** (requires editing `exchange/mod.rs`, `capabilities_for`, core config and
their tests; not implemented by this document):

| Dimension | Declaration | Consequence |
|---|---|---|
| Ordering | Receive-fence only; advisory feed timestamp | No sequence-gap discontinuities from venue data; continuity from fences + freshness policy. |
| Snapshot support | Full venue-pushed snapshots on `l2Book`; no request/ack snapshot opcode (**verify at H0**) | `request_snapshot` maps to resubscribe/wait-for-next-push; `Control::Recover` still fences by receive_seq and re-anchors before deltas. |
| Heartbeat | `HeartbeatMechanism::Disabled` unless H0 fixtures prove an application ping/pong; native WS controls handled by `ws.rs`/`protocol.rs` | Liveness from frame receipt + `connection_idle_timeout_secs`; `heartbeat_disabled` prevents `RecoveryCause::HeartbeatFailure` from authorizing a reconnect. |
| Replay | Venue has no public history; journal schema v2 is the sole replay source | `replay` drives the same `CollectorCore` from journal/fixtures; no venue replay claim. |
| Backpressure | Unchanged bounded writer/journal | Frames are journaled before core dispatch; journal enqueue timeout fails the run, never silently drops. |
| Lifecycle plane | No separate lifecycle channel | `lifecycle_session_factory()` returns `None`; `capacity.rs` allocates zero lifecycle replicas; new instruments enter via REST discovery. |
| Subscription scope | Subscribe/unsubscribe by coin within one DEX scope | `apply_subscriptions` reconciles the exact desired set; DEX scoping per source is fixed in H0. |

## 6. Test strategy (H0-H1 implemented; H2-H3 planned)

- **Protocol fixtures.** `fixtures/hyperliquid/` mirroring the existing layout: `meta` for two
  DEXs with a shared coin name and `spotMeta` with index and pair forms (`reference.json`, H0),
  plus `all_mids`, `all_mids_malformed`, `fast_asset_ctxs`, and `fast_asset_ctxs_malformed` (H1).
  H1 fixtures include unknown additive fields (ignored, not rejected) and inexact numeric values
  (rejected, not rounded). Golden bytes are retained for journal replay. The H2 `l2_book`,
  `trades`, subscribe/unsubscribe acknowledgements and malformed-JSON fixtures remain planned.
- **Numeric precision.** Table-driven boundary values for price and size scales, `szDecimals`
  high-precision cases rejected at SBE scale 6, tick-grid and `NumericSpec::validate()` checks,
  and `price_to_sbe`→`price_from_sbe` round-trips. Negative prices remain valid in the model;
  HL-specific price domains are a fixture assertion, not a global rule.
- **Latency.** Record `EventMeta.received_at_us` and the feed timestamp as separate fields with
  an explicit clock type; percentiles over fixed one-second bins per `docs/stress-lab-design.md`
  §6. Latency is reported, never gated in CI.
- **Recovery.** `recovery::decide()` cases for transport/decode/protocol causes with heartbeat
  disabled; `RecoveryReady` acknowledgements enumerate exact symbols; stale generations and old
  epochs fence nothing; a reconnect drill proves fresh `l2Book` re-anchoring before promoted
  deltas.
- **Journal and replay.** H1: `tests/hyperliquid_telemetry.rs` drives the real worker with a v2
  journal and proves exact raw bytes, typed `ProtocolControl` observations, and zero book events
  (hence zero anchors). Planned: `journal verify`/`journal recover` on golden fixtures and the
  broader replay assertions; v1 readability remains untouched.
- **Stress-lab reuse.** Add an HL wire mock under `tests/stress_support/` per
  `docs/stress-lab-design.md` §9; reuse the event ledger, three-set coverage hashes, manifest
  schema, and failure classification. HL workload shapes: mid fan-out, fast-context rate,
  `l2Book` refresh cadence, and trade bursts. Synthetic results never satisfy a live tier.

## 7. Implemented / current versus planned

| Item | Status |
|---|---|
| `Exchange`, `SessionFactory`, `MarketDataSession`, `SessionFrame`, typed protocol/journal vocabulary, `ExchangeCapabilities` (one field), normalized `ExchangeEvent`, `NumericSpec`, exact numeric conversion, catalog, recovery policy, completeness/coverage/verify, stress lab | **Current** — in this tree, cited in §1. |
| Kalshi and Polymarket adapters behind those seams | **Current** — public prediction-market books only. |
| Hyperliquid H0 discovery, settings, adapter module, identity map, reference fixtures | **Current** — deterministic two-DEX `meta`/`spotMeta` discovery behind the existing seams. |
| Hyperliquid H1 telemetry session, codec, golden telemetry fixtures, zero-event journal evidence | **Current** — `allMids`/`fastAssetCtxs` only; `assetCtxs` not subscribed; no book events, anchors, heartbeat satisfaction, live capture, coverage or latency claim. |
| Hyperliquid H2 selected books/trades, H3 capacity expansion, capability breadth extension, stress mock, live evidence | **Planned** — H2–H3 above. None implemented. |
| ADR-0001 instrument/listing reference model | **Accepted design, not implemented**; this plan does not depend on it and does not claim it. |

## 8. Follow-on exchange template (Binance / Coinbase / Kraken / OKX — not implemented)

Reuse this order for each venue; do not implement several at once.

1. **Config and identity.** Add settings and a connector selector arm; decide the venue-native
   scope (product type, venue/DEX/market segment, symbol) that must be part of `native_symbol`.
   `ExchangeKind` is a connector selector, not a venue registry (ADR-0001); prefer a shared
   connector for venues with the same protocol instead of one variant per facility.
2. **Reference data.** REST discovery → `MarketInfo` + `NumericSpec` for the chosen products;
   define venue-specific identity rules (base/quote vs contract vs token), freeze `symbol`
   format, and validate counts against `DiscoveryStats`.
3. **Adapter module.** `src/exchange/<venue>/{mod,codec,discovery,session}.rs`; implement both
   session traits and the `Exchange` trait; no state-machine branch.
4. **Capability declaration.** Fill every dimension in §5: ordering (is there a cross-replica
   sequence or feed clock?), snapshots (request/ack vs streamed vs delta-only), heartbeat
   (text/native/disabled), replay, backpressure, lifecycle plane, subscription scope.
5. **Event mapping.** Only genuine full books become `Snapshot`; only genuine deltas become
   `LevelChange`; statistics/trades are telemetry until a separate normalized event is designed.
   Never manufacture depth.
6. **Evidence plumbing.** JSON protocol fixtures, exact numeric tests, journal v2 observations,
   replay golden cases.
7. **Containment tests.** Per-symbol coverage, completeness, verify, typed recovery drills, and
   a loopback wire mock in the stress lab with the shared ledger and manifest schema.
8. **Staged rollout.** Reference → stats → selected books → capacity, each with its own gate;
   live evidence only from a permitted connection, with access restrictions respected.

Venue-specific questions to answer before step 2 (not answered here): Binance — spot vs USDⓈ-M /
COIN-M futures, combined vs raw streams, book tick weights, update-speed cadence; Coinbase —
product/level versus advanced-trade feeds, sequence numbers, heartbeat channel; Kraken — asset
pairs versus instrument names, snapshot/delta semantics, sequence/heartbeat behavior; OKX —
spot/swap/futures `instType` scoping, books/ Books5 versus delta channels, checksum/sequence
support. No endpoint, symbol, or capability is asserted for these venues.

## 9. Key decisions

1. DEX and market type are part of Hyperliquid identity; bare coin text never keys a book.
2. H1 statistics are telemetry and must not anchor books or satisfy heartbeats/staleness.
3. Only full-book snapshots can anchor; trades never become book deltas.
4. Precision is exact or rejected at SBE scale 6; no rounding, with unsupported ticks excluded.
5. `has_sequence_clock` stays `false`; capability breadth is a separate, explicit planned change.
6. Each stage has a falsifiable evidence gate; no live or production claim from synthetic runs.
7. Venue additions are template-driven behind the existing seams; no state-machine changes.
