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
    reason = "stress_support is shared by several integration test crates; each crate exercises a different subset of its selectors"
)]

//! Deterministic lifecycle-churn cells (M5.3, scenarios L1-L10).
//!
//! Each cell drives the production collector through the loopback Kalshi mock:
//! the typed scenario selector resolves a seed + explicit event sequence, the
//! mock mutates its REST catalog and lifecycle broadcast at the pinned steps,
//! and the runner certifies coverage, event accounting, SBE and journal
//! fail-closed before any manifest is written.
//!
//! Evidence class: synthetic/loopback. These tests are **not** live validation
//! of Kalshi lifecycle notifications.

#![recursion_limit = "512"]

mod stress_support;

use std::collections::BTreeSet;

use stress_support::{
    lifecycle::LifecycleScenario,
    runner::{CellConfig, HotSelection, canonical_set_sha256, run_cell},
    universe::SyntheticUniverse,
};

fn churn_config(
    scenario: LifecycleScenario,
    seed: u64,
    symbols: usize,
    events: usize,
) -> CellConfig {
    CellConfig {
        seed,
        symbols,
        decoy_fraction_ppm: 20_000,
        shards: Some(1),
        // One lifecycle replica: the scheduled duplicate is the duplicate under
        // test. Cross-replica dedup is certified separately by
        // `tests/kalshi_lifecycle_plane.rs`.
        replicas: 1,
        lifecycle_replicas: 1,
        events,
        hot_pair: false,
        uniform_spread: true,
        hot_fraction_ppm: 0,
        hot_event_share_pct: 0,
        hot_selection: HotSelection::Weighted,
        burst: None,
        journal: true,
        limits: Some((45, 45)),
        force_capacity_reject: false,
        fail_inject: None,
        lifecycle_scenario: scenario,
        writer_queue: None,
        worktree_dir: None,
        require_clean_worktree: None,
        fault_schedule: None,
        capacity: None,
    }
}

/// Common fail-closed assertions shared by every lifecycle cell.
fn assert_certified(output: &stress_support::runner::CellOutput, events: usize) {
    assert!(
        output.ledger.reconciled(),
        "event ledger must reconcile: {:?}",
        output.ledger
    );
    assert_eq!(output.ledger.unexplained_drops, Some(0));
    assert_eq!(output.ledger.persistence_gap, Some(0));
    assert_eq!(output.events_published, events);
    assert_eq!(output.sbe_deltas, events as u64);
    assert!(output.verify_valid, "SBE verification failed");
    assert!(output.completeness_valid, "completeness failed");
    assert!(output.journal_complete, "journal did not finalize");
    assert!(output.decoy_absent, "a decoy entered the capture");
    let coverage = output.coverage.as_ref().expect("coverage must be present");
    assert!(coverage.valid, "coverage invalid: {coverage:?}");
    assert_eq!(
        canonical_set_sha256(&output.expected_symbols),
        coverage.expected_set_sha256
    );
    assert_eq!(
        canonical_set_sha256(&output.acknowledged_symbols),
        coverage.expected_set_sha256,
        "acknowledged and expected sets must be identical"
    );
    assert_eq!(
        output.lifecycle_applied,
        output.lifecycle_schedule.expected_applied(),
        "applied lifecycle events must match the explicit schedule"
    );
    assert_eq!(
        output.lifecycle_duplicates,
        output.lifecycle_schedule.expected_duplicates()
    );
    assert_eq!(
        output.lifecycle_stale_dropped,
        output.lifecycle_schedule.expected_stale()
    );
    assert!(output.lifecycle_barrier_completed);
}

fn metadata_titles(directory: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    let text = std::fs::read_to_string(directory.join("markets.json")).expect("metadata sidecar");
    let value: serde_json::Value = serde_json::from_str(&text).expect("metadata JSON");
    value["markets"]
        .as_array()
        .expect("markets array")
        .iter()
        .filter_map(|market| {
            let symbol = market["symbol"].as_str()?.to_owned();
            let title = market["title"].as_str()?.to_owned();
            Some((symbol, title))
        })
        .collect()
}

/// L1/L2/L3: a lifecycle-added market is discovered, subscribed and
/// snapshot-anchored; an `activated` notification on a tracked market is
/// applied. Coverage is exact over the enlarged final universe.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lifecycle_add_activate_proves_exact_coverage() {
    let directory = tempfile::tempdir().unwrap();
    let config = churn_config(LifecycleScenario::AddActivate, 17, 200, 120);
    let universe = SyntheticUniverse::generate(17, 200, 20_000);
    let initial: BTreeSet<String> = universe.eligible_tickers_sorted().into_iter().collect();
    let outcome = run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "add/activate cell failed: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_certified(&output, 120);

    let added = output
        .lifecycle_schedule
        .added_ticker
        .as_ref()
        .expect("add scenario has an added ticker");
    assert!(!initial.contains(added));
    assert_eq!(output.expected_symbols.len(), initial.len() + 1);
    assert!(output.expected_symbols.contains(added));
    assert!(output.acknowledged_symbols.contains(added));
    assert!(output.anchored_symbols.contains(added));
    assert_eq!(output.lifecycle_subscribed, 1, "lifecycle plane subscribed");
    assert_eq!(
        output.lifecycle_sent_frames, 2,
        "both events reached the wire"
    );
    assert_eq!(output.lifecycle_applied, 2);
    assert_eq!(output.lifecycle_duplicates, 0);
    assert_eq!(
        output
            .coverage
            .as_ref()
            .unwrap()
            .anchored_not_expected
            .count,
        0
    );
}

/// L5: a terminal `settled` retires the market exactly once. Its final empty
/// snapshot is an anchored extra, never a permanent coverage obligation.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lifecycle_retire_is_not_a_permanent_coverage_obligation() {
    let directory = tempfile::tempdir().unwrap();
    let config = churn_config(LifecycleScenario::RetireSettle, 19, 200, 120);
    let outcome = run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "retire cell failed: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_certified(&output, 120);

    let retired = output
        .lifecycle_schedule
        .retired_ticker
        .as_ref()
        .expect("retire scenario has a retired ticker");
    assert!(!output.expected_symbols.contains(retired));
    assert!(!output.acknowledged_symbols.contains(retired));
    assert!(
        output.anchored_symbols.contains(retired),
        "the retirement boundary must be present in the capture"
    );
    assert!(output.anchored_extras_ok);
    let coverage = output.coverage.as_ref().unwrap();
    assert_eq!(coverage.anchored_not_expected.count, 1);
    assert_eq!(coverage.anchored_not_expected.sample, vec![retired.clone()]);
    assert_eq!(output.report.core.removed_symbols, 1);
    assert_eq!(output.report.core.empty_snapshots, 1);
    assert_eq!(output.report.core.current_symbols, 199);
    assert_eq!(output.lifecycle_applied, 1);
}

/// L6: an identical redelivery is suppressed by the (market, kind, exchange
/// timestamp, payload fingerprint) dedup key, while the first delivery is
/// applied once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lifecycle_duplicate_event_is_deduped() {
    let directory = tempfile::tempdir().unwrap();
    let config = churn_config(LifecycleScenario::Duplicate, 23, 200, 80);
    let outcome = run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "duplicate cell failed: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_certified(&output, 80);
    assert_eq!(output.lifecycle_sent_frames, 2);
    assert_eq!(output.lifecycle_applied, 1);
    assert_eq!(output.lifecycle_duplicates, 1);
    assert_eq!(output.lifecycle_stale_dropped, 0);
    let titles = metadata_titles(directory.path());
    let tracked = output
        .lifecycle_schedule
        .final_expected()
        .iter()
        .next()
        .expect("non-empty universe")
        .clone();
    assert_eq!(
        titles.get(&tracked).map(String::as_str),
        Some("Duplicate payload")
    );
}

/// L7: an older nonterminal notification is rejected by the per-market
/// high-water mark; the newer title survives.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lifecycle_out_of_order_event_is_dropped() {
    let directory = tempfile::tempdir().unwrap();
    let config = churn_config(LifecycleScenario::OutOfOrder, 29, 200, 80);
    let outcome = run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "out-of-order cell failed: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_certified(&output, 80);
    assert_eq!(output.lifecycle_sent_frames, 2);
    assert_eq!(output.lifecycle_applied, 1);
    assert_eq!(output.lifecycle_stale_dropped, 1);
    assert_eq!(output.lifecycle_duplicates, 0);
    let titles = metadata_titles(directory.path());
    let tracked = output
        .lifecycle_schedule
        .final_expected()
        .iter()
        .next()
        .expect("non-empty universe")
        .clone();
    assert_eq!(
        titles.get(&tracked).map(String::as_str),
        Some("Newer metadata title")
    );
}

/// L8: two different payloads with the same kind and timestamp are distinct
/// venue updates; both are applied and the later one wins metadata.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lifecycle_conflicting_replica_updates_keep_newer_payload() {
    let directory = tempfile::tempdir().unwrap();
    let config = churn_config(LifecycleScenario::ConflictingReplica, 31, 200, 80);
    let outcome = run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "conflicting-update cell failed: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_certified(&output, 80);
    assert_eq!(output.lifecycle_applied, 2);
    assert_eq!(output.lifecycle_duplicates, 0);
    let titles = metadata_titles(directory.path());
    let tracked = output
        .lifecycle_schedule
        .final_expected()
        .iter()
        .next()
        .expect("non-empty universe")
        .clone();
    assert_eq!(
        titles.get(&tracked).map(String::as_str),
        Some("Second conflicting payload")
    );
}

// L9 (reappearance after retirement) is proven by the focused state-machine
// test `src/collector/state/tests.rs::l9_reappearance_after_retirement_needs_a_later_discovery_round`.
// A full cell that retires and then re-adds the same market exposes a pending
// recovery batch that the current core cannot acknowledge after removal; that
// is an existing contract limit, not a new behavior to invent here.

/// L10: lifecycle churn and the full 4k/400 delta workload in one cell. Every
/// epoch is recorded in the manifest; the retired instrument is not carried as
/// a coverage obligation.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lifecycle_churn_under_load_reconciles_and_certifies() {
    let directory = tempfile::tempdir().unwrap();
    let config = churn_config(LifecycleScenario::ChurnUnderLoad, 41, 4_000, 400);
    let outcome = run_cell(directory.path(), &config, None).await;
    assert!(
        outcome.acceptance.valid,
        "churn-under-load cell failed: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_certified(&output, 400);
    assert_eq!(output.lifecycle_applied, 4);
    assert_eq!(output.lifecycle_duplicates, 1);
    assert_eq!(output.lifecycle_stale_dropped, 1);
    assert_eq!(output.lifecycle_sent_frames, 6);
    assert_eq!(output.lifecycle_schedule.epochs.len(), 3);
    assert_eq!(output.expected_symbols.len(), 4_000);
    let retired = output
        .lifecycle_schedule
        .retired_ticker
        .as_ref()
        .expect("churn scenario has a retired ticker");
    assert!(!output.expected_symbols.contains(retired));
    assert!(output.anchored_symbols.contains(retired));
    assert_eq!(
        output
            .coverage
            .as_ref()
            .unwrap()
            .anchored_not_expected
            .sample,
        vec![retired.clone()]
    );
}

/// Ignored reference generator. Publishes the v16 lifecycle-churn manifest under
/// `STRESS_MANIFEST_OUT`; it never overwrites earlier v3-v15 artifacts because
/// the cell id carries `lifecycle-churn_under_load-v16`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "reference artifact generator; run explicitly with STRESS_MANIFEST_OUT"]
async fn reference_lifecycle_churn_manifest_is_versioned() {
    let out = std::env::var("STRESS_MANIFEST_OUT")
        .expect("set STRESS_MANIFEST_OUT to produce the artifact");
    let out = std::path::PathBuf::from(out);
    let directory = tempfile::tempdir().unwrap();
    let config = churn_config(LifecycleScenario::ChurnUnderLoad, 41, 4_000, 400);
    let outcome = run_cell(directory.path(), &config, Some(&out)).await;
    assert!(
        outcome.acceptance.valid,
        "reference churn cell failed: {:?}",
        outcome.acceptance.reasons
    );
    let output = outcome.output.expect("valid cell must carry output");
    assert_certified(&output, 400);

    let index = stress_support::manifest::read_index(&out.join("index.json")).expect("index");
    let entries = index["cells"].as_object().expect("cells");
    let (cell_id, entry) = entries
        .iter()
        .find(|(id, value)| {
            id.contains("lifecycle-churn_under_load")
                && value["publication_state"] == stress_support::manifest::PUBLICATION_PUBLISHED
                && value["run_status"] == "success"
        })
        .expect("a published lifecycle-churn success entry");
    assert!(cell_id.contains("lifecycle-churn_under_load-v16"));
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join(entry["path"].as_str().unwrap())).expect("manifest body"),
    )
    .expect("manifest JSON");
    assert_eq!(manifest["scenario_version"], 16);
    assert_eq!(
        manifest["scenario"]["lifecycle_scenario"],
        "churn_under_load"
    );
    assert_eq!(
        manifest["coverage"]["per_epoch"].as_array().unwrap().len(),
        3
    );
    assert!(
        manifest["coverage"]["retired_not_expected"]["count"]
            .as_u64()
            .unwrap()
            > 0
    );
}
