# Universal Platform Architecture (status and direction)

This document records the architecture the repository is moving toward. It is
explicitly divided into **implemented today** and **intended future contract**.
Nothing here should be read as an implemented guarantee unless marked as such.

## Implemented today

- Venue adapters (Kalshi, Polymarket) produce typed `ExchangeEvent`s from raw
  transport observations. The collector core is independent of websocket
  libraries.
- A versioned raw-frame journal stores application data and native controls,
  with v1 readers and a v2 writer. `read_records()` is data-only;
  `read_observations()` exposes the full observation stream.
- Three independent per-source/per-epoch sequence domains are verified:
  `receive_seq`, `control_seq`, `observation_seq`.
- Typed recovery policy, independent instrument/source queues and exact
  per-instrument acknowledgement fences are implemented and tested.
- A deterministic stress laboratory generates 4,000-instrument mixed workloads,
  certifies event accounting and terminal delta dispositions, and retains
  versioned success/failure manifests. As of this document, burst, lifecycle,
  disconnect, backpressure, stage-timing and capacity increments are being
  integrated incrementally through `main`.

## Target layering

| Layer | Responsibility | Current status |
|---|---|---|
| Source adapters | Connect to venues; emit typed observations/events | Implemented for Kalshi and Polymarket; Hyperliquid planned |
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

Hyperliquid, generalized connector SDK, distributed runtime, OMS/strategy
execution, Kubernetes, Kafka or other infrastructure adoption are not
implemented here. They require their own evidence gates.
