# Engineering Guarantees

This document distinguishes guarantees that are currently demonstrated from
guarantees that are only intended. A guarantee requires a definition, an
implementation, an adversarial test and retained evidence.

## Demonstrated guarantees (reference workload)

| Guarantee | Definition | Evidence |
|---|---|---|
| Exact-once data payload accounting | Every delivered application payload appears once in the data-only `read_records()` view | `journal_accounts_for_every_delivered_payload`, manifest index |
| Journal integrity | Checksums, counts, bounds, sequence continuity and version checks fail closed | journal tests, `journal verify` |
| Transport observation preservation | Native Ping/Pong payloads, direction and send results survive to the journal | S1a/S1b tests, v1/v2 fixtures |
| Recovery scope isolation | Quiet instruments never reconnect a source; ACKs fence exact instruments | M3/M4 tests |
| Deterministic workload generation | Fixed seed reproduces universe, hot set and event assignment | stress tests, v5+ manifests |
| Complete delta disposition | Every decoded delta has exactly one terminal disposition and totals reconcile | `DeltaDisposition` counters, stress certification |
| Bidirectional stage reconciliation | Shortfall and excess are computed for comparable stage pairs | `two_sided`, ledger acceptance tests |
| Fail-closed certification | Missing evidence, resets or reversed discrepancies block success | adversarial ledger tests, failure manifests |
| Governed integration | Protected `main`, PRs, merge commits, branch cleanup | `git-policy.json`, `docs/git-workflow.md` |
| Bounded streaming reads | A `BufRead` source reads exact line bytes and physical offsets with a buffer capped at `MAX_LINE_BYTES` | `streaming_reads_exact_bytes_offsets_and_fingerprint`, `buffer_capacity_is_bounded_across_many_lines` |
| Content-bound resume | A checkpoint is usable only when source id, full-file SHA-256 and target id all match | `resume_is_bound_to_source_identity_fingerprint_and_target`, `changed_file_cannot_resume_from_a_stale_checkpoint` |
| Atomic local commit unit | One checksummed record carries ids, dispositions/quarantines and the new checkpoint; reopen verifies and fails closed | `commit_and_reopen_preserve_ids_dispositions_and_checkpoint`, `partial_or_corrupt_commit_record_fails_closed`, `sequence_gap_is_rejected_on_reopen` |
| Quarantine before progression | A rejected record advances the checkpoint only after durable quarantine succeeds | `quarantine_must_succeed_before_the_checkpoint_advances` |
| Simulated isolation | Dry-run/replay reports `Simulated`, writes no target and no store, and does not move the checkpoint | `simulated_modes_report_simulated_and_never_write_externally`, `replay_disables_target_side_effects` |
| Independent target obligations | Failure of target A does not suppress delivery to target B | `multi_target_obligations_are_independent` |
| SQL contract shape | Parameterized statements and target-specific idempotency keys; no COMMIT on a non-terminal receipt | `commit_transcript_has_full_shape_and_target_specific_idempotency_key`, `no_commit_is_emitted_for_non_terminal_receipts`, `sql_executor_seam_runs_offline_without_a_database` |

## Explicitly not guaranteed yet

- Replicated or multi-site durability.
- End-to-end exactly-once delivery to external systems.
- Nanosecond clock accuracy or cross-host synchronization.
- Live venue certification.
- Horizontal scalability beyond synthetic loopback evidence.
- A live PostgreSQL connection, driver or server-side transaction.
- Production durability of the P6 local store beyond process/host recovery.
- Performance, throughput, latency or scale for the reference application.

## Terms

- **Correctness certified**: data and state invariants verified for a declared workload.
- **Performance characterized**: throughput/latency/resource measurements exist with reproducible methodology.
- **Capacity certified**: declared scale meets correctness, resource and recovery limits.

Passing one class must never be presented as passing another. The P6 local
store is a real fsync-backed local commit store, not a database, and the
`SqlExecutor` trait is a contract seam, not a connected driver.
