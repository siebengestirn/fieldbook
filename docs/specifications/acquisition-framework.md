# Specification 04 — Acquisition Framework

## Purpose

This specification defines the connector contract, source capability
registry and protocol architecture for Fieldbook V2. A connector declares
what a source provides. A type named an order book does not make a source
an L3 venue, and a reachable endpoint does not grant redistribution rights.

The Rust implementation is `src/adapter.rs`, `src/protocol.rs` and
`src/taxonomy.rs`. `SourceConnector` is the connector contract and
`SourceRegistry` holds validated manifests.

## Normative rules

1. A source registers one validated `SourceCapabilityManifest` before use.
   `validate()` fails closed on empty identity, empty coverage, empty
   protocol or transport declaration, and a zero `schema_version`.
2. Two manifests with the same `source_id` and different content produce a
   `ManifestError::Conflict`. A registry does not resolve an ambiguous source.
3. `book_model` states the depth a source offers: `level1`, `level2`,
   `level3`, `consolidated`, `indicative` or `participant_entitled`.
   An aggregated L2 feed is not promoted to order-by-order MBO.
4. `protocols` and `transports` use stable strings from the `Protocol` and
   `Transport` enums rather than free text.
5. `recovery` records snapshot, incremental recovery, packet sequence,
   exchange sequence and gap request as separate capabilities. A source with
   no packet sequence does not claim packet-gap repair.
6. `timestamps` records exchange event time, exchange publication time,
   packet capture time, sequence numbers and a named `clock_source`.
7. `access` is `public`, `licensed`, `delayed` or `participant_entitled`.
   Rights checks live in `src/rights.rs`.
8. `health` returns a `SourceHealthState` and a reason. Connection health is
   a declared state, distinct from instrument activity and book continuity.
9. `recover` returns a `RecoveryReport` with attempts, outcome, gap count and
   reason. Each recovery attempt is auditable.

## Connector contract

```rust
pub trait SourceConnector: Send + Sync {
    fn manifest(&self) -> &SourceCapabilityManifest;
    async fn connect(&mut self) -> Result<()>;
    async fn discover(&mut self) -> Result<Vec<String>>;
    async fn subscribe(&mut self, identifiers: &[String]) -> Result<()>;
    async fn recover(&mut self) -> Result<RecoveryReport>;
    async fn health(&mut self) -> Result<SourceHealth>;
    async fn disconnect(&mut self) -> Result<()>;
}
```

The seven methods are the full surface. Discovery returns object identities;
subscription takes an explicit object set. A connector does not infer a
subscription from a display label.

## Capability manifest

A source declares its real coverage in one machine-readable manifest:

```yaml
source_id: example_exchange
provider: Example Market Operator
venue_mic: XABC
country: US
venue_category: exchange
asset_classes: [equity]
data_categories: [trades, level3_book, order_events]
protocols: [fix_sbe, itch, native_binary]
transports: [tcp, udp_multicast]
book_model: level3
timestamps:
  exchange_event_time: true
  exchange_publication_time: false
  packet_capture_time: true
  sequence_numbers: true
  clock_source: ptp
recovery:
  snapshot: true
  incremental_recovery: true
  packet_sequence: true
  exchange_sequence: true
  gap_request: true
historical:
  available: true
  earliest_us: 0
  frequency: real_time
access: licensed
schema_version: 1
```

`SourceRegistry::register` rejects a manifest that fails validation or that
conflicts with a registered identity. A source without a validated manifest
is not admissible into the acquisition plane.

## Protocol architecture

Standardised definitions and native feeds are complementary inputs.

### FIX Orchestra

FIX Orchestra is the machine-readable source for messaging and interface
definitions, including rules of engagement. Two published revisions are
inputs to the schema registry: the v1.0 Technical Standard dated 2021-02-18
and the v1.1 Release Candidate 2 dated 2024-11-26. The specification text is
licensed CC BY-ND 4.0; that licence applies to the specification, not to
Fieldbook source files.

- FIX Orchestra specification:
  https://github.com/FIXTradingCommunity/fix-orchestra-spec

### FIX SBE

FIX SBE is a presentation-layer encoding. The FIX Trading Community
repository supplies a Rust generator that emits dependency-free safe crates.
The underlying Simple Binary Encoding specification is maintained in the
Aeron project. These are upstream artefacts; Fieldbook does not claim to
distribute either project.

- FIX SBE repository:
  https://github.com/FIXTradingCommunity/fix-simple-binary-encoding
- Simple Binary Encoding repository:
  https://github.com/aeron-io/simple-binary-encoding

### Native exchange feeds

Native feed semantics are declared per source. Nasdaq TotalView-ITCH is full
depth, order-by-order market data carried over MoldUDP64 on UDP multicast.
Its event lifecycle covers Add, Execute, Cancel, Delete and Replace. Book
identity survives those events, so collapsing them into price levels loses
order identity.

- Nasdaq TotalView-ITCH overview:
  https://databento.com/docs/datasets/xnas-itch.md

A/B and X/Y feed pairs provide at least four streams. Redundant feed
arbitration selects among them, and packet sequence numbers support gap
filling. Sequence scope is declared in the source manifest rather than
assumed from the transport.

- Redundant feed architecture:
  https://databento.com/docs/architecture/architecture-diagram.md

CME MDP 3.0 uses End-of-Event markers, Channel Reset messages and weekly MBO
snapshots. These signals delimit incremental state and define snapshot
anchors. A connector records them as declared source events; a Channel Reset
is not converted into a trading action.

- CME MDP 3.0 dataset overview:
  https://databento.com/docs/datasets/glbx-mdp3.md

FIX is not a substitute for a native exchange feed. Where a venue exposes an
order-by-order feed, the framework supports a native connector alongside
standardised definitions; the two do not replace each other.

## Published timing figures

Databento publishes 6.1 microseconds of normalisation latency, 5 to 35
microseconds of capture-server processing, 4 to 10 nanoseconds of FPGA
timestamp precision, PTP synchronisation from sub-microsecond to below 50
microseconds, NTP near 100 milliseconds, and SmartNIC or DPDK paths at 64
microseconds median and 311 microseconds p99. These are published figures
for a specialised colocated architecture. They are neither Fieldbook
measurements nor Fieldbook targets.

- Databento documentation: https://databento.com/docs
- Timestamping guide:
  https://databento.com/docs/architecture/timestamping-guide.md

## Validation and evidence

The adapter tests pin declaration completeness and conflicting identity.
Source health carries a reason, and recovery carries a gap count. Quality
dispositions for decoded observations are defined in Specification 05 and
implemented in `src/quality.rs`. Protocol decoding and feed arbitration are
not demonstrated by these unit tests.

## Non-claims

This specification does not claim direct exchange connectivity, multicast
ingestion, FIX session certification, order-by-order coverage, feed
arbitration or packet-gap repair in the current Fieldbook build. It does not
claim a latency figure, a throughput figure or production readiness. It does
not include order entry, execution or strategy logic. It does not treat the
published Databento figures as a comparison against Fieldbook.

## Sources

- FIX Orchestra specification:
  https://github.com/FIXTradingCommunity/fix-orchestra-spec
- FIX SBE repository:
  https://github.com/FIXTradingCommunity/fix-simple-binary-encoding
- Simple Binary Encoding repository:
  https://github.com/aeron-io/simple-binary-encoding
- Nasdaq TotalView-ITCH:
  https://databento.com/docs/datasets/xnas-itch.md
- Redundant feed architecture:
  https://databento.com/docs/architecture/architecture-diagram.md
- CME MDP 3.0:
  https://databento.com/docs/datasets/glbx-mdp3.md
- Databento documentation: https://databento.com/docs
- Timestamping guide:
  https://databento.com/docs/architecture/timestamping-guide.md
