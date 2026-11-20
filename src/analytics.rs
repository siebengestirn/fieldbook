//! Offline analytics input verification and capture identity.
//!
//! This module verifies the artifacts of one retained capture before any
//! export work starts. It never writes into `captures/` and never trusts a
//! hash recorded in `evidence.json` without recalculating it.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    completeness, coverage, journal,
    report::{MarketMetadata, RunReport},
    verify,
};

/// Version of the analytics export contract implemented here.
pub const EXPORT_SCHEMA_VERSION: u32 = 1;
const CAPTURE_ID_DOMAIN: &[u8] = b"fieldbook.capture.v1";

/// Domain-separated capture identity derived from verified input artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureVerification {
    pub capture_id: String,
    pub capture_root: String,
    pub venue: String,
    pub source_commit: String,
    pub journal_code_commit: String,
    pub journal_provenance_status: String,
    pub artifact_sha256: BTreeMap<String, String>,
    pub journal_manifest_sha256: String,
    pub journal_segment_sha256: BTreeMap<String, String>,
    pub journal_records: u64,
    pub journal_complete: bool,
    pub sbe_messages: u64,
    pub sbe_valid: bool,
    pub completeness_valid: bool,
    pub coverage_valid: bool,
    pub instrument_count: usize,
    pub discovered_symbols: usize,
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("opening `{}` for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).context("reading file for hashing")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_of(&hasher.finalize()))
}

fn hex_of(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[(byte >> 4) as usize]));
        text.push(char::from(DIGITS[(byte & 0x0f) as usize]));
    }
    text
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    hasher.update(length.to_le_bytes());
    hasher.update(bytes);
}

/// Reject absolute paths and parent-directory traversal in sidecar entries.
fn contained_path(root: &Path, relative: &str) -> anyhow::Result<PathBuf> {
    let candidate = Path::new(relative);
    anyhow::ensure!(
        !candidate.is_absolute()
            && candidate
                .components()
                .all(|part| !matches!(part, Component::ParentDir | Component::RootDir)),
        "artifact path escapes the capture: {relative}"
    );
    let joined = root.join(candidate);
    anyhow::ensure!(
        joined.starts_with(root),
        "artifact path escapes the capture: {relative}"
    );
    Ok(joined)
}

fn text_field(value: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("evidence.json has no string `{key}`"))
}

/// Verify every input artifact of one retained capture.
pub fn verify_capture(capture_root: &Path) -> anyhow::Result<CaptureVerification> {
    anyhow::ensure!(capture_root.is_dir(), "capture directory does not exist");
    let evidence_path = capture_root.join("evidence.json");
    let evidence: serde_json::Value = serde_json::from_slice(
        &fs::read(&evidence_path)
            .with_context(|| format!("reading {}", evidence_path.display()))?,
    )
    .context("parsing evidence.json")?;

    let mut artifact_sha256 = BTreeMap::new();
    let artifacts = evidence
        .get("artifacts")
        .and_then(serde_json::Value::as_object)
        .context("evidence.json has no artifacts object")?;
    for (name, entry) in artifacts {
        let relative = text_field(entry, "path")?;
        let expected = text_field(entry, "sha256")?;
        let path = contained_path(capture_root, &relative)?;
        let actual = sha256_file(&path)?;
        anyhow::ensure!(
            actual == expected,
            "artifact `{name}` hash mismatch: evidence {expected}, computed {actual}"
        );
        artifact_sha256.insert(name.clone(), actual);
    }

    let sbe_path = contained_path(capture_root, "orderbook.sbe")?;
    let markets_path = contained_path(capture_root, "markets.json")?;
    let report_path = contained_path(capture_root, "report.json")?;
    let journal_text = text_field(&evidence, "journal_directory")?;
    let journal_root = if Path::new(&journal_text).is_absolute() {
        let root = fs::canonicalize(capture_root).context("canonicalizing capture root")?;
        let journal = fs::canonicalize(&journal_text).context("canonicalizing journal root")?;
        anyhow::ensure!(
            journal.starts_with(&root),
            "journal directory escapes the capture: {journal_text}"
        );
        journal
    } else {
        contained_path(capture_root, &journal_text)?
    };
    anyhow::ensure!(journal_root.is_dir(), "capture has no journal directory");
    let manifest_path = journal_root.join("manifest.json");
    let journal_manifest_sha256 = sha256_file(&manifest_path)?;
    let journal_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path)?).context("parsing journal manifest")?;
    let journal_code_commit =
        text_field(&journal_manifest, "code_commit").unwrap_or_else(|_| "unknown".to_owned());
    let journal_provenance_status = if journal_code_commit == "unknown" {
        "legacy_incomplete"
    } else {
        "verified"
    };

    let journal_verification = journal::verify_run(&journal_root)?;
    anyhow::ensure!(
        journal_verification.complete,
        "journal run is not complete: {}",
        journal_verification.errors.join("; ")
    );
    let mut journal_segment_sha256 = BTreeMap::new();
    for segment in &journal_verification.segments {
        let path = contained_path(&journal_root, &segment.file)?;
        journal_segment_sha256.insert(segment.file.clone(), sha256_file(&path)?);
    }
    let evidence_journal_files = evidence
        .get("journal_files")
        .and_then(serde_json::Value::as_array)
        .context("evidence.json has no journal_files list")?;
    for entry in evidence_journal_files {
        let relative = text_field(entry, "path")?;
        let expected = text_field(entry, "sha256")?;
        let path = contained_path(&journal_root, &relative)?;
        let actual = sha256_file(&path)?;
        anyhow::ensure!(
            actual == expected,
            "journal file `{relative}` hash mismatch: evidence {expected}, computed {actual}"
        );
    }

    let sbe_verification = verify::run(&sbe_path, "auto")?;
    anyhow::ensure!(sbe_verification.valid, "SBE verification failed");
    let completeness_summary = completeness::run(&sbe_path, "auto", false)?;
    let coverage_summary = coverage::run(&sbe_path, &markets_path, &report_path, "auto")?;
    let markets: MarketMetadata =
        serde_json::from_slice(&fs::read(&markets_path)?).context("parsing markets.json")?;
    let report: RunReport =
        serde_json::from_slice(&fs::read(&report_path)?).context("parsing report.json")?;

    let mut hasher = Sha256::new();
    hash_field(&mut hasher, CAPTURE_ID_DOMAIN);
    hash_field(&mut hasher, journal_manifest_sha256.as_bytes());
    for digest in journal_segment_sha256.values() {
        hash_field(&mut hasher, digest.as_bytes());
    }
    hash_field(
        &mut hasher,
        artifact_sha256
            .get("orderbook.sbe")
            .context("missing SBE hash")?
            .as_bytes(),
    );
    hash_field(
        &mut hasher,
        artifact_sha256
            .get("markets.json")
            .context("missing metadata hash")?
            .as_bytes(),
    );
    hash_field(
        &mut hasher,
        artifact_sha256
            .get("report.json")
            .context("missing report hash")?
            .as_bytes(),
    );

    Ok(CaptureVerification {
        capture_id: hex_of(&hasher.finalize()),
        capture_root: capture_root.display().to_string(),
        venue: report.exchange.clone(),
        source_commit: evidence
            .get("source_commit")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        journal_code_commit,
        journal_provenance_status: journal_provenance_status.to_owned(),
        artifact_sha256,
        journal_manifest_sha256,
        journal_segment_sha256,
        journal_records: journal_verification.record_count,
        journal_complete: journal_verification.complete,
        sbe_messages: sbe_verification.messages,
        sbe_valid: sbe_verification.valid,
        completeness_valid: completeness_summary.valid,
        coverage_valid: coverage_summary.valid,
        instrument_count: markets.markets.len(),
        discovered_symbols: report.discovered_symbols,
    })
}

/// Write a JSON summary through an explicit, errors-visible helper.
pub fn write_summary(path: &Path, summary: &CaptureVerification) -> anyhow::Result<()> {
    let mut file = File::create(path).with_context(|| format!("creating `{}`", path.display()))?;
    let bytes = serde_json::to_vec_pretty(summary).context("serializing capture verification")?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .with_context(|| format!("writing `{}`", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests;
