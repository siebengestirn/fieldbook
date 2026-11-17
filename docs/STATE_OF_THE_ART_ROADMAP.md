# State of the Art Roadmap

This is a living programme document. It records the current verified state and
the next evidence gates. It does not claim future work is complete.

## Current verified state

- `main` contains S1a/S1b, M3/M4, GIT-001, M5.1, M5.1-CERT and M5.1-CERT.1.
- M5.2 and M5.5 are merged; M5.3, M5.4, M5.6 and M5.7 are in integration.
- Assurance: formatting, Clippy `-D warnings`, all-target tests, doc tests,
  architecture traceability, matrix validation and generated-path policy are
  enforced by `scripts/assurance/polish.sh`.
- Manifest evidence is versioned; v2-v5 archives remain immutable. New schema
  versions publish new cell IDs rather than overwriting history.

## Next gates

| Gate | Exit condition |
|---|---|
| M5.3 lifecycle | Reproducible universe epochs, exact coverage, complete dispositions |
| M5.4 disconnect | Deterministic fault schedule, exact recovery, bounded queues |
| M5.6 performance | Real stage histograms with sample counts; no fabricated metrics |
| M5.7 capacity | 10k/25k/50k opt-in correctness cells; no capacity claim without evidence |
| Hyperliquid | Reference-data/product model, then lower-complexity feeds, then selected books |
| Additional venues | Binance/Coinbase/Kraken/OKX only after adapter contract is stable |

## Governing standard

> Understand every layer. Specify every guarantee. Measure every bottleneck. Preserve every relevant fact. Make every decision explainable. Prove every important claim.

No performance, capacity or durability claim may outrun its evidence.
