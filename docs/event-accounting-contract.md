# Event accounting contract

This document defines the accounting contract currently implemented for the
market-data collector and its stress laboratory. It intentionally separates
what is implemented from future platform-wide guarantees.

## Identities

| Entity | Owner | Current representation |
|---|---|---|
| Transport observation | Session worker | `SourceStatus.data_frames_received`, native control counters |
| Decoded event | Adapter | `SourceStatus.delta_events_decoded` |
| Core-ingress event | Collector core | `CollectorStats.core_ingress_events`, `core_delta_events` |
| State application | Collector core | `DeltaDisposition` counters |
| Writer admission | Writer queue | `WriterStats.delta_admissions` |
| Successful SBE write | Writer sink | `WriterStats.delta_messages` |
| Persisted evidence | Independent verifier | decoded SBE capture, journal verification |

Core-ingress counters are deliberately **not** transport counters. They are
retained as a separate boundary and never substituted for receive or decode
evidence.

## Transport accounting

Every raw data frame observed at the transport boundary is counted once,
including frames whose subsequent decode fails. Native WebSocket control
frames are accounted separately and never counted as application data frames.

`transport_frames_received = decoded_frames + decode_failed + protocol_rejected + unsupported`

## Delta disposition accounting

Every `ExchangeEvent::LevelChange` admitted to `CollectorCore::handle_event` is
counted at core ingress and must reach exactly one terminal
`DeltaDisposition`:

- `Emitted`
- `SuppressedReplica`
- `AppliedWithNoActiveSource`
- `UnknownSymbol`
- `ContinuityNotLive`
- `SourceNotAssigned`
- `InvalidSourceBook`
- `ShadowClockRegression`
- `ApplyRejected`
- `ZeroDeltaNoop`
- `SbeConversionFailed`

The partition identity is:

```
decoded deltas =
    emitted + suppressed replica + no active source + unknown symbol
  + non-live + unassigned source + invalid book + shadow clock regression
  + apply rejected + zero noop + SBE conversion failed
```

All counters are monotonic and sampled over the same baseline/final window.
A missing counter, a reset, or a nonzero shortfall/excess blocks certification.

## Directional reconciliation

For each comparable boundary pair, certification computes both directions:

- `shortfall = max(0, expected - observed)`
- `excess = max(0, observed - expected)`

Both must be zero. Saturating subtraction is never used to hide an over-count.

Comparable pairs:

- enqueued vs transmitted (with explicit drops and in-flight work)
- transmitted vs decoded (`transmit_decode_shortfall` / `transmit_decode_excess`)
- decoded vs sum of terminal dispositions
- applied vs writer admission
- writer admission vs successful writer completion
- applied vs independently decoded SBE records

## Durability boundary

Writer queue admission, successful sink write, flush, synchronization and
independent verification are distinct operations. A successful in-memory write
is not a durability acknowledgement. The current collector does not claim
replicated durability; that remains future work.

## Platform P6 receipt accounting

The non-market-data reference application has its own disjoint accounting in
`ImportReport`. Every physical input line read in a run lands in exactly one
bucket (`accounted() == total_read`):

```
total_read = delivered + duplicates + rejected + outstanding + uncertain
           + accepted + skipped + target_delivered + simulated
```

`durable_committed` and `quarantined` are informational projections of
`delivered` and `rejected` respectively; they are not separate buckets.

Receipts are reported in six categories: `Delivered`, `DurableCommitted`,
`Simulated`, `Outstanding`, `Uncertain`, `Rejected`. Only a terminal durable
commit or a quarantined rejection advances the checkpoint. `Outstanding`,
`Uncertain`, `Accepted`/`Delivered` and `Simulated` do not. A simulated receipt
from `DryRun` or `Replay` is never counted as `DurableCommitted`, does not call
an external target and does not move the durable checkpoint.

Every durable commit record carries the committed event ids, the disposition
and quarantine evidence, and the new checkpoint in one checksummed fsynced
file. A retry with the same target-specific idempotency key
`(event_id, target_id, output_contract_version)` is a no-op at the target, which
is how an uncertain acknowledgement is resolved without a second row. Targets
are independent: a failure for target A neither suppresses nor counts as a
failure for target B.

## Status

Implemented for the synthetic reference workload through scenario schema v16.
Five stage-specific histograms are implemented and documented in
[`performance-methodology.md`](performance-methodology.md); they are synthetic
measurements, not a performance certificate. Destination obligations are
implemented for the non-market-data reference application (Platform P6) but are
not wired into the market collector. Replication, live database delivery and
production exactly-once remain future contracts. See
[`programme-status.md`](programme-status.md) for authoritative status.
