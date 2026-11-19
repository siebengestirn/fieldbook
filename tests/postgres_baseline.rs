//! Opt-in single-event baseline for the durable PostgreSQL delivery path.
//!
//! ```bash
//! FIELDBOOK_TEST_PG_URL='host=/tmp port=5432 user=tourist dbname=postgres' \
//! FIELDBOOK_BASELINE_OUTPUT=/path/to/baseline.json \
//! FIELDBOOK_BASELINE_COMMIT=<git-commit> \
//!   cargo test --test postgres_baseline -- --ignored --nocapture
//! ```
//!
//! Each sample is one complete single-event delivery through the real file
//! source and durable local commit, with either the in-memory simulator or
//! the real synchronous PostgreSQL target. This is a baseline for the P6.1
//! batch work, not a production throughput claim.

#![expect(
    clippy::expect_used,
    reason = "an opt-in baseline needs direct, named failures when the operator server is unavailable"
)]

use std::{
    env, fs,
    str::FromStr,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use engineering_challenge::application::{
    durable::{CommitStore, run_streaming_pipeline},
    file_source::{CheckpointBinding, DurableCheckpoint, FileSource, SourceIdentity},
    postgres::{PgTarget, PostgresSimulator, apply_migrations},
    transform_observation,
};
use postgres::{
    Client, //
    Config, //
    NoTls,  //
};
use serde::Serialize;

#[derive(Serialize)]
struct PathSummary {
    path: &'static str,
    samples: usize,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    p999_ms: f64,
}

#[derive(Serialize)]
struct Report {
    source_commit: String,
    generated_at_unix: u64,
    server_version: String,
    samples_per_path: usize,
    outstanding: usize,
    uncertain: usize,
    paths: Vec<PathSummary>,
}

fn record(index: usize) -> String {
    format!(
        "{{\"id\":\"baseline-{index}\",\"payload\":{{\"entity_id\":\"acct\",\"kind\":\"credit\",\"value\":{index},\"observed_at\":{index}}},\"received_at\":{index}}}"
    )
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = (quantile * (sorted.len() - 1) as f64).round() as usize;
    sorted[position.min(sorted.len() - 1)]
}

fn cleanup(client: &mut Client, target_id: &str) {
    for statement in [
        "DELETE FROM canonical_business_events WHERE target_id = $1",
        "DELETE FROM delivery_dispositions WHERE target_id = $1",
        "DELETE FROM target_checkpoints WHERE target_id = $1",
    ] {
        client
            .execute(statement, &[&target_id])
            .expect("scoped baseline cleanup");
    }
}

fn summarize(path: &'static str, samples: &mut Vec<f64>) -> PathSummary {
    samples.sort_by(f64::total_cmp);
    PathSummary {
        path,
        samples: samples.len(),
        p50_ms: percentile(samples, 0.50),
        p95_ms: percentile(samples, 0.95),
        p99_ms: percentile(samples, 0.99),
        p999_ms: percentile(samples, 0.999),
    }
}

#[test]
#[ignore = "requires an operator-supplied PostgreSQL server; set FIELDBOOK_TEST_PG_URL"]
fn postgres_baseline_single_event() {
    let url = env::var("FIELDBOOK_TEST_PG_URL")
        .expect("set FIELDBOOK_TEST_PG_URL to an operator-supplied PostgreSQL connection string");
    let output = env::var("FIELDBOOK_BASELINE_OUTPUT")
        .expect("set FIELDBOOK_BASELINE_OUTPUT to the JSON report path");
    let commit = env::var("FIELDBOOK_BASELINE_COMMIT").unwrap_or_default();
    let count: usize = env::var("FIELDBOOK_BASELINE_EVENTS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(200);

    let config = Config::from_str(&url).expect("valid PostgreSQL URL");
    let mut client = config.connect(NoTls).expect("connect to live PostgreSQL");
    apply_migrations(&mut client).expect("migration applies");
    let server_version: String = client
        .query_one("select version()", &[])
        .expect("server version")
        .get(0);

    let root = tempfile::tempdir().expect("tempdir");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let mut local_samples = Vec::with_capacity(count);
    let mut real_samples = Vec::with_capacity(count);
    let mut outstanding = 0usize;
    let mut uncertain = 0usize;

    for path in ["local-store-only", "real-postgresql"] {
        for index in 0..count {
            let target_id = format!("fieldbook-baseline-{path}-{unique}-{index}");
            if path == "real-postgresql" {
                cleanup(&mut client, &target_id);
            }
            let source_name = format!("baseline-{path}-{unique}-{index}");
            let bytes = format!("{}\n", record(index));
            let input_path = root.path().join(format!("{source_name}.jsonl"));
            fs::write(&input_path, &bytes).expect("write baseline record");

            let identity = SourceIdentity::register(&source_name, bytes.as_bytes());
            let binding = CheckpointBinding::new(&identity, &target_id);
            let anchor = DurableCheckpoint::new(&binding);
            let store_dir = root.path().join(format!("store-{path}-{index}"));
            fs::create_dir_all(&store_dir).expect("store directory");
            let mut store = CommitStore::open(&store_dir, binding).expect("open store");
            let mut source = FileSource::open(&input_path, &source_name).expect("open source");

            let started = Instant::now();
            if path == "real-postgresql" {
                let mut target =
                    PgTarget::with_source(&target_id, identity.source_id().as_str(), &mut client);
                let result = run_streaming_pipeline(
                    &mut source,
                    transform_observation,
                    &mut target,
                    &anchor,
                    &mut store,
                );
                real_samples.push(started.elapsed().as_secs_f64() * 1000.0);
                outstanding += usize::try_from(result.report.outstanding).expect("count fits");
                uncertain += usize::try_from(result.report.uncertain).expect("count fits");
                assert_eq!(result.report.delivered, 1, "one event was delivered");
            } else {
                let mut target = PostgresSimulator::new(&target_id);
                let result = run_streaming_pipeline(
                    &mut source,
                    transform_observation,
                    &mut target,
                    &anchor,
                    &mut store,
                );
                local_samples.push(started.elapsed().as_secs_f64() * 1000.0);
                outstanding += usize::try_from(result.report.outstanding).expect("count fits");
                uncertain += usize::try_from(result.report.uncertain).expect("count fits");
                assert_eq!(result.report.delivered, 1, "one event was delivered");
            }
        }
    }
    assert_eq!(outstanding, 0, "no outstanding delivery in baseline");
    assert_eq!(uncertain, 0, "no uncertain delivery in baseline");

    let report = Report {
        source_commit: commit,
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_secs(),
        server_version,
        samples_per_path: count,
        outstanding,
        uncertain,
        paths: vec![
            summarize("local-store-only", &mut local_samples),
            summarize("real-postgresql", &mut real_samples),
        ],
    };
    fs::write(
        &output,
        serde_json::to_string_pretty(&report).expect("serialize report"),
    )
    .expect("write baseline report");
    println!("single-event baseline written to {output}");
}

#[test]
fn baseline_percentile_helper_is_monotonic() {
    let values = [1.0, 2.0, 3.0, 4.0];
    assert!(
        percentile(&values, 0.5) <= percentile(&values, 1.0),
        "percentiles are ordered"
    );
}
