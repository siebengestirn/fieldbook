# Durability contract

## Current implementation

The collector currently provides a **local, append-only, checksummed journal**
and a separate SBE output path. The acknowledgement boundaries are:

`queued` — record accepted into the bounded journal writer queue.
`written` — record handed to the segment buffer.
`flushed` — removed from the process buffer.
`synced` — covered by `File::sync_all` under the configured policy.
`finalized` — segment footer, manifest entry and directory sync complete.

Only `finalized` is evidence of a complete run. A full queue, write, flush,
sync, rotation or finalization error fails the run; no frame is silently
dropped. Verification recomputes checksums and rejects truncation, corruption,
unsupported versions, reordering and incomplete runs.

SBE output has a separate path: writer queue admission, successful sink write,
flush and independent capture verification are distinct operations. A writer
acceptance is not a durability acknowledgement.

## Platform P6 local reference commit store

The non-market-data reference application has its own, deliberately separate
local store in `src/application/durable.rs`. It is **not** the market-data
journal and reuses none of its types or formats. It is built on the shared
`write_json_atomic` helper and `sha2`.

Each commit appends a new, never-rewritten file
  `commit-<12-digit sequence>.json`.
One record atomically carries the committed event ids, dispositions and
  quarantine evidence, and the new checkpoint, plus a SHA-256 of the exact
  serialized record.
`write_json_atomic` writes a sibling temporary file, fsyncs it, renames it
  into place and fsyncs the parent directory. One checksummed file is the
  commit unit (the local analogue of a SQL transaction).
- Reopen verifies schema version, file position, checksum and checkpoint
  binding. Truncation, corruption, sequence gaps, schema drift and a changed
  source fingerprint or target fail closed.
Deterministic fault injection covers fail-before-write, truncated append and
  wrong checksum; a partial append is rejected on the next open.
A resume anchor is bound to `(source identity, SHA-256 of full source bytes,
  target id)`, records the last terminal sequence and its physical byte offset,
  and is verified before any line is re-read.

This store is local, single-writer and process/host-recovery scoped. It is not a
database transaction, not replicated, and not a live PostgreSQL connection. The
PostgreSQL side remains a parameterized transcript plus an executor trait; no
driver or server is involved.

## Not implemented

- Replication or quorum acknowledgement.
- Multi-site durability.
- Transactional exactly-once coupling between journal position and SBE output.
- Defined RPO/RTO per failure model beyond local process/host recovery.
- Cross-run retention or archival policy.
- Live PostgreSQL durability, migrations or server-side exactly-once behavior.

## Failure model

| Failure | Current behavior |
|---|---|
| Process crash during append | Incomplete run reported; committed segments verified |
| OS crash | Local `sync_mode` determines acknowledged boundary |
| Disk full / write failure | Run fails closed, no silent drop |
| Corrupt segment | Verification fails closed |
| Network partition | Out of scope; no replication |
| Disk failure | Out of scope; no replica |
| P6 partial commit record | Next `open`/`load_state` rejects truncation or checksum mismatch |
| P6 changed source bytes | Stale resume anchor is rejected; no checkpoint reuse |
| P6 unavailable target | Checkpoint and durable store stay unmoved |
| P6 simulated run | No external write and no durable checkpoint progression |

Any future replication layer must preserve these boundaries and add explicit
quorum acknowledgements; it must not silently weaken local durability.
