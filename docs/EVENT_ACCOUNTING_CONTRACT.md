# Event Accounting Contract

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

## Status

Implemented for the synthetic reference workload through scenario schema v16.
Five stage-specific histograms are implemented and documented in
[`PERFORMANCE_METHODOLOGY.md`](PERFORMANCE_METHODOLOGY.md); they are synthetic
measurements, not a performance certificate. Replication and generalized
destination obligations remain future contracts. See
[`PROGRAMME_STATUS.md`](PROGRAMME_STATUS.md) for authoritative status.
