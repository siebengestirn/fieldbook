# Baseline after S1/S4 recovery milestones

This report freezes the canonical starting point for the M5 mixed-workload
programme. It does not add features or change behavior.

## Repository identities

| Repository | Role | Branch / commit |
|---|---|---|
| `engineering-challenge-template` | Canonical, connected to `origin` | `feat/mixed-workload-evidence` at `c480da8`, branched from `feat/feed-continuity-contracts` at `c480da8` |
| `voluntas-market-data-next` | Secondary working copy, no remote | local import `7ef1467`, same content as `34befa4` |

Commit history on the recovery branch (published):

| Commit | Increment |
|---|---|
| `966bcaf` | S1a/S1b transport observations and journal v2 |
| `5f1c7fc` | M3.1 typed recovery decisions |
| `6975ae3` | M3.2 independent recovery queues |
| `b6ffd80` | M3.3 per-instrument acknowledgement fences |
| `34befa4` | M4 blast-radius evidence |
| `c480da8` | Documentation reconciliation (no behavior change) |

`feat/mixed-workload-evidence` is the focused branch for M5. The recovery
branch remains the historical implementation of M3/M4.

## Verification evidence

Commands run from the canonical repository on `c480da8`:

```text
cargo fmt --all -- --check                                  pass
cargo clippy --all-targets --all-features -- -D warnings    pass
cargo test --all-targets                                    501 passed, 0 failed, 9 ignored
cargo test --doc                                            1 passed, 0 failed
scripts/assurance/polish.sh                                 RESULT: PASS
python3 scripts/check_matrix_tests.py                       176 references, 0 errors
python3 scripts/check_architecture.py --check               52 Rust sources mapped
git diff --check                                            clean
```

The ZIP `engineering-challenge-template-34befa4.zip` contains the source
snapshot at `34befa4`; it is not a Git repository or a CI evidence bundle.

## Protected tests retained

- `tests/e2e_mock_exchange.rs::journal_accounts_for_every_delivered_payload`
- `tests/e2e_mock_exchange.rs::journal_accounts_for_control_observations_without_advancing_receive_seq`
- `src/collector/state/tests.rs::quiet_instrument_on_healthy_source_never_escalates_to_source_reconnect`
- `tests/session_recovery.rs::recovery_ack_precedes_snapshot_and_uses_last_payload_fence`
- `tests/session_recovery.rs::reconnect_recovery_advances_epoch_and_resets_sequence`
- `tests/e2e_mock_exchange/failover.rs::single_socket_loss_fails_over_and_emits_anchoring_snapshot`
- `tests/e2e_mock_exchange/failover.rs::silent_subscription_is_detected_and_repaired`

All were exercised in the 501 passing tests above.

## Ignored tests (9)

Ignored tests are opt-in and were not silently represented as passing:

- 1 opt-in scale harness in `tests/e2e_mock_exchange.rs`, driven by `SCALE_*`
  environment variables.
- 5 opt-in full-universe stress scenarios in `tests/full_universe_stress.rs`.
- 1 authenticated Kalshi live test (requires authorized production
  credentials).
- 2 Polymarket live tests (require `RUN_LIVE_TESTS=1` and public network
  access).

None of these is part of the offline correctness gate. They are candidates for
M5/M6 evidence once the workload and venue certifications are in place.

## Performance baselines

No new performance claim is made here. Existing artifacts under
`measurements/` remain the reference; M5 must produce its own versioned
manifest with workload definition, environment, raw results, and reproducible
commands before any throughput or latency statement is upgraded from a claim
to evidence.

## Scope boundary

This baseline freezes S1 and recovery M3/M4 as complete. It does not imply that
the older Platform M2–M8 labels in `docs/architecture-migration.md` (instrument
identity, subscription capabilities, canonical events, Hyperliquid) are
complete. The versioned status table in that document is authoritative for the
distinction.
