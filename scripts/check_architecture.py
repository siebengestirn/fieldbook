#!/usr/bin/env python3
"""Check architecture traceability without generated inventories.

Run from any working directory. --check is read-only and is used by CI. The
gate verifies source responsibility coverage, module reachability, runtime
integration anchors, falsifiable source anchors, required traceability text,
and the structural shape of the Mermaid diagrams. It is not a substitute for
Mermaid's syntax parser.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/architecture.md"

# Every tracked Rust source module must have an assigned responsibility.
SOURCE_RESPONSIBILITIES = {
    "src/cli.rs": "clap command model and CLI-to-settings overrides",
    "src/config.rs": "configuration defaults, deserialization, and validation",
    "src/completeness.rs": "per-symbol feed-completeness accounting for captures: anchored vs unanchored deltas, quiet intervals",
    "src/coverage.rs": "per-symbol live-coverage certification: expected vs acknowledged vs snapshot-anchored symbols",
    "src/coordinator.rs": "startup, orchestration loop, discovery reconciliation, controls, shutdown",
    "src/egress.rs": "egress policy: direct/proxy routing, fail-closed tunnelling, route provenance and redaction",
    "src/fixed.rs": "fixed-point price/quantity parsing and checked arithmetic",
    "src/histogram.rs": "fixed-bucket stage-latency histograms, nearest-rank percentiles and unavailable-boundary reporting",
    "src/inspect.rs": "offline SBE capture inspection command",
    "src/identity.rs": "persistent opaque instrument/venue/listing identities, effective-dated mappings and as-of resolution",
    "src/journal.rs": "raw-frame journal writer, segment/manifest format, verification, ordered reading",
    "src/journal/tests.rs": "journal unit tests for framing, checksums, durability, recovery, and failure handling",
    "src/latency.rs": "latency experiment orchestration and result output",
    "src/lib.rs": "library module exports and public entry points",
    "src/main.rs": "process entry point and collect/discover/replay/inspect/verify/completeness/coverage/latency dispatch",
    "src/metrics.rs": "metrics registry, Prometheus text encoding, HTTP health/metrics endpoint",
    "src/model.rs": "normalized market, source, event, control, and output domain types",
    "src/orderbook.rs": "normalized order-book representation and mutation invariants",
    "src/probe.rs": "network-probe command: stage-timed egress transport diagnostic and JSON report",
    "src/protocol.rs": "shared venue-neutral transport vocabulary: frame class/direction, control opcodes, session signals, heartbeat mechanism, send result, and transport observations",
    "src/recovery.rs": "typed recovery policy: explicit causes, authorized actions, and bounded attempt decisions",
    "src/replay.rs": "deterministic JSONL replay into the collector/output path",
    "src/report.rs": "market metadata and run-report sidecars; atomic JSON helper",
    "src/session.rs": "generic websocket session/factory contract and supervised worker lifecycle",
    "src/side.rs": "normalized buy/sell side semantics",
    "src/time.rs": "wall-clock and monotonic timing helpers",
    "src/verify.rs": "SBE delta-to-snapshot self-consistency verification",
    "src/writer.rs": "bounded writer queue, dedicated writer thread, SBE sink lifecycle",
    "src/ws.rs": "websocket connection options, source binding and connection experiments",
    "src/capacity.rs": "startup and fixed-topology discovery capacity admission",
    "src/catalog.rs": "versioned discovery checkpoint with integrity envelope, age, and counts",
    "src/persistence.rs": "latest-wins metadata/catalog background writer and lifecycle status",
    "src/products.rs": "typed data-product and source-capability contracts, deterministic subscription keys, and explicit subscription state transitions",
    "src/canonical.rs": "canonical event envelope and normalized event taxonomy with deterministic opaque ids, explicit provenance, schema-versioned fail-closed reading, and a conceptual normalized store contract with a test-only in-memory implementation",
    "src/application.rs": "generic non-market-data source/target reference application: SourceAdapter, JsonlSource with exact bytes/sequence/rejection evidence and duplicate detection, deterministic CanonicalBusinessEvent transform with separate source/receive time, six-category receipts and ExecutionMode, CommitSink seam, DeliveryObligation accounting, quarantine-before-progression runner, and side-effect-free replay connector",
    "src/application/postgres.rs": "parameterized PostgreSQL SQL contract, target-specific idempotency keys, BEGIN/INSERT/UPSERT/COMMIT transcript builder, SqlExecutor seam and deterministic in-memory simulator with idempotency by event id, transaction commit/rollback and unavailable/ack-lost fault injection; no driver or live connection",
    "src/application/file_source.rs": "bounded-memory BufRead line source with exact bytes, physical offsets, registered source identity, one-shot SHA-256 fingerprint, duplicate/schema/malformed evidence and a resume anchor bound to source identity, fingerprint and target",
    "src/application/durable.rs": "local append-only checksummed fsync-capable commit store built on atomic JSON and sha2, one checksummed record carrying ids, dispositions/quarantines and checkpoint, reopen verification with fail-closed truncation/corruption checks, fault injection and a streaming pipeline runner",
    "src/collector/mod.rs": "collector core module boundary",
    "src/collector/state.rs": "serialized per-symbol/source state machine, discovery and failover decisions",
    "src/collector/state/tests.rs": "collector state-machine tests for discovery, ordering, capabilities, snapshots, recovery, and readiness",
    "src/exchange/mod.rs": "exchange capabilities and adapter-independent session factory abstraction",
    "src/exchange/kalshi/auth.rs": "Kalshi request signing and auth header construction",
    "src/exchange/kalshi/codec.rs": "Kalshi wire-frame parsing and normalized events",
    "src/exchange/kalshi/discovery.rs": "Kalshi REST pagination, market eligibility and metadata conversion",
    "src/exchange/kalshi/discovery/tests.rs": "Kalshi REST discovery tests for pagination, eligibility, caps, retries, and duplicate detection",
    "src/exchange/kalshi/mod.rs": "Kalshi adapter construction and Exchange implementation",
    "src/exchange/kalshi/session.rs": "Kalshi websocket protocol, subscriptions, acknowledgements and sequence recovery",
    "src/exchange/polymarket/codec.rs": "Polymarket CLOB payload validation and normalized events",
    "src/exchange/polymarket/discovery.rs": "Gamma keyset pagination, version-aware identifiers and outcome expansion",
    "src/exchange/polymarket/mod.rs": "Polymarket adapter construction and Exchange implementation",
    "src/exchange/polymarket/session.rs": "public CLOB websocket protocol and subscription/resubscription behavior",
    "src/exchange/coinbase/mod.rs": "Coinbase H0 exchange composition, clockless capability declaration and offline reference-data tests",
    "src/exchange/coinbase/codec.rs": "Coinbase products to MarketInfo decoding with explicit spot identity, active filtering and exact increment-derived numeric specs",
    "src/exchange/coinbase/discovery.rs": "offline-fixture and REST products discovery with H0 page/record telemetry",
    "src/exchange/coinbase/session.rs": "H0 no-network session seam with disabled heartbeat and no book or telemetry feed",
    "src/exchange/hyperliquid/mod.rs": "Hyperliquid H0 exchange composition, H1 telemetry adapter wiring and H2 selected-book wiring",
    "src/exchange/hyperliquid/codec.rs": "Hyperliquid meta/spotMeta to MarketInfo decoding with exact numeric specs",
    "src/exchange/hyperliquid/discovery.rs": "offline-fixture and REST reference-data discovery with telemetry",
    "src/exchange/hyperliquid/books.rs": "H2 exact-decimal decoding of l2Book full snapshots into normalized snapshots and trades into telemetry, with exact selected-perpetual identity scoping and no LevelChange",
    "src/exchange/hyperliquid/telemetry.rs": "H1 exact-decimal decoding of allMids/fastAssetCtxs telemetry into typed observations, never book events",
    "src/exchange/hyperliquid/session.rs": "H1 telemetry and H2 selected-book websocket session: exact subscription reconciliation, raw-frame decoding, snapshot-only events, resubscribe snapshot requests and disabled heartbeat",
    "src/exchange/kraken/mod.rs": "Kraken H0 exchange composition, capability declaration and fixture-backed reference-data tests",
    "src/exchange/kraken/codec.rs": "Kraken AssetPairs to MarketInfo decoding with namespaced identity, exact numeric specs and active-only filter",
    "src/exchange/kraken/discovery.rs": "offline-fixture and REST AssetPairs reference-data discovery with telemetry and explicit pair selection",
    "src/exchange/kraken/session.rs": "H0 no-network session seam with disabled heartbeat",
    "src/exchange/okx/mod.rs": "OKX H0 exchange composition: identity, clockless capability declaration, offline discovery wiring and inert no-network session factory",
    "src/exchange/okx/codec.rs": "OKX instruments document to validated MarketInfo decoding: product-type scoping, explicit namespace identity, exact tickSz/lotSz scales, live-state filter and no silent rounding",
    "src/exchange/okx/discovery.rs": "offline OKX instruments reference-data discovery with reconciled page/record telemetry and fail-closed missing fixture",
    "src/exchange/okx/session.rs": "OKX H0 no-network session stub with disabled heartbeat that fails closed instead of opening a socket",
    "src/exchange/binance/mod.rs": "Binance H0 reference-data exchange composition, clockless capability declaration and no-network session factory wiring",
    "src/exchange/binance/codec.rs": "Binance exchangeInfo to MarketInfo decoding with explicit spot/perp namespace identity, canonical quote filter and exact price/lot steps",
    "src/exchange/binance/discovery.rs": "offline-fixture-only Binance reference-data discovery with page/record telemetry, finite caps and no network fallback",
    "src/exchange/binance/session.rs": "Binance H0 no-network session stub: disabled heartbeat, fatal-on-network operations and no subscriptions or lifecycle plane",
    "src/util/aligned_sbe_reader.rs": "aligned SBE capture reader",
    "src/util/aligned_sbe_writer.rs": "aligned SBE chunk writer and sink helpers",
    "src/util/build.rs": "build-time SBE DTO/codec generation integration",
    "src/util/dtos.rs": "generated domain DTO bridge for supplied SBE schema",
    "src/util/mod.rs": "SBE utility module exports",
    "src/util/numeric.rs": "exact instrument-scale to SBE numeric conversion",
    "src/util/orderbook_schema.rs": "schema-facing book message definitions/adaptation",
    "src/util/sbe_yaml.rs": "supplied SBE schema loading/validation support",
}

# Anchors that make the main structural claims falsifiable against code.
REQUIRED_ANCHORS = {
    "src/main.rs": [
        "Command::Collect",
        "Command::Replay",
        "Command::Inspect",
        "Command::Verify",
        "Command::Latency",
        "Command::Discover",
        "Command::Journal",
    ],
    "src/coordinator.rs": [
        "run_with_shutdown",
        "discover_with_retry",
        "process_worker_event",
        "dispatch_controls",
        "tokio::select!",
        "core.finalize",
        "writer.shutdown_with_stats",
    ],
    "src/capacity.rs": [
        "derive_shard_count",
        "pub fn plan",
        "CapacityPlan",
    ],
    "src/collector/state.rs": [
        "apply_discovery",
        "handle_event",
        "handle_snapshot",
        "handle_level_change",
        "handle_lifecycle",
        "handle_stream_reset",
        "handle_status",
        "pub fn tick",
        "pub fn finalize",
        "fn reseat",
    ],
    "src/session.rs": [
        "pub trait MarketDataSession",
        "pub trait SessionFactory",
        "pub async fn run_worker",
        "backoff_or_stop",
        "handle_control",
    ],
    "src/writer.rs": [
        "pub struct WriterHandle",
        "pub async fn send",
        "shutdown_inner",
        "blocking_recv",
        "sync_on_close",
    ],
    "src/exchange/kalshi/auth.rs": ["pub struct KalshiAuth", "pub fn headers"],
    "src/exchange/kalshi/discovery.rs": ["fetch_markets", "fetch_page"],
    "src/exchange/kalshi/session.rs": [
        "struct SequenceTracker",
        "on_snapshot",
        "on_delta",
    ],
    "src/exchange/polymarket/discovery.rs": [
        "fetch_markets",
        "fetch_page",
        "identifier_kind",
    ],
    "src/exchange/polymarket/codec.rs": [
        "price_change",
        "unknown_event_types_are_counted_but_not_fatal",
    ],
    "src/exchange/polymarket/session.rs": [
        "send_initial",
        "next_frame",
        "send_keepalive",
    ],
    "src/exchange/coinbase/mod.rs": ["CoinbaseExchange", "capabilities"],
    "src/exchange/coinbase/codec.rs": [
        "parse_reference_document",
        "parse_products",
    ],
    "src/exchange/coinbase/discovery.rs": ["discover_with_stats"],
    "src/exchange/coinbase/session.rs": ["CoinbaseSessionFactory"],
    "src/exchange/hyperliquid/mod.rs": ["HyperliquidExchange", "capabilities"],
    "src/exchange/hyperliquid/codec.rs": [
        "parse_reference_document",
        "parse_spot_meta",
    ],
    "src/exchange/hyperliquid/discovery.rs": ["discover_with_stats"],
    "src/exchange/hyperliquid/books.rs": [
        "CHANNEL_L2_BOOK",
        "CHANNEL_TRADES",
        "decode_l2_book",
        "decode_trades",
        "into_snapshot",
        "selected_symbols",
        "subscription_request",
    ],
    "src/exchange/hyperliquid/telemetry.rs": [
        "decode_telemetry",
        "CHANNEL_ALL_MIDS",
        "CHANNEL_FAST_ASSET_CTXS",
        "CHANNEL_ASSET_CTXS_NOT_SUBSCRIBED",
    ],
    "src/exchange/hyperliquid/session.rs": [
        "HyperliquidSessionFactory",
        "apply_subscriptions",
        "next_frame",
        "ProtocolControl",
        "decode_l2_book",
        "selected_books",
        "request_snapshot",
    ],
    "src/exchange/kraken/mod.rs": ["KrakenExchange", "capabilities"],
    "src/exchange/kraken/codec.rs": [
        "parse_asset_pairs",
        "decode_pair",
        "parse_reference_document",
    ],
    "src/exchange/kraken/discovery.rs": ["discover_with_stats"],
    "src/exchange/kraken/session.rs": ["KrakenSessionFactory"],
    "src/exchange/okx/mod.rs": [
        "OkxExchange",
        "capabilities",
        "pub const fn kind",
    ],
    "src/exchange/okx/codec.rs": [
        "parse_instruments_document",
        "tickSz",
        "lotSz",
        "state",
        "native_symbol",
    ],
    "src/exchange/okx/discovery.rs": ["discover_with_stats"],
    "src/exchange/okx/session.rs": [
        "OkxSessionFactory",
        "OkxNoNetworkSession",
        "fn heartbeat_spec",
        "Disabled",
    ],
    "src/exchange/binance/mod.rs": ["BinanceExchange", "capabilities", "kind"],
    "src/exchange/binance/codec.rs": [
        "parse_reference_document",
        "parse_section",
        "exact_step",
        "parse_decimal",
    ],
    "src/exchange/binance/discovery.rs": [
        "discover_with_stats",
        "reference_fixture",
        "network discovery is not implemented",
    ],
    "src/exchange/binance/session.rs": [
        "BinanceSessionFactory",
        "BinanceSession",
        "heartbeat_spec",
        "HeartbeatSpec::disabled",
    ],
    "src/verify.rs": ["pub fn run"],
    "src/identity.rs": [
        "pub struct InstrumentId",
        "pub struct ReferenceCatalog",
        "pub fn venue_mappings_from_markets",
    ],
    "src/products.rs": [
        "pub enum DataProduct",
        "pub enum ProductKind",
        "pub struct SubscriptionSet",
        "pub struct SourceCapabilities",
        "pub fn from_legacy",
        "pub enum SubscriptionState",
        "pub struct SubscriptionLease",
    ],
    "src/canonical.rs": [
        "pub const CANONICAL_SCHEMA_VERSION",
        "pub struct EventId",
        "pub struct SequenceMetadata",
        "pub enum CanonicalKind",
        "pub enum CanonicalEvent",
        "pub struct CanonicalEnvelope",
        "pub struct NormalizationContext",
        "pub struct CanonicalReader",
        "pub trait NormalizedStore",
        "pub struct InMemoryNormalizedStore",
    ],
    "src/exchange/mod.rs": [
        "pub fn capabilities_for",
        "pub fn source_capabilities_for",
        "fn source_capabilities(&self) -> SourceCapabilities",
    ],
    "src/journal.rs": [
        "pub struct JournalHandle",
        "pub fn verify_run",
        "pub fn read_records",
        "pub fn read_observations",
        "struct Manifest",
        "struct SegmentFooter",
        "RunState::Finalized",
    ],
    "src/protocol.rs": [
        "pub enum SessionSignal",
        "pub enum FrameClassification",
        "pub struct TransportObservation",
    ],
    "src/recovery.rs": [
        "pub enum RecoveryAction",
        "pub enum RecoveryCause",
        "pub fn decide",
        "pub struct RecoveryTarget",
        "pub struct RecoveryFence",
        "pub struct InstrumentRecoveryQueue",
        "pub struct SourceRecoveryQueue",
        "pub struct RecoveryCoordinator",
    ],
    "src/util/build.rs": ["generate_str_to_out_dir", "GenerationConfig"],
    "src/application.rs": [
        "pub trait SourceAdapter",
        "pub trait TargetConnector",
        "pub struct JsonlSource",
        "pub struct RawObservation",
        "pub enum SourceError",
        "pub struct CanonicalBusinessEvent",
        "pub enum DeliveryReceipt",
        "pub struct DeliveryObligation",
        "pub enum DeliveryState",
        "pub struct Checkpoint",
        "pub struct ImportReport",
        "pub struct ReplayConnector",
        "pub fn run_jsonl_to_target",
        "pub fn transform_observation",
        "pub mod postgres",
        "pub mod durable",
        "pub mod file_source",
        "pub enum ExecutionMode",
        "pub enum ReceiptCategory",
        "pub trait CommitSink",
        "pub fn run_jsonl_to_target_with_mode",
    ],
    "src/application/file_source.rs": [
        "pub struct SourceFingerprint",
        "pub struct SourceIdentity",
        "pub struct SourceRegistry",
        "pub struct CheckpointBinding",
        "pub struct DurableCheckpoint",
        "pub struct FileSource",
        "pub enum DuplicatePolicy",
        "pub const MAX_LINE_BYTES",
        "impl<R: BufRead> SourceAdapter for FileSource<R>",
    ],
    "src/application/durable.rs": [
        "pub struct CommitRecord",
        "pub struct CommitEnvelope",
        "pub struct CommitStore",
        "pub enum StoreFault",
        "pub struct DurableState",
        "pub fn open",
        "pub fn commit",
        "pub fn load_state",
        "pub fn run_streaming_pipeline",
        "impl CommitSink for CommitStore",
    ],
    "src/application/postgres.rs": [
        "pub const INSERT_SQL",
        "pub struct ParameterizedStatement",
        "pub enum SqlValue",
        "pub fn generate_insert_statement",
        "pub fn generate_batch_statements",
        "pub enum PostgresFault",
        "pub struct PostgresSimulator",
        "pub struct PostgresTarget",
        "impl TargetConnector for PostgresSimulator",
        "impl TargetConnector for PostgresTarget",
    ],
}

# The P5 reference application must stay independent of market-data modules.
# These tokens are checked only against the two new source files.
REFERENCE_APP_SOURCES = (
    "src/application.rs",
    "src/application/postgres.rs",
    "src/application/file_source.rs",
    "src/application/durable.rs",
)
REFERENCE_APP_FORBIDDEN_TOKENS = (
    "crate::model",
    "crate::coordinator",
    "crate::collector",
    "crate::exchange",
    "crate::journal",
    "crate::replay",
    "crate::catalog",
    "crate::orderbook",
    "crate::writer",
    "crate::products",
    "ExchangeEvent",
    "MarketInfo",
    "MarketDataSession",
    "BookSnapshot",
    "LevelChange",
    "OrderBook",
)

MERMAID_RE = re.compile(r"```mermaid\s*\n(.*?)\n```", re.DOTALL)
NODE_RE = re.compile(
    r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:\[[^\n]*\]|\([^\n]*\)|\{[^\n]*\})\s*$"
)
EDGE_RE = re.compile(
    r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s+-->\s+([A-Za-z_][A-Za-z0-9_]*)\s*$"
)


MODULE_DECL_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;\s*(?://.*)?$",
    re.MULTILINE,
)


def collect_module_reachability(root: Path = ROOT) -> set[str]:
    """Return source files reachable through `mod foo;` from lib/bin crate roots.

    This is a conservative source-tree check, not a Rust parser. Inline modules
    are not treated as separate files.
    """
    # `src/util/build.rs` is Cargo's configured build-script crate root (`build = ...` in Cargo.toml).
    roots = [
        root / "src/lib.rs",
        root / "src/main.rs",
        root / "src/util/build.rs",
    ]
    reachable: set[str] = set()
    pending = [path for path in roots if path.exists()]
    while pending:
        current = pending.pop()
        rel = current.relative_to(root).as_posix()
        if rel in reachable:
            continue
        reachable.add(rel)
        content = current.read_text(encoding="utf-8")
        if current.name in {"lib.rs", "main.rs", "build.rs", "mod.rs"}:
            module_dir = current.parent
        else:
            module_dir = current.parent / current.stem
        for match in MODULE_DECL_RE.finditer(content):
            name = match.group(1)
            candidates = [
                module_dir / f"{name}.rs",
                module_dir / name / "mod.rs",
            ]
            found = [
                candidate for candidate in candidates if candidate.exists()
            ]
            if len(found) == 1:
                pending.append(found[0])
    return reachable


def module_integration_errors(root: Path = ROOT) -> list[str]:
    """Check falsifiable runtime call-site integration, not stale verdicts."""
    errors: list[str] = []

    # Check active source files reference missing dependencies
    config = (root / "src/config.rs").read_text(encoding="utf-8")
    capacity_path = root / "src/capacity.rs"
    if capacity_path.exists():
        capacity = capacity_path.read_text(encoding="utf-8")
        collector_struct = re.search(
            r"pub struct CollectorSettings\s*\{(.*?)\n\}", config, re.DOTALL
        )
        declared = (
            set(re.findall(r"pub\s+(\w+)\s*:", collector_struct.group(1)))
            if collector_struct
            else set()
        )
        referenced = set(re.findall(r"settings\.collector\.(\w+)", capacity))
        for field in sorted(referenced - declared):
            errors.append(
                f"src/capacity.rs references CollectorSettings.{field}, absent from src/config.rs"
            )
    catalog_path = root / "src/catalog.rs"
    if catalog_path.exists() and "util::sha256_hex" in catalog_path.read_text(
        encoding="utf-8"
    ):
        util_text = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (root / "src/util").rglob("*.rs")
        )
        if not re.search(r"\bfn\s+sha256_hex\b", util_text):
            errors.append(
                "src/catalog.rs references util::sha256_hex, but no such function exists under src/util/"
            )

    coordinator = (root / "src/coordinator.rs").read_text(encoding="utf-8")
    required = {
        "capacity startup": "capacity::plan(&settings, &initial_markets)",
        "capacity refresh admission": "capacity::plan_fixed_topology(&settings, &markets, &loads)",
        "catalog loading": "Catalog::load",
        "persistence startup": "PersistenceHandle::start(",
        "persistence submit": "persistence.submit(",
        "persistence final drain": "persistence.submit_final(",
        "persistence shutdown": "persistence.shutdown(send_timeout)",
    }
    for name, anchor in required.items():
        if anchor not in coordinator:
            errors.append(
                f"runtime integration missing: {name} anchor `{anchor}`"
            )
    return errors


def reference_app_independence_errors(root: Path = ROOT) -> list[str]:
    """Fail if the P5 reference app imports market-data modules or concepts."""
    errors: list[str] = []
    for rel in REFERENCE_APP_SOURCES:
        path = root / rel
        if not path.exists():
            errors.append(f"P5 reference source is missing: {rel}")
            continue
        source = path.read_text(encoding="utf-8")
        for token in REFERENCE_APP_FORBIDDEN_TOKENS:
            if token in source:
                errors.append(
                    f"{rel} contains market-data dependency token {token!r}"
                )
    return errors


def collect_sources() -> list[str]:
    """Exercise collect sources."""
    return sorted(
        p.relative_to(ROOT).as_posix() for p in (ROOT / "src").rglob("*.rs")
    )


def check_mermaid_structure(document: str) -> list[str]:
    """Check graph structure conservatively; GitHub Actions runs the full Mermaid parser."""
    errors: list[str] = []
    blocks = MERMAID_RE.findall(document)
    if len(blocks) < 3:
        errors.append(
            f"expected at least 3 Mermaid diagrams, found {len(blocks)}"
        )
    # Mermaid edge variants used here: --> and -->|edge label|. Keep each edge on one line.
    arrow_re = re.compile(r"-->(?:\|[^|]+\|)?")
    side_re = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*)(.*)$")
    shaped_re = re.compile(r"^\s*(?:\[[^\n]*\]|\([^\n]*\)|\{[^\n]*\})")
    for index, body in enumerate(blocks, start=1):
        lines = body.splitlines()
        if not lines or not re.match(
            r"^\s*flowchart\s+(?:LR|RL|TB|TD|BT)\s*$", lines[0]
        ):
            errors.append(
                f"diagram {index}: first line must be a supported flowchart direction"
            )
            continue
        declared: set[str] = set()
        edges: list[tuple[str, str, int]] = []
        for line_number, line in enumerate(lines[1:], start=2):
            stripped = line.strip()
            if (
                not stripped
                or stripped.startswith(("%%", "subgraph "))
                or stripped == "end"
            ):
                continue
            if arrow_re.search(line):
                parts = arrow_re.split(line, maxsplit=1)
                if len(parts) != 2:
                    errors.append(
                        f"diagram {index}, line {line_number}: malformed edge: {stripped}"
                    )
                    continue
                lhs, rhs = parts
                left = side_re.match(lhs)
                right = side_re.match(rhs)
                if not left or not right:
                    errors.append(
                        f"diagram {index}, line {line_number}: cannot parse edge endpoints: {stripped}"
                    )
                    continue
                source_id, source_tail = left.group(1), left.group(2)
                target_id, target_tail = right.group(1), right.group(2)
                if shaped_re.match(source_tail):
                    declared.add(source_id)
                if shaped_re.match(target_tail):
                    declared.add(target_id)
                # A bare endpoint must have been declared on another line.
                edges.append((source_id, target_id, line_number))
                continue
            node = NODE_RE.match(line)
            if node:
                declared.add(node.group(1))
                continue
            errors.append(
                f"diagram {index}, line {line_number}: unsupported/unparsed Mermaid statement: {stripped}"
            )
        for source, target, line_number in edges:
            for node_id in (source, target):
                if node_id not in declared:
                    errors.append(
                        f"diagram {index}, line {line_number}: edge references undeclared node {node_id}"
                    )
    return errors


def main() -> int:
    """Exercise main."""
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="check architecture traceability (read-only; used by CI)",
    )
    args = parser.parse_args()
    if not args.check:
        parser.error(
            "this repository only supports read-only architecture checks; use --check"
        )

    errors: list[str] = []
    errors.extend(module_integration_errors())
    errors.extend(reference_app_independence_errors())
    actual_sources = set(collect_sources())
    documented_sources = set(SOURCE_RESPONSIBILITIES)
    missing_docs = sorted(actual_sources - documented_sources)
    stale_docs = sorted(documented_sources - actual_sources)
    reachable = collect_module_reachability()
    unreachable = sorted(actual_sources - reachable)
    if missing_docs:
        errors.append(
            "source files need reviewed responsibility mappings: "
            + ", ".join(missing_docs)
        )
    if stale_docs:
        errors.append(
            "source responsibility mappings point to removed files: "
            + ", ".join(stale_docs)
        )
    if unreachable:
        errors.append("unreachable source files: " + ", ".join(unreachable))
    if not DOC.exists():
        errors.append(f"missing architecture document: {DOC.relative_to(ROOT)}")
        document = ""
    else:
        document = DOC.read_text(encoding="utf-8")
    for anchor_path, anchors in REQUIRED_ANCHORS.items():
        file_path = ROOT / anchor_path
        if not file_path.exists():
            errors.append(
                f"required source anchor file is missing: {anchor_path}"
            )
            continue
        source = file_path.read_text(encoding="utf-8")
        for anchor in anchors:
            if anchor not in source:
                errors.append(
                    f"source anchor `{anchor}` not found in `{anchor_path}`"
                )
    for required_text in [
        "docs/requirement-test-matrix.md",
        "docs/canonical-event-model.md",
        "docs/jsonl-postgres-reference.md",
        "Current operational caveat",
        "not a production certification",
    ]:
        if required_text not in document:
            errors.append(
                f"architecture document is missing required traceability text: {required_text!r}"
            )
    errors.extend(check_mermaid_structure(document))

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    reachable = collect_module_reachability()
    print(
        f"Architecture check passed: {len(actual_sources)} Rust source files, "
        f"{len(reachable)} files reachable from crate roots, and "
        f"{len(MERMAID_RE.findall(document))} structurally checked Mermaid diagrams."
    )
    print(
        "Note: actual Mermaid grammar is parsed in the separate GitHub Actions Mermaid-validation job."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
