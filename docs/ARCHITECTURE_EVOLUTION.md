# Architecture Evolution

## Principle

Evolve by proving contracts in the existing demanding reference workload, then
generalizing stable boundaries. Do not build a broad framework before the
contracts have been demonstrated by more than one workload.

## Completed foundations

- S1a/S1b: transport observations and journal v2 with v1 compatibility.
- M3/M4: typed recovery, independent queues, exact fences, blast-radius evidence.
- M5.1/M5.1-CERT/M5.1-CERT.1: deterministic mixed workload, bidirectionally
  reconciled accounting and complete terminal delta dispositions.

## Completed workload and venue milestones

M5.2 burst, M5.3 lifecycle, M5.4 disconnect, M5.5 backpressure, M5.6 stage
instrumentation and M5.7 capacity characterization are merged into `main`.
Hyperliquid H0-H3 and H0 reference-data adapters for Coinbase, Kraken, OKX and
Binance are also merged. Exact per-milestone implementation/CI/evidence status
is in [`PROGRAMME_STATUS.md`](PROGRAMME_STATUS.md).

## Platform contracts now present (additive)

Platform M2 identity, M3 data products/capabilities, M4 canonical events and the
P5 JSONL-to-target reference application are merged. See
[`PROGRAMME_STATUS.md`](PROGRAMME_STATUS.md) for exact status and the limits of
each foundational phase.

## Future sequence

1. Rewire live ingest to consume typed identities/products and the canonical envelope.
2. Real PostgreSQL target driver (separate dependency decision) and live database proof.
3. Durability/replication subsystem with defined failure models.
4. Partitioned deterministic execution engine.
6. Distributed runtime only after single-node correctness and capacity are proven.

## Non-goals now

Hyperliquid and additional exchanges are the next asset-class work after the M
series, but they must consume the same contracts rather than create parallel
domain logic. Universal connectors, OMS/strategy execution and distributed
consensus remain deferred.
