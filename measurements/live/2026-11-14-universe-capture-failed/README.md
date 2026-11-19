# 2026-11-14 — full-universe live capture (60 minutes): FAILED, evidence preserved

Raw artifacts from a `LIVE_EVIDENCE_SCOPE=universe`,
`LIVE_DURATION_SECS=3600` run against the live Polymarket public feed using
the reviewed envelope in `config.live-universe.toml`
(`scripts/run-live-evidence.sh`, direct egress, read-only, no order entry).

- Source commit: `5e095a471ab742556654de6032ae47e90629b02c` (unchanged; the
  evidence keeps this identity regardless of later fixes)
- Collector binary: sha256 `f11e5b17ef4d72b3994215ad9ddf35d58c3cf46775e85018400842ee91ba7859`
  (built by the run itself from the commit above)
- Outcome: **FAILED — SBE reconstruction verification refused the capture.**
  Recorded as a failure, not a success; no acceptance criterion was relaxed.

## Declared universe (at discovery time)

Discovery returned **385,694 symbols** in the run's initial round (387,120
added, 1,516 removed over the run; 386,744 symbols present in the capture).
The universe fingerprint is the metadata sidecar itself:

- `markets.json` sha256 `fe5ea58ed20e6cf1b4afa4e70b1401a3b37e4c672d647651b6fc7da6b9919087`
  (370 MB, retained in the run directory, not checked in here)

These counts are the **declared eligible universe at its discovery time**,
not an unqualified claim about every Polymarket instrument.

## What was captured (before the verdict)

| Stage | Value |
|---|---|
| SBE messages | 28,232,751 (8,120,172,544 bytes) |
| Snapshots / deltas | 22,326,814 / 5,905,937 |
| Symbols reconstructed | 386,744 / 386,744 |
| Writer errors | 0 (queue high-water 16385 on the 16384 bound) |
| Journal | finalized: 6,539,955 raw records, 66 segments, 17.7 GB |
| Deltas before first snapshot | 0 |
| Deltas rejected by the collector | 0 |
| Run duration | 3600 s, clean `duration elapsed` stop |

## Why it failed

`verify` refutes delta-reconstruction for **2,755,685 of 22,326,814
snapshots (12.3%)** — see `sbe-verify.json` (`snapshot_mismatches`, with
per-symbol mismatch samples). The collector log shows the mechanism:

- **756 websocket connection errors** across the 78-socket fleet
  (`Connection reset by peer`, `Broken pipe`), beginning about ten minutes
  into the capture;
- **10,051,317 failover snapshots** and **1,204,615 rejected recovery
  snapshots** as every reset shard re-anchored;
- deltas inside reset gaps are protocol-invisibly lost; the next snapshot
  then disagrees with the reconstructed state — precisely the loss class
  the verification gate exists to catch.

Whether the resets originated venue-side (78 concurrent subscriber
sockets) or host-side (an 8 GB machine under memory pressure with a
1.58 GiB book estimate) is **not determined by this run**; the log does
not distinguish them. What the run does establish is that this host plus
this venue topology did not sustain a reconstructable full-universe
capture.

## Configured budget vs. measured reality

| Item | Value |
|---|---|
| Configured book budget | `max_estimated_book_memory_mb = 4096` |
| Planner estimate at startup | `capacity.estimated_book_memory_bytes = 1.58 GiB` |
| Peak RSS | **not observed** — the live path does not sample RSS |
| Swap / memory pressure | not observed — no instrumentation in the live path |
| Disk used by the run | ~26 GB (8.1 GB SBE + 17.7 GB journal + sidecars) |

**Evidence gap:** the live evidence path samples no
process RSS, swap or memory-pressure signal, so "the host could not sustain
the envelope" cannot be separated from "the venue reset a 78-socket
subscriber" by this run alone. Configured budget is a limit, not a
measurement. A follow-up live run should sample RSS/pressure like the
stress harness does before any full-universe retry.

## Preserved files

| File | Content |
|---|---|
| `run-manifest.json` | run-level manifest as written by the evidence script (venues: polymarket failed, kalshi skipped) |
| `report-summary.json` | full core/writer/capacity/coverage/journal/sources sections of the collector report |
| `collector.log` | complete collector log (756 connection errors, recovery churn, clean stop) |
| `sbe-verify.json` + `sbe-verify.log` | the failing verification: mismatch counts and per-symbol samples |
| `hashes.txt` | sha256 of binary, capture, markets.json, report.json, journal manifest |

The full 8.12 GB capture, 370 MB sidecar and 17.7 GB journal were deleted
from this machine after hashing (disk hygiene; the artifacts are not
byte-for-byte reproducible). The sha256 values in `hashes.txt` are the
provenance anchor for what existed; `report.json`, `collector.log` and
`sbe-verify.json` remain under `captures/live-20261114T200044Z/` (34 MB)
for follow-up analysis, and their reduced copies are checked in here.
