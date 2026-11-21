//! Coverage certification for global sources, markets and instruments.
//!
//! A coverage record discloses what a source actually provides. Missing data
//! is never represented as a zero value or as full coverage.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    quality::QualityReport,
    rights::{LicensingRegistry, UsageRight},
    taxonomy::{AssetClass, CountryCode, DataCategory, DepthModel},
};

/// Observable maturity of a source, market or instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Discovered,
    ConnectorImplemented,
    TechnicallyValidated,
    Licensed,
    Live,
    HistoricallyPopulated,
    Reconciled,
    Certified,
}

impl CoverageStatus {
    /// Return the stable contract string for this coverage status.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::ConnectorImplemented => "connector_implemented",
            Self::TechnicallyValidated => "technically_validated",
            Self::Licensed => "licensed",
            Self::Live => "live",
            Self::HistoricallyPopulated => "historically_populated",
            Self::Reconciled => "reconciled",
            Self::Certified => "certified",
        }
    }

    /// Return the ordinal rank of this status.
    pub const fn rank(self) -> u8 {
        match self {
            Self::Discovered => 0,
            Self::ConnectorImplemented => 1,
            Self::TechnicallyValidated => 2,
            Self::Licensed => 3,
            Self::Live => 4,
            Self::HistoricallyPopulated => 5,
            Self::Reconciled => 6,
            Self::Certified => 7,
        }
    }
}

/// Dimension in which a source declares coverage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoverageScope {
    pub country: CountryCode,
    pub asset_class: AssetClass,
    pub category: DataCategory,
    pub depth: DepthModel,
}

/// Public, independently measurable coverage record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageRecord {
    pub source_id: String,
    pub dataset: String,
    pub scope: CoverageScope,
    pub status: CoverageStatus,
    pub history_start_us: Option<i64>,
    pub history_end_us: Option<i64>,
    pub real_time: bool,
    pub known_gaps: Vec<String>,
    pub evidence: Vec<String>,
}

impl CoverageRecord {
    /// Check the structural requirements for certification.
    pub fn certifiable(&self) -> bool {
        self.status == CoverageStatus::Certified
            && self.known_gaps.is_empty()
            && !self.evidence.is_empty()
            && self.history_start_us.is_some()
    }
}

/// A country-level market coverage dossier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountryDossier {
    pub country: CountryCode,
    pub official_exchange_registry: Option<String>,
    pub venues: Vec<String>,
    pub regulators: Vec<String>,
    pub economic_sources: Vec<String>,
    pub market_calendar: Option<String>,
    pub data_residency_restrictions: Vec<String>,
}

/// Certification failure for one coverage record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CertificationError {
    /// The source lacks a required maturity status.
    #[error("coverage status {status:?} is below certified for {source_id}")]
    InsufficientStatus {
        source_id: String,
        status: CoverageStatus,
    },
    /// Rights or quality evidence is missing.
    #[error("certification prerequisite missing for {source_id}: {requirement}")]
    MissingPrerequisite {
        source_id: String,
        requirement: &'static str,
    },
}

/// Promote a record only when all applicable prerequisites are present.
pub fn certify(
    record: &CoverageRecord,
    rights: &LicensingRegistry,
    quality: &QualityReport,
    at_us: i64,
) -> Result<CoverageRecord, CertificationError> {
    if record.status < CoverageStatus::TechnicallyValidated {
        return Err(CertificationError::InsufficientStatus {
            source_id: record.source_id.clone(),
            status: record.status,
        });
    }
    if !record.known_gaps.is_empty() {
        return Err(CertificationError::MissingPrerequisite {
            source_id: record.source_id.clone(),
            requirement: "known_gaps",
        });
    }
    rights
        .authorize(
            &record.dataset,
            &record.scope.country,
            UsageRight::InternalProcessing,
            at_us,
        )
        .map_err(|_| CertificationError::MissingPrerequisite {
            source_id: record.source_id.clone(),
            requirement: "licensing",
        })?;
    if !quality.valid() {
        return Err(CertificationError::MissingPrerequisite {
            source_id: record.source_id.clone(),
            requirement: "reconciliation",
        });
    }
    let mut promoted = record.clone();
    promoted.status = CoverageStatus::Certified;
    Ok(promoted)
}

/// Aggregate matrix of coverage records by source.
#[derive(Debug, Default)]
pub struct CoverageMatrix {
    records: BTreeMap<String, CoverageRecord>,
}

impl CoverageMatrix {
    /// Create an empty matrix.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a coverage record under its source and dataset key.
    pub fn insert(&mut self, record: CoverageRecord) {
        let key = format!("{}:{}", record.source_id, record.dataset);
        self.records.insert(key, record);
    }

    /// Return the number of records at or above a status threshold.
    pub fn count_at_least(&self, minimum: CoverageStatus) -> usize {
        self.records
            .values()
            .filter(|record| record.status >= minimum)
            .count()
    }

    /// Return all certified records.
    pub fn certified(&self) -> Vec<&CoverageRecord> {
        self.records
            .values()
            .filter(|record| record.certifiable())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        quality::Reconciliation,
        rights::{LicensingRecord, RightDecision},
    };
    use std::collections::BTreeMap;

    fn scope() -> CoverageScope {
        CoverageScope {
            country: CountryCode::new("US").expect("country code"),
            asset_class: AssetClass::Equity,
            category: DataCategory::Level2Book,
            depth: DepthModel::Level2,
        }
    }

    fn record() -> CoverageRecord {
        CoverageRecord {
            source_id: "source".to_owned(),
            dataset: "quotes".to_owned(),
            scope: scope(),
            status: CoverageStatus::TechnicallyValidated,
            history_start_us: Some(0),
            history_end_us: Some(100),
            real_time: true,
            known_gaps: Vec::new(),
            evidence: vec!["fixture".to_owned()],
        }
    }

    fn rights() -> LicensingRegistry {
        let mut decision = BTreeMap::new();
        decision.insert(UsageRight::InternalProcessing, RightDecision::Permitted);
        let mut registry = LicensingRegistry::new();
        registry.register(LicensingRecord {
            provider: "example".to_owned(),
            dataset: "quotes".to_owned(),
            territory: CountryCode::new("US").expect("country code"),
            rights: decision,
            valid_until_us: Some(200),
            audit_required: false,
            reference: "contract".to_owned(),
        });
        registry
    }

    fn quality() -> QualityReport {
        QualityReport {
            source_id: "source".to_owned(),
            dataset: "quotes".to_owned(),
            window_start_us: 0,
            window_end_us: 100,
            states: BTreeMap::new(),
            reasons: BTreeMap::new(),
            reconciliation: Reconciliation {
                expected: 1,
                observed: 1,
                missing: 0,
                extra: 0,
                duplicated: 0,
                rejected: 0,
                quarantined: 0,
            },
        }
    }

    #[test]
    fn certification_requires_all_prerequisites() {
        let certified = certify(&record(), &rights(), &quality(), 50).expect("certification");
        assert_eq!(certified.status, CoverageStatus::Certified);
        assert!(certified.certifiable());

        let mut with_gap = record();
        with_gap.known_gaps.push("missing auction data".to_owned());
        assert!(certify(&with_gap, &rights(), &quality(), 50).is_err());
    }

    #[test]
    fn matrix_counts_only_declared_status() {
        let mut matrix = CoverageMatrix::new();
        matrix.insert(record());
        assert_eq!(matrix.count_at_least(CoverageStatus::Certified), 0);
        assert_eq!(
            matrix.count_at_least(CoverageStatus::TechnicallyValidated),
            1
        );
    }
}
