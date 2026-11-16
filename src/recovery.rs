//! Typed recovery decisions (M3.1).
//!
//! Recovery policy is a pure function of observed conditions. A cause is
//! evidence, not an instruction: the policy decides which action, if any, a
//! cause authorizes. Keeping this decision separate from the collector state
//! machine makes every recovery choice explainable and independently testable,
//! and prevents a quiet instrument from being silently escalated into a
//! source-wide reconnect.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::model::SourceId;

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

/// Exact target an acknowledgement or fence applies to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecoveryTarget {
    pub source: SourceId,
    pub instrument: String,
    pub generation: u64,
}

/// A receive fence bound to one exact recovery target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryFence {
    pub target: RecoveryTarget,
    pub connection_epoch: u64,
    pub receive_seq: u64,
}

impl RecoveryFence {
    /// Whether a frame read at `(epoch, seq)` is strictly newer than the
    /// fence. Snapshots at or before the fence cannot repair a gap.
    #[must_use]
    pub const fn is_before(&self, epoch: u64, seq: u64) -> bool {
        epoch > self.connection_epoch || (epoch == self.connection_epoch && seq > self.receive_seq)
    }

    /// Whether an acknowledgement from an older connection cannot fence this
    /// target.
    #[must_use]
    pub const fn rejects_stale_ack(&self, epoch: u64, seq: u64) -> bool {
        epoch < self.connection_epoch || (epoch == self.connection_epoch && seq < self.receive_seq)
    }
}

/// Identity of one instrument-scoped recovery request. The instrument and
/// generation are part of the key: two instruments on the same source can
/// never be coalesced into one request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InstrumentRecoveryKey {
    pub source: SourceId,
    pub instrument: String,
    pub generation: u64,
}

/// Identity of one source-scoped recovery request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceRecoveryKey {
    pub source: SourceId,
    pub generation: u64,
}

/// Terminal or observed state of a recovery request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RecoveryOutcome {
    /// The worker acknowledged the request before acting on it.
    Acknowledged {
        connection_epoch: u64,
        receive_seq: u64,
    },
    /// The request was cancelled before it was dispatched.
    Cancelled,
    /// The request failed; the reason is retained as evidence.
    Failed { reason: String },
}

/// One queued recovery request. Causes are accumulated, attempts are bounded,
/// and the outcome records what actually happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRequest {
    pub action: RecoveryAction,
    pub causes: Vec<RecoveryCause>,
    pub attempt: u32,
    pub max_attempts: u32,
    pub deadline_us: Option<i64>,
    pub outcome: Option<RecoveryOutcome>,
}

impl RecoveryRequest {
    fn new(
        action: RecoveryAction,
        causes: Vec<RecoveryCause>,
        max_attempts: u32,
        deadline_us: Option<i64>,
    ) -> Self {
        Self {
            action,
            causes,
            attempt: 0,
            max_attempts,
            deadline_us,
            outcome: None,
        }
    }

    fn merge_causes(&mut self, causes: &[RecoveryCause]) {
        let mut all: BTreeSet<RecoveryCause> = self.causes.iter().copied().collect();
        all.extend(causes.iter().copied());
        self.causes = all.into_iter().collect();
    }
}

/// Why a queue operation was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryQueueError {
    /// A request was routed to the queue that does not own its action.
    ActionMismatch {
        expected: RecoveryAction,
        found: RecoveryAction,
    },
    /// The queue is at its configured bound.
    CapacityExceeded {
        queue: &'static str,
        capacity: usize,
    },
}

impl fmt::Display for RecoveryQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActionMismatch { expected, found } => write!(
                formatter,
                "recovery action {found:?} cannot enter the {expected:?} queue"
            ),
            Self::CapacityExceeded { queue, capacity } => {
                write!(formatter, "{queue} recovery queue is full ({capacity})")
            }
        }
    }
}

impl std::error::Error for RecoveryQueueError {}

/// Bounded, ordered request storage shared by both recovery queues.
#[derive(Debug, Clone)]
struct RecoveryQueue<K: Ord> {
    name: &'static str,
    capacity: usize,
    requests: BTreeMap<K, RecoveryRequest>,
}

impl<K: Ord> RecoveryQueue<K> {
    fn insert(
        &mut self,
        key: K,
        request: RecoveryRequest,
        expected: RecoveryAction,
    ) -> Result<(), RecoveryQueueError> {
        if request.action != expected {
            return Err(RecoveryQueueError::ActionMismatch {
                expected,
                found: request.action,
            });
        }
        if !self.requests.contains_key(&key) && self.requests.len() >= self.capacity {
            return Err(RecoveryQueueError::CapacityExceeded {
                queue: self.name,
                capacity: self.capacity,
            });
        }
        self.requests.insert(key, request);
        Ok(())
    }

    fn get(&self, key: &K) -> Option<&RecoveryRequest> {
        self.requests.get(key)
    }

    fn get_mut(&mut self, key: &K) -> Option<&mut RecoveryRequest> {
        self.requests.get_mut(key)
    }

    fn remove(&mut self, key: &K) -> Option<RecoveryRequest> {
        self.requests.remove(key)
    }

    fn len(&self) -> usize {
        self.requests.len()
    }

    fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    fn drain_due(&self, now_us: i64) -> Vec<(K, RecoveryRequest)>
    where
        K: Clone,
    {
        // BTreeMap iteration is deterministic by key: (source, instrument,
        // generation) for instruments and (source, generation) for sources.
        self.requests
            .iter()
            .filter(|(_, request)| {
                request.outcome.is_none()
                    && request
                        .deadline_us
                        .is_none_or(|deadline| deadline <= now_us)
            })
            .map(|(key, request)| (key.clone(), request.clone()))
            .collect()
    }
}

/// Instrument-scoped recovery ownership. Entries remain independent even when
/// many quiet instruments share one source.
#[derive(Debug, Clone)]
pub struct InstrumentRecoveryQueue {
    inner: RecoveryQueue<InstrumentRecoveryKey>,
}

impl InstrumentRecoveryQueue {
    #[must_use]
    pub const fn new(capacity: usize) -> Self {
        Self {
            inner: RecoveryQueue {
                name: "instrument",
                capacity,
                requests: BTreeMap::new(),
            },
        }
    }

    /// Insert an in-place refresh. A reconnect request is rejected here.
    pub fn insert(
        &mut self,
        key: InstrumentRecoveryKey,
        request: RecoveryRequest,
    ) -> Result<(), RecoveryQueueError> {
        self.inner
            .insert(key, request, RecoveryAction::RefreshInstruments)
    }

    #[must_use]
    pub fn get(&self, key: &InstrumentRecoveryKey) -> Option<&RecoveryRequest> {
        self.inner.get(key)
    }

    pub fn merge_causes(&mut self, key: &InstrumentRecoveryKey, causes: &[RecoveryCause]) -> bool {
        let Some(request) = self.inner.get_mut(key) else {
            return false;
        };
        request.merge_causes(causes);
        true
    }

    pub fn set_outcome(&mut self, key: &InstrumentRecoveryKey, outcome: RecoveryOutcome) -> bool {
        let Some(request) = self.inner.get_mut(key) else {
            return false;
        };
        request.outcome = Some(outcome);
        true
    }

    pub fn remove(&mut self, key: &InstrumentRecoveryKey) -> Option<RecoveryRequest> {
        self.inner.remove(key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[must_use]
    pub fn drain_due(&self, now_us: i64) -> Vec<(InstrumentRecoveryKey, RecoveryRequest)> {
        self.inner.drain_due(now_us)
    }
}

/// Source-scoped recovery ownership. No instrument identity is stored here.
#[derive(Debug, Clone)]
pub struct SourceRecoveryQueue {
    inner: RecoveryQueue<SourceRecoveryKey>,
}

impl SourceRecoveryQueue {
    #[must_use]
    pub const fn new(capacity: usize) -> Self {
        Self {
            inner: RecoveryQueue {
                name: "source",
                capacity,
                requests: BTreeMap::new(),
            },
        }
    }

    /// Insert a source reconnect. A refresh request is rejected here.
    pub fn insert(
        &mut self,
        key: SourceRecoveryKey,
        request: RecoveryRequest,
    ) -> Result<(), RecoveryQueueError> {
        self.inner
            .insert(key, request, RecoveryAction::ReconnectSource)
    }

    #[must_use]
    pub fn get(&self, key: &SourceRecoveryKey) -> Option<&RecoveryRequest> {
        self.inner.get(key)
    }

    pub fn merge_causes(&mut self, key: &SourceRecoveryKey, causes: &[RecoveryCause]) -> bool {
        let Some(request) = self.inner.get_mut(key) else {
            return false;
        };
        request.merge_causes(causes);
        true
    }

    pub fn set_outcome(&mut self, key: &SourceRecoveryKey, outcome: RecoveryOutcome) -> bool {
        let Some(request) = self.inner.get_mut(key) else {
            return false;
        };
        request.outcome = Some(outcome);
        true
    }

    pub fn remove(&mut self, key: &SourceRecoveryKey) -> Option<RecoveryRequest> {
        self.inner.remove(key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[must_use]
    pub fn drain_due(&self, now_us: i64) -> Vec<(SourceRecoveryKey, RecoveryRequest)> {
        self.inner.drain_due(now_us)
    }
}

/// A deterministic snapshot of due work from both queues.
#[derive(Debug, Clone, Default)]
pub struct DueRecoveryWork {
    pub instruments: Vec<(InstrumentRecoveryKey, RecoveryRequest)>,
    pub sources: Vec<(SourceRecoveryKey, RecoveryRequest)>,
}

/// Counters describing the two independent queues.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryQueueStats {
    pub instrument_requests: usize,
    pub source_requests: usize,
}

/// The M3.2 coordinator: two disjoint queues with independent generations.
#[derive(Debug, Clone)]
pub struct RecoveryCoordinator {
    pub instruments: InstrumentRecoveryQueue,
    pub sources: SourceRecoveryQueue,
    next_generation: u64,
    max_instrument_attempts: u32,
    max_source_attempts: u32,
}

impl RecoveryCoordinator {
    #[must_use]
    pub const fn new(
        instrument_capacity: usize,
        source_capacity: usize,
        max_instrument_attempts: u32,
        max_source_attempts: u32,
    ) -> Self {
        Self {
            instruments: InstrumentRecoveryQueue::new(instrument_capacity),
            sources: SourceRecoveryQueue::new(source_capacity),
            next_generation: 1,
            max_instrument_attempts,
            max_source_attempts,
        }
    }

    fn allocate_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        generation
    }

    /// Request an in-place refresh of one instrument on one source.
    pub fn request_instrument_refresh(
        &mut self,
        source: SourceId,
        instrument: impl Into<String>,
        causes: &[RecoveryCause],
        deadline_us: Option<i64>,
    ) -> Result<InstrumentRecoveryKey, RecoveryQueueError> {
        let key = InstrumentRecoveryKey {
            source,
            instrument: instrument.into(),
            generation: self.allocate_generation(),
        };
        let request = RecoveryRequest::new(
            RecoveryAction::RefreshInstruments,
            causes.to_vec(),
            self.max_instrument_attempts,
            deadline_us,
        );
        self.instruments.insert(key.clone(), request)?;
        Ok(key)
    }

    /// Request a reconnect of one source. No instrument identity is involved.
    pub fn request_source_reconnect(
        &mut self,
        source: SourceId,
        causes: &[RecoveryCause],
        deadline_us: Option<i64>,
    ) -> Result<SourceRecoveryKey, RecoveryQueueError> {
        let key = SourceRecoveryKey {
            source,
            generation: self.allocate_generation(),
        };
        let request = RecoveryRequest::new(
            RecoveryAction::ReconnectSource,
            causes.to_vec(),
            self.max_source_attempts,
            deadline_us,
        );
        self.sources.insert(key.clone(), request)?;
        Ok(key)
    }

    /// Coalesce repeated causes onto the exact same target and generation.
    pub fn merge_instrument_causes(
        &mut self,
        key: &InstrumentRecoveryKey,
        causes: &[RecoveryCause],
    ) -> bool {
        self.instruments.merge_causes(key, causes)
    }

    pub fn merge_source_causes(
        &mut self,
        key: &SourceRecoveryKey,
        causes: &[RecoveryCause],
    ) -> bool {
        self.sources.merge_causes(key, causes)
    }

    pub fn acknowledge_instrument(
        &mut self,
        key: &InstrumentRecoveryKey,
        connection_epoch: u64,
        receive_seq: u64,
    ) -> bool {
        self.instruments.set_outcome(
            key,
            RecoveryOutcome::Acknowledged {
                connection_epoch,
                receive_seq,
            },
        )
    }

    pub fn acknowledge_source(
        &mut self,
        key: &SourceRecoveryKey,
        connection_epoch: u64,
        receive_seq: u64,
    ) -> bool {
        self.sources.set_outcome(
            key,
            RecoveryOutcome::Acknowledged {
                connection_epoch,
                receive_seq,
            },
        )
    }

    pub fn cancel_instrument(&mut self, key: &InstrumentRecoveryKey) -> bool {
        self.instruments
            .set_outcome(key, RecoveryOutcome::Cancelled)
    }

    pub fn cancel_source(&mut self, key: &SourceRecoveryKey) -> bool {
        self.sources.set_outcome(key, RecoveryOutcome::Cancelled)
    }

    #[must_use]
    pub fn stats(&self) -> RecoveryQueueStats {
        RecoveryQueueStats {
            instrument_requests: self.instruments.len(),
            source_requests: self.sources.len(),
        }
    }

    /// Deterministic due-work snapshot: instruments first, then sources, each
    /// in key order. Acknowledged and cancelled requests are not due.
    #[must_use]
    pub fn drain_due(&self, now_us: i64) -> DueRecoveryWork {
        DueRecoveryWork {
            instruments: self.instruments.drain_due(now_us),
            sources: self.sources.drain_due(now_us),
        }
    }

    /// Drop completed requests so successful retries can allocate fresh
    /// generations without unbounded queue growth.
    pub fn reap_completed(&mut self) -> usize {
        let before = self.stats();
        self.instruments
            .inner
            .requests
            .retain(|_, request| request.outcome.is_none());
        self.sources
            .inner
            .requests
            .retain(|_, request| request.outcome.is_none());
        let after = self.stats();
        before
            .instrument_requests
            .saturating_sub(after.instrument_requests)
            + before.source_requests.saturating_sub(after.source_requests)
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
    fn a_fence_is_bound_to_its_exact_instrument_and_generation() {
        let target = RecoveryTarget {
            source: SourceId::new(0, 0),
            instrument: "A".to_owned(),
            generation: 7,
        };
        let fence = RecoveryFence {
            target: target.clone(),
            connection_epoch: 2,
            receive_seq: 9,
        };
        let other = RecoveryTarget {
            instrument: "B".to_owned(),
            ..target.clone()
        };
        assert_ne!(target, other);
        assert_ne!(target.generation + 1, target.generation);
        assert!(fence.is_before(2, 10));
        assert!(fence.is_before(3, 1));
        assert!(!fence.is_before(2, 9));
        assert!(!fence.is_before(1, 100));
        assert!(fence.rejects_stale_ack(1, 100));
        assert!(!fence.rejects_stale_ack(2, 9));
    }

    #[test]
    fn three_thousand_quiet_instruments_fill_only_the_instrument_queue() {
        let mut coordinator = RecoveryCoordinator::new(4_000, 16, 3, 3);
        let source = SourceId::new(0, 0);
        for index in 0..3_000 {
            coordinator
                .request_instrument_refresh(
                    source,
                    format!("instrument-{index:04}"),
                    &[RecoveryCause::InstrumentQuiet],
                    None,
                )
                .unwrap();
        }
        let stats = coordinator.stats();
        assert_eq!(stats.instrument_requests, 3_000);
        assert_eq!(
            stats.source_requests, 0,
            "quiet instruments must not reconnect the source"
        );
    }

    #[test]
    fn instrument_requests_keep_independent_identities_and_generations() {
        let mut coordinator = RecoveryCoordinator::new(16, 16, 3, 3);
        let first = coordinator
            .request_instrument_refresh(
                SourceId::new(0, 0),
                "a",
                &[RecoveryCause::InstrumentQuiet],
                None,
            )
            .unwrap();
        let second = coordinator
            .request_instrument_refresh(
                SourceId::new(0, 0),
                "b",
                &[RecoveryCause::SnapshotTimeout],
                None,
            )
            .unwrap();
        assert_eq!(first.source, second.source);
        assert_ne!(first.instrument, second.instrument);
        assert_ne!(first.generation, second.generation);
        assert!(coordinator.instruments.get(&first).is_some());
        assert!(coordinator.instruments.get(&second).is_some());
    }

    #[test]
    fn reconnect_and_refresh_ownership_are_disjoint() {
        let mut coordinator = RecoveryCoordinator::new(16, 16, 3, 3);
        let source = SourceId::new(0, 0);
        let source_key = coordinator
            .request_source_reconnect(source, &[RecoveryCause::TransportClosed], None)
            .unwrap();
        let instrument_key = coordinator
            .request_instrument_refresh(source, "a", &[RecoveryCause::InstrumentQuiet], None)
            .unwrap();

        assert_eq!(source_key.source, source);
        assert_eq!(coordinator.stats().source_requests, 1);
        assert_eq!(coordinator.stats().instrument_requests, 1);
        assert!(coordinator.sources.get(&source_key).is_some());
        assert!(coordinator.instruments.get(&instrument_key).is_some());
        assert_eq!(
            coordinator.sources.get(&source_key).unwrap().action,
            RecoveryAction::ReconnectSource
        );
        assert_eq!(
            coordinator.instruments.get(&instrument_key).unwrap().action,
            RecoveryAction::RefreshInstruments
        );
    }

    #[test]
    fn queue_actions_are_checked_when_inserted_directly() {
        let mut instruments = InstrumentRecoveryQueue::new(4);
        let key = InstrumentRecoveryKey {
            source: SourceId::new(0, 0),
            instrument: "a".to_owned(),
            generation: 1,
        };
        let error = instruments
            .insert(
                key,
                RecoveryRequest::new(RecoveryAction::ReconnectSource, vec![], 3, None),
            )
            .unwrap_err();
        assert_eq!(
            error,
            RecoveryQueueError::ActionMismatch {
                expected: RecoveryAction::RefreshInstruments,
                found: RecoveryAction::ReconnectSource,
            }
        );
        assert!(instruments.is_empty());
    }

    #[test]
    fn coalescing_retains_the_exact_target_and_generation() {
        let mut coordinator = RecoveryCoordinator::new(16, 16, 3, 3);
        let key = coordinator
            .request_instrument_refresh(
                SourceId::new(0, 1),
                "a",
                &[RecoveryCause::InstrumentQuiet],
                None,
            )
            .unwrap();
        assert!(coordinator.merge_instrument_causes(
            &key,
            &[
                RecoveryCause::InstrumentQuiet,
                RecoveryCause::ScheduledRefresh
            ]
        ));
        let request = coordinator.instruments.get(&key).unwrap();
        assert_eq!(
            request.causes,
            vec![
                RecoveryCause::InstrumentQuiet,
                RecoveryCause::ScheduledRefresh
            ]
        );
        assert_eq!(coordinator.stats().instrument_requests, 1);
        assert!(!coordinator.merge_instrument_causes(
            &InstrumentRecoveryKey {
                generation: key.generation + 1,
                ..key.clone()
            },
            &[RecoveryCause::InstrumentQuiet]
        ));
    }

    #[test]
    fn queue_capacity_is_bounded() {
        let mut coordinator = RecoveryCoordinator::new(1, 1, 3, 3);
        let source = SourceId::new(0, 0);
        coordinator
            .request_instrument_refresh(source, "a", &[], None)
            .unwrap();
        let error = coordinator
            .request_instrument_refresh(source, "b", &[], None)
            .unwrap_err();
        assert_eq!(
            error,
            RecoveryQueueError::CapacityExceeded {
                queue: "instrument",
                capacity: 1,
            }
        );
    }

    #[test]
    fn due_work_is_deterministic_and_excludes_completed_requests() {
        let mut coordinator = RecoveryCoordinator::new(16, 16, 3, 3);
        let source = SourceId::new(0, 0);
        coordinator
            .request_instrument_refresh(source, "b", &[], Some(20))
            .unwrap();
        coordinator
            .request_instrument_refresh(source, "a", &[], Some(10))
            .unwrap();
        let acknowledged = coordinator
            .request_instrument_refresh(source, "c", &[], Some(5))
            .unwrap();
        coordinator.acknowledge_instrument(&acknowledged, 2, 7);

        let due_work = coordinator.drain_due(20);
        assert!(due_work.sources.is_empty());
        let due = due_work.instruments;
        let instruments: Vec<_> = due.iter().map(|(key, _)| key.instrument.as_str()).collect();
        assert_eq!(
            instruments,
            vec!["a", "b"],
            "key order, not insertion order"
        );
        let due_work = coordinator.drain_due(100);
        assert_eq!(
            due_work.instruments.len(),
            2,
            "acknowledged requests are never due again"
        );
    }

    #[test]
    fn outcomes_and_generations_are_recorded() {
        let mut coordinator = RecoveryCoordinator::new(16, 16, 3, 3);
        let source = SourceId::new(0, 0);
        let instrument = coordinator
            .request_instrument_refresh(source, "a", &[], Some(1))
            .unwrap();
        let source_key = coordinator
            .request_source_reconnect(source, &[], Some(1))
            .unwrap();
        assert!(coordinator.acknowledge_instrument(&instrument, 4, 9));
        assert!(coordinator.acknowledge_source(&source_key, 4, 9));
        assert_eq!(
            coordinator.instruments.get(&instrument).unwrap().outcome,
            Some(RecoveryOutcome::Acknowledged {
                connection_epoch: 4,
                receive_seq: 9,
            })
        );
        assert_eq!(coordinator.reap_completed(), 2);
        assert_eq!(coordinator.stats(), RecoveryQueueStats::default());

        let fresh = coordinator
            .request_instrument_refresh(source, "a", &[], None)
            .unwrap();
        assert!(
            fresh.generation > instrument.generation,
            "generations are never reused"
        );
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
