//! Backpressure workload profiles for the M5.5 stress increment.
//!
//! A backpressure cell is not a generic "something was slow" fixture. Each
//! boundary has its own capacity, admission rule, timeout, drop/reject policy,
//! recovery path and observable evidence. The five terminal states are kept
//! separate so a frame that is waiting in a bounded queue can never be
//! reported as dropped, a rejected frame can never be reported as completed,
//! and an outcome cut short by shutdown is explicitly uncertain.
//!
//! The profile is derived from the cell configuration *before* the run, and
//! the manifest is validated against the profile-specific required fields
//! before a success verdict survives. Evidence that cannot be observed stays
//! `null` and fails certification; it is never defaulted to zero.

use engineering_challenge::journal::RunState;
use serde_json::{Map, Value, json};

use super::runner::{CellConfig, CellOutput, FailInject};

/// The boundary under test. These are distinct admission domains; one profile
/// never substitutes for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureBoundary {
    /// Mock websocket outbound queue (`try_send`, hard bound).
    MockOutboundQueue,
    /// Production SBE writer queue (bounded `send` with a timeout).
    WriterQueue,
    /// Slow socket consumer: frames are admitted but complete late.
    SlowDownstream,
    /// Raw-frame journal admission and durable storage path.
    JournalStorage,
    /// Shutdown cut before the drain barrier; outcome is not observable.
    ShutdownDrain,
}

impl BackpressureBoundary {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MockOutboundQueue => "mock_outbound_queue",
            Self::WriterQueue => "writer_queue",
            Self::SlowDownstream => "slow_downstream",
            Self::JournalStorage => "journal_storage",
            Self::ShutdownDrain => "shutdown_drain",
        }
    }
}

/// Terminal state of one offered frame at the boundary. These variants are
/// mutually exclusive by construction; the observation counts are sourced
/// from independent counters and reconciled by [`BackpressureObservation::primary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueDisposition {
    /// Admitted into the bounded queue; completion not yet observed.
    AcceptedWaiting,
    /// Admission refused by the boundary (error returned to the caller).
    Rejected,
    /// Accepted for transmission but discarded without an acknowledgement.
    Dropped,
    /// Send/write/record completed and was observed.
    Completed,
    /// Shutdown ended observation before the outcome could be established.
    UncertainAcknowledgement,
}

impl QueueDisposition {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AcceptedWaiting => "accepted_but_waiting",
            Self::Rejected => "rejected",
            Self::Dropped => "dropped",
            Self::Completed => "completed",
            Self::UncertainAcknowledgement => "uncertain_acknowledgement",
        }
    }
}

/// Frozen policy of one backpressure profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackpressureProfile {
    pub boundary: BackpressureBoundary,
    pub disposition: QueueDisposition,
    pub capacity: u64,
    /// Admission contract in force at the boundary.
    pub admission: &'static str,
    /// Bound that governs admission, in milliseconds (`0` for the shutdown
    /// cut, which has no wait at all).
    pub timeout_ms: u64,
    pub drop_reject_policy: &'static str,
    pub recovery: &'static str,
}

impl BackpressureProfile {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self.boundary {
            BackpressureBoundary::MockOutboundQueue => "mock_queue_saturation",
            BackpressureBoundary::WriterQueue => "writer_queue_saturation",
            BackpressureBoundary::SlowDownstream => "slow_downstream",
            BackpressureBoundary::JournalStorage => "journal_storage_pressure",
            BackpressureBoundary::ShutdownDrain => "shutdown_uncertain_acknowledgement",
        }
    }

    /// Derive the declared profile from the cell configuration. A cell with
    /// no bound override is not a backpressure profile and carries no
    /// backpressure-specific acceptance gate.
    #[must_use]
    pub fn from_config(config: &CellConfig) -> Option<Self> {
        match config.fail_inject {
            Some(FailInject::MockQueueSaturate { capacity }) => Some(Self {
                boundary: BackpressureBoundary::MockOutboundQueue,
                disposition: QueueDisposition::Dropped,
                capacity: capacity as u64,
                admission: "non-blocking try_send into a per-connection bounded queue",
                timeout_ms: config
                    .limits
                    .map(|(_, drain)| drain.saturating_mul(1_000))
                    .unwrap_or(10_000),
                drop_reject_policy: "full queue is an accounted drop (queue_saturated); no retry",
                recovery: "none inside the run: forwarder is stalled until shutdown",
            }),
            Some(FailInject::SlowDownstream { delay_ms: _ }) => Some(Self {
                boundary: BackpressureBoundary::SlowDownstream,
                disposition: QueueDisposition::AcceptedWaiting,
                capacity: super::mock::DEFAULT_OUTBOUND_CAPACITY as u64,
                admission: "ordinary bounded enqueue; completion delayed after admission",
                timeout_ms: config
                    .limits
                    .map(|(_, drain)| drain.saturating_mul(1_000))
                    .unwrap_or(10_000),
                drop_reject_policy: "no drop and no rejection: frames remain in flight",
                recovery: "the queue drains after the injected delay; this cell cuts at the barrier",
            }),
            Some(FailInject::JournalQueuePressure {
                queue_capacity,
                enqueue_timeout_ms,
            }) => Some(Self {
                boundary: BackpressureBoundary::JournalStorage,
                disposition: QueueDisposition::Rejected,
                capacity: queue_capacity as u64,
                admission: "bounded send_timeout with an fsync on every record",
                timeout_ms: enqueue_timeout_ms,
                drop_reject_policy: "enqueue timeout rejects the record and latches the run failed",
                recovery: "latched failure is terminal; accepted records drain at finalization",
            }),
            Some(FailInject::ShutdownBeforeDrain) => Some(Self {
                boundary: BackpressureBoundary::ShutdownDrain,
                disposition: QueueDisposition::UncertainAcknowledgement,
                capacity: 0,
                admission: "normal admission until shutdown is signalled",
                timeout_ms: 0,
                drop_reject_policy: "no drop policy can be verified: observation ends at shutdown",
                recovery: "none in this cell; the outcome stays uncertain by contract",
            }),
            None if config.writer_queue.is_some() => Some(Self {
                boundary: BackpressureBoundary::WriterQueue,
                disposition: QueueDisposition::Completed,
                capacity: config.writer_queue.unwrap_or(0) as u64,
                admission: "bounded send awaits capacity, at most writer_send_timeout_secs",
                timeout_ms: 10_000,
                drop_reject_policy: "a timed-out send never enters the queue and is reported as an error",
                recovery: "shutdown closes admission and drains every accepted message",
            }),
            _ => None,
        }
    }
}

/// Independently sourced counters for the five terminal states.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackpressureObservation {
    pub accepted_waiting: u64,
    pub rejected: u64,
    pub dropped: u64,
    pub completed: u64,
    pub uncertain_acknowledgement: u64,
}

impl BackpressureObservation {
    /// The dominant non-zero state, with fail-closed priority: an uncertain
    /// or rejected outcome outranks a later completion that happened to be
    /// observed before shutdown.
    #[must_use]
    pub fn primary(&self) -> QueueDisposition {
        if self.uncertain_acknowledgement > 0 {
            return QueueDisposition::UncertainAcknowledgement;
        }
        if self.rejected > 0 {
            return QueueDisposition::Rejected;
        }
        if self.dropped > 0 {
            return QueueDisposition::Dropped;
        }
        if self.accepted_waiting > 0 {
            return QueueDisposition::AcceptedWaiting;
        }
        QueueDisposition::Completed
    }

    #[must_use]
    pub fn to_json(self) -> Value {
        json!({
            "accepted_waiting": self.accepted_waiting,
            "rejected": self.rejected,
            "dropped": self.dropped,
            "completed": self.completed,
            "uncertain_acknowledgement": self.uncertain_acknowledgement,
        })
    }
}

fn journal_state_name(state: RunState) -> &'static str {
    match state {
        RunState::Starting => "starting",
        RunState::Open => "open",
        RunState::Finalized => "finalized",
        RunState::Incomplete => "incomplete",
        RunState::Failed => "failed",
    }
}

fn journal_json(output: Option<&CellOutput>, reasons: &[String]) -> Value {
    let Some(output) = output else {
        // A collector that stopped before producing a report still carries
        // its own terminal error. When that error is the journal rejection
        // latch, record it as a failed journal state instead of losing the
        // boundary classification.
        let rejected = reasons.iter().any(|reason| {
            reason.contains("journal run failed")
                || reason.contains("journal queue stayed full")
                || reason == "journal_pressure_rejected"
        });
        return if rejected {
            let mut errors: Vec<Value> = reasons
                .iter()
                .map(|reason| Value::String(reason.clone()))
                .collect();
            // Preserve the boundary-specific diagnostic even when the
            // collector stopped before emitting a run report.
            errors.push(Value::String(
                "journal queue stayed full (journal pressure admission rejected)".to_owned(),
            ));
            json!({
                "state": "failed",
                "complete": false,
                "record_count": Value::Null,
                "errors": errors,
            })
        } else {
            json!({
                "state": Value::Null,
                "complete": Value::Null,
                "record_count": Value::Null,
                "errors": Value::Null,
            })
        };
    };
    match &output.report.journal_verification {
        Some(verification) => json!({
            "state": journal_state_name(verification.state),
            "complete": verification.complete,
            "record_count": verification.record_count,
            "errors": verification.errors,
        }),
        None => json!({
            "state": Value::Null,
            "complete": Value::Null,
            "record_count": Value::Null,
            "errors": Value::Null,
        }),
    }
}

fn sbe_json(output: Option<&CellOutput>) -> Value {
    let Some(output) = output else {
        return json!({
            "verify_valid": super::manifest::unavailable("capture not produced before shutdown"),
            "completeness_valid": super::manifest::unavailable("capture not produced before shutdown"),
            "deltas": super::manifest::unavailable("capture not produced before shutdown"),
        });
    };
    json!({
        "verify_valid": output.verify_valid,
        "completeness_valid": output.completeness_valid,
        "deltas": output.sbe_deltas,
    })
}

/// Reconcile the declared profile with the run counters.
#[must_use]
pub fn observe(
    profile: Option<&BackpressureProfile>,
    output: Option<&CellOutput>,
) -> BackpressureObservation {
    let (Some(profile), Some(output)) = (profile, output) else {
        return BackpressureObservation::default();
    };
    match profile.boundary {
        BackpressureBoundary::MockOutboundQueue => BackpressureObservation {
            accepted_waiting: output.mock.in_flight,
            rejected: 0,
            dropped: output
                .mock
                .queue_saturated
                .saturating_add(output.mock.injected_pre_wire_drops),
            completed: output.mock.sent,
            uncertain_acknowledgement: 0,
        },
        BackpressureBoundary::WriterQueue => BackpressureObservation {
            accepted_waiting: output.report.writer.queue_depth,
            rejected: output.report.writer.errors,
            dropped: 0,
            completed: output.ledger.writer_delta_writes.unwrap_or(0),
            uncertain_acknowledgement: 0,
        },
        BackpressureBoundary::SlowDownstream => BackpressureObservation {
            accepted_waiting: output.mock.in_flight,
            rejected: 0,
            dropped: output.mock.queue_saturated,
            completed: output.mock.sent,
            uncertain_acknowledgement: 0,
        },
        BackpressureBoundary::JournalStorage => {
            let failed = output
                .report
                .journal_verification
                .as_ref()
                .is_some_and(|verification| verification.state == RunState::Failed);
            BackpressureObservation {
                accepted_waiting: 0,
                rejected: u64::from(failed),
                dropped: 0,
                completed: output.journaled_records.unwrap_or(0),
                uncertain_acknowledgement: 0,
            }
        }
        BackpressureBoundary::ShutdownDrain => BackpressureObservation {
            accepted_waiting: output.mock.in_flight,
            rejected: 0,
            dropped: 0,
            completed: output.applied_events.unwrap_or(0),
            uncertain_acknowledgement: output
                .ledger
                .generated_events
                .saturating_sub(output.applied_events.unwrap_or(0)),
        },
    }
}

fn evidence_json(
    profile: Option<&BackpressureProfile>,
    output: Option<&CellOutput>,
    reasons: &[String],
) -> Value {
    let Some(profile) = profile else {
        return Value::Null;
    };
    let Some(output) = output else {
        if profile.boundary == BackpressureBoundary::JournalStorage {
            return json!({
                "journaled_records": Value::Null,
                "journal_complete": false,
                "generated_events": Value::Null,
                "applied_events": Value::Null,
                "application_shortfall": Value::Null,
                "rejection_detail": reasons,
            });
        }
        return Value::Null;
    };
    match profile.boundary {
        BackpressureBoundary::MockOutboundQueue => json!({
            "mock_queue_capacity": output.mock.outbound_capacity,
            "mock_queue_high_water": output.mock.queue_high_water,
            "mock_queue_saturated": output.mock.queue_saturated,
            "mock_enqueued": output.mock.enqueued,
            "mock_enqueued_deltas": output.mock.enqueued_deltas,
            "mock_socket_written": output.mock.sent,
            "mock_in_flight_at_end": output.mock.in_flight,
            "enqueue_transmit_shortfall": output.ledger.enqueue_transmit_shortfall,
        }),
        BackpressureBoundary::WriterQueue => json!({
            "writer_queue_capacity": profile.capacity,
            "writer_queue_high_water": output.writer_queue_high_water,
            "writer_queue_depth_at_end": output.report.writer.queue_depth,
            "writer_errors": output.report.writer.errors,
            "writer_delta_admissions": output.ledger.writer_delta_admissions,
            "writer_delta_writes": output.ledger.writer_delta_writes,
            "writer_write_shortfall": output.ledger.writer_write_shortfall,
        }),
        BackpressureBoundary::SlowDownstream => json!({
            "forwarder_delay_ms": output.mock.forwarder_delay_ms,
            "mock_queue_capacity": output.mock.outbound_capacity,
            "mock_queue_high_water": output.mock.queue_high_water,
            "mock_queue_saturated": output.mock.queue_saturated,
            "mock_in_flight_at_end": output.mock.in_flight,
            "drain_completed": output.drain_completed,
        }),
        BackpressureBoundary::JournalStorage => json!({
            "journaled_records": output.journaled_records,
            "journal_complete": output.journal_complete,
            "writer_errors": output.report.writer.errors,
            "generated_events": output.events_published,
            "applied_events": output.applied_events,
            "application_shortfall": output.ledger.application_shortfall,
        }),
        BackpressureBoundary::ShutdownDrain => json!({
            "generated_events": output.ledger.generated_events,
            "applied_events": output.applied_events,
            "mock_in_flight_at_end": output.mock.in_flight,
            "writer_queue_depth_at_end": output.report.writer.queue_depth,
            "drain_completed": output.drain_completed,
        }),
    }
}

/// The complete `backpressure` manifest object. Non-profile cells carry a
/// `none` marker; a profile cell that cannot produce evidence leaves the
/// relevant fields null so the manifest gate rejects it.
#[must_use]
pub fn manifest_json(
    profile: Option<&BackpressureProfile>,
    observation: &BackpressureObservation,
    output: Option<&CellOutput>,
    reasons: &[String],
) -> Value {
    let Some(profile) = profile else {
        return json!({
            "profile": "none",
            "boundary": Value::Null,
            "disposition": Value::Null,
            "capacity": Value::Null,
            "admission": Value::Null,
            "timeout_ms": Value::Null,
            "drop_reject_policy": Value::Null,
            "recovery": Value::Null,
            "states": Value::Null,
            "evidence": Value::Null,
            "journal": journal_json(output, reasons),
            "sbe": sbe_json(output),
        });
    };
    let states = if output.is_none() && profile.boundary == BackpressureBoundary::JournalStorage {
        // The collector reported the rejection but died before a report could
        // be written; the latched rejection is still a distinct state.
        BackpressureObservation {
            rejected: 1,
            ..*observation
        }
    } else {
        *observation
    };
    json!({
        "profile": profile.name(),
        "boundary": profile.boundary.as_str(),
        "disposition": profile.disposition.as_str(),
        "capacity": profile.capacity,
        "admission": profile.admission,
        "timeout_ms": profile.timeout_ms,
        "drop_reject_policy": profile.drop_reject_policy,
        "recovery": profile.recovery,
        "states": states.to_json(),
        "evidence": evidence_json(Some(profile), output, reasons),
        "journal": journal_json(output, reasons),
        "sbe": sbe_json(output),
    })
}

fn resolve<'a>(document: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let mut current = document.get(parts.next()?)?;
    for part in parts {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

/// Fields a success manifest must carry when this cell declares a profile.
/// Global fields are checked separately by `manifest::missing_required_fields`.
#[must_use]
pub fn missing_required_fields(config: &CellConfig, document: &Map<String, Value>) -> Vec<String> {
    let Some(profile) = BackpressureProfile::from_config(config) else {
        return Vec::new();
    };
    let mut paths = vec![
        "backpressure.profile".to_owned(),
        "backpressure.boundary".to_owned(),
        "backpressure.disposition".to_owned(),
        "backpressure.capacity".to_owned(),
        "backpressure.admission".to_owned(),
        "backpressure.timeout_ms".to_owned(),
        "backpressure.drop_reject_policy".to_owned(),
        "backpressure.recovery".to_owned(),
        "backpressure.states.accepted_waiting".to_owned(),
        "backpressure.states.rejected".to_owned(),
        "backpressure.states.dropped".to_owned(),
        "backpressure.states.completed".to_owned(),
        "backpressure.states.uncertain_acknowledgement".to_owned(),
        "backpressure.journal.state".to_owned(),
        "backpressure.journal.complete".to_owned(),
        "backpressure.sbe.verify_valid".to_owned(),
        "backpressure.sbe.completeness_valid".to_owned(),
    ];
    paths.push(format!(
        "backpressure.evidence.{}",
        match profile.boundary {
            BackpressureBoundary::MockOutboundQueue => "mock_queue_saturated",
            BackpressureBoundary::WriterQueue => "writer_queue_high_water",
            BackpressureBoundary::SlowDownstream => "forwarder_delay_ms",
            BackpressureBoundary::JournalStorage => "journaled_records",
            BackpressureBoundary::ShutdownDrain => "generated_events",
        }
    ));
    paths
        .into_iter()
        .filter(|path| matches!(resolve(document, path), None | Some(Value::Null)))
        .collect()
}

/// Boundary-specific acceptance reasons. These are appended to the ordinary
/// ledger reasons, so a backpressure cell cannot pass on the strength of a
/// reconciled ledger alone.
#[must_use]
pub fn acceptance_reasons(
    config: &CellConfig,
    observation: &BackpressureObservation,
    output: Option<&CellOutput>,
) -> Vec<String> {
    let Some(profile) = BackpressureProfile::from_config(config) else {
        return Vec::new();
    };
    let mut reasons = Vec::new();
    match profile.boundary {
        BackpressureBoundary::MockOutboundQueue => {
            if observation.dropped > 0 {
                reasons.push("mock_outbound_queue_dropped".to_owned());
            }
        }
        BackpressureBoundary::WriterQueue => {
            if let Some(output) = output
                && output.writer_queue_high_water > profile.capacity.saturating_add(1)
            {
                reasons.push("writer_queue_bound_breached".to_owned());
            }
            if observation.rejected > 0 {
                reasons.push("writer_queue_rejected".to_owned());
            }
        }
        BackpressureBoundary::SlowDownstream => {
            if observation.accepted_waiting > 0 {
                reasons.push("slow_downstream_accepted_waiting".to_owned());
            }
        }
        BackpressureBoundary::JournalStorage => {
            // A journal-pressure cell that never reached capture verification
            // cannot certify either; the collector's own terminal error is
            // carried in the failure reasons by the runner.
            if output.is_none()
                || output
                    .and_then(|output| output.report.journal_verification.as_ref())
                    .is_some_and(|verification| {
                        verification.state == RunState::Failed || !verification.complete
                    })
            {
                reasons.push("journal_pressure_rejected".to_owned());
            }
        }
        BackpressureBoundary::ShutdownDrain => {
            reasons.push("uncertain_acknowledgement".to_owned());
        }
    }
    // A generic mismatch is a secondary observation: the boundary-specific
    // class must lead so the failure class names the real cause.
    if observation.primary() != profile.disposition {
        reasons.push("backpressure_disposition_mismatch".to_owned());
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> CellConfig {
        CellConfig {
            limits: Some((10, 2)),
            ..CellConfig::default()
        }
    }

    #[test]
    fn every_boundary_declares_a_distinct_policy_and_disposition() {
        let cases = [
            (
                FailInject::MockQueueSaturate { capacity: 8 },
                "mock_outbound_queue",
                "dropped",
            ),
            (
                FailInject::SlowDownstream { delay_ms: 5 },
                "slow_downstream",
                "accepted_but_waiting",
            ),
            (
                FailInject::JournalQueuePressure {
                    queue_capacity: 1,
                    enqueue_timeout_ms: 1,
                },
                "journal_storage",
                "rejected",
            ),
            (
                FailInject::ShutdownBeforeDrain,
                "shutdown_drain",
                "uncertain_acknowledgement",
            ),
        ];
        let mut dispositions = std::collections::BTreeSet::new();
        for (inject, boundary, disposition) in cases {
            let profile = BackpressureProfile::from_config(&CellConfig {
                fail_inject: Some(inject),
                ..config()
            })
            .expect("injection declares a profile");
            assert_eq!(profile.boundary.as_str(), boundary);
            assert_eq!(profile.disposition.as_str(), disposition);
            assert!(!profile.admission.is_empty());
            assert!(!profile.drop_reject_policy.is_empty());
            assert!(!profile.recovery.is_empty());
            assert!(dispositions.insert(disposition));
        }
        let writer = BackpressureProfile::from_config(&CellConfig {
            writer_queue: Some(128),
            ..config()
        })
        .expect("writer queue declares a profile");
        assert_eq!(writer.boundary.as_str(), "writer_queue");
        assert_eq!(writer.disposition.as_str(), "completed");
        assert_eq!(writer.capacity, 128);
        assert!(dispositions.insert("completed"));
        assert_eq!(dispositions.len(), 5);
        assert!(BackpressureProfile::from_config(&config()).is_none());
    }

    #[test]
    fn observation_never_collapses_waiting_dropped_and_uncertain() {
        let mut observation = BackpressureObservation {
            accepted_waiting: 100,
            rejected: 0,
            dropped: 5,
            completed: 90,
            uncertain_acknowledgement: 0,
        };
        assert_eq!(observation.primary(), QueueDisposition::Dropped);
        observation.dropped = 0;
        assert_eq!(observation.primary(), QueueDisposition::AcceptedWaiting);
        observation.rejected = 1;
        assert_eq!(observation.primary(), QueueDisposition::Rejected);
        observation.uncertain_acknowledgement = 2;
        assert_eq!(
            observation.primary(),
            QueueDisposition::UncertainAcknowledgement
        );
        let json = observation.to_json();
        assert_eq!(json["accepted_waiting"], 100);
        assert_eq!(json["rejected"], 1);
        assert_eq!(json["dropped"], 0);
        assert_eq!(json["completed"], 90);
        assert_eq!(json["uncertain_acknowledgement"], 2);
    }

    #[test]
    fn profile_manifest_requires_boundary_specific_evidence() {
        let cell = CellConfig {
            writer_queue: Some(128),
            ..config()
        };
        let document = Map::new();
        let missing = missing_required_fields(&cell, &document);
        assert!(missing.contains(&"backpressure.evidence.writer_queue_high_water".to_owned()));
        assert!(missing.contains(&"backpressure.states.completed".to_owned()));
        assert!(missing_required_fields(&config(), &document).is_empty());
    }

    #[test]
    fn acceptance_reasons_name_the_dominant_state() {
        let cell = CellConfig {
            fail_inject: Some(FailInject::ShutdownBeforeDrain),
            ..config()
        };
        let observation = BackpressureObservation {
            uncertain_acknowledgement: 7,
            ..BackpressureObservation::default()
        };
        let reasons = acceptance_reasons(&cell, &observation, None);
        assert!(
            reasons
                .iter()
                .any(|reason| reason == "uncertain_acknowledgement")
        );
        assert!(acceptance_reasons(&config(), &observation, None).is_empty());
    }
}
