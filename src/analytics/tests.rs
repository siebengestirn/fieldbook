use std::fs;

use tempfile::TempDir;

use super::*;

fn capture_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("captures")
        .join(name)
}

#[test]
fn all_retained_captures_verify_independently() {
    for name in [
        "live-20261119T181218Z/polymarket",
        "live-20261119T181441Z/polymarket",
        "live-hyperliquid-20261119T181441Z",
    ] {
        let directory = capture_fixture(name);
        let result = verify_capture(&directory)
            .unwrap_or_else(|error| panic!("{name} must verify: {error:?}"));
        assert!(result.journal_complete, "{name} journal must be complete");
        assert!(result.sbe_valid, "{name} SBE must be valid");
        assert!(result.coverage_valid, "{name} coverage must be valid");
        assert!(
            result.journal_records > 0,
            "{name} must carry journal records"
        );
        assert!(result.sbe_messages > 0, "{name} must carry SBE messages");
        assert_eq!(
            result.journal_provenance_status, "legacy_incomplete",
            "{name} must retain its legacy provenance status"
        );
        assert_eq!(
            result.journal_code_commit, "unknown",
            "{name} journal commit must stay as recorded"
        );
    }
}

#[test]
fn capture_identity_is_deterministic() {
    let directory = capture_fixture("live-20261119T181218Z/polymarket");
    let first = verify_capture(&directory).expect("first verification");
    let second = verify_capture(&directory).expect("second verification");
    assert_eq!(first.capture_id, second.capture_id, "capture id is stable");
    assert_eq!(first.capture_id.len(), 64, "capture id is a SHA-256 digest");
}

#[test]
fn modified_evidence_hash_is_rejected() {
    let source = capture_fixture("live-20261119T181218Z/polymarket");
    let temp = TempDir::new().expect("tempdir");
    let target = temp.path().join("capture");
    fs::create_dir_all(&target).expect("create capture directory");
    for entry in fs::read_dir(&source).expect("read capture") {
        let path = entry.expect("entry").path();
        if path.is_file() {
            let name = path.file_name().expect("file name");
            if name == "evidence.json" {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).expect("read evidence"))
                        .expect("parse evidence");
                if let Some(entry) = value
                    .get_mut("artifacts")
                    .and_then(serde_json::Value::as_object_mut)
                    .and_then(|artifacts| artifacts.get_mut("orderbook.sbe"))
                {
                    entry
                        .as_object_mut()
                        .expect("artifact object")
                        .insert("sha256".to_owned(), serde_json::Value::from("0".repeat(64)));
                }
                fs::write(
                    target.join(name),
                    serde_json::to_vec_pretty(&value).expect("serialize evidence"),
                )
                .expect("write modified evidence");
            } else {
                fs::copy(&path, target.join(name)).expect("copy artifact");
            }
        }
    }
    let error = verify_capture(&target).expect_err("modified evidence rejected");
    assert!(
        error.to_string().contains("hash mismatch"),
        "hash mismatch must be reported: {error}"
    );
}

#[test]
fn artifact_path_traversal_is_rejected() {
    let error =
        contained_path(Path::new("/tmp/capture"), "../outside").expect_err("traversal rejected");
    assert!(
        error.to_string().contains("escapes the capture"),
        "path containment must be reported: {error}"
    );
}
