//! Independence and end-to-end evidence for the Platform P5 reference app.
//!
//! The compile-time `include_str!` checks below prove that the new source
//! files contain no references to market-data modules or market concepts. The
//! Rust compiler also compiles those modules as part of every `cargo test`
//! invocation, so this file is both a static independence check and a normal
//! integration test.

use std::collections::BTreeSet;

use engineering_challenge::application::{
    Checkpoint, JsonlSource, SourceAdapter, TargetConnector, TransformError,
    postgres::{PostgresFault, PostgresTarget},
    run_jsonl_to_target, transform_observation,
};

const SOURCES: [(&str, &str); 4] = [
    ("src/application.rs", include_str!("../src/application.rs")),
    (
        "src/application/postgres.rs",
        include_str!("../src/application/postgres.rs"),
    ),
    (
        "src/application/file_source.rs",
        include_str!("../src/application/file_source.rs"),
    ),
    (
        "src/application/durable.rs",
        include_str!("../src/application/durable.rs"),
    ),
];

#[test]
fn reference_modules_have_no_market_data_dependencies() {
    let forbidden = [
        "crate::model",
        "crate::coordinator",
        "crate::collector",
        "crate::exchange",
        "crate::journal",
        "crate::replay",
        "crate::catalog",
        "crate::orderbook",
        "crate::writer",
        "crate::products",
        "ExchangeEvent",
        "MarketInfo",
        "MarketDataSession",
        "BookSnapshot",
        "LevelChange",
        "OrderBook",
    ];
    for (name, source) in SOURCES {
        for token in forbidden {
            assert!(
                !source.contains(token),
                "{name} contains market-data dependency token {token}"
            );
        }
    }
}

#[test]
fn reference_modules_only_share_generic_canonical_and_identity_types() {
    let allowed = ["crate::canonical", "crate::identity", "crate::application"];
    let mut seen = BTreeSet::new();
    for (_name, source) in SOURCES {
        for token in allowed {
            if source.contains(token) {
                seen.insert(token);
            }
        }
    }
    assert!(seen.contains("crate::canonical"));
    assert!(seen.contains("crate::application"));
}

#[test]
fn end_to_end_jsonl_to_postgres_reference_accounts_every_record() {
    let lines: Vec<String> = (0..4)
        .map(|index| {
            format!(
                "{{\"id\":\"id-{index}\",\"payload\":{{\"entity_id\":\"entity-{index}\",\"kind\":\"credit\",\"value\":{index},\"observed_at\":{index}}}}}"
            )
        })
        .collect();
    let mut source = JsonlSource::from_str("reference", &lines.join("\n"));
    let mut target = PostgresTarget::with_fault(
        "pg-reference",
        PostgresFault::ServerUnavailableAfter { successful: 2 },
    );
    let first = run_jsonl_to_target(
        &mut source,
        transform_observation,
        &mut target,
        &Checkpoint::default(),
    );
    assert_eq!(first.total_read, 4);
    assert_eq!(first.delivered, 2);
    assert_eq!(first.outstanding, 2);
    assert_eq!(first.accounted(), first.total_read);
    assert_eq!(first.last_committed_sequence, 2);

    let mut resumed_source = JsonlSource::from_str("reference", &lines.join("\n"));
    let mut healthy = PostgresTarget::new("pg-reference");
    let resumed = run_jsonl_to_target(
        &mut resumed_source,
        transform_observation,
        &mut healthy,
        &Checkpoint {
            last_committed_sequence: first.last_committed_sequence,
        },
    );
    assert_eq!(resumed.skipped, 2);
    assert_eq!(resumed.delivered, 2);
    assert_eq!(resumed.accounted(), resumed.total_read);
    assert_eq!(resumed.last_committed_sequence, 4);
    assert_eq!(healthy.simulator().committed_count(), 2);
    assert_eq!(healthy.target_id(), "pg-reference");
}

#[test]
fn transform_errors_are_fail_closed_in_the_reference_path() {
    let mut source = JsonlSource::from_str(
        "reference",
        "{\"id\":\"x\",\"payload\":{\"entity_id\":\"\",\"kind\":\"credit\",\"value\":1,\"observed_at\":1}}",
    );
    let observation = source.read_one().expect("read").expect("observation");
    assert!(matches!(
        transform_observation(&observation),
        Err(TransformError::Schema(_))
    ));
}
