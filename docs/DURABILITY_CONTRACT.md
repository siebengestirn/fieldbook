# Durability Contract

## Current implementation

The collector currently provides a **local, append-only, checksummed journal**
and a separate SBE output path. The acknowledgement boundaries are:

1. `queued` — record accepted into the bounded journal writer queue.
2. `written` — record handed to the segment buffer.
3. `flushed` — removed from the process buffer.
4. `synced` — covered by `File::sync_all` under the configured policy.
5. `finalized` — segment footer, manifest entry and directory sync complete.

Only `finalized` is evidence of a complete run. A full queue, write, flush,
sync, rotation or finalization error fails the run; no frame is silently
dropped. Verification recomputes checksums and rejects truncation, corruption,
unsupported versions, reordering and incomplete runs.

SBE output has a separate path: writer queue admission, successful sink write,
flush and independent capture verification are distinct operations. A writer
acceptance is not a durability acknowledgement.

## Not implemented

- Replication or quorum acknowledgement.
- Multi-site durability.
- Transactional exactly-once coupling between journal position and SBE output.
- Defined RPO/RTO per failure model beyond local process/host recovery.
- Cross-run retention or archival policy.

## Failure model

| Failure | Current behavior |
|---|---|
| Process crash during append | Incomplete run reported; committed segments verified |
| OS crash | Local `sync_mode` determines acknowledged boundary |
| Disk full / write failure | Run fails closed, no silent drop |
| Corrupt segment | Verification fails closed |
| Network partition | Out of scope; no replication |
| Disk failure | Out of scope; no replica |

Any future replication layer must preserve these boundaries and add explicit
quorum acknowledgements; it must not silently weaken local durability.
