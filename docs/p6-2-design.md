# P6.2 design review: segmented local durability

This document settles the six design questions required before implementing
segmented, checkpointed local durability. It is a design, not a claim that the
capabilities already exist. The current `CommitStore` and its `commit-N.json`
files remain the compatibility format; the new format is versioned and
introduced alongside it.

## Scope and invariants

Three histories remain separate and are never collapsed into one flag:

- **Source progress** — what the source emitted and where it can resume.
- **Target durability** — what the PostgreSQL transaction committed.
- **Local recovery evidence** — what the local store durably recorded.

An acknowledgement level is reported explicitly: `Accepted`, `Target
committed`, `Local recovery committed`, or `Fully acknowledged`. The
configured policy determines which level the application exposes as final. A
target commit is never reported as full system durability while a local
append is outstanding.

## 1. Segment commit and the precise sync boundary

A segment is a bounded, sequential range of commit records in one framed file
under `segments/seg-<first:012>-<last:012>.jsonl`. Its header carries a version,
binding, first and last commit sequence, and expected record count. Each
record is a length-prefixed, individually checksummed frame. A finalized
footer carries the record count and a checksum over all frames.

Finalization order is exact:

1. Create `segments/.tmp-<unique>.jsonl`.
2. Append framed records with their checksums.
3. Flush and `File::sync_all()` the temp file.
4. Write and sync the footer.
5. Atomically rename into the committed segment namespace.
6. Open the containing directory and `sync_all()` it.

The segment is committed **only after step 6**. A crash before that leaves a
detectable temp file; it is never silently discarded, because it may contain
acknowledged data. On reopen, any `.tmp-*` file produces
`StoreError::IncompleteSegment` with its exact path. The sync policy is
configurable (`full` performs steps 3, 4 and 6; `none` skips explicit syncs but
still renames atomically) and is recorded in the segment header.

## 2. Exact state in a recovery snapshot

A snapshot is a checksummed JSON file under `snapshots/`. It contains:

- schema version and a checksum over the canonical serialized body;
- the full binding (source id, fingerprint, target id);
- the segment frontier it covers: last finalized segment name, first and last
  sequence, record count and segment checksum;
- the materialized `DurableCheckpoint` after that frontier;
- a bounded Bloom filter of committed event ids for lookup acceleration;
- counts of commits, events, dispositions and quarantines for audit
  reconciliation.

It does **not** contain every historical event id or disposition. It is a
recovery accelerator and never a replacement for the audit log. On startup the
store verifies schema, checksum, binding and frontier, validates the segment
chain up to that frontier, and replays only records after it. Gaps, conflicting
snapshots, incomplete acknowledged segments or unexplained sequence changes
fail closed.

## 3. Querying old event ids and dispositions after compaction

`SegmentedStore::is_committed` and `lookup` use the snapshot Bloom filter only
to prove an id is *definitely absent*. A positive result is never accepted as
proof of commitment: it triggers an exact scan of hot and archived segments
for the full canonical id and disposition. This guarantees no false
"already committed" decision.

The resource contract is explicit: lookup scans segments in descending order
and stops at the first exact match; worst case is proportional to archived
history. A durable on-disk index may be added later without changing the
contract. No age-based deletion removes evidence needed by this lookup.

## 4. Archive verification and restore

Archived segments live under `archive/` with a checksummed `manifest.json`
listing each segment's path, first and last sequence, record count and
checksum. On open the manifest is verified: every listed file must exist,
match its checksum and continue the sequence chain. A missing or corrupt
archived segment is a hard error, never a skipped gap.

`restore_archive(destination)` verifies the manifest and copies each segment
into the destination, returning per-segment checksums. The originals are never
modified. A segment is eligible for archival only after its replacement
evidence is durable and independently verified; archival never deletes the
audit log by itself.

## 5. Missing acknowledged segment

A missing, truncated or checksum-mismatched segment that lies at or below the
snapshot frontier is reported as `StoreError::MissingAcknowledgedSegment` with
the exact expected name. The store refuses to open. It never rewinds the
checkpoint, never invents a replacement record and never continues from a
later sequence, because doing so would acknowledge progress whose evidence no
longer exists. Recovery requires restoring the segment from archive or backup
and re-verifying the chain.

## 6. Bounding growth without losing auditability

Hot recovery segments are capped by count and total bytes. When the cap is
reached, older finalized segments are moved to `archive/` after verification;
the snapshot frontier advances over them. Snapshots are replaced atomically
(temp file, sync, rename, directory sync) and old snapshots are retained until
the replacement is verified.

Recovery memory is bounded to the snapshot, the active segment's records and
the Bloom filter. Historical ids and dispositions are resolved through the
archive lookup in section 3. Storage growth is therefore bounded in the hot
path while the immutable archive retains the complete, independently verifiable
audit history. No policy in this stage deletes historical evidence based on
age alone.

## Migration from the current format

The existing `commit-N.json` files remain valid. `SegmentedStore::migrate_legacy`
verifies every legacy record using the current `load_state`, then writes an
initial snapshot whose frontier covers the full legacy range and seeds the
first segment from the verified records. Legacy files are left in place and are
never rewritten or deleted. Mixed directories (legacy files plus new segments)
fail closed with an explicit error rather than being merged implicitly.

## Testing obligations before merge

- Fault injection at temp creation, frame append, footer write, rename,
  directory sync and snapshot replacement.
- Disk-full, corrupt snapshot, truncated segment, sequence gap, missing
  archived segment and snapshot-plus-subsequent-commits recovery.
- Tests run against the real filesystem; atomic rename and `fsync` behaviour
  are filesystem-dependent and the contract names that boundary.
- Migration test proving a legacy store reopens through the segmented store
  with identical checkpoint, ids, dispositions and quarantine counts.
