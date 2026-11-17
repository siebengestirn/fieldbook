# Programme Status (authoritative)

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
| `1488412` | PR #65 — Binance H0 (current integrated head at review) |

PR #45 is **MERGED** (`d54aa08`); its earlier draft description was obsolete and
GitHub metadata now agrees with the integrated history. It was never merged
twice.

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
| Hyperliquid H0 | Yes | Yes | N/A | Golden reference fixture | No | N/A |
| Hyperliquid H1 | Yes | Yes | N/A | Telemetry fixtures + e2e | No | No |
| Hyperliquid H2 | Yes | Yes | N/A | Books e2e + recovery drill | No | No |
| Hyperliquid H3 | Yes | Yes | Reference executed | Yes (v16 Hyperliquid manifest in bundle) | No | No |
| Coinbase/Kraken/OKX/Binance H0 | Yes | Yes | N/A | Golden reference fixtures | No | No |

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
persistent instrument identity (Platform M2), typed data products (Platform
M3), canonical normalized event store (Platform M4), JSONL-to-PostgreSQL
reference application, distributed runtime, nanosecond clock accuracy, or any
live venue certification.
