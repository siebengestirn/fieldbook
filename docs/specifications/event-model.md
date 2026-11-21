# Specification 03 — Universal Event Model

## Purpose

This specification defines typed event payloads for every financial
information domain supported by Fieldbook. It replaces the assumption that
every observation is an aggregated book update.

The Rust implementation is `src/event_model.rs`. The existing
`canonical::CanonicalEnvelope<T>` remains the V1 envelope and stays valid;
`UniversalEnvelope<T>` adds explicit timestamp roles, provenance and quality
state for V2 domains.

## Normative rules

1. An event has one typed payload. An untyped JSON dictionary is not a
   payload contract.
2. Instrument identity, entity identity and venue identity are references.
   A venue symbol is never promoted to an instrument identifier.
3. Source event time, publication time, network receive time, application
   receive time, normalization time and durable-write time are separate
   values. One timestamp field does not serve all roles.
4. Missing data remains missing. A null field is not a zero value.
5. Original mantissas and scales are retained. Derived floating-point values
   are separate labelled calculations.
6. A rejected or quarantined event remains in evidence with a machine-readable
   reason.
7. Transaction identity, instrument identity and event identity are distinct.
   ISO 23897 UTI, where available, belongs to a transaction payload rather
   than to the instrument master.

## Envelope fields

| Field | Meaning |
|---|---|
| `event_id` | Deterministic identity from immutable assertion inputs |
| `instrument_id` | Reference to the instrument graph, when one applies |
| `entity_id` | Reference to a legal or natural entity, when one applies |
| `event_type` | Stable domain event label |
| `timestamps` | Typed list of observed timestamps and clock metadata |
| `sequence` | Source sequence, scope declared by the source manifest |
| `source_schema_version` | Wire or publication schema version |
| `normalizer_version` | Decoder or normalizer that produced the payload |
| `quality` | Current data-quality state |
| `provenance` | Raw record, source, feed, venue, decoder and license reference |
| `payload` | Strongly typed domain payload |

## Domain payloads

The first payload set covers equity quotes, order events, trades, bond quotes,
FX quotes, futures quotes, option quotes, commodity assessments, economic
observations, corporate actions, news events and blockchain events. Each
payload is a distinct serde-tagged variant. Adding a payload does not change
the meaning of an existing variant.

Book visibility is explicit: L1, L2, L3, consolidated, indicative and
participant-entitled. An aggregated L2 book cannot represent order-level
add, modify and cancel events. Where a venue offers ITCH-style order-by-order
data, the collector must retain MBO events with order identity instead of
collapsing them into price levels.

## Timestamp requirements

Each `ObservedTimestamp` carries a role, microsecond value, unit, clock
source, precision, optional uncertainty and optional UTC offset. UTC is the
reference clock. Clock class, measured divergence, granularity and the
timestamp application point are recorded per source rather than as a single
global assumption.

MiFIR Article 22c extends clock synchronization obligations to venues,
systematic internalisers, designated publishing entities, approved
publication mechanisms and consolidated tape providers. Existing technical
rules set UTC as the reference and require documented traceability. These
rules are a design input for timestamp semantics, not a claim that Fieldbook
is compliant with MiFIR.

## Validation and evidence

`UniversalEnvelope::validate()` fails closed when event identity or
provenance is empty, when no receive timestamp is present, or when a timestamp
declares a unit other than microseconds. Tests cover validation and payload
distinctness. Future certification requires per-source timestamp metadata and
independent reconstruction from the raw journal.

## Non-claims

This specification does not assert order-level coverage for any current
venue, does not create a universal consolidated book, and does not provide a
trading, execution or strategy engine.

## Sources

- Nasdaq TotalView-ITCH and MoldUDP64:
  https://databento.com/docs/datasets/xnas-itch.md
- MiFIR clock synchronization (EU 2024/791):
  https://eur-lex.europa.eu/eli/reg/2024/791/oj/eng
- Delegated Regulation (EU) 2017/574:
  https://eur-lex.europa.eu/eli/reg_del/2017/574/oj/eng
- Apache Iceberg evolution:
  https://iceberg.apache.org/docs/latest/evolution/
