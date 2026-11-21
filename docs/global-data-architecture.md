# Fieldbook global data infrastructure architecture

Fieldbook V1 is a read-only exchange market-data collector with journaling,
replay and Parquet/KDB-X analytics. Fieldbook V2 is the target architecture for
global financial and economic information acquisition. This document is a
design target, not a claim of current coverage.

Architecture remains authoritative for the implemented system. The six
specifications under `docs/specifications/` define the V2 contracts.

## Mission and boundary

Fieldbook acquires, preserves, verifies, normalizes, indexes and distributes
financial and economic information from authorized sources. It does not place
orders, route orders, operate brokerage accounts, manage portfolios or run
trading strategies. Observing market activity is in scope; participating in it
is outside the boundary.

## Repository layout

The source tree follows the domain planes rather than venue names:

```text
src/
├── acquisition/     transports, venue connectors, codecs, arbitration
├── domain/          taxonomy, identity, instruments, typed events
├── market_state/    books, collector core, recovery, protocol vocabulary
├── integrity/       journal, verification, replay, quality, timestamps
├── storage/         archive, event store, analytics, reference application
├── governance/      source registry, rights, coverage certification
├── distribution/    CLI, coordinator, probe, metrics, inspection
├── util/            shared numeric, SBE and IO helpers
└── generated/       generated SBE codec
```

Crate-root re-exports preserve existing `crate::` paths. This keeps the live
collector and offline tools unchanged while the physical layout follows the
target architecture.

## Six planes

| Plane | Responsibility |
|---|---|
| Global source acquisition | Exchange, vendor, government, issuer, news and alternative-source connectors |
| Raw capture | Original payload and packet preservation before normalization |
| Universal data core | Typed normalization, instrument and entity identity, quality |
| Preservation | Raw archive, normalized event history, reference store, hot state |
| Access | Streaming APIs, historical APIs, bulk files, search, SDKs |
| Governance | Source registry, licensing, entitlements, retention, audit |

These are logical boundaries. A modular codebase precedes any decision to
split processes or deploy distributed services.

## Data flow

```text
sources -> connectors -> raw capture -> decode -> typed normalize
       -> quality -> reference graph -> normalized history
       -> certification -> access APIs
```

The live path and the historical preservation path are independent. A slow
archive must not block live ingestion, and a live restart must not corrupt the
archive. Different data classes have different service targets; a daily
central-bank release is not engineered like a multicast exchange feed.

## Mandatory V2 specifications

| Number | Specification | Document |
|---|---|---|
| 01 | Universal Data Taxonomy | `specifications/data-taxonomy.md` |
| 02 | Universal Instrument Master | `specifications/instrument-master.md` |
| 03 | Universal Event Model | `specifications/event-model.md` |
| 04 | Acquisition Framework | `specifications/acquisition-framework.md` |
| 05 | Data Integrity Standard | `specifications/data-integrity-standard.md` |
| 06 | Global Coverage and Rights | `specifications/coverage-rights.md` |

## Module mapping

| V2 contract | Rust module |
|---|---|
| Taxonomy | `src/domain/taxonomy.rs` |
| Instrument and entity graph | `src/domain/reference_data.rs`, `src/domain/identity.rs` |
| Typed events | `src/domain/event_model.rs`, `src/domain/canonical.rs` |
| Connector contract | `src/governance/adapter.rs`, `src/domain/products.rs` |
| Quality and reconciliation | `src/integrity/quality.rs` |
| Rights and coverage | `src/governance/rights.rs`, `src/governance/coverage_certification.rs` |

These modules are additive. The live collector continues to use its existing
SBE, journal and replay contracts. Integration happens through declared seams
such as source capability assertions and canonical envelope construction.

## Engineering principles

1. Original information is preserved.
2. No silent data loss.
3. No destructive normalization.
4. Every data point has traceable provenance.
5. Historical information and revisions are retained.
6. Coverage and limitations are transparent.
7. Data rights are enforced.
8. Data quality is independently verifiable.
9. Supported asset classes have appropriate domain models.
10. New sources are integrated without architectural redesign.

## Scale reference points

LSEG publicly describes reference-data coverage above 100 million instruments
and 500 venues and contributors. Databento publishes 6.1 microseconds of
normalization latency, 5 to 35 microseconds of capture-server processing and
FPGA-assisted packet capture with losses near one in 10 to the ninth packets
per stream. These are external benchmarks. Fieldbook does not claim them.

- LSEG data analytics: https://www.lseg.com/en/data-analytics
- Databento architecture:
  https://databento.com/docs/architecture/architecture-diagram.md

## Non-claims

This document does not claim production readiness, 90-country coverage,
exchange-grade latency, hardware timestamping, multicast capture, order-level
data for any venue, or rights to redistribute any dataset. Each claim requires
its own source, license, test and retained evidence.
