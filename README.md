# Cross-Venue Market Data Infrastructure

A Rust market-data platform for the ingestion, normalization, validation,
journaling, replay, and analysis of financial-market data across asset classes
and trading venues. The current implementation ingests live prediction-market
order books from Kalshi and Polymarket, maintains one normalized book per
outcome symbol, writes the supplied FIX SBE schema, and can record the exact
public websocket frames. The two adapters share the same core.

This is a market-data system, not a trading system. It does not submit orders,
hold positions, or run strategy or pre-trade risk. The Kalshi adapter has an
opt-in authenticated live test, but no live Kalshi run is claimed until a
permitted production connection produces a verified capture. Polymarket
full-universe discovery is verified, but simultaneous subscription at that
fan-out is not claimed.

## Integrated foundation: feed continuity, recovery and multi-venue evidence

A preserved 60-minute full-universe Polymarket run
([evidence](measurements/live/2026-11-14-universe-capture-failed/)) showed a
completed process — zero writer errors, a finalized journal — that was **not a
correct dataset**: 12.34% of snapshots failed SBE reconstruction after 756
websocket connection errors drove 10 million failover snapshots. The
integrated continuity and recovery work (merged into `main`) enforces one
distinction throughout the collector:

> An instrument's lack of updates must not be interpreted as evidence that its
> WebSocket connection is unhealthy. A connection's health must not be
> interpreted as proof that every instrument's book is continuous.

Three state domains are kept independent — connection health (per source,
including typed heartbeat acknowledgements: application-text for Polymarket,
native WebSocket control frames for Kalshi), instrument activity (per
instrument), and book continuity (per instrument+source, with recovery fencing).
Exhausting a symbol's stale-probe budget records a symbol continuity incident
and requests a symbol-scoped refresh; it does not reconnect the shared source.
Source-level reconnects remain a separate transport/heartbeat or explicit
recovery decision.

Recovery decisions are now typed (`RecoveryAction`, `RecoveryCause`) and
bounded by an explicit attempt policy. Instrument-scoped refreshes and
source-scoped reconnects are owned by independent bounded queues keyed by
exact `(source, instrument, generation)` and `(source, generation)` targets.
Recovery acknowledgements enumerate the instruments they cover and are fenced
to those exact targets; stale generations, old connection epochs, and unknown
targets fence nothing. Milestone and evidence status is in
[`docs/programme-status.md`](docs/programme-status.md).
Kalshi's margin (perps) API is a separate surface with its own wire contract
and is not covered by these adapters.

The current code supports prediction markets only. The target domain, adapter
boundaries, numeric model, event model, and compatibility rules are in the
[platform charter](docs/platform-charter.md). That document describes design
intent; it is not a claim that those capabilities are implemented.

## Review path

[Programme status](docs/programme-status.md) — what is implemented, tested and certified.
[Architecture](docs/architecture.md) — current topology and failure model.
[Requirement-to-test matrix](docs/requirement-test-matrix.md) — test evidence per requirement.
[Programme status](docs/programme-status.md) — current gate and certification status.
[Design decisions](docs/design-decisions.md) — why the boundaries exist.
[ADR-0002](docs/adr/0002-persist-transport-observations-without-changing-receive-order.md) — journal observation contract.
[Hyperliquid integration](docs/hyperliquid-integration-plan.md) — reference venue design; H0-H3 are synthetic-loopback evidence only.
[JSONL-to-target reference](docs/jsonl-postgres-reference.md) — non-market-data source/target pipeline.

Format contracts: [replay format](docs/replay-format.md),
[raw-frame journal](docs/raw-frame-journal.md),
[event accounting](docs/event-accounting-contract.md),
[durability](docs/durability-contract.md), and
[operating envelope](docs/operating-envelope.md).

## What it does

- Discovers live markets continuously and adds or removes symbols without a
  restart.
- Keeps a book per symbol, applies snapshots and deltas, and rejects invalid
  states.
Writes a full snapshot every 60 seconds and on failover, recovery, removal,
  and shutdown; writes deltas as they arrive.
Assigns symbols to fixed shards and opens `replicas` sockets per shard. A
  backup is promoted only when its book is valid and fresh, and promotion is
  anchored by a full snapshot.
Handles SIGINT and SIGTERM: stops discovery, emits final snapshots, closes
  websockets, flushes and syncs output, and writes sidecars.
Exposes Prometheus text metrics, `/healthz`, and `/readyz` when configured.
Provides deterministic replay, offline SBE inspect and verify, and an optional
  checksummed raw-frame journal with `journal verify` and `journal recover`.
Egress is direct by default or one explicitly configured proxy (HTTP
  CONNECT or SOCKS5), fail-closed with no silent fallback, sharing one policy
  between REST discovery and websockets. Route provenance — mode, kind, host,
  port, DNS mode, fingerprint — is recorded in the journal manifest; the
  `network-probe` subcommand reports the path stage by stage as JSON.

## Quick start (Polymarket, no credentials)

```bash
cargo run --release -- --exchange polymarket --max-markets 20 \
  --output captures/polymarket.sbe \
  --metadata captures/polymarket-markets.json \
  --report captures/polymarket-report.json \
  collect
```

Add `--journal-dir captures/journal` to record raw frames, then verify:

```bash
cargo run --release -- journal verify captures/journal/<run-id>
```

The journal writes schema v2: application data frames and native WebSocket
Ping/Pong controls are retained as typed observations with independent
`receive_seq`, `control_seq`, and `observation_seq` domains per source and
connection epoch. `journal::read_records()` remains the data-only compatibility
API; `journal::read_observations()` returns the complete data/control stream.
v1 journals remain readable, and controls never become market-data events.

Other commands: `replay`, `inspect`, `verify`, `latency`, `discover`, `journal`,
`network-probe`, `completeness`, `coverage`.
See `config.example.toml` and `config.polymarket.example.toml`.

## Build and test

Rust is pinned in `rust-toolchain.toml` (1.99.1); Python formatters and linters
are pinned in `requirements-dev.txt`. Install the latter once with `make tools`
before running the full gate. The one-command pre-commit gate is the polish
script — it certifies the guard scripts themselves, checks every language,
runs the CI-parity core, verifies tree stability, and can repeat the whole
gate (`--loop N`) to surface flaky behavior:

```bash
make tools                          # install pinned Pyink, Ruff and ShellCheck
scripts/assurance/polish.sh         # full gate, once
scripts/assurance/polish.sh --loop 2 # twice: a gate that only passes sometimes is a failing gate
```

`Makefile` is a thin facade over `cargo xtask`, which owns the gate
definitions and writes a machine-readable report to `target/xtask/last-run.json`.
CI installs the same pinned tools and runs the same commands.

```bash
cargo xtask style     # Python, shell, Makefile, JSON and Markdown
cargo xtask style-fix # apply safe Pyink and Markdown fixes
cargo xtask fmt       # rustfmt check
cargo xtask lint      # clippy -D warnings
cargo xtask test      # all-target and doc tests
cargo xtask docs      # rustdoc with broken links rejected
cargo xtask verify    # matrix, architecture and generated-path gates
cargo xtask e2e       # curated end-to-end suites (no live venues)
cargo xtask e2e --list-ignored       # opt-in tests and why they are skipped
cargo xtask e2e --run-ignored <name> # run one registered opt-in test
cargo xtask check     # style, Rust gates, checks and e2e
cargo xtask ci        # the full gate, including release build
cargo xtask fixtures  # verify checked-in journal fixtures
```

The complete cross-language contract is in [`docs/style-guide.md`](docs/style-guide.md).
Rust follows the [Linux kernel Rust guidelines](https://docs.kernel.org/rust/coding-guidelines.html)
and the [Google Rust style guide](https://google.github.io/styleguide/rustguide.html);
Python follows [Google's Python guide](https://google.github.io/styleguide/pyguide.html)
via Pyink and Ruff; shell follows [Google's shell guide](https://google.github.io/styleguide/shellguide.html)
via ShellCheck; Markdown and JSON follow their Google guides. All checks are
fail-closed: there are no warning-level suppressions. The only documented
exceptions are long string constants in three named Python files and
byte-exact immutable evidence, both recorded in the style guide.

Dependency maintenance uses the same gate: `scripts/assurance/update.sh`
refreshes the lockfile and keeps it only if the full polish gate passes,
restoring `Cargo.lock` otherwise. `scripts/assurance/upgrade.sh` moves the
pinned Rust toolchain (in lockstep with the CI workflow) and the lockfile
through the gate, disarming its restore trap only after the gate passes.
`main` accepts only reviewed PR merge commits — the policy and its conditions
are in `git-policy.json`.

Live venue tests are opt-in and must run from an operator-managed machine in a location where access is permitted. Kalshi requires authenticated read-only API credentials and an explicit permission check. Do not use a VPN, proxy, or hosted runner to bypass geographic restrictions, and do not treat a venue's published blocklist as proof that access is permitted. See the [live evidence runbook](docs/live-evidence-runbook.md).

To produce actual SBE and raw-frame evidence for both venues from the local
machine:

```bash
export POLYMARKET_MARKET_DATA_ACCESS_CONFIRMED=1
export KALSHI_READ_ONLY_ACCESS_CONFIRMED=1
export KALSHI_API_KEY_ID='…'
export KALSHI_PRIVATE_KEY_PATH="$HOME/.config/kalshi/private-key.pem"
LIVE_DURATION_SECS=120 LIVE_MAX_MARKETS=5 scripts/run-live-evidence.sh
```

The script refuses to claim a dual-venue success if either venue is skipped or a capture/journal verification fails. It certifies per-symbol coverage: every expected symbol in `markets.json` must appear in the run report's acknowledged subscription set and must have its own snapshot in the capture. A five-market run emits `LIVE_SMOKE_OK`, never `LIVE_UNIVERSE_OK`. To claim the universe tier, run `LIVE_EVIDENCE_SCOPE=universe LIVE_MAX_MARKETS=0 LIVE_DURATION_SECS=900 scripts/run-live-evidence.sh` — pass `LIVE_CONFIG=config.live-universe.toml` for the reviewed full-universe capacity envelope (capacity planning uses 548,942 discovered outcome tokens; the failed 2026-11-14 live capture expected 385,604 symbols. These are different runs and different evidence stages, not interchangeable counts; the default envelope is refused fail-closed by the capacity planner). The script then requires the expected set to equal the full discovered universe. `LIVE_SOAK_OK` is reserved for a full-universe run held for the stated soak duration and is not emitted by this script. The manual live workflow runs the same script on the protected runner and uploads the complete `captures/` bundle as a retained artifact.

Evidence discipline: a failed run is preserved as a failure — raw captures are
deleted only after hashing, with the hashes, reduced reports and logs checked
in under `measurements/live/` — and no acceptance criterion is relaxed to
obtain a success. Requested evidence tiers and achieved outcomes are recorded
separately.

## Scope and provenance

`sbe_schemas/` is the supplied challenge schema. `src/util/build.rs` generates
the SBE bridge from it. The aligned reader and writer helpers are
challenge-compatible implementations; changes require template-reader
compatibility tests. The rest of `src/` is the project implementation.
Dependency licenses and the pinned graph are in `Cargo.lock`.

Measured limits, including the 100,000-symbol scale run, are in
[`docs/operating-envelope.md`](docs/operating-envelope.md). This is **not a production certification** for order entry or execution.
