# Universal Platform Architecture (status and direction)

This document records the architecture the repository is moving toward. It is
explicitly divided into **implemented today** and **intended future contract**.
Nothing here should be read as an implemented guarantee unless marked as such.

## Implemented today

- Venue adapters (Kalshi, Polymarket, Hyperliquid) produce typed `ExchangeEvent`s
  from raw transport observations. The collector core is independent of websocket
  libraries. Hyperliquid H0-H3 are implemented: discovery, telemetry, selected
  l2Book snapshots/trades and synthetic loopback capacity cells; no live venue
  capture is claimed.
- A versioned raw-frame journal stores application data and native controls,
  with v1 readers and a v2 writer. `read_records()` is data-only;
  `read_observations()` exposes the full observation stream.
- Three independent per-source/per-epoch sequence domains are verified:
  `receive_seq`, `control_seq`, `observation_seq`.
- Typed recovery policy, independent instrument/source queues and exact
  per-instrument acknowledgement fences are implemented and tested.
- A deterministic stress laboratory generates 4,000-instrument mixed workloads,
  certifies event accounting and terminal delta dispositions, and retains
  versioned success/failure manifests. Burst, lifecycle, disconnect, backpressure,
  stage-timing and Kalshi capacity increments are integrated at
  `scenario_version = 16`. The Hyperliquid H3 increment adds a synthetic loopback
  wire mock and ten-instrument capacity cells that reuse the same ledger,
  coverage-hash, manifest and versioning seams; its evidence is **not live
  certification** and carries no latency/capacity performance claim.

## Target layering

| Layer | Responsibility | Current status |
|---|---|---|
| Source adapters | Connect to venues; emit typed observations/events | Implemented for Kalshi, Polymarket and Hyperliquid (H0-H3, synthetic loopback evidence only for H3) |
| Ingress/validation | Preserve raw evidence; classify and sequence | Implemented in session + journal |
| Core/state | Deterministic normalization and book state | Implemented in `CollectorCore` |
| Recovery | Ownership, fencing, scoped decisions | Implemented for source/instrument scopes |
| Durability | Journal, SBE, verification, replay | Partially implemented; replication deferred |
| Distribution | Route outputs to targets | Single SBE sink today |
| Audit | Provenance, dispositions, manifests | Implemented for stress reference workload |
| Governance | Config, policy, branch integration | Implemented via `git-policy.json`, `docs/git-workflow.md` |

## Intended future contract

The long-term system is a hexagonal, source- and destination-independent
platform. The core must own identity, ordering, provenance, determinism and
durability contracts; adapters must own protocol specifics. A new source or
destination must be addable without changing unrelated adapters or the domain
model.

Planned boundaries: connector SDK, capability negotiation, canonical event
envelope, deterministic processor contracts, replicated durability policies,
partition ownership, delivery obligations and independent verification.

## Non-goals for the current increment

Hyperliquid live capture/certification, generalized connector SDK, distributed
runtime, OMS/strategy execution, Kubernetes, Kafka or other infrastructure
adoption are not implemented here. The Hyperliquid H0-H3 stages are implemented
against loopback fixtures/mocks only; H3 in particular is synthetic evidence and
makes no live, latency, throughput or capacity claim. They require their own
evidence gates.
