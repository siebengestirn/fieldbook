//! Machine-readable data rights and entitlement decisions.
//!
//! Access to a source feed does not grant storage or redistribution rights.
//! Every dataset is denied until an explicit, unexpired right is registered.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use crate::taxonomy::CountryCode;

/// Right being exercised over a dataset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageRight {
    RealTimeAccess,
    HistoricalStorage,
    InternalProcessing,
    DerivedData,
    Redistribution,
    Display,
    NonDisplay,
}

impl UsageRight {
    /// Return the stable contract string for this right.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RealTimeAccess => "real_time_access",
            Self::HistoricalStorage => "historical_storage",
            Self::InternalProcessing => "internal_processing",
            Self::DerivedData => "derived_data",
            Self::Redistribution => "redistribution",
            Self::Display => "display",
            Self::NonDisplay => "non_display",
        }
    }
}

/// Recorded decision for one right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RightDecision {
    Permitted,
    Prohibited,
    ContractRequired,
    Unknown,
}

impl RightDecision {
    /// Return the stable contract string for this decision.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Permitted => "permitted",
            Self::Prohibited => "prohibited",
            Self::ContractRequired => "contract_required",
            Self::Unknown => "unknown",
        }
    }
}

/// Contract-derived record for one source product in one territory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicensingRecord {
    pub provider: String,
    pub dataset: String,
    pub territory: CountryCode,
    pub rights: BTreeMap<UsageRight, RightDecision>,
    pub valid_until_us: Option<i64>,
    pub audit_required: bool,
    pub reference: String,
}

impl LicensingRecord {
    /// Check whether the record grants a right at an observation time.
    pub fn grants(&self, right: UsageRight, at_us: i64) -> bool {
        self.valid_until_us.is_none_or(|end| at_us <= end)
            && self.rights.get(&right) == Some(&RightDecision::Permitted)
    }
}

/// Failure to authorize a dataset.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RightsError {
    /// No record covers the source and territory.
    #[error("no licensing record for {dataset} in {territory}")]
    MissingRecord { dataset: String, territory: String },
    /// The right is absent, denied, conditional or expired.
    #[error("right {right} not granted for {dataset}")]
    RightNotGranted {
        dataset: String,
        right: &'static str,
    },
}

/// Fail-closed registry of licensing records.
#[derive(Debug, Default)]
pub struct LicensingRegistry {
    records: Vec<LicensingRecord>,
}

impl LicensingRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a licensing record.
    pub fn register(&mut self, record: LicensingRecord) {
        if !self.records.contains(&record) {
            self.records.push(record);
        }
    }

    /// Authorize one right for one dataset and territory.
    pub fn authorize(
        &self,
        dataset: &str,
        territory: &CountryCode,
        right: UsageRight,
        at_us: i64,
    ) -> Result<(), RightsError> {
        let candidates: Vec<&LicensingRecord> = self
            .records
            .iter()
            .filter(|record| record.dataset == dataset && record.territory == *territory)
            .collect();
        if candidates.is_empty() {
            return Err(RightsError::MissingRecord {
                dataset: dataset.to_owned(),
                territory: territory.as_str().to_owned(),
            });
        }
        if candidates.iter().any(|record| record.grants(right, at_us)) {
            Ok(())
        } else {
            Err(RightsError::RightNotGranted {
                dataset: dataset.to_owned(),
                right: right.as_str(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> LicensingRecord {
        let mut rights = BTreeMap::new();
        rights.insert(UsageRight::HistoricalStorage, RightDecision::Permitted);
        rights.insert(UsageRight::Redistribution, RightDecision::Prohibited);
        LicensingRecord {
            provider: "example".to_owned(),
            dataset: "quotes".to_owned(),
            territory: CountryCode::new("US").expect("country code"),
            rights,
            valid_until_us: Some(100),
            audit_required: true,
            reference: "contract-1".to_owned(),
        }
    }

    #[test]
    fn absent_right_is_denied() {
        let mut registry = LicensingRegistry::new();
        let territory = CountryCode::new("US").expect("country code");
        assert!(
            registry
                .authorize("quotes", &territory, UsageRight::HistoricalStorage, 1)
                .is_err(),
            "unregistered dataset must be denied"
        );
        registry.register(record());
        assert!(
            registry
                .authorize("quotes", &territory, UsageRight::HistoricalStorage, 1)
                .is_ok(),
            "registered right must be granted"
        );
        assert!(
            registry
                .authorize("quotes", &territory, UsageRight::Redistribution, 1)
                .is_err(),
            "prohibited right must be denied"
        );
    }

    #[test]
    fn expired_right_is_denied() {
        let mut registry = LicensingRegistry::new();
        registry.register(record());
        let territory = CountryCode::new("US").expect("country code");
        assert!(
            registry
                .authorize("quotes", &territory, UsageRight::HistoricalStorage, 101)
                .is_err(),
            "expired right must be denied"
        );
    }
}
