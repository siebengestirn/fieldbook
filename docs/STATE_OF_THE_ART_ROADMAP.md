# State of the Art Roadmap

This is a living programme document. It records the current verified state and
the next evidence gates. It does not claim future work is complete.

## Current verified state

- `main` contains S1a/S1b, M3/M4, GIT-001, M5.1 through M5.7, Hyperliquid H0-H3,
  and H0 reference data for Coinbase, Kraken, OKX and Binance.
- Exact per-milestone status is in [`PROGRAMME_STATUS.md`](PROGRAMME_STATUS.md).
- Assurance: formatting, Clippy `-D warnings`, all-target tests, doc tests,
  architecture traceability, matrix validation and generated-path policy are
  enforced by `scripts/assurance/polish.sh`.
- Manifest evidence is versioned; v2-v5 archives remain immutable. New schema
  versions publish new cell IDs rather than overwriting history.

## Next gates

| Gate | Exit condition |
|---|---|
| Platform M2 | Persistent instrument and reference identity (ADR-0001) |
| Platform M3 | Typed data-product and capability contracts |
| Platform M4 | Canonical event model and normalized persistence |
| P5 | Independent JSONL-to-PostgreSQL reference application |
| P8 | Replicated durability profiles with explicit failure models |
| Performance | End-to-end reproducible measurements; current histograms are stage-only |

## Governing standard

> Understand every layer. Specify every guarantee. Measure every bottleneck. Preserve every relevant fact. Make every decision explainable. Prove every important claim.

No performance, capacity or durability claim may outrun its evidence.
