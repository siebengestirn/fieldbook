# Specification 05 — Data Integrity Standard

## Purpose

This specification defines capture, timestamp, recovery, reconciliation and
certification rules for every Fieldbook V2 dataset. The unit of accounting
is an observation with a declared disposition. A failed observation stays in
evidence with a machine-readable reason.

The Rust implementation is `src/quality.rs`, `src/journal.rs`, `src/time.rs`
and `src/coverage_certification.rs`. `QualityState`, `QualityReason` and
`Reconciliation` are the normative types.

## Normative rules

1. Every inbound observation receives one disposition from `QualityState`.
2. A rejected or quarantined observation remains addressable by `event_id`
   and by a pointer into the raw journal.
3. Only transitions permitted by `QualityState::can_transition_to` apply.
   The `transition` function returns `IllegalTransition` for all others.
4. Sequence gaps, duplicate observations, out-of-order delivery and failed
   snapshot reconstruction carry distinct `QualityReason` values.
5. A reconciliation boundary reports expected, observed, missing, extra,
   duplicated, rejected and quarantined counts.
6. Original payload bytes and original numerical representation stay
   separate from normalised values and derived calculations.
7. Timestamp roles are recorded separately with clock source, precision and
   uncertainty. One field does not serve all roles.
8. Corrections append to history. A prior value stays queryable.
9. Certification requires a reconciled quality report, provenance, declared
   timestamp metadata, recovery evidence and a rights decision.

## Quality state model

The forward path is ordered. Quarantine and rejection are explicit terminal
or re-work dispositions rather than discarded records.

```text
raw_captured -> decoded -> normalized -> validated -> reconciled
      |             |            |            |
      +-> quarantined <----------+------------+
      +-> rejected

quarantined -> decoded
rejected -> rejected
```

The permitted set in `src/quality.rs` also allows `raw_captured` and
`decoded` to move to `quarantined` or `rejected`, and `normalized` and
`validated` to move to `quarantined`. A transition from `rejected` to
`reconciled` is illegal.

## Quality reasons

| Reason | Condition |
|---|---|
| `decode_failure` | Decoder rejected a retained payload |
| `schema_version_unsupported` | Version outside the accepted set |
| `sequence_gap` | Missing sequence position at a declared scope |
| `duplicate_observation` | Repeated source identity for one position |
| `out_of_order` | Arrival order differs from source order |
| `missing_instrument_mapping` | No instrument reference resolved |
| `ambiguous_instrument_mapping` | More than one reference resolved |
| `invalid_numeric_representation` | Mantissa or scale violates the schema |
| `timestamp_anomaly` | Timestamp violates the declared clock model |
| `stale_observation` | Publication time outside the freshness bound |
| `crossed_book` | Bid exceeds ask after a book update |
| `snapshot_reconstruction_failure` | Replay did not reproduce a snapshot |
| `reference_conflict` | Two references assert incompatible facts |
| `rights_denied` | No permitted right for the operation |
| `source_unavailable` | Source did not deliver within the bound |

A `QualityRecord` stores `event_id`, state, reason, observation time and
verifier. Verifier identity is recorded with the disposition.

## Reconciliation accounting

`Reconciliation::reconciled` is the accounting invariant:

```rust
pub fn reconciled(&self) -> bool {
    self.missing == 0
        && self.extra == 0
        && self.duplicated == 0
        && self.expected == self.observed + self.rejected + self.quarantined
}
```

A `QualityReport` also requires `window_end_us >= window_start_us`. A report
with an unaccounted record does not reconcile. Zero writer errors are not a
substitute for this check: a writer can persist observations that fail
reconstruction.

## Timestamp integrity

Each observation carries separate values for exchange event time, exchange
publication time, network receive time, application receive time,
normalisation time and durable-write time. UTC is the reference. Clock
source, measured divergence, granularity and the timestamp application point
are recorded per source. The current collector reads receive time from the
system wall clock in `src/time.rs`; monotonic timers measure intervals and
are not serialised as event time.

MiFIR Article 22c, as amended by Regulation (EU) 2024/791 and implemented by
Delegated Regulation (EU) 2017/574, sets UTC as the reference and defines
divergence and granularity bounds. These are design inputs for timestamp
metadata, not a statement of Fieldbook compliance.

| Context | Maximum divergence | Granularity |
|---|---|---|
| Venue with latency above 1 ms | 1 ms | 1 ms |
| Venue with latency at or below 1 ms | 100 microseconds | 1 microsecond |
| High-frequency trading participant | 100 microseconds | 1 microsecond |

The rules require the timestamp application point to be documented and the
synchronisation arrangement to be reviewed annually. A source manifest
records clock source and measured divergence against these thresholds.

- Regulation (EU) 2024/791:
  https://eur-lex.europa.eu/eli/reg/2024/791/oj/eng
- Delegated Regulation (EU) 2017/574:
  https://eur-lex.europa.eu/eli/reg_del/2017/574/oj/eng

## Raw capture and durability

The raw journal is an independent persistence path. It retains original
payload bytes, including frames the decoder rejects. A record passes through
distinct durability states: written, flushed, synced and finalised. The
state of a record is reported explicitly; a successful write is not reported
as a synced record.

Original packet boundaries are retained for packet-based sources, and
original application payloads are retained for application feeds. The journal
holds a checksum and a decoder version. A subsequent decoder can be applied
to the same retained bytes.

## Recovery and sequence continuity

Sequence continuity is evaluated where the source publishes a denominator.
Packet sequence gaps are identified and filled through the declared gap
facility. Exchange sequence gaps are reported at the exchange scope. A
missing sequence position is recorded as `sequence_gap`, not converted into
a neutral market event.

Snapshot anchoring uses the source's own markers. Channel Reset messages
invalidate affected book state and require a fresh snapshot. End-of-Event
markers delimit incremental batches. Weekly MBO snapshots provide an
independent reconstruction target. These are source semantics recorded in
evidence, not trading signals.

## Historical preservation

Historical data is a versioned product. Apache Iceberg 1.12.0 records schema
and partition evolution in metadata: adding, renaming or dropping a column
does not rewrite existing data files. Each column has a unique identifier,
so a rename retains column identity across versions. Snapshots support time
travel to a declared dataset version, and branches and tags name snapshot
references. An audit branch follows a forward-advance pattern: corrections
append to the branch and advance its pointer; the main branch advances after
review. This gives a correction an auditable path from original observation
through revision.

- Apache Iceberg evolution:
  https://iceberg.apache.org/docs/latest/evolution/
- Apache Iceberg branching:
  https://iceberg.apache.org/docs/latest/branching/

## Published timing figures

Databento publishes 6.1 microseconds of normalisation latency, 5 to 35
microseconds of capture-server processing, 4 to 10 nanoseconds of FPGA
timestamp precision, PTP synchronisation from sub-microsecond to below 50
microseconds, NTP near 100 milliseconds, and SmartNIC or DPDK paths at 64
microseconds median and 311 microseconds p99. These are published figures
for a specialised architecture. They are not Fieldbook measurements,
targets or certification thresholds.

- Databento documentation: https://databento.com/docs
- Timestamping guide:
  https://databento.com/docs/architecture/timestamping-guide.md

## Certification

Certification consumes a reconciled `QualityReport`, a retention check on
the raw journal, per-source timestamp metadata, recovery evidence, a rights
decision and a correction history. The operation returns a promoted record
or a named failure. Historic evidence revisions stay append-only. A dataset
without a measurable source denominator reports that limitation instead of
a completeness percentage.

## Non-claims

This specification does not claim that an external source cannot lose a
message; it claims that a declared gap is identifiable. It does not claim
MiFIR compliance, PTP-synchronised hardware timestamps or FPGA capture in
the current build. It does not certify p50, p95, p99 or p99.9 latency,
throughput, CPU consumption or production readiness. It does not treat the
published Databento figures as Fieldbook results. It does not provide order
entry, execution or strategy validation.

## Sources

- Regulation (EU) 2024/791:
  https://eur-lex.europa.eu/eli/reg/2024/791/oj/eng
- Delegated Regulation (EU) 2017/574:
  https://eur-lex.europa.eu/eli/reg_del/2017/574/oj/eng
- Apache Iceberg evolution:
  https://iceberg.apache.org/docs/latest/evolution/
- Apache Iceberg branching:
  https://iceberg.apache.org/docs/latest/branching/
- Databento documentation: https://databento.com/docs
- Timestamping guide:
  https://databento.com/docs/architecture/timestamping-guide.md
