//! Universal data-quality states and reconciliation accounting.
//!
//! A failed event carries a state and a machine-readable reason. It never
//! disappears silently. Reconciliation is independent of source-specific
//! semantics and applies to every data domain.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

/// Stage in the observation lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityState {
    RawCaptured,
    Decoded,
    Normalized,
    Validated,
    Reconciled,
    Quarantined,
    Rejected,
}

impl QualityState {
    /// Return the stable contract string for this state.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RawCaptured => "raw_captured",
            Self::Decoded => "decoded",
            Self::Normalized => "normalized",
            Self::Validated => "validated",
            Self::Reconciled => "reconciled",
            Self::Quarantined => "quarantined",
            Self::Rejected => "rejected",
        }
    }

    /// Check whether a transition is permitted.
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::RawCaptured, Self::Decoded)
                | (Self::RawCaptured, Self::Quarantined)
                | (Self::RawCaptured, Self::Rejected)
                | (Self::Decoded, Self::Normalized)
                | (Self::Decoded, Self::Quarantined)
                | (Self::Decoded, Self::Rejected)
                | (Self::Normalized, Self::Validated)
                | (Self::Normalized, Self::Quarantined)
                | (Self::Validated, Self::Reconciled)
                | (Self::Validated, Self::Quarantined)
                | (Self::Quarantined, Self::Decoded)
                | (Self::Rejected, Self::Rejected)
        )
    }
}

/// Machine-readable reason for a quality disposition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityReason {
    DecodeFailure,
    SchemaVersionUnsupported,
    SequenceGap,
    DuplicateObservation,
    OutOfOrder,
    MissingInstrumentMapping,
    AmbiguousInstrumentMapping,
    InvalidNumericRepresentation,
    TimestampAnomaly,
    StaleObservation,
    CrossedBook,
    SnapshotReconstructionFailure,
    ReferenceConflict,
    RightsDenied,
    SourceUnavailable,
}

impl QualityReason {
    /// Return the stable contract string for this reason.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::DecodeFailure => "decode_failure",
            Self::SchemaVersionUnsupported => "schema_version_unsupported",
            Self::SequenceGap => "sequence_gap",
            Self::DuplicateObservation => "duplicate_observation",
            Self::OutOfOrder => "out_of_order",
            Self::MissingInstrumentMapping => "missing_instrument_mapping",
            Self::AmbiguousInstrumentMapping => "ambiguous_instrument_mapping",
            Self::InvalidNumericRepresentation => "invalid_numeric_representation",
            Self::TimestampAnomaly => "timestamp_anomaly",
            Self::StaleObservation => "stale_observation",
            Self::CrossedBook => "crossed_book",
            Self::SnapshotReconstructionFailure => "snapshot_reconstruction_failure",
            Self::ReferenceConflict => "reference_conflict",
            Self::RightsDenied => "rights_denied",
            Self::SourceUnavailable => "source_unavailable",
        }
    }
}

/// Outcome of one quality transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRecord {
    pub event_id: String,
    pub state: QualityState,
    pub reason: Option<QualityReason>,
    pub observed_at_us: i64,
    pub verifier: String,
}

/// Illegal quality transition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("illegal quality transition from {from:?} to {to:?}")]
pub struct IllegalTransition {
    pub from: QualityState,
    pub to: QualityState,
}

/// Apply a transition without weakening the state model.
pub fn transition(
    current: QualityState,
    next: QualityState,
) -> Result<QualityState, IllegalTransition> {
    if current.can_transition_to(next) {
        Ok(next)
    } else {
        Err(IllegalTransition {
            from: current,
            to: next,
        })
    }
}

/// Two-sided record accounting for one reconciliation boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reconciliation {
    pub expected: u64,
    pub observed: u64,
    pub missing: u64,
    pub extra: u64,
    pub duplicated: u64,
    pub rejected: u64,
    pub quarantined: u64,
}

impl Reconciliation {
    /// Check the accounting invariant for this boundary.
    pub fn reconciled(&self) -> bool {
        self.missing == 0
            && self.extra == 0
            && self.duplicated == 0
            && self.expected == self.observed + self.rejected + self.quarantined
    }
}

/// Quality report for one source and dataset over a stated time window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityReport {
    pub source_id: String,
    pub dataset: String,
    pub window_start_us: i64,
    pub window_end_us: i64,
    pub states: BTreeMap<String, u64>,
    pub reasons: BTreeMap<String, u64>,
    pub reconciliation: Reconciliation,
}

impl QualityReport {
    /// Check that the report reconciles and its counters are coherent.
    pub fn valid(&self) -> bool {
        self.window_end_us >= self.window_start_us && self.reconciliation.reconciled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_records_remain_accounted() {
        let report = Reconciliation {
            expected: 10,
            observed: 7,
            missing: 0,
            extra: 0,
            duplicated: 0,
            rejected: 2,
            quarantined: 1,
        };
        assert!(report.reconciled(), "balanced accounting must reconcile");
        let lossy = Reconciliation {
            missing: 1,
            ..report
        };
        assert!(
            !lossy.reconciled(),
            "missing records must fail reconciliation"
        );
    }

    #[test]
    fn transitions_are_explicit() {
        assert_eq!(
            transition(QualityState::RawCaptured, QualityState::Decoded),
            Ok(QualityState::Decoded),
            "decode transition must be allowed"
        );
        assert!(
            transition(QualityState::Rejected, QualityState::Reconciled).is_err(),
            "rejected records must not reach reconciled"
        );
    }

    #[test]
    fn quality_reasons_have_stable_strings() {
        let text = serde_json::to_string(&QualityReason::SequenceGap).expect("serialize reason");
        assert_eq!(text, "\"sequence_gap\"", "reason string must be stable");
    }
}
