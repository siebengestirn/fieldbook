# 2026-11-14 — Polymarket live smoke capture: VERIFIED

`LIVE_EVIDENCE_SCOPE=smoke` (120 s, 5 markets, 1 shard, 1 replica) against
the live public Polymarket feed via `scripts/run-live-evidence.sh`,
direct egress, read-only, no order entry. Source commit
`756197558e38f307590b6bd44f81510cc0ba69f5`.

All eleven evidence checks passed (`venue_matches`,
`discovery_nonempty`, `exchange_snapshot_observed`, `sbe_messages_written`,
`sbe_reconstruction_valid`, `symbol_stream_anchored`, `coverage_valid`,
`coverage_expected_matches_selected`, `coverage_every_expected_anchored`,
`journal_complete`, `journal_records_nonzero`):

```
LIVE_FEED_OK venue=polymarket scope=smoke discovered=10 expected=10
acknowledged=10 anchored=10 exchange_snapshots=10 deltas=516
sbe_messages=556 journal_records=276
```

`evidence.json` carries the artifact hashes; `run-manifest.json` records
the dual-venue state honestly: kalshi was
`skipped_permission_not_confirmed` (no credentials), so this is
single-venue evidence. The bulkier raw capture was deleted after
verification; these JSON artifacts and the hashes are the retained record.
