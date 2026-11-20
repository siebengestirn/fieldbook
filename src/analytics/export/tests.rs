use std::{collections::BTreeMap, path::Path};

use super::*;
use crate::{
    analytics::verify_capture,
    model::{PriceUnit, QuantityUnit},
};

#[test]
fn symbol_mapping_reports_ambiguity_without_guessing() {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    map.insert("one".to_owned(), vec!["native-a".to_owned()]);
    map.insert("many".to_owned(), vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(
        resolve_symbol(&map, "one"),
        (Some("native-a".to_owned()), "resolved")
    );
    assert_eq!(resolve_symbol(&map, "many"), (None, "ambiguous"));
    assert_eq!(resolve_symbol(&map, "missing"), (None, "unresolved"));
}

#[test]
fn unit_parts_preserve_kind_and_code() {
    assert_eq!(
        unit_parts(&PriceUnit::Currency("USDC".to_owned())),
        ("Currency".to_owned(), Some("USDC".to_owned()))
    );
    assert_eq!(
        unit_parts(&QuantityUnit::Contracts),
        ("Contracts".to_owned(), None)
    );
}

#[test]
fn date_days_is_utc_and_integer() {
    assert_eq!(date_days(0), 0);
    assert_eq!(date_days(86_400_000_000), 1);
    assert_eq!(date_days(-1), -1);
}

fn capture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("captures")
        .join(name)
}

#[test]
#[ignore = "requires operator-local retained captures"]
fn all_captures_export_to_staging() {
    for name in [
        "live-20261119T181218Z/polymarket",
        "live-20261119T181441Z/polymarket",
        "live-hyperliquid-20261119T181441Z",
    ] {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let staging = temp.path().join("staging");
        let summary = export_capture(&capture(name), &staging)
            .unwrap_or_else(|error| panic!("{name} export failed: {error:?}"));
        assert!(
            summary.verification_results.values().all(|ok| *ok),
            "{name} verification must pass: {:?}",
            summary.verification_results
        );
        assert!(
            staging.join("export-manifest.json").is_file(),
            "manifest exists"
        );
        assert!(
            staging.join("journal_observation.ndjson").is_file(),
            "journal rows exist"
        );
        assert!(
            staging.join("sbe_message.ndjson").is_file(),
            "SBE rows exist"
        );
        assert_eq!(
            summary.row_counts.get("journal_observation"),
            Some(
                &verify_capture(&capture(name))
                    .expect("verify")
                    .journal_records
            ),
            "{name} journal rows must reconcile"
        );
    }
}
