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

## Explicitly not guaranteed yet

- Replicated or multi-site durability.
- End-to-end exactly-once delivery to external systems.
- Nanosecond clock accuracy or cross-host synchronization.
- Live venue certification.
- Horizontal scalability beyond synthetic loopback evidence.
- Generalized source/target connector contracts.
- Destination obligation accounting outside the single SBE sink.

## Terms

- **Correctness certified**: data and state invariants verified for a declared workload.
- **Performance characterized**: throughput/latency/resource measurements exist with reproducible methodology.
- **Capacity certified**: declared scale meets correctness, resource and recovery limits.

Passing one class must never be presented as passing another.
