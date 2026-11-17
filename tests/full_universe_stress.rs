#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    reason = "integration tests drive failure paths with unwrap/expect/panic and print diagnostics"
)]
#![allow(
    clippy::allow_attributes,
    reason = "test crates relax the production lint set wholesale; `expect` cannot express lints that may not fire"
)]

//! Deterministic synthetic full-universe stress laboratory.
//!
//! The synthetic Kalshi venue serves a seeded universe of
//! `STRESS-{seed:04x}-{i:06}` markets (regular challenge-eligible markets
//! plus MVE/closed decoys as negative controls) over real REST pagination
//! and WebSocket subscriptions. Every run drives the production collector
//! path (adapter → discovery → capacity planning → coordinator → books →
//! writer → journal) and is certified with `verify`, `completeness` and
//! `coverage` inside the runner, fail-closed, before any manifest exists.
//!
//! Evidence class: synthetic/loopback. See `docs/stress-lab-design.md`.
//!
//! CI runs the 4k correctness smoke plus deterministic self-tests of the
//! failure fixtures. The full matrix is opt-in (`--ignored`).

#![recursion_limit = "512"]

mod stress_support;

use std::collections::BTreeSet;

use stress_support::{
    rng::SplitMix64,
    runner::{
        BurstProfile, CellConfig, CellOutcome, FailInject, HotSelection, canonical_set_sha256,
        worktree_provenance,
    },
    universe::{GENERATOR_VERSION, SyntheticUniverse},
};

/// Deterministic RNG contract, exercised against pinned vectors.
#[test]
fn splitmix64_is_pinned() {
    let mut rng = SplitMix64::new(0);
    let first: Vec<u64> = (0..4).map(|_| rng.next_u64()).collect();
    assert_eq!(
        first,
        vec![
            0xE220_A839_7B1D_CDAF,
            0x6E78_9E6A_A1B9_65F4,
            0x06C4_5D18_8009_454F,
            0xF88B_B8A8_724C_81EC,
        ]
    );
    assert_eq!(stress_support::rng::PRNG_ALGORITHM_VERSION, 1);
}

/// Universe contract: eligible and decoy sets are disjoint and complete.
#[test]
fn universe_generation_is_disjoint_and_deterministic() {
    let universe = SyntheticUniverse::generate(42, 64, 100_000);
    assert_eq!(GENERATOR_VERSION, 2);
    let eligible = universe.eligible_tickers_sorted();
    let excluded = universe.excluded_tickers_sorted();
    assert_eq!(eligible.len(), 64);
    assert!(!excluded.is_empty());
    for ticker in &excluded {
        assert!(!eligible.contains(ticker));
    }
}

fn headless_cell_config(seed: u64, symbols: usize) -> CellConfig {
    CellConfig {
        seed,
        symbols,
        decoy_fraction_ppm: 20_000,
        shards: Some(1),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 40,
        hot_pair: false,
        uniform_spread: true,
        hot_fraction_ppm: 0,
        hot_event_share_pct: 0,
        hot_selection: HotSelection::Weighted,
        burst: None,
        journal: true,
        limits: Some((20, 2)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    }
}

/// One small smoke cell: anchor a 4k-eligible-symbol universe through the
/// real pipeline, publish deltas, prove delivery with the drain barrier,
/// stop and certify. This is the CI regression gate — invariants only, no
/// throughput claims.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn smoke_cell_anchors_applies_and_certifies() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        seed: 7,
        symbols: 4_000,
        decoy_fraction_ppm: 20_000,
        shards: Some(1),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 200,
        hot_pair: false,
        uniform_spread: true,
        hot_fraction_ppm: 0,
        hot_event_share_pct: 0,
        hot_selection: HotSelection::Weighted,
        burst: None,
        journal: true,
        limits: Some((45, 10)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "smoke cell failed acceptance: reasons={:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");

    assert!(
        !output.anchor_wait_expired,
        "anchoring did not complete in time"
    );
    assert!(output.drain_completed, "drain barrier did not complete");
    assert!(
        output.decoy_absent,
        "an MVE/closed decoy entered the capture"
    );
    assert!(output.verify_valid, "SBE verification failed");
    assert!(
        output.completeness_valid,
        "completeness verification failed"
    );
    assert!(
        output.journal_complete,
        "journal did not finalize completely"
    );
    let coverage = output.coverage.as_ref().expect("coverage must be present");
    assert!(coverage.valid, "coverage not valid: {coverage:?}");
    assert_eq!(coverage.expected, output.eligible_count);
    assert_eq!(coverage.anchored, output.eligible_count);
    assert_eq!(output.capacity_connections, 3); // 1 shard x 2 replicas + 1 lifecycle

    // Independent event accounting must reconcile exactly.
    assert_eq!(output.events_published, 200);
    assert_eq!(output.sbe_deltas, 200);
    assert!(output.ledger.reconciled(), "ledger: {:?}", output.ledger);
    assert_eq!(output.ledger.unexplained_drops, Some(0));
    assert_eq!(output.ledger.persistence_gap, Some(0));

    // The three canonical set hashes must agree, and the expected hash must
    // equal the hash of the independently regenerated universe.
    let universe = SyntheticUniverse::generate(7, 4_000, 20_000);
    let independent: BTreeSet<String> = universe.eligible_tickers_sorted().into_iter().collect();
    let expected_hash = canonical_set_sha256(&independent);
    assert_eq!(coverage.expected_set_sha256, expected_hash);
    assert_eq!(
        canonical_set_sha256(&output.acknowledged_symbols),
        expected_hash
    );
    assert_eq!(
        canonical_set_sha256(&output.anchored_symbols),
        expected_hash
    );
}

/// Fixture: one eligible market receives no snapshot. It must never enter the
/// snapshot-anchored set and the cell must fail closed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_snapshot_fixture_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        fail_inject: Some(FailInject::MissingSnapshot { skip_one: 1 }),
        events: 30,
        limits: Some((10, 2)),
        ..headless_cell_config(11, 200)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        !outcome.acceptance.valid,
        "missing snapshot must fail acceptance"
    );
    assert!(
        outcome
            .acceptance
            .reasons
            .iter()
            .any(|reason| reason.contains("anchor") || reason.contains("coverage")),
        "expected anchor/coverage failure, got {:?}",
        outcome.acceptance.reasons
    );
    if let Some(output) = &outcome.output {
        let universe = SyntheticUniverse::generate(11, 200, 20_000);
        let skipped = universe.eligible_tickers_sorted()[0].clone();
        assert!(
            !output.anchored_symbols.contains(&skipped),
            "the skipped market must not be snapshot-anchored"
        );
        assert!(
            output.acknowledged_symbols.contains(&skipped),
            "the skipped market must still be acknowledged"
        );
    }
}

/// Fixture: one market receives a delta but no snapshot. The delta must not
/// anchor it, the applied count must fall by exactly one, and the event
/// ledger must report the gap.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn update_without_snapshot_fixture_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        fail_inject: Some(FailInject::UpdateOnly { skip_one: 1 }),
        events: 40,
        limits: Some((10, 2)),
        ..headless_cell_config(12, 200)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        !outcome.acceptance.valid,
        "update without anchor must fail acceptance"
    );
    assert!(
        outcome
            .acceptance
            .reasons
            .iter()
            .any(|reason| reason == "unexplained_drops" || reason.contains("coverage")),
        "expected accounting/coverage failure, got {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.unwrap_or_else(|| {
        panic!(
            "fixture cell must produce output; acceptance reasons: {:?}",
            outcome.acceptance.reasons
        )
    });
    let universe = SyntheticUniverse::generate(12, 200, 20_000);
    let skipped = universe.eligible_tickers_sorted()[0].clone();
    assert!(
        !output.anchored_symbols.contains(&skipped),
        "an update-only symbol must not be counted as snapshot-anchored"
    );
    assert_eq!(
        output.sbe_deltas,
        output.events_published as u64 - 1,
        "exactly the update-only delta must be missing from the capture"
    );
    assert!(
        output
            .ledger
            .unexplained_drops
            .is_some_and(|drops| drops >= 1),
        "ledger must report the unaccounted delta: {:?}",
        output.ledger
    );
}

/// Fixture: delta frames dropped before the wire. With two replicas the
/// first `count` frames vanish in replica-pair order, so exactly
/// `count / 2` events are lost and only event accounting can catch it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dropped_delta_fixture_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        fail_inject: Some(FailInject::DroppedFrames { count: 10 }),
        events: 100,
        ..headless_cell_config(13, 300)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        !outcome.acceptance.valid,
        "dropped frames must fail acceptance"
    );
    assert!(
        outcome
            .acceptance
            .reasons
            .iter()
            .any(|reason| reason == "unexplained_drops"),
        "expected unexplained_drops, got {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.unwrap_or_else(|| {
        panic!(
            "fixture cell must produce output; acceptance reasons: {:?}",
            outcome.acceptance.reasons
        )
    });
    assert_eq!(
        output.sbe_deltas,
        output.events_published as u64 - 5,
        "exactly the injected events must be missing from the capture"
    );
    assert_eq!(output.mock.injected_pre_wire_drops, 10);
    assert_eq!(output.ledger.unexplained_drops, Some(5));
}

/// Fixture: the production writer queue bound is set to its minimum and the
/// final-snapshot burst must drive it to saturation. The channel bound must
/// hold (the counter observes it with at most one slot of lag), the run must
/// either reconcile under the visible backpressure (success with evidence)
/// or fail — never silently pass.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn writer_queue_saturation_fixture_preserves_accounting() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        writer_queue: Some(128),
        events: 2_000,
        hot_pair: true,
        ..headless_cell_config(15, 2_000)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    let output = outcome.output.unwrap_or_else(|| {
        panic!(
            "writer fixture must produce output; acceptance reasons: {:?}",
            outcome.acceptance.reasons
        )
    });
    assert!(
        output.writer_queue_high_water >= 128,
        "the writer queue must reach its configured bound (high-water {})",
        output.writer_queue_high_water
    );
    assert!(
        output.writer_queue_high_water <= 129,
        "the channel bound must hold; only the counter's one-slot \
         observation lag may exceed it (high-water {})",
        output.writer_queue_high_water
    );
    assert_eq!(
        outcome.acceptance.valid,
        output.ledger.reconciled(),
        "valid ⟺ reconciled; reasons={:?} ledger={:?}",
        outcome.acceptance.reasons,
        output.ledger
    );
}

/// Fixture: the mock's outbound queue is bounded hard and the forwarder is
/// stalled. Saturation must be visible and the drain barrier must refuse to
/// certify the run.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mock_queue_saturation_fixture_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        fail_inject: Some(FailInject::MockQueueSaturate { capacity: 8 }),
        events: 500,
        limits: Some((8, 2)),
        ..headless_cell_config(14, 200)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        !outcome.acceptance.valid,
        "stalled mock queue must fail acceptance"
    );
    let output = outcome.output.unwrap_or_else(|| {
        panic!(
            "saturation fixture must produce output; acceptance reasons: {:?}",
            outcome.acceptance.reasons
        )
    });
    assert!(
        output.mock.queue_saturated > 0,
        "the bounded queue must record saturation events"
    );
    assert!(
        output.mock.queue_high_water <= 8,
        "the mock queue bound must be respected"
    );
    assert!(
        outcome
            .acceptance
            .reasons
            .iter()
            .any(|reason| reason == "drain_incomplete"),
        "unexpected reasons: {:?}",
        outcome.acceptance.reasons
    );
}

/// Fixture: shutdown requested before the drain barrier completes. The cell
/// must never certify as success.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shutdown_before_drain_fixture_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        fail_inject: Some(FailInject::ShutdownBeforeDrain),
        events: 50,
        ..headless_cell_config(16, 100)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        !outcome.acceptance.valid,
        "shutdown before drain must not produce a success manifest"
    );
    assert!(
        !outcome.acceptance.valid
            && outcome
                .acceptance
                .reasons
                .iter()
                .any(|reason| reason == "drain_incomplete" || reason == "unexplained_drops"),
        "unexpected reasons: {:?}",
        outcome.acceptance.reasons
    );
}

/// Fixture: the index document is corrupt; the cell must not produce a
/// success artifact and the run must fail closed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn malformed_index_fixture_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("manifests");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("index.json"), b"this is not json").unwrap();

    let config = CellConfig {
        seed: 17,
        symbols: 60,
        decoy_fraction_ppm: 20_000,
        shards: Some(1),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 5,
        hot_pair: false,
        uniform_spread: true,
        hot_fraction_ppm: 0,
        hot_event_share_pct: 0,
        hot_selection: HotSelection::Weighted,
        burst: None,
        journal: false,
        limits: Some((20, 2)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    };
    let outcome =
        stress_support::runner::run_cell(directory.path(), &config, Some(out.as_path())).await;
    assert!(
        !outcome.acceptance.valid,
        "a corrupt index must flip the cell to a failed artifact, got {:?}",
        outcome.acceptance
    );
    assert_eq!(
        outcome.acceptance.failure_class.as_deref(),
        Some("manifest_index_error"),
        "corrupt index must be reported as manifest_index_error"
    );
    let manifest_files: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        manifest_files,
        vec!["index.json".to_owned()],
        "no manifest artifact may be written when the index is malformed"
    );
    let corrupt_index = std::fs::read_to_string(out.join("index.json")).unwrap();
    assert_eq!(
        corrupt_index, "this is not json",
        "corrupt index must not be overwritten"
    );
}

/// The clean-worktree gate must detect tracked modifications and hash the
/// diff, while staying silent about untracked files.
#[test]
fn worktree_gate_detects_dirty_tracked_files() {
    fn git(repo: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .status()
            .expect("git must be available");
        assert!(status.success(), "git {args:?} failed");
    }
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q"]);
    std::fs::write(repo.path().join("tracked.txt"), "original\n").unwrap();
    git(repo.path(), &["add", "tracked.txt"]);
    git(
        repo.path(),
        &[
            "-c",
            "user.email=stress@example.invalid",
            "-c",
            "user.name=stress",
            "commit",
            "-q",
            "-m",
            "init",
        ],
    );

    let clean = worktree_provenance(repo.path());
    assert_eq!(clean.tracked_clean, Some(true));
    assert!(clean.tracked_modified.is_empty());
    assert!(clean.tracked_diff_sha256.is_none());

    std::fs::write(repo.path().join("tracked.txt"), "modified\n").unwrap();
    let dirty = worktree_provenance(repo.path());
    assert_eq!(dirty.tracked_clean, Some(false));
    assert!(
        dirty
            .tracked_modified
            .iter()
            .any(|entry| entry.contains("tracked.txt"))
    );
    assert!(dirty.tracked_diff_sha256.is_some());
    // Untracked files are recorded but never mark the worktree dirty.
    std::fs::write(repo.path().join("untracked.txt"), "x\n").unwrap();
    let with_untracked = worktree_provenance(repo.path());
    assert_eq!(with_untracked.tracked_clean, Some(false));
    assert_eq!(with_untracked.untracked_count, Some(1));
}

/// Fixture: a run executed from a dirty worktree must refuse to start.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dirty_worktree_fixture_fails_closed() {
    let repo = tempfile::tempdir().unwrap();
    let init = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .status()
            .expect("git must be available");
        assert!(status.success());
    };
    init(&["init", "-q"]);
    std::fs::write(repo.path().join("tracked.txt"), "original\n").unwrap();
    init(&["add", "tracked.txt"]);
    init(&[
        "-c",
        "user.email=stress@example.invalid",
        "-c",
        "user.name=stress",
        "commit",
        "-q",
        "-m",
        "init",
    ]);
    std::fs::write(repo.path().join("tracked.txt"), "dirty\n").unwrap();

    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        symbols: 60,
        events: 5,
        limits: Some((5, 2)),
        worktree_dir: Some(repo.path().to_path_buf()),
        require_clean_worktree: Some(true),
        ..headless_cell_config(18, 60)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        !outcome.acceptance.valid,
        "dirty worktree must fail acceptance"
    );
    assert_eq!(
        outcome.acceptance.failure_class.as_deref(),
        Some("dirty_worktree")
    );
    assert!(
        outcome.output.is_none(),
        "a gated-off cell must not produce collector evidence"
    );
}

/// The same gate must let a clean repository run to completion.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn worktree_gate_accepts_clean_repository() {
    let repo = tempfile::tempdir().unwrap();
    let init = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .status()
            .expect("git must be available");
        assert!(status.success());
    };
    init(&["init", "-q"]);
    std::fs::write(repo.path().join("tracked.txt"), "clean\n").unwrap();
    init(&["add", "tracked.txt"]);
    init(&[
        "-c",
        "user.email=stress@example.invalid",
        "-c",
        "user.name=stress",
        "commit",
        "-q",
        "-m",
        "init",
    ]);

    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        symbols: 60,
        events: 5,
        limits: Some((20, 2)),
        worktree_dir: Some(repo.path().to_path_buf()),
        require_clean_worktree: Some(true),
        ..headless_cell_config(19, 60)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "a clean worktree must pass the gate: {:?}",
        outcome.acceptance.reasons
    );
}

/// Deliberately-failing reference cell: forces a capacity-plan rejection so
/// a failure manifest is produced on demand (ignored in CI; run locally to
/// regenerate `measurements/stress/reference-*.json`). The manifest preserves
/// the exact planner rejection reason — an infeasible cell is recorded, not
/// silently skipped.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly"]
async fn reference_failed_cell_produces_manifest() {
    assert!(
        std::env::var("STRESS_MANIFEST_OUT").is_ok(),
        "set STRESS_MANIFEST_OUT to produce the artifact"
    );
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        seed: 9,
        symbols: 1_000,
        force_capacity_reject: true,
        ..CellConfig::default()
    };
    let outcome =
        stress_support::runner::run_cell(directory.path(), &config, Some(manifest_out().as_path()))
            .await;
    assert!(
        !outcome.acceptance.valid,
        "capacity rejection must fail acceptance"
    );
    assert!(
        outcome
            .acceptance
            .failure_class
            .as_deref()
            .is_some_and(|class| class.starts_with("collector_terminated")),
        "unexpected failure class: {:?}",
        outcome.acceptance.failure_class
    );
}

/// Reference generator: the successful 4k smoke manifest.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly"]
async fn reference_success_manifest_is_reproducible() {
    assert!(
        std::env::var("STRESS_MANIFEST_OUT").is_ok(),
        "set STRESS_MANIFEST_OUT to produce the artifact"
    );
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        seed: 7,
        symbols: 4_000,
        decoy_fraction_ppm: 20_000,
        shards: Some(1),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 200,
        hot_pair: false,
        uniform_spread: true,
        hot_fraction_ppm: 0,
        hot_event_share_pct: 0,
        hot_selection: HotSelection::Weighted,
        burst: None,
        journal: true,
        limits: Some((45, 10)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    };
    let outcome =
        stress_support::runner::run_cell(directory.path(), &config, Some(manifest_out().as_path()))
            .await;
    assert!(
        outcome.acceptance.valid,
        "reference smoke must pass: {:?}",
        outcome.acceptance.reasons
    );
}

/// Reference generator: failed manifest for the missing-anchor injection.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly"]
async fn reference_missing_snapshot_manifest() {
    assert!(
        std::env::var("STRESS_MANIFEST_OUT").is_ok(),
        "set STRESS_MANIFEST_OUT to produce the artifact"
    );
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        fail_inject: Some(FailInject::MissingSnapshot { skip_one: 1 }),
        events: 30,
        limits: Some((10, 2)),
        ..headless_cell_config(11, 200)
    };
    let outcome =
        stress_support::runner::run_cell(directory.path(), &config, Some(manifest_out().as_path()))
            .await;
    assert!(!outcome.acceptance.valid);
}

/// Reference generator: failed manifest for the dropped-frame injection.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly"]
async fn reference_dropped_frame_manifest() {
    assert!(
        std::env::var("STRESS_MANIFEST_OUT").is_ok(),
        "set STRESS_MANIFEST_OUT to produce the artifact"
    );
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        fail_inject: Some(FailInject::DroppedFrames { count: 10 }),
        events: 100,
        ..headless_cell_config(13, 300)
    };
    let outcome =
        stress_support::runner::run_cell(directory.path(), &config, Some(manifest_out().as_path()))
            .await;
    assert!(!outcome.acceptance.valid);
}

/// Reference generator: bounded-queue saturation result.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly"]
async fn reference_queue_saturation_manifest() {
    assert!(
        std::env::var("STRESS_MANIFEST_OUT").is_ok(),
        "set STRESS_MANIFEST_OUT to produce the artifact"
    );
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        fail_inject: Some(FailInject::MockQueueSaturate { capacity: 8 }),
        events: 500,
        limits: Some((8, 2)),
        ..headless_cell_config(14, 200)
    };
    let outcome =
        stress_support::runner::run_cell(directory.path(), &config, Some(manifest_out().as_path()))
            .await;
    assert!(!outcome.acceptance.valid);
    let output = outcome.output.unwrap_or_else(|| {
        panic!(
            "saturation fixture must produce output; acceptance reasons: {:?}",
            outcome.acceptance.reasons
        )
    });
    assert!(output.mock.queue_saturated > 0);
}

fn manifest_out() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("STRESS_MANIFEST_OUT").expect("STRESS_MANIFEST_OUT is checked by callers"),
    )
}

/// The success artifact must be complete, indexed and hash-verified before the
/// cell reports success. This is the end-to-end acceptance rule.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn successful_manifest_is_complete_and_published() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("manifests");
    let config = CellConfig {
        seed: 21,
        symbols: 60,
        events: 5,
        limits: Some((20, 2)),
        ..headless_cell_config(21, 60)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, Some(&out)).await;
    assert!(
        outcome.acceptance.valid,
        "reasons={:?}",
        outcome.acceptance.reasons
    );

    let index = stress_support::manifest::read_index(&out.join("index.json")).unwrap();
    let cells = index["cells"].as_object().expect("cells object");
    let (cell_id, entry) = cells.iter().next().expect("one published cell");
    assert_eq!(
        entry["publication_state"].as_str(),
        Some("published"),
        "cell {cell_id} is not published"
    );
    let body_path = out.join(entry["path"].as_str().expect("path"));
    let bytes = std::fs::read(&body_path).unwrap();
    assert_eq!(
        entry["body_sha256"].as_str(),
        Some(engineering_challenge::util::sha256_hex(&bytes).as_str()),
        "the indexed hash must match the published body"
    );

    let document: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&bytes).unwrap();
    let missing = stress_support::manifest::missing_required_fields(&document);
    assert!(missing.is_empty(), "missing required fields: {missing:?}");
    assert_eq!(document["run_status"].as_str(), Some("success"));
    assert!(
        document["incidents"]["discontinuity_incidents"].is_u64(),
        "incidents.discontinuity_incidents must be an exported count"
    );
}

/// A directory in place of the index is an I/O error, not an empty index, and
/// the cell must publish no body.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn index_directory_fixture_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("manifests");
    std::fs::create_dir_all(out.join("index.json")).unwrap();

    let config = CellConfig {
        seed: 22,
        symbols: 60,
        events: 5,
        limits: Some((20, 2)),
        ..headless_cell_config(22, 60)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, Some(&out)).await;
    assert!(!outcome.acceptance.valid, "an unreadable index must fail");
    assert_eq!(
        outcome.acceptance.failure_class.as_deref(),
        Some("manifest_index_error")
    );
    let entries: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        entries,
        vec!["index.json".to_owned()],
        "no body may be published when the index cannot be read"
    );
}

/// A permission error is likewise an error, not an empty index. The test
/// skips itself when the runner is privileged enough to bypass the mode.
#[cfg(unix)]
#[test]
fn unreadable_index_fails_closed() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("index.json");
    std::fs::write(&path, br#"{"schema_version":1,"cells":{}}"#).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_to_string(&path).is_ok() {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        return;
    }
    let error = stress_support::manifest::read_index(&path)
        .expect_err("an unreadable index must not read as empty");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

/// Two-sided accounting: a collector that applies more than the expected
/// ledger must fail with an over-application class. The fixture publishes
/// duplicate wire frames for already-generated logical events; a dedup-correct
/// collector would still apply one copy, so `applied > expected` is exactly
/// the duplicate-application defect the audit describes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn over_application_fixture_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let duplicates = 7usize;
    let config = CellConfig {
        fail_inject: Some(FailInject::OverApplication { duplicates }),
        events: 60,
        limits: Some((20, 3)),
        ..headless_cell_config(16, 200)
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        !outcome.acceptance.valid,
        "over-application must fail acceptance: reasons={:?}",
        outcome.acceptance.reasons
    );
    assert!(
        outcome
            .acceptance
            .reasons
            .iter()
            .any(|reason| reason == "over_application"),
        "expected over_application, got {:?}",
        outcome.acceptance.reasons
    );
    assert!(
        outcome
            .acceptance
            .reasons
            .iter()
            .any(|reason| reason == "duplicate_application"),
        "expected duplicate_application, got {:?}",
        outcome.acceptance.reasons
    );

    let output = outcome.output.unwrap_or_else(|| {
        panic!(
            "fixture cell must produce output; acceptance reasons: {:?}",
            outcome.acceptance.reasons
        )
    });
    assert_eq!(
        output.ledger.over_applied,
        Some(duplicates as u64),
        "every duplicate frame must be counted as over-application: {:?}",
        output.ledger
    );
    assert_eq!(
        output.ledger.application_shortfall,
        Some(0),
        "the correct ledger was fully applied: {:?}",
        output.ledger
    );
    assert!(
        !output.ledger.reconciled(),
        "an over-applied ledger must not reconcile"
    );
    assert!(
        output
            .ledger
            .suppressed_replica_duplicates
            .is_some_and(|count| count > 0),
        "the shadow replica must count suppressed transport duplicates: {:?}",
        output.ledger
    );
}

/// The ledger arithmetic is two-sided: a clean ledger reconciles, and each
/// direction of every gap fails independently.
#[test]
fn ledger_reconciliation_is_two_sided() {
    use stress_support::runner::{ledger_acceptance_reasons, two_sided};

    fn ledger(applied: u64, sbe: u64) -> stress_support::runner::EventLedger {
        stress_support::runner::EventLedger {
            generated_events: 100,
            expected_applied: 100,
            applied_events: Some(applied),
            sbe_deltas: Some(sbe),
            suppressed_replica_duplicates: Some(applied),
            unexplained_drops: Some(100u64.saturating_sub(applied)),
            application_shortfall: Some(100u64.saturating_sub(applied)),
            over_applied: Some(applied.saturating_sub(100)),
            persistence_gap: Some(applied.saturating_sub(sbe)),
            persistence_excess: Some(sbe.saturating_sub(applied)),
            enqueued_deltas: Some(100),
            transmitted_deltas: Some(100),
            received_frames: Some(100),
            delta_events_decoded: Some(applied),
            decode_errors: Some(0),
            core_ingress_events: Some(100),
            core_delta_events: Some(applied),
            no_active_source: Some(0),
            unknown_symbol: Some(0),
            non_live: Some(0),
            unassigned_source: Some(0),
            invalid_book: Some(0),
            shadow_clock_regression: Some(0),
            zero_noop: Some(0),
            sbe_conversion_failed: Some(0),
            disposition_shortfall: Some(0),
            disposition_excess: Some(0),
            writer_delta_admissions: Some(applied),
            writer_delta_writes: Some(applied),
            enqueue_transmit_shortfall: Some(0),
            enqueue_transmit_excess: Some(0),
            transmit_decode_shortfall: Some(0),
            transmit_decode_excess: Some(0),
            writer_admission_shortfall: Some(0),
            writer_admission_excess: Some(0),
            writer_write_shortfall: Some(0),
            writer_write_excess: Some(0),
            decode_failure_count: Some(0),
            ..Default::default()
        }
    }

    // Direction arithmetic itself: both sides are computed, not clamped.
    assert_eq!(two_sided(Some(100), Some(100)), (Some(0), Some(0)));
    assert_eq!(two_sided(Some(100), Some(95)), (Some(5), Some(0)));
    assert_eq!(two_sided(Some(100), Some(105)), (Some(0), Some(5)));
    assert_eq!(two_sided(None, Some(100)), (None, None));

    // Real ledger-construction helper, with the wire count first.
    use stress_support::runner::transmit_decode_gaps;
    assert_eq!(
        transmit_decode_gaps(Some(100), Some(100)),
        (Some(0), Some(0))
    );
    assert_eq!(
        transmit_decode_gaps(Some(100), Some(95)),
        (Some(5), Some(0))
    );
    assert_eq!(
        transmit_decode_gaps(Some(100), Some(105)),
        (Some(0), Some(5))
    );

    assert!(ledger(100, 100).reconciled(), "exact accounting reconciles");
    assert!(
        !ledger(90, 90).reconciled(),
        "a shortfall must not reconcile"
    );
    assert!(
        !ledger(110, 110).reconciled(),
        "over-application must not reconcile"
    );
    assert!(
        !ledger(100, 101).reconciled(),
        "persistence excess must not reconcile"
    );
    assert!(
        !ledger(101, 100).reconciled(),
        "persistence gap must not reconcile"
    );

    // The certification path, not just the helper, must reject each defect.
    for (mut bad, reason) in [
        (ledger(100, 100), "pre_wire_loss"),
        (ledger(100, 100), "enqueue_excess"),
        (ledger(100, 100), "decode_failures"),
        (ledger(100, 100), "receive_evidence_missing"),
        (ledger(100, 100), "decode_evidence_missing"),
        (ledger(100, 100), "writer_admission_shortfall"),
        (ledger(100, 100), "writer_admission_excess"),
        (ledger(100, 100), "writer_write_shortfall"),
        (ledger(100, 100), "writer_write_excess"),
        (ledger(100, 100), "writer_admission_evidence_missing"),
        (ledger(100, 100), "writer_write_evidence_missing"),
        (ledger(100, 100), "transmit_decode_evidence_missing"),
        (ledger(100, 100), "replica_duplicate_evidence_missing"),
    ] {
        match reason {
            "pre_wire_loss" => bad.enqueue_transmit_shortfall = Some(1),
            "enqueue_excess" => bad.enqueue_transmit_excess = Some(1),
            "decode_failures" => bad.decode_failure_count = Some(1),
            "receive_evidence_missing" => bad.received_frames = None,
            "decode_evidence_missing" => bad.delta_events_decoded = None,
            "writer_admission_shortfall" => bad.writer_admission_shortfall = Some(1),
            "writer_admission_excess" => bad.writer_admission_excess = Some(1),
            "writer_write_shortfall" => bad.writer_write_shortfall = Some(1),
            "writer_write_excess" => bad.writer_write_excess = Some(1),
            "writer_admission_evidence_missing" => bad.writer_delta_admissions = None,
            "writer_write_evidence_missing" => bad.writer_delta_writes = None,
            "transmit_decode_evidence_missing" => bad.transmit_decode_shortfall = None,
            "replica_duplicate_evidence_missing" => bad.suppressed_replica_duplicates = None,
            _ => unreachable!(),
        }
        let reasons = ledger_acceptance_reasons(&bad, 0);
        assert!(
            reasons.iter().any(|found| found == reason),
            "expected {reason} to block certification, got {reasons:?}"
        );
        if reason != "replica_duplicate_evidence_missing" {
            assert!(!bad.reconciled(), "{reason} must not reconcile");
        }
    }
}

#[test]
fn historical_zero_receive_manifest_is_preserved_as_superseded_evidence() {
    // Diagnostic regression: the v3 coverage-mismatch manifest recorded zero
    // received frames because the earlier sampler read per-source status
    // gauges that reset across reconnects. The artifact is immutable evidence;
    // this test pins that it remains byte-for-byte available and is not used
    // as a current certification claim.
    let raw =
        include_str!("../measurements/stress/failed-v3-seed-000b-200-coverage_set_mismatch.json");
    let manifest: serde_json::Value = serde_json::from_str(raw).expect("manifest JSON");
    assert_eq!(manifest["run_status"], "failed");
    assert_eq!(manifest["event_ledger"]["received_frames"], 0);
    assert_eq!(manifest["event_ledger"]["reconciled"], true);
    assert_eq!(manifest["scenario_version"], 3);
}

#[test]
fn counter_reset_never_becomes_a_zero_count() {
    use stress_support::runner::metric_window_delta;
    assert_eq!(metric_window_delta(Some(10), Some(15)), Some(5));
    assert_eq!(metric_window_delta(Some(10), Some(10)), Some(0));
    assert_eq!(metric_window_delta(Some(15), Some(10)), None);
    assert_eq!(metric_window_delta(None, Some(10)), None);
    assert_eq!(metric_window_delta(Some(10), None), None);
}

/// M5 profile: 4,000 instruments with exactly 5% designated hot instruments
/// receiving 80% of deltas. The run must reconcile the full event ledger and
/// certify the exact universe; no throughput claim is made here.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mixed_hot_quiet_4000_cell_reconciles_and_certifies() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        seed: 23,
        symbols: 4_000,
        decoy_fraction_ppm: 20_000,
        shards: Some(2),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 400,
        hot_pair: false,
        uniform_spread: false,
        hot_fraction_ppm: 50_000,
        hot_event_share_pct: 80,
        hot_selection: HotSelection::Weighted,
        burst: None,
        journal: true,
        limits: Some((45, 10)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "mixed hot/quiet cell failed acceptance: reasons={:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_eq!(
        output.hot_tickers, 200,
        "5% of 4,000 instruments must be hot"
    );
    assert_eq!(output.events_published, 400);
    assert_eq!(
        output.hot_events + output.cold_events,
        output.events_published,
        "weighted draws must split the workload into disjoint hot/cold sets"
    );
    assert!(output.hot_events > 0 && output.cold_events > 0);
    assert_eq!(output.sbe_deltas, 400);
    assert!(
        output.ledger.reconciled(),
        "ledger must reconcile: {:?}",
        output.ledger
    );
    assert_eq!(output.ledger.unexplained_drops, Some(0));
    assert_eq!(output.ledger.persistence_gap, Some(0));
    let coverage = output.coverage.as_ref().expect("coverage must be present");
    assert_eq!(coverage.expected, 4_000);
    assert_eq!(coverage.anchored, 4_000);
    assert!(output.verify_valid && output.completeness_valid && output.journal_complete);
}

/// Reference generator for the M5 mixed workload manifest.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly"]
async fn reference_mixed_hot_quiet_manifest_is_reproducible() {
    assert!(
        std::env::var("STRESS_MANIFEST_OUT").is_ok(),
        "set STRESS_MANIFEST_OUT to produce the artifact"
    );
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        seed: 23,
        symbols: 4_000,
        decoy_fraction_ppm: 20_000,
        shards: Some(2),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 400,
        hot_pair: false,
        uniform_spread: false,
        hot_fraction_ppm: 50_000,
        hot_event_share_pct: 80,
        hot_selection: HotSelection::Weighted,
        burst: None,
        journal: true,
        limits: Some((45, 10)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    };
    let outcome =
        stress_support::runner::run_cell(directory.path(), &config, Some(manifest_out().as_path()))
            .await;
    assert!(
        outcome.acceptance.valid,
        "reference mixed cell failed: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_eq!(output.hot_tickers, 200);
    assert!(output.ledger.reconciled());
}

/// M5 exact-quota profile: the same 4,000-instrument universe receives exactly
/// 320 hot and 80 cold logical deltas. Event assignment, not just membership,
/// is pinned by the seed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mixed_hot_quiet_exact_4000_cell_assigns_320_hot_events() {
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        seed: 29,
        symbols: 4_000,
        decoy_fraction_ppm: 20_000,
        shards: Some(2),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 400,
        hot_pair: false,
        uniform_spread: false,
        hot_fraction_ppm: 50_000,
        hot_event_share_pct: 80,
        hot_selection: HotSelection::ExactQuota,
        burst: None,
        journal: true,
        limits: Some((45, 10)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    };
    let outcome = stress_support::runner::run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "exact-quota cell failed acceptance: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_eq!(output.hot_tickers, 200);
    assert_eq!(
        output.hot_events, 320,
        "exactly 80% of 400 events must be hot"
    );
    assert_eq!(output.cold_events, 80);
    assert_eq!(
        output.hot_events + output.cold_events,
        output.events_published
    );
    assert!(output.ledger.reconciled(), "ledger: {:?}", output.ledger);
    let universe = SyntheticUniverse::generate(29, 4_000, 20_000);
    let hot = universe.hot_tickers(50_000);
    assert_eq!(hot.len(), 200);
    assert!(hot.iter().all(|ticker| ticker.starts_with("STRESS-")));
}

/// Reference generator for the exact-quota M5 profile.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly"]
async fn reference_exact_hot_quiet_manifest_is_reproducible() {
    assert!(
        std::env::var("STRESS_MANIFEST_OUT").is_ok(),
        "set STRESS_MANIFEST_OUT to produce the artifact"
    );
    let directory = tempfile::tempdir().unwrap();
    let config = CellConfig {
        seed: 29,
        symbols: 4_000,
        decoy_fraction_ppm: 20_000,
        shards: Some(2),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 400,
        hot_pair: false,
        uniform_spread: false,
        hot_fraction_ppm: 50_000,
        hot_event_share_pct: 80,
        hot_selection: HotSelection::ExactQuota,
        burst: None,
        journal: true,
        limits: Some((45, 10)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    };
    let outcome =
        stress_support::runner::run_cell(directory.path(), &config, Some(manifest_out().as_path()))
            .await;
    assert!(outcome.acceptance.valid, "{:?}", outcome.acceptance.reasons);
    let output = outcome.output.expect("valid cell must carry output");
    assert_eq!(output.hot_events, 320);
    assert_eq!(output.cold_events, 80);
}

// ---------- M5.2 deterministic burst workloads ----------

/// The 4,000-instrument exact/role workload reused for burst cells. Only the
/// phase decomposition changes; seed, universe, hot set and event budget stay
/// identical to the frozen M5.1 exact profile.
fn burst_cell_config(profile: BurstProfile, seed: u64, selection: HotSelection) -> CellConfig {
    CellConfig {
        seed,
        symbols: 4_000,
        decoy_fraction_ppm: 20_000,
        shards: Some(2),
        replicas: 2,
        lifecycle_replicas: 1,
        events: 400,
        hot_pair: false,
        uniform_spread: false,
        hot_fraction_ppm: 50_000,
        hot_event_share_pct: 80,
        hot_selection: selection,
        burst: Some(profile),
        journal: true,
        limits: Some((45, 10)),
        force_capacity_reject: false,
        fail_inject: None,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
    }
}

/// Drive one burst cell and prove the full M5.2 contract: acceptance, exact
/// per-phase publication with cumulative drain barriers, reconciled ledger,
/// exact coverage, SBE/completeness/journal verification, empty fault
/// schedule and a manifest that records the explicit workload schedule.
async fn assert_burst_cell(
    profile: BurstProfile,
    seed: u64,
    selection: HotSelection,
    manifest_out: Option<&std::path::Path>,
) -> CellOutcome {
    let directory = tempfile::tempdir().unwrap();
    let scratch_manifests = directory.path().join("manifests");
    let out = manifest_out.unwrap_or(scratch_manifests.as_path());
    let config = burst_cell_config(profile, seed, selection);
    let outcome = stress_support::runner::run_cell(directory.path(), &config, Some(out)).await;
    assert!(
        outcome.acceptance.valid,
        "burst {profile:?} cell failed acceptance: {:?}; ledger={:?}",
        outcome.acceptance.reasons,
        outcome.output.as_ref().map(|o| &o.ledger)
    );
    let phase_count = outcome
        .output
        .as_ref()
        .expect("valid cell must carry output")
        .phase_results
        .len();
    {
        let output = outcome.output.as_ref().expect("valid cell must carry output");

        // Schedule is explicit, deterministic and fault-free for M5.2.
        assert_eq!(output.schedule.profile, profile.label());
        assert_eq!(output.schedule.seed, seed);
        assert_eq!(output.schedule.instrument_count, 4_000);
        assert_eq!(output.schedule.eligible_count, 4_000);
        assert_eq!(output.schedule.expected_total_events, 400);
        assert!(output.schedule.fault_schedule.is_empty());
        assert_eq!(output.schedule.hot_set.member_count, 200);
        assert_eq!(output.schedule.phases.len(), phase_count);

        // Every phase published its exact declared count, drained, and the
        // cumulative drain target matches the events published so far.
        let mut running = 0u64;
        for (declared, actual) in output.schedule.phases.iter().zip(&output.phase_results) {
            assert_eq!(actual.published, declared.events, "phase {} count", actual.name);
            running += actual.published as u64;
            assert_eq!(actual.cumulative_expected, running);
            assert!(actual.drain_completed, "phase {} did not drain", actual.name);
        }
        assert_eq!(running, 400);
        let phase_total: usize = output.phase_results.iter().map(|phase| phase.published).sum();
        assert_eq!(phase_total, 400);
        assert_eq!(output.events_published, 400);
        assert_eq!(output.hot_events + output.cold_events, 400);
        assert!(output.drain_completed, "final drain did not complete");

        // One final ledger reconciles the whole multi-phase run.
        assert!(output.ledger.reconciled(), "ledger: {:?}", output.ledger);
        assert_eq!(output.ledger.unexplained_drops, Some(0));
        assert_eq!(output.ledger.application_shortfall, Some(0));
        assert_eq!(output.ledger.over_applied, Some(0));
        assert_eq!(output.ledger.persistence_gap, Some(0));
        assert_eq!(output.applied_events, Some(400));
        assert_eq!(output.sbe_deltas, 400);

        // Exact coverage and independent verification still hold at final.
        let coverage = output.coverage.as_ref().expect("coverage must be present");
        assert!(coverage.valid, "coverage invalid: {coverage:?}");
        assert_eq!(coverage.expected, 4_000);
        assert_eq!(coverage.acknowledged, 4_000);
        assert_eq!(coverage.anchored, 4_000);
        assert!(output.verify_valid, "SBE verification failed");
        assert!(
            output.completeness_valid,
            "completeness verification failed"
        );
        assert!(output.journal_complete, "journal did not finalize");
        assert!(output.decoy_absent, "an MVE/closed decoy entered the capture");

        // Queues drained: no mock frame remains in flight and every wire /
        // writer stage reconciles in both directions.
        assert_eq!(output.mock.in_flight, 0);
        assert_eq!(output.ledger.enqueue_transmit_shortfall, Some(0));
        assert_eq!(output.ledger.enqueue_transmit_excess, Some(0));
        assert_eq!(output.ledger.writer_write_shortfall, Some(0));
        assert_eq!(output.ledger.writer_write_excess, Some(0));
    }

    // The manifest carries the declared schedule and per-phase drain evidence.
    let index = stress_support::manifest::read_index(&out.join("index.json")).unwrap();
    let cells = index["cells"].as_object().expect("cells object");
    assert_eq!(cells.len(), 1);
    let (cell_id, entry) = cells.iter().next().expect("one published cell");
    assert_eq!(entry["publication_state"].as_str(), Some("published"));
    assert!(
        cell_id.contains(profile.label()),
        "cell id {cell_id} must carry the profile label"
    );
    let bytes = std::fs::read(out.join(entry["path"].as_str().unwrap())).unwrap();
    let document: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&bytes).unwrap();
    assert!(
        stress_support::manifest::missing_required_fields(&document).is_empty(),
        "burst manifest is incomplete"
    );
    let expected_id = format!("{}-seed-{seed:04x}-4000", profile.label());
    let schedule = &document["workload_schedule"];
    assert_eq!(schedule["workload_id"].as_str(), Some(expected_id.as_str()));
    assert_eq!(schedule["profile"].as_str(), Some(profile.label()));
    assert_eq!(schedule["seed"].as_u64(), Some(seed));
    assert_eq!(schedule["instrument_count"].as_u64(), Some(4_000));
    assert_eq!(schedule["expected_total_events"].as_u64(), Some(400));
    assert_eq!(schedule["phases"].as_array().unwrap().len(), phase_count);
    assert!(schedule["hot_set"]["sha256"].is_string());
    assert_eq!(schedule["fault_schedule"].as_array().unwrap().len(), 0);
    assert_eq!(
        document["delivery"]["phases"].as_array().unwrap().len(),
        phase_count
    );
    assert_eq!(document["scenario_version"].as_u64(), Some(6));
    outcome
}

/// Step: one sustained doubling of the declared rate. Three phases, exact
/// published counts 100 / 200 / 100, fully drained between phases.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn burst_step_cell_reconciles_all_phases() {
    let outcome = assert_burst_cell(BurstProfile::Step, 31, HotSelection::ExactQuota, None).await;
    let counts: Vec<usize> = outcome
        .output
        .as_ref()
        .unwrap()
        .phase_results
        .iter()
        .map(|phase| phase.published)
        .collect();
    assert_eq!(counts, vec![100, 200, 100]);
    let multipliers: Vec<u64> = outcome
        .output
        .as_ref()
        .unwrap()
        .schedule
        .phases
        .iter()
        .map(|phase| u64::from(phase.rate_multiplier))
        .collect();
    assert_eq!(multipliers, vec![1, 2, 1]);
}

/// Impulse: one concentrated spike. Three phases, exact published counts
/// 40 / 280 / 80, fully drained between phases.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn burst_impulse_cell_reconciles_all_phases() {
    let outcome = assert_burst_cell(BurstProfile::Impulse, 37, HotSelection::ExactQuota, None).await;
    let counts: Vec<usize> = outcome
        .output
        .as_ref()
        .unwrap()
        .phase_results
        .iter()
        .map(|phase| phase.published)
        .collect();
    assert_eq!(counts, vec![40, 280, 80]);
    let multipliers: Vec<u64> = outcome
        .output
        .as_ref()
        .unwrap()
        .schedule
        .phases
        .iter()
        .map(|phase| u64::from(phase.rate_multiplier))
        .collect();
    assert_eq!(multipliers, vec![1, 5, 1]);
}

/// Ignored reference generator for the Step burst manifest. Run explicitly
/// with `STRESS_MANIFEST_OUT` set; no performance claim is attached to it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly"]
async fn reference_burst_step_manifest_is_reproducible() {
    assert!(
        std::env::var("STRESS_MANIFEST_OUT").is_ok(),
        "set STRESS_MANIFEST_OUT to produce the artifact"
    );
    assert_burst_cell(
        BurstProfile::Step,
        31,
        HotSelection::ExactQuota,
        Some(manifest_out().as_path()),
    )
    .await;
}
