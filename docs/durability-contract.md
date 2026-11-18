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
database transaction, not replicated, and not a live PostgreSQL connection.
The separate PostgreSQL target is implemented in
`src/application/postgres.rs::PgTarget` using the synchronous `postgres` crate;
it executes the parameterized transcript through `PgExecutor` against the
explicit migration. Its live-server test is opt-in and is not run by CI.

## Not implemented

- Replication or quorum acknowledgement.
- Multi-site durability.
- Transactional exactly-once coupling between journal position and SBE output.
- Defined RPO/RTO per failure model beyond local process/host recovery.
- Cross-run retention or archival policy.

## Real PostgreSQL target (opt-in)

`src/application/postgres.rs::PgTarget` applies the existing
`build_commit_transcript` / `SqlExecutor` contract through a real
`postgres::Client`. It is an additional target, not a replacement for the
deterministic simulator. `MIGRATION_SQL` creates three tables:
`canonical_business_events`, `delivery_dispositions` and `target_checkpoints`.
Database uniqueness on `idempotency_key` (and on
`(target_id, output_contract_version)` for checkpoints) is the idempotency
authority; the client never relies on an in-memory set. The event, disposition
and checkpoint writes are one `BEGIN ... COMMIT` transaction. Checkpoint updates
must advance both sequence and byte offset. An exact retry at the current
position is allowed only when its event id matches the persisted
`last_idempotency_key`; a different event cannot claim the same position. The
current SHA-256 checkpoint checksum binds the target, position and last event
id. Rows created by the earlier schema migrate with an empty last key and
retain legacy checksum validation; they cannot accept an unverifiable
same-position retry, but can advance to a new position. A zero-row conditional
update rolls the transaction back. Reusing an event key
also compares all persisted canonical event columns, including source identity;
the same key with changed content is rejected and the transaction is rolled
back.

The checkpoint namespace is
`(target_id, source_id, output_contract_version)`. One physical database
target can carry several independent source streams; no adapter has to pretend
a different source is a different target. The checksum binds all three
identity components, sequence, byte offset and last idempotency key. Event and
disposition rows persist `source_id` alongside their canonical provenance.
Legacy rows migrated from the earlier schema retain the explicit empty-source
namespace until an operator calls `assign_legacy_checkpoint_source`; ownership
is never inferred from an event id. `PgTarget::with_source` binds an instance
to one source identity and rejects an envelope whose source does not match.
`PgTarget` accepts exactly one event per delivery and rejects empty or
multi-event batches rather than silently dropping records. A failed statement
triggers `ROLLBACK` and returns an `Uncertain` receipt; a retry with the same
key is resolved as a duplicate by the database.

`tests/postgres_live.rs` is an ignored, opt-in test against an operator-supplied
server. It exercises database idempotency, transaction behavior, checkpoint
progression, lost-acknowledgement recovery and restart from the persisted
checkpoint. The operator reports a PASS against PostgreSQL 18.6. CI does not
run it because no server is provisioned there, and this source export contains
no server transcript or database evidence manifest.

### Guarantee boundaries

No absolute zero-loss claim is made. The following table states what the real
target guarantees and what configuration each guarantee requires.

| Failure | Guarantee | Required configuration |
|---|---|---|
| Process crash before `COMMIT` | Transaction rolls back; no row and no checkpoint advance | Single transaction; driver returns error; `ROLLBACK` attempted |
| Process crash after `COMMIT` but before receipt | At-least-once delivery; retry is deduplicated by the database key | `synchronous_commit = on` |
| Host loss | Committed rows survive if the database server and its storage survive | `synchronous_commit = on`; durable storage with a verified backup |
| Disk loss on the database host | No local guarantee; requires database redundancy | PostgreSQL replication with a documented RPO/RTO |
| Database failover | Acknowledged transactions preserved only with synchronous replication | `synchronous_commit = on` and quorum-synchronous standby |
| Network partition between collector and database | Uncertain receipts; no checkpoint advance in the collector | Fail-closed retry with the same idempotency key |
| Database restart | Checkpoint row and committed rows persist | Normal PostgreSQL crash recovery |

RPO is zero for transactions acknowledged with synchronous commit only when
the storage and synchronous replication configuration above hold. RTO is the
database's own recovery time plus collector restart; this repository does not
measure or claim either.

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
| Live PG statement failure | Transaction rolled back; uncertain receipt; no local checkpoint advance |
| Live PG commit then lost ack | Retry is deduplicated by database key; checkpoint advances once |
| Live PG restart | Persisted checkpoint reloads and sequence continues monotonically |

Any future replication layer must preserve these boundaries and add explicit
quorum acknowledgements; it must not silently weaken local durability.
