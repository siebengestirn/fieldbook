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
#![allow(
    dead_code,
    reason = "stress_support is shared by several integration test crates; each crate exercises a different subset"
)]

//! Hyperliquid H3 capacity cells (synthetic loopback evidence).
//!
//! Each cell drives the real Hyperliquid adapter, supervisor, journal and
//! `CollectorCore` against a seeded loopback wire mock for the exact ten
//! selected instruments. It proves exact three-set coverage, a reconciled
//! event ledger and dispositions, bounded queues, journal/SBE integrity, zero
//! `LevelChange`, and re-anchoring under reconnect.
//!
//! This evidence is synthetic and loopback only. It is **not** live
//! certification and makes no latency, throughput or capacity performance
//! claim. See `docs/HYPERLIQUID_INTEGRATION_PLAN.md` and
//! `docs/stress-lab-design.md`.

#![recursion_limit = "512"]

mod stress_support;

use std::{collections::BTreeSet, path::PathBuf};

use stress_support::{
    hyperliquid::{
        HyperliquidProfile, assert_certified, build_hyperliquid_manifest, derive_schedule,
        publish_hyperliquid_manifest, run_hyperliquid_cell,
    },
    manifest, runner,
};

/// Full H3 evidence gate for the balanced profile: exact coverage, reconciled
/// ledger/dispositions, bounded queue, journal/SBE integrity, zero `LevelChange`
/// and reconnect recovery.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn h3_balanced_cell_certifies_every_evidence_gate() {
    let directory = tempfile::tempdir().unwrap();
    let outcome = run_hyperliquid_cell(directory.path(), HyperliquidProfile::Balanced, 0x0B).await;
    assert_certified(&outcome);

    assert_eq!(outcome.schedule.coins.len(), 10);
    assert_eq!(outcome.coverage_expected, 10);
    assert_eq!(outcome.coverage_anchored, 10);
    assert_eq!(
        outcome.ledger.generated_events,
        outcome.schedule.expected_book_snapshots()
    );
    assert_eq!(outcome.ledger.applied_events, Some(outcome.sbe_messages));
    assert_eq!(outcome.ledger.sbe_deltas, Some(outcome.sbe_messages));
    assert_eq!(outcome.ledger.unexplained_drops, Some(0));
    assert_eq!(outcome.ledger.persistence_gap, Some(0));
    assert_eq!(outcome.ledger.decode_failure_count, Some(0));
    assert_eq!(outcome.ledger.disposition_shortfall, Some(0));
    assert_eq!(outcome.ledger.disposition_excess, Some(0));
    assert_eq!(outcome.book_enqueued, outcome.book_transmitted);
    assert_eq!(outcome.sbe_messages, outcome.sbe_snapshots);
    assert_eq!(
        outcome.journal_records,
        outcome.schedule.expected_data_frames()
    );
    assert_eq!(
        outcome.telemetry_observations,
        outcome.schedule.expected_telemetry_frames()
    );
    assert_eq!(
        outcome.trade_observations,
        outcome.schedule.expected_trade_frames()
    );

    // Three-set coverage hashes agree with the independent coverage certificate.
    assert_eq!(outcome.expected_set_sha256, outcome.acknowledged_set_sha256);
    assert_eq!(outcome.acknowledged_set_sha256, outcome.anchored_set_sha256);
    let expected: BTreeSet<String> = outcome
        .schedule
        .coins
        .iter()
        .map(|coin| format!("hyperliquid:perp::{coin}"))
        .collect();
    assert_eq!(
        outcome.expected_set_sha256,
        runner::canonical_set_sha256(&expected)
    );

    // Manifest reuses the shared schema and scenario version (16).
    let manifest = build_hyperliquid_manifest(&outcome);
    assert_eq!(
        manifest["schema_version"].as_u64(),
        Some(manifest::STRESS_MANIFEST_SCHEMA_VERSION as u64)
    );
    assert_eq!(manifest["scenario_version"].as_u64(), Some(16));
    assert_eq!(manifest["run_status"].as_str(), Some("success"));
    assert_eq!(
        manifest["evidence_class"].as_str(),
        Some("synthetic-loopback")
    );
    assert!(manifest::missing_required_fields(&manifest).is_empty());
    assert_eq!(manifest["hyperliquid"]["live_certification"], false);
    assert_eq!(manifest["hyperliquid"]["latency_or_capacity_claim"], false);
    assert_eq!(manifest["hyperliquid"]["zero_level_change"], true);
    assert_eq!(manifest["hyperliquid"]["reconnect_recovery"], true);
}

/// Telemetry-heavy profile: most wire frames are `allMids`/`fastAssetCtxs`,
/// which must never anchor books or produce deltas. The cell still reconciles.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn h3_telemetry_heavy_profile_keeps_telemetry_out_of_books() {
    let directory = tempfile::tempdir().unwrap();
    let outcome =
        run_hyperliquid_cell(directory.path(), HyperliquidProfile::TelemetryHeavy, 0x21).await;
    assert_certified(&outcome);

    assert!(
        outcome.telemetry_observations > outcome.schedule.expected_book_snapshots(),
        "the telemetry-heavy profile must fan out more telemetry frames than book snapshots"
    );
    assert_eq!(
        outcome.telemetry_observations,
        outcome.schedule.expected_telemetry_frames()
    );
    assert_eq!(
        outcome.sbe_messages,
        outcome.schedule.expected_book_snapshots()
    );
    assert_eq!(outcome.sbe_snapshots, outcome.sbe_messages);
    assert_eq!(outcome.ledger.transmit_decode_excess, Some(0));
    assert_eq!(outcome.ledger.transmit_decode_shortfall, Some(0));
}

/// Book-refresh-burst profile: frequent l2Book refreshes and trade bursts must
/// reconcile exactly across the reconnect, with bounded queues and no deltas.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn h3_book_refresh_burst_profile_reconciles_and_recovers() {
    let directory = tempfile::tempdir().unwrap();
    let outcome =
        run_hyperliquid_cell(directory.path(), HyperliquidProfile::BookRefreshBurst, 0x33).await;
    assert_certified(&outcome);

    assert!(
        outcome.schedule.expected_book_snapshots() > 20,
        "the burst profile must issue multiple refreshes per instrument across epochs"
    );
    assert_eq!(
        outcome.book_enqueued as u64,
        outcome.schedule.expected_book_snapshots()
    );
    assert_eq!(
        outcome.book_transmitted as u64,
        outcome.schedule.expected_book_snapshots()
    );
    assert_eq!(
        outcome.sbe_messages,
        outcome.schedule.expected_book_snapshots()
    );
    assert!(outcome.ws_connections >= 2);
    assert!(outcome.writer_queue_high_water <= 129);
}

/// The workload schedule is a pure function of (profile, seed): identical
/// inputs reproduce byte-identical frames, while changing either input changes
/// the schedule. No wall-clock timing is involved.
#[test]
fn h3_schedule_is_seed_deterministic_and_profile_distinct() {
    let a = derive_schedule(HyperliquidProfile::Balanced, 0xABCD);
    let b = derive_schedule(HyperliquidProfile::Balanced, 0xABCD);
    assert_eq!(
        a, b,
        "the same profile and seed must reproduce the same schedule"
    );
    assert_eq!(a.workload_id(), "hyperliquid-balanced-seed-abcd");
    assert_eq!(a.coins.len(), 10);
    assert!(a.expected_book_snapshots() >= 20); // 10 instruments x 2 epochs
    assert_eq!(a.expected_telemetry_frames(), 6); // allMids + fastAssetCtxs

    let different_seed = derive_schedule(HyperliquidProfile::Balanced, 0xABCE);
    assert_ne!(
        a.frames().map(|frame| &frame.payload).collect::<Vec<_>>(),
        different_seed
            .frames()
            .map(|frame| &frame.payload)
            .collect::<Vec<_>>()
    );

    let heavy = derive_schedule(HyperliquidProfile::TelemetryHeavy, 0xABCD);
    assert!(
        heavy.expected_telemetry_frames() > a.expected_telemetry_frames(),
        "profile selection must change the declared telemetry rate"
    );
    let burst = derive_schedule(HyperliquidProfile::BookRefreshBurst, 0xABCD);
    assert!(
        burst.expected_book_snapshots() > a.expected_book_snapshots(),
        "profile selection must change the declared book refresh cadence"
    );
}

/// Ignored reference generator. Publishes a v16 H3 manifest under
/// `STRESS_MANIFEST_OUT` without overwriting any existing evidence: cell ids
/// are content-addressed by scenario version, profile and seed, and an
/// existing cell is left untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly with STRESS_MANIFEST_OUT"]
async fn reference_hyperliquid_manifest_is_versioned() {
    let out = PathBuf::from(
        std::env::var("STRESS_MANIFEST_OUT")
            .expect("set STRESS_MANIFEST_OUT to produce the artifact"),
    );
    let directory = tempfile::tempdir().unwrap();
    let outcome = run_hyperliquid_cell(directory.path(), HyperliquidProfile::Balanced, 0x0B).await;
    assert_certified(&outcome);

    let cell_id = format!(
        "hyperliquid-balanced-v{}-seed-000b",
        runner::SCENARIO_VERSION
    );
    let index_path = out.join("index.json");
    let already_published = manifest::read_index(&index_path)
        .ok()
        .and_then(|index| {
            index
                .get("cells")
                .and_then(|cells| cells.get(&cell_id).cloned())
        })
        .is_some();
    if already_published {
        // Preserve existing evidence; never overwrite an earlier artifact.
        return;
    }
    let published = publish_hyperliquid_manifest(&out, &outcome).expect("publish manifest");
    assert_eq!(published, cell_id);
    assert!(published.contains("v16"));

    let index = manifest::read_index(&index_path).expect("read index");
    let entry = index
        .get("cells")
        .and_then(|cells| cells.get(published.as_str()))
        .expect("published cell entry");
    assert_eq!(
        entry["publication_state"].as_str(),
        Some(manifest::PUBLICATION_PUBLISHED)
    );
    let body = out.join(entry["path"].as_str().unwrap());
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&body).expect("manifest body"))
            .expect("manifest JSON");
    assert_eq!(document["scenario_version"].as_u64(), Some(16));
    assert_eq!(
        document["evidence_class"].as_str(),
        Some("synthetic-loopback")
    );
    assert_eq!(document["hyperliquid"]["live_certification"], false);
}
