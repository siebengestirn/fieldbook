//! Deterministic disconnect/recovery fault schedule for the stress laboratory.
//!
//! The sweep is *explicit*, not random-only chaos. A root seed derives the
//! concrete step parameters (target replica, failure offsets, deadlines), but
//! the resulting [`FaultSchedule`] is a plain list of named steps that is
//! replayed in order by the cell runner and copied verbatim into the manifest.
//! Every step carries the evidence fields the disconnect fault matrix needs:
//! fault id, affected source/instruments, start epoch, failure time, type,
//! expected action/outcome, and deadline.
//!
//! Scenarios the loopback mock cannot drive (`D5`, `D6`, `D9`) are still listed
//! in the full matrix as `reference_only` steps. They are covered by honest
//! regression tests against the existing recovery/fence machinery; the harness
//! never fabricates a "recovered" result for a fault it did not inject.

use serde::Serialize;

use super::rng::SplitMix64;

/// Fault identifier in the M5.4 disconnect fault matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum FaultId {
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    D10,
}

impl FaultId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::D1 => "D1",
            Self::D2 => "D2",
            Self::D3 => "D3",
            Self::D4 => "D4",
            Self::D5 => "D5",
            Self::D6 => "D6",
            Self::D7 => "D7",
            Self::D8 => "D8",
            Self::D9 => "D9",
            Self::D10 => "D10",
        }
    }
}

/// Which book socket the mock closes for a single-replica loss. The collector,
/// not the mock, assigns the active/standby role; ordering by connection id is
/// the harness's deterministic proxy (the earlier socket wins the initial
/// snapshot race and becomes active).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionOrdinal {
    First,
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultType {
    /// Close exactly one book socket (standby- or active-role replica).
    ReplicaLoss,
    /// Close every book socket for all shards.
    BothReplicasLost,
    /// Close every socket, including the lifecycle control plane.
    AllConnectionsLost,
    /// Refuse replacement sockets so re-anchoring cannot complete.
    RejectReconnect,
    /// No mock action: the scenario is certified by a named regression test
    /// against the production recovery/fence machinery.
    ReferenceOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AffectedSource {
    FirstBookConnection,
    LastBookConnection,
    AllBookSources,
    AllSources,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AffectedInstruments {
    /// Every instrument owned by the affected source.
    AllOnSource,
    /// All eligible instruments in the universe.
    AllEligible,
    /// A specific instrument; covered by the recovery/fence regression tests.
    One,
}

/// One explicit step in the disconnect sweep.
#[derive(Debug, Clone, Serialize)]
pub struct FaultStep {
    pub fault_id: FaultId,
    pub fault_type: FaultType,
    pub affected_source: AffectedSource,
    pub affected_instruments: AffectedInstruments,
    /// Logical connection epoch at which the fault is scheduled. Epoch 1 is
    /// the first successful connection; each reconnect advances it.
    pub start_epoch: u64,
    /// Planned offset from anchor completion, in milliseconds.
    pub failure_time_ms: u64,
    /// Recovery deadline measured from the fault, in milliseconds. `None`
    /// means the scenario has no recovery (timeout/exhaustion).
    pub deadline_ms: Option<u64>,
    /// Target replica ordinal when `fault_type == ReplicaLoss`.
    pub connection_ordinal: Option<ConnectionOrdinal>,
    pub expected_action: String,
    pub expected_outcome: String,
    /// When true the runner must not attempt the mock action; the step is a
    /// matrix entry whose evidence lives in a named regression test.
    pub reference_only: bool,
}

/// A deterministic, ordered list of fault steps.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FaultSchedule {
    pub steps: Vec<FaultStep>,
}

impl FaultSchedule {
    /// Steps executed by the cell runner through the mock.
    #[must_use]
    pub fn driven_steps(&self) -> Vec<&FaultStep> {
        self.steps
            .iter()
            .filter(|step| !step.reference_only)
            .collect()
    }
}

/// Which slice of the disconnect matrix a cell runs. Each profile derives an
/// explicit schedule from the cell seed; no per-frame randomness is involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepProfile {
    /// D1: close the newest (standby-role) book socket.
    StandbyReplicaLoss,
    /// D2: close the oldest (active-role) book socket.
    ActiveReplicaLoss,
    /// D3 + D4: close all book sockets, then re-anchor with new epochs.
    BothReplicasLost,
    /// D7: full-universe re-anchoring after losing all book sockets.
    ManyInstruments,
    /// D8 only: refuse reconnects past the deadline; the run must surface the
    /// timeout as incomplete instead of silently claiming success.
    RecoveryTimeout,
    /// D8 + D10: refuse reconnects past a deadline, then a storm of reconnects.
    RecoveryStorm,
    /// D10 only: repeated reconnects with independent generations.
    RepeatedReconnects,
    /// The complete D1-D10 matrix, including reference-only entries.
    FullMatrix,
}

#[allow(
    clippy::too_many_arguments,
    reason = "flat arguments keep each manifest column explicit at the call site"
)]
fn step(
    fault_id: FaultId,
    fault_type: FaultType,
    affected_source: AffectedSource,
    affected_instruments: AffectedInstruments,
    start_epoch: u64,
    failure_time_ms: u64,
    deadline_ms: Option<u64>,
    connection_ordinal: Option<ConnectionOrdinal>,
    expected_action: &str,
    expected_outcome: &str,
) -> FaultStep {
    FaultStep {
        fault_id,
        fault_type,
        affected_source,
        affected_instruments,
        start_epoch,
        failure_time_ms,
        deadline_ms,
        connection_ordinal,
        expected_action: expected_action.to_owned(),
        expected_outcome: expected_outcome.to_owned(),
        reference_only: false,
    }
}

fn reference(
    fault_id: FaultId,
    fault_type: FaultType,
    start_epoch: u64,
    expected_action: &str,
    expected_outcome: &str,
) -> FaultStep {
    FaultStep {
        fault_id,
        fault_type,
        affected_source: AffectedSource::None,
        affected_instruments: AffectedInstruments::One,
        start_epoch,
        failure_time_ms: 0,
        deadline_ms: None,
        connection_ordinal: None,
        expected_action: expected_action.to_owned(),
        expected_outcome: expected_outcome.to_owned(),
        reference_only: true,
    }
}

/// Derive an explicit schedule for `profile`. The seed perturbs only timing and
/// target selection within pinned ranges; it never changes the scenario set.
#[must_use]
pub fn derive_schedule(profile: SweepProfile, seed: u64) -> FaultSchedule {
    // Dedicated stream: workload/publish streams must not be perturbed.
    let mut rng = SplitMix64::new(seed ^ 0xD1C0_FFEE_D100_0000);
    let mut offset = |lo: u64, hi: u64| lo + (rng.below(hi - lo + 1));
    let mut step = |id, kind, source, instruments, epoch, deadline, ordinal, action, outcome| {
        step(
            id,
            kind,
            source,
            instruments,
            epoch,
            offset(40, 160),
            deadline,
            ordinal,
            action,
            outcome,
        )
    };

    let steps = match profile {
        SweepProfile::StandbyReplicaLoss => vec![step(
            FaultId::D1,
            FaultType::ReplicaLoss,
            AffectedSource::LastBookConnection,
            AffectedInstruments::AllOnSource,
            1,
            Some(4_000),
            Some(ConnectionOrdinal::Last),
            "close the newest book socket; surviving replica re-anchors",
            "one discontinuity incident, one failover snapshot, Live continuity",
        )],
        SweepProfile::ActiveReplicaLoss => vec![step(
            FaultId::D2,
            FaultType::ReplicaLoss,
            AffectedSource::FirstBookConnection,
            AffectedInstruments::AllOnSource,
            1,
            Some(4_000),
            Some(ConnectionOrdinal::First),
            "close the oldest (active-role) book socket; shadow takes over",
            "one discontinuity incident, one failover snapshot from the shadow truth",
        )],
        SweepProfile::BothReplicasLost => vec![
            step(
                FaultId::D3,
                FaultType::BothReplicasLost,
                AffectedSource::AllBookSources,
                AffectedInstruments::AllEligible,
                1,
                Some(6_000),
                None,
                "close every book socket; both workers reconnect with new epochs",
                "incident per previously active symbol; fresh snapshots re-anchor all",
            ),
            step(
                FaultId::D4,
                FaultType::ReplicaLoss,
                AffectedSource::LastBookConnection,
                AffectedInstruments::AllOnSource,
                2,
                Some(4_000),
                Some(ConnectionOrdinal::Last),
                "reconnect established a new epoch; verify fence rejects stale frames",
                "new connection id and epoch; no stale snapshot promoted",
            ),
        ],
        SweepProfile::ManyInstruments => vec![
            step(
                FaultId::D7,
                FaultType::BothReplicasLost,
                AffectedSource::AllBookSources,
                AffectedInstruments::AllEligible,
                1,
                Some(8_000),
                None,
                "lose every book socket so the full eligible universe needs recovery",
                "all instruments re-anchor; exact coverage certifies with no drops",
            ),
            step(
                FaultId::D4,
                FaultType::ReplicaLoss,
                AffectedSource::LastBookConnection,
                AffectedInstruments::AllOnSource,
                2,
                Some(6_000),
                Some(ConnectionOrdinal::Last),
                "reconnect established a new epoch; verify fence rejects stale frames",
                "new connection id and epoch; no stale snapshot promoted",
            ),
        ],
        SweepProfile::RecoveryTimeout => vec![step(
            FaultId::D8,
            FaultType::RejectReconnect,
            AffectedSource::AllBookSources,
            AffectedInstruments::AllEligible,
            1,
            Some(2_000),
            None,
            "close all book sockets and refuse replacements past the deadline",
            "recovery timeout surfaces as incomplete, never silent success",
        )],
        SweepProfile::RecoveryStorm => {
            let mut steps = vec![step(
                FaultId::D8,
                FaultType::RejectReconnect,
                AffectedSource::AllBookSources,
                AffectedInstruments::AllEligible,
                1,
                Some(2_000),
                None,
                "close all book sockets and refuse replacements past the deadline",
                "recovery timeout surfaces as incomplete, never silent success",
            )];
            for round in 0..3u64 {
                steps.push(step(
                    FaultId::D10,
                    if round == 0 {
                        FaultType::AllConnectionsLost
                    } else {
                        FaultType::BothReplicasLost
                    },
                    if round == 0 {
                        AffectedSource::AllSources
                    } else {
                        AffectedSource::AllBookSources
                    },
                    AffectedInstruments::AllEligible,
                    2 + round,
                    Some(6_000),
                    None,
                    "accept reconnects and cycle all sockets again",
                    "independent generations; bounded queues; healthy books; SBE verifies",
                ));
            }
            steps
        }
        SweepProfile::RepeatedReconnects => (0..3u64)
            .map(|round| {
                step(
                    FaultId::D10,
                    if round == 0 {
                        FaultType::AllConnectionsLost
                    } else {
                        FaultType::BothReplicasLost
                    },
                    if round == 0 {
                        AffectedSource::AllSources
                    } else {
                        AffectedSource::AllBookSources
                    },
                    AffectedInstruments::AllEligible,
                    1 + round,
                    Some(12_000),
                    None,
                    "cycle all sockets, then verify re-anchoring and accounting",
                    "independent generations; bounded queues; healthy books; SBE verifies",
                )
            })
            .collect(),
        SweepProfile::FullMatrix => vec![
            step(
                FaultId::D1,
                FaultType::ReplicaLoss,
                AffectedSource::LastBookConnection,
                AffectedInstruments::AllOnSource,
                1,
                Some(4_000),
                Some(ConnectionOrdinal::Last),
                "close the standby-role book socket",
                "surviving replica re-anchors; one incident and one failover snapshot",
            ),
            step(
                FaultId::D2,
                FaultType::ReplicaLoss,
                AffectedSource::FirstBookConnection,
                AffectedInstruments::AllOnSource,
                2,
                Some(4_000),
                Some(ConnectionOrdinal::First),
                "close the active-role book socket",
                "shadow truth is promoted by a failover snapshot",
            ),
            step(
                FaultId::D3,
                FaultType::BothReplicasLost,
                AffectedSource::AllBookSources,
                AffectedInstruments::AllEligible,
                3,
                Some(6_000),
                None,
                "close all book sockets",
                "all previously active symbols re-anchor from fresh snapshots",
            ),
            step(
                FaultId::D4,
                FaultType::ReplicaLoss,
                AffectedSource::LastBookConnection,
                AffectedInstruments::AllOnSource,
                4,
                Some(4_000),
                Some(ConnectionOrdinal::Last),
                "reconnect with a new epoch",
                "stale-epoch frames cannot satisfy the recovery fence",
            ),
            reference(
                FaultId::D5,
                FaultType::ReferenceOnly,
                4,
                "deliver an old-epoch snapshot after the fence",
                "regression test proves the stale snapshot is rejected before mutation",
            ),
            reference(
                FaultId::D6,
                FaultType::ReferenceOnly,
                4,
                "deliver an old-epoch delta after the fence",
                "regression test proves stale deltas cannot apply to the new book",
            ),
            step(
                FaultId::D7,
                FaultType::BothReplicasLost,
                AffectedSource::AllBookSources,
                AffectedInstruments::AllEligible,
                5,
                Some(6_000),
                None,
                "re-anchor the full eligible universe after total book loss",
                "anchored set equals the expected universe; coverage certifies",
            ),
            step(
                FaultId::D8,
                FaultType::RejectReconnect,
                AffectedSource::AllBookSources,
                AffectedInstruments::AllEligible,
                6,
                Some(2_000),
                None,
                "refuse reconnects past the recovery deadline",
                "run is incomplete with resyncing symbols; no silent success",
            ),
            reference(
                FaultId::D9,
                FaultType::ReferenceOnly,
                6,
                "exhaust the bounded recovery attempt budget",
                "regression test proves exhaustion is an explicit non-escalating outcome",
            ),
            step(
                FaultId::D10,
                FaultType::AllConnectionsLost,
                AffectedSource::AllSources,
                AffectedInstruments::AllEligible,
                7,
                Some(6_000),
                None,
                "storm: cycle every socket repeatedly",
                "independent generations, bounded queues, healthy books, SBE verifies",
            ),
        ],
    };
    FaultSchedule { steps }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fault_ids_are_stable_and_distinct() {
        let all = [
            FaultId::D1,
            FaultId::D2,
            FaultId::D3,
            FaultId::D4,
            FaultId::D5,
            FaultId::D6,
            FaultId::D7,
            FaultId::D8,
            FaultId::D9,
            FaultId::D10,
        ];
        let mut names = std::collections::BTreeSet::new();
        for id in all {
            assert!(names.insert(id.as_str()), "duplicate fault id {id:?}");
        }
        assert_eq!(names.len(), 10);
    }

    #[test]
    fn schedule_is_seed_derived_and_explicit() {
        let a = derive_schedule(SweepProfile::RepeatedReconnects, 7);
        let b = derive_schedule(SweepProfile::RepeatedReconnects, 7);
        let c = derive_schedule(SweepProfile::RepeatedReconnects, 8);
        let times = |s: &FaultSchedule| {
            s.steps
                .iter()
                .map(|step| step.failure_time_ms)
                .collect::<Vec<_>>()
        };
        assert_eq!(times(&a), times(&b), "same seed must replay identically");
        assert_ne!(
            times(&a),
            times(&c),
            "different seeds derive different offsets"
        );
        for step in &a.steps {
            assert!((40..=160).contains(&step.failure_time_ms));
        }
    }

    #[test]
    fn full_matrix_declares_all_ten_faults() {
        let schedule = derive_schedule(SweepProfile::FullMatrix, 42);
        let ids: Vec<FaultId> = schedule.steps.iter().map(|step| step.fault_id).collect();
        assert_eq!(
            ids,
            vec![
                FaultId::D1,
                FaultId::D2,
                FaultId::D3,
                FaultId::D4,
                FaultId::D5,
                FaultId::D6,
                FaultId::D7,
                FaultId::D8,
                FaultId::D9,
                FaultId::D10,
            ]
        );
        assert_eq!(schedule.driven_steps().len(), 7);
        // D5, D6 and D9 are honest reference-only entries.
        assert_eq!(
            schedule
                .steps
                .iter()
                .filter(|step| step.reference_only)
                .count(),
            3
        );
    }
}
