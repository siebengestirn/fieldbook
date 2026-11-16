//! Typed recovery decisions (M3.1).
//!
//! Recovery policy is a pure function of observed conditions. A cause is
//! evidence, not an instruction: the policy decides which action, if any, a
//! cause authorizes. Keeping this decision separate from the collector state
//! machine makes every recovery choice explainable and independently testable,
//! and prevents a quiet instrument from being silently escalated into a
//! source-wide reconnect.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// The action a recovery decision authorizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    /// Refresh one or more instruments on their existing connection.
    RefreshInstruments,
    /// Reconnect the source, which establishes a new connection epoch.
    ReconnectSource,
}

/// Why a recovery decision was made. Causes are retained on the decision even
/// when a stronger cause determines the action, so the audit record explains
/// all contributing evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryCause {
    /// One instrument went quiet while its source stayed healthy.
    InstrumentQuiet,
    /// An expected snapshot did not arrive within the policy window.
    SnapshotTimeout,
    /// The attempt budget for this recovery is already spent.
    RecoveryExhausted,
    /// The venue heartbeat failed. A disabled heartbeat never produces this
    /// cause.
    HeartbeatFailure,
    /// The transport connection closed or a read failed.
    TransportClosed,
    /// A payload was rejected by decoding or validation.
    DecodeFailure,
    /// The venue protocol reported a violation such as a sequence gap.
    ProtocolViolation,
    /// A periodic refresh was due.
    ScheduledRefresh,
    /// An operator explicitly requested recovery.
    OperatorRequested,
}

/// Conditions observed by the caller. This is deliberately a snapshot of
/// facts, not a decision: the policy owns the cause-to-action mapping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecoverySignals {
    pub instrument_quiet: bool,
    pub snapshot_timeout: bool,
    pub heartbeat_failed: bool,
    /// When the venue has no heartbeat mechanism, a missed heartbeat is not
    /// evidence of failure and must not authorize a reconnect.
    pub heartbeat_disabled: bool,
    pub transport_closed: bool,
    pub decode_failed: bool,
    pub protocol_violation: bool,
    pub scheduled_refresh: bool,
    pub operator_requested: bool,
    /// Attempts already consumed for this recovery target.
    pub attempts: u32,
    /// Maximum attempts permitted by policy. Zero permits none.
    pub max_attempts: u32,
}

/// The auditable outcome of applying policy to observed conditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryDecision {
    /// Every contributing cause, in stable order. Empty when there is no
    /// evidence of a recovery need.
    pub causes: Vec<RecoveryCause>,
    /// The authorized action, or `None` when no action is justified or the
    /// attempt budget is exhausted.
    pub action: Option<RecoveryAction>,
    /// True when causes exist but the attempt budget forbids another action.
    pub exhausted: bool,
}

impl RecoveryDecision {
    /// Whether this decision authorizes reconnecting the source.
    #[must_use]
    pub const fn reconnects_source(&self) -> bool {
        matches!(self.action, Some(RecoveryAction::ReconnectSource))
    }

    /// Whether this decision authorizes refreshing instruments in place.
    #[must_use]
    pub const fn refreshes_instruments(&self) -> bool {
        matches!(self.action, Some(RecoveryAction::RefreshInstruments))
    }
}

/// Decide the recovery action for one target from observed conditions.
///
/// The function is pure: identical signals always produce an identical
/// decision. A quiet instrument, a snapshot timeout, and a scheduled refresh
/// authorize an in-place refresh only. Transport, heartbeat, decode, protocol,
/// and operator causes authorize a source reconnect. When both classes are
/// present the stronger action wins, but every cause is retained. Exhausting
/// the attempt budget records [`RecoveryCause::RecoveryExhausted`] and withholds
/// the action instead of escalating without limit.
#[must_use]
pub fn decide(signals: RecoverySignals) -> RecoveryDecision {
    let mut causes = BTreeSet::new();
    if signals.instrument_quiet {
        causes.insert(RecoveryCause::InstrumentQuiet);
    }
    if signals.snapshot_timeout {
        causes.insert(RecoveryCause::SnapshotTimeout);
    }
    if signals.transport_closed {
        causes.insert(RecoveryCause::TransportClosed);
    }
    if signals.decode_failed {
        causes.insert(RecoveryCause::DecodeFailure);
    }
    if signals.protocol_violation {
        causes.insert(RecoveryCause::ProtocolViolation);
    }
    if signals.heartbeat_failed && !signals.heartbeat_disabled {
        causes.insert(RecoveryCause::HeartbeatFailure);
    }
    if signals.scheduled_refresh {
        causes.insert(RecoveryCause::ScheduledRefresh);
    }
    if signals.operator_requested {
        causes.insert(RecoveryCause::OperatorRequested);
    }

    let reconnect = causes.iter().any(|cause| {
        matches!(
            cause,
            RecoveryCause::HeartbeatFailure
                | RecoveryCause::TransportClosed
                | RecoveryCause::DecodeFailure
                | RecoveryCause::ProtocolViolation
                | RecoveryCause::OperatorRequested
        )
    });
    let refresh = causes.iter().any(|cause| {
        matches!(
            cause,
            RecoveryCause::InstrumentQuiet
                | RecoveryCause::SnapshotTimeout
                | RecoveryCause::ScheduledRefresh
        )
    });
    let action = if reconnect {
        Some(RecoveryAction::ReconnectSource)
    } else if refresh {
        Some(RecoveryAction::RefreshInstruments)
    } else {
        None
    };

    let exhausted = action.is_some() && signals.attempts >= signals.max_attempts;
    if exhausted {
        causes.insert(RecoveryCause::RecoveryExhausted);
    }
    RecoveryDecision {
        causes: causes.into_iter().collect(),
        action: if exhausted { None } else { action },
        exhausted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quiet_instrument_never_reconnects_its_source() {
        let decision = decide(RecoverySignals {
            instrument_quiet: true,
            max_attempts: 3,
            ..RecoverySignals::default()
        });
        assert!(decision.refreshes_instruments());
        assert!(!decision.reconnects_source());
        assert_eq!(decision.causes, vec![RecoveryCause::InstrumentQuiet]);
        assert!(!decision.exhausted);
    }

    #[test]
    fn transport_failure_authorizes_a_source_reconnect() {
        let decision = decide(RecoverySignals {
            transport_closed: true,
            max_attempts: 3,
            ..RecoverySignals::default()
        });
        assert!(decision.reconnects_source());
        assert_eq!(decision.causes, vec![RecoveryCause::TransportClosed]);
    }

    #[test]
    fn disabled_heartbeat_cannot_produce_a_false_failure() {
        let signals = RecoverySignals {
            heartbeat_failed: true,
            heartbeat_disabled: true,
            max_attempts: 3,
            ..RecoverySignals::default()
        };
        assert_eq!(decide(signals), decide(RecoverySignals::default()));
    }

    #[test]
    fn heartbeat_timeout_is_distinct_from_missing_market_data() {
        let heartbeat = decide(RecoverySignals {
            heartbeat_failed: true,
            max_attempts: 3,
            ..RecoverySignals::default()
        });
        let quiet = decide(RecoverySignals {
            instrument_quiet: true,
            max_attempts: 3,
            ..RecoverySignals::default()
        });
        assert!(heartbeat.reconnects_source());
        assert!(quiet.refreshes_instruments());
        assert_ne!(heartbeat.causes, quiet.causes);
    }

    #[test]
    fn budget_exhaustion_is_an_explicit_outcome_not_unbounded_escalation() {
        let decision = decide(RecoverySignals {
            instrument_quiet: true,
            attempts: 3,
            max_attempts: 3,
            ..RecoverySignals::default()
        });
        assert!(decision.exhausted);
        assert_eq!(decision.action, None);
        assert_eq!(
            decision.causes,
            vec![
                RecoveryCause::InstrumentQuiet,
                RecoveryCause::RecoveryExhausted
            ]
        );
    }

    #[test]
    fn repeated_causes_do_not_escalate_the_action() {
        let signals = RecoverySignals {
            instrument_quiet: true,
            attempts: 1,
            max_attempts: 3,
            ..RecoverySignals::default()
        };
        let first = decide(signals);
        let second = decide(signals);
        assert_eq!(first, second);
        assert_eq!(first.causes, vec![RecoveryCause::InstrumentQuiet]);
    }

    #[test]
    fn a_stronger_cause_wins_without_erasing_other_evidence() {
        let decision = decide(RecoverySignals {
            instrument_quiet: true,
            snapshot_timeout: true,
            transport_closed: true,
            max_attempts: 3,
            ..RecoverySignals::default()
        });
        assert!(decision.reconnects_source());
        assert_eq!(
            decision.causes,
            vec![
                RecoveryCause::InstrumentQuiet,
                RecoveryCause::SnapshotTimeout,
                RecoveryCause::TransportClosed,
            ]
        );
    }

    #[test]
    fn no_signals_produce_no_action() {
        let decision = decide(RecoverySignals::default());
        assert_eq!(decision.action, None);
        assert!(decision.causes.is_empty());
        assert!(!decision.exhausted);
    }

    #[test]
    fn decisions_serialize_for_audit_evidence() {
        let decision = decide(RecoverySignals {
            operator_requested: true,
            max_attempts: 1,
            ..RecoverySignals::default()
        });
        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("reconnect_source"));
        assert!(json.contains("operator_requested"));
        assert_eq!(
            serde_json::from_str::<RecoveryDecision>(&json).unwrap(),
            decision
        );
    }
}
