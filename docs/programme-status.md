# Programme status

This is the definitive status record for the engineering programme. Other
documents may describe design, history or intent; when a status statement here
differs from another document, this file is authoritative.

Status vocabulary is deliberately multi-dimensional:

| Column | Meaning |
|---|---|
| Code | Implementation exists in source |
| CI | Passes the repository's required checks on the integrated head |
| Opt-in | A conditional test/measurement was executed by an operator |
| Evidence | A manifest/artifact is retained in this repository with verified hash |
| Live | Validated against a permitted live venue |
| Perf | Throughput/latency/resource claim with reproducible measurements |

`N/A` means the dimension does not apply. `No` means it was not done — never
read `No` as a defect when the milestone did not claim that dimension.

## Integration history (exact commits)

| Commit | Integration |
|---|---|
| `d54aa08` | PR #45 — S1a/S1b, recovery M3/M4 foundation merged |
| `9ba0141` | PR #47 — M5.1 mixed workload evidence |
| `b885644` | PR #48 — M5.1-CERT accounting hardening |
| `aa52203` | PR #49 — CERT.1 event disposition closure |
| `27f8506` | PR #50 — M5.2 burst workloads |
| `14b8a94` | PR #51 — M5.5 backpressure |
| `65c2de1` | PR #52 — platform governance documents |
| `1aff42f` | PR #53 — Hyperliquid integration plan |
| `fdac36d` | PR #54 — M5.4 disconnect sweeps |
| `83c7251` | PR #55 — M5.3 lifecycle churn |
| `0137b95` | PR #56 — M5.6 stage instrumentation |
| `0efc74e` | PR #57 — M5.7 capacity stages |
| `8538cab` | PR #58 — Hyperliquid H0 reference data |
| `4f7ccb0` | PR #59 — Hyperliquid H1 telemetry |
| `d25c784` | PR #60 — Hyperliquid H2 books |
| `372da5b` | PR #61 — Hyperliquid H3 loopback harness |
| `b0885d9` | PR #62 — Coinbase H0 |
| `4088baa` | PR #63 — Kraken H0 |
| `4a68c64` | PR #64 — OKX H0 |
| `1488412` | PR #65 — Binance H0 |
| `9135a11` | PR #66 — GOV-002 programme status reconciliation |
| `432cd32` | PR #67 — Platform M2 persistent identity |
| `69f87c9` | PR #68 — Platform M3 typed data products |
| `a9a6b23` | PR #69 — Platform M4 canonical events |
| `127e926` | PR #70 — P5 JSONL-to-target reference application |
| `339b4ad` | PR #71 — final status reconciliation |
| `56d5960` | PR #72 — comment audit and duplicate roadmap removal |
| `0c669a2` | PR #74 — P6 durable file-to-target pipeline |
| `d856ee7` | PR #76 — xtask, Rust style guides and assurance hardening |
| `e647667` | PR #77 — real Hyperliquid wire decoding and live certification |
| `23fe17c` | PR #78 — live Hyperliquid reconnect recovery drill |
| `1445068` | PR #79 — Google/Linux style gates for all languages |

PR #45 is **MERGED** (`d54aa08`); its earlier draft description was obsolete and
GitHub metadata now agrees with the integrated history. It was never merged
twice.

## Previous source export (2026-11-18)

The preceding source export identified `main` at
`e9433fd8ab84c6879399d8449bc8e8093de40d41`, following PR #81 (architecture
anchor masking and polish argument validation) and PR #82 (the bounded D10
reconnect-drain deadline correction). Its verification record reported
`cargo xtask ci` and `scripts/assurance/polish.sh` passing, 850 Rust tests and
37 Python tests passing, and four consecutive locked full-suite runs passing
before the PR #82 merge. Hosted run identifiers were not included.

## Subsequent integrations (operator-reported)

The operator reports that PRs #83, #84 and #85 were merged in order: #83 adds
the disk-backed exact duplicate index and visible xtask failure output; #84
adds the synchronous PostgreSQL target, migrations and opt-in live-server test;
PR #85 adds the independent loopback TCP source and reconnect/accounting tests.
The operator reports `cargo xtask ci` and `scripts/assurance/polish.sh` passing
on `main` at `b3c45c4a1da1cfc6e0d04682468a2e3fbabeb404`, with a clean, pushed
worktree.

The accompanying `engineering-challenge-template-ac33b81.zip` contains the
implementation and tests for these changes but has no `.git` metadata. The
archive cannot independently establish that it is byte-for-byte the reported
`b3c45c4` checkout, recover PR merge commits, verify remote branch cleanliness,
or reproduce the operator's PostgreSQL run. Treat those points as
operator-reported until checked in the canonical Git checkout and evidence
logs. The opt-in database test has code in the archive; no database transcript
or server manifest is retained here.

## Current head (7edfb92)

PR #87 (`15d2993`) applied the reviewed PostgreSQL checkpoint-integrity
hardening: the checksum binds target, position and the last idempotency key;
competing writers cannot regress a checkpoint; and an event key reused with
changed canonical content fails closed. PR #88 (`7edfb92`) bounded the loopback
mock-server socket waits so a loaded host cannot turn a healthy read into an
uncertain receipt. Both PRs were merged after `cargo xtask ci` and
`scripts/assurance/polish.sh --loop 2` passed; the live PostgreSQL test passed
against a real PostgreSQL 18.6 server.

Two retained evidence records are deliberately distinguished:

- `measurements/live/2026-11-18-postgres-checkpoint-integrity/` records the
  PASS on the earlier source commit `ac33b81`.
- `measurements/live/2026-11-18-postgres-checkpoint-integrity-head-7edfb92/`
  records the exact command, output and SHA-256 of the rerun from clean
  `main` at `7edfb92`.

The earlier record does not prove the later head passed; the second record
does. Neither is a production certification.

## Source-scoped durable delivery (implementation under review)

The PostgreSQL target now namespaces checkpoints by
`(target_id, source_id, output_contract_version)`. Event and disposition rows
carry source identity, the checkpoint checksum binds all identity components,
and `PgTarget::with_source` rejects an envelope from a different source.
Legacy rows stay in an explicit empty-source namespace and are moved only by
the operator-controlled `assign_legacy_checkpoint_source` procedure. The
opt-in live test exercises two sources on one physical target, stale writers,
exact retries, changed payloads, a failed checkpoint race, explicit legacy
mapping and concurrent writers. Command, output and hashes are retained in
`measurements/live/2026-11-18-postgres-source-scoped/`. This is an implemented
and live-tested integration, not a production certification or a throughput
claim.

## Milestone status

| Milestone | Code | CI | Opt-in | Evidence | Live | Perf |
|---|---|---|---|---|---|---|
| S1a/S1b journal v2 | Yes | Yes | N/A | Yes (v1/v2 golden fixtures) | N/A | N/A |
| Recovery M3/M4 | Yes | Yes | N/A | Yes (blast-radius tests) | No | No |
| M5.1 mixed workload | Yes | Yes | N/A | Yes (v5 manifests) | No | No |
| M5.1-CERT / CERT.1 | Yes | Yes | N/A | Yes (v5 manifests, disposition tests) | No | No |
| M5.2 burst | Yes | Yes | Reference Step executed | Yes (v16 step manifest in bundle) | No | No |
| M5.3 lifecycle | Yes | Yes | Reference executed | Yes (v16 lifecycle manifest in bundle) | No | No |
| M5.4 disconnect | Yes | Yes | Reference executed | Yes (v16 sweep manifest in bundle) | No | No |
| M5.5 backpressure | Yes | Yes | Reference executed | Yes (v16 profiles bundle) | No | No |
| M5.6 stage timing | Yes | Yes | Workload-dependent | Synthetic stage histograms; no full v16 timing report retained | No | Synthetic measurements only; no performance certificate |
| M5.7 capacity | Stages 4k/10k/25k/50k present | Deterministic selectors CI-tested | 10k short cell executed | Yes (v16 10k manifest in bundle); 25k/50k not executed | No | No capacity claim |
| Hyperliquid H0 | Yes | Yes | N/A | Golden fixture + live REST discovery | Yes (read-only mainnet) | N/A |
| Hyperliquid H1 | Yes | Yes | N/A | Telemetry fixtures + real frames | Yes (read-only mainnet) | No |
| Hyperliquid H2 | Yes | Yes | N/A | Books e2e + retained live captures | Yes (read-only mainnet) | No |
| Hyperliquid H3 | Yes | Yes | Reference executed | Yes (v16 Hyperliquid manifest in bundle) | No | No |
| Coinbase/Kraken/OKX/Binance H0 | Yes | Yes | N/A | Golden reference fixtures | No | No |
| GOV-002 | Yes | Yes | N/A | v16 evidence bundle + ledger | N/A | N/A |
| Platform M2 identity (Phases 1-3) | Yes | Yes | N/A | 11 identity tests | No | N/A |
| Platform M3 products (Phase 1) | Yes | Yes | N/A | Product/capability/state-machine tests | No | N/A |
| Platform M4 canonical events (Phase 1) | Yes | Yes | N/A | Canonical conversion/schema tests | No | N/A |
| P5 JSONL-to-target | Yes (simulator + synchronous PostgreSQL `PgTarget`) | Yes | Live PostgreSQL test PASS at `7edfb92` | Exact command, output and hashes retained for current head | Opt-in server test only; no production certification | No |
| P6 durable file-to-target | Yes (local durable store, file source, `PgTarget`, loopback TCP source) | Yes | PostgreSQL test PASS at `7edfb92`; loopback reconnect tests in CI | Source/tests and live DB output retained for current head | Opt-in PostgreSQL test PASS; TCP source remains loopback-only; no production certification | No |

## Contract foundation vs live integration

This table is the authoritative distinction between an implemented contract and
an integrated production path. Do not read a foundation as a production
capability.

| Capability | Contract foundation | Live integration | Production certification |
|---|---|---|---|
| Persistent identity | Implemented (`src/identity.rs`) | Partial/not migrated into collector ownership | No |
| Typed products | Implemented (`src/products.rs`) | Not consumed by legacy session `apply_subscriptions` | No |
| Canonical events | Implemented (`src/canonical.rs`) | Not authoritative live pipeline | No |
| JSONL-to-target | Generic source/target runner with deterministic simulator and real `PgTarget` (`postgres::Client`) | Operator reports opt-in live PostgreSQL test passed; no server transcript retained in this export | No production certification |
| Local reference commit store | Implemented (`src/application/durable.rs`): append-only checksummed fsync records, reopen verification, fault injection | Single-writer local filesystem only; no replication or multi-host durability | No |
| Streaming file source | Implemented (`src/application/file_source.rs`): bounded `BufRead`, exact bytes/offsets, fingerprint-bound resume | File ingestion plus a loopback TCP source; network-source tests use a local server and do not certify a production network adapter | No |
| PostgreSQL execution | Synchronous `postgres` driver, `PgExecutor`, `PgTarget`, explicit migration and database idempotency/checkpoint tables; one event per delivery | Opt-in live server test reported PASS; monotonic target-scoped checkpoint; no connection pool, failover soak or replication | No |
| Hyperliquid | H0-H3 implemented; H1/H2 wire codecs fixed from real frames | Read-only mainnet captures in `captures/`, including a reconnect drill; no orders | No |
| Additional H0 venues | Reference data only | No live product integration | No |

## Live validation

The operator reports that `captures/INDEX.md` records five real, direct,
read-only Hyperliquid mainnet captures with source commits, configurations,
raw journals, SBE files, verification reports and SHA-256 manifests. Two clean
`HL_LIVE_OK` runs and one `HL_LIVE_RECOVERY_OK` reconnect drill pass all
acceptance criteria: exact selected-universe coverage, exchange-originated
snapshots, valid SBE reconstruction, complete journals, zero decode/writer
errors and reconciled accounting. The two failed captures are retained as
evidence that exposed the wire-shape, precision and accounting defects.

The `captures/` directory is Git-ignored and is not present in this source
export. Consequently, the live evidence and its SHA-256 manifests are not
independently verifiable from this archive; use the evidence-bearing checkout
and validate each manifest before relying on those claims.

Synthetic loopback (`evidence_class = synthetic-loopback`) is not live
evidence. Polymarket live connections remain disabled because permitted access
from the operator's estimated location was not documented; Kalshi remains an
opt-in authenticated read-only test with no live capture claimed.

## Evidence bundle

`measurements/stress/m5-v16-bundle/` contains nine v16 reference manifests and
`ledger.json` with per-file SHA-256, workload, status and provenance fields.
These are deterministic **synthetic loopback** runs: burst, lifecycle churn,
disconnect sweep, backpressure profiles, 10k capacity, Hyperliquid H3, plus
three intentionally failed cells. They establish test execution and fail-closed
behavior; they are not live venue or capacity certifications.

The main `measurements/stress/index.json` retains the earlier v2–v5 reference
cells. New v16 evidence is in the bundle, not mixed into that index, so old
hashes remain unchanged.

## Ignored tests

Ignored tests remain opt-in by design: live venue tests requiring credentials
or network (`live_kalshi`, `live_polymarket`), large/capacity cells, and
reference-artifact generators. Reference generators listed above were executed
for this ledger; capacity 25k/50k and live tiers were not executed.

## Governance

`main` is protected: PR required, CI required on the exact head, merge commits
only, force push and branch deletion disabled, strict freshness. Local branch
inventory is `main` only; one worktree; zero stashes. See
[`git-workflow.md`](git-workflow.md) and the dated GitHub protection audit.

## Not claimed

Replicated/multi-site durability, end-to-end exactly-once to external targets,
distributed runtime, nanosecond clock accuracy,
or any live venue certification. Platform M2/M3/M4 are additive contract
foundations (Phases 1-3 / Phase 1): they do not yet rewire collector ownership,
subscription consumption or the SBE writer. P5 includes a deterministic
simulator and a real synchronous PostgreSQL `PgTarget` behind the generic
target contract. P6 adds a **real local durable
store**, a streaming file source, and an independent loopback TCP source. The
PostgreSQL driver applies the parameterized transcript with database uniqueness,
transactions and checkpoint progression; its opt-in live test is operator-run
and is not part of CI. The loopback TCP source proves parser reuse, reconnect
resume, malformed-line accounting and a 2,000-record test soak; it is not a
production network-source certification. None of these components is wired
into the market-data collector, and no replication, production deployment or
performance claim is made.
