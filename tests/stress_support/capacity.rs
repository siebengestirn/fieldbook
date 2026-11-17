//! Capacity characterization stages and workload-model selection.
//!
//! A capacity cell is described by two independent choices: a **stage**, which
//! fixes the eligible-instrument count, and a **workload model**, which decides
//! how many logical deltas the existing publish scheduler offers. This module
//! only selects configuration; it does not generate anything. The returned
//! [`CellConfig`] is executed by the same `SyntheticUniverse` generator and
//! `publish_events` scheduler used by every other stress cell, so a capacity
//! run exercises the production collector path without a second harness.
//!
//! Evidence class: synthetic/loopback. A certified cell proves that one
//! instrument count and one workload shape were collected correctly; it does
//! not by itself establish a throughput or discovery ceiling.

use super::{
    lifecycle::LifecycleScenario,
    runner::{CellConfig, HotSelection},
};

/// Logical deltas per instrument for the fixed-per-instrument-rate model.
///
/// One delta per instrument keeps a 200-instrument selection test short and
/// makes the expected event count a transparent function of the stage.
pub const PER_INSTRUMENT_EVENTS: usize = 1;

/// Total logical deltas for the fixed-aggregate and skewed hot/quiet models.
/// A bounded budget is what makes 10k/25k/50k short correctness cells short.
pub const AGGREGATE_EVENTS: usize = 400;

/// Portion of instruments designated hot for the skewed model (5%).
pub const HOT_FRACTION_PPM: u32 = 50_000;

/// Portion of deltas routed to designated hot instruments (80%).
pub const HOT_EVENT_SHARE_PCT: u32 = 80;

/// One eligible-instrument stage of the capacity progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityStage {
    /// Filesystem/manifest-safe label (`4k`, `10k`, `25k`, `50k`).
    pub label: &'static str,
    /// Exact eligible-instrument count the generator must produce.
    pub symbols: usize,
}

impl CapacityStage {
    pub const FOUR_K: Self = Self {
        label: "4k",
        symbols: 4_000,
    };
    pub const TEN_K: Self = Self {
        label: "10k",
        symbols: 10_000,
    };
    pub const TWENTY_FIVE_K: Self = Self {
        label: "25k",
        symbols: 25_000,
    };
    pub const FIFTY_K: Self = Self {
        label: "50k",
        symbols: 50_000,
    };

    /// All supported stages, in ascending instrument count.
    pub const ALL: [Self; 4] = [
        Self::FOUR_K,
        Self::TEN_K,
        Self::TWENTY_FIVE_K,
        Self::FIFTY_K,
    ];

    /// Parse a stage label or raw instrument count.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "4k" | "4000" => Some(Self::FOUR_K),
            "10k" | "10000" => Some(Self::TEN_K),
            "25k" | "25000" => Some(Self::TWENTY_FIVE_K),
            "50k" | "50000" => Some(Self::FIFTY_K),
            _ => None,
        }
    }

    /// Stage selected by `STRESS_CAPACITY_STAGE`, if set and recognized.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        std::env::var("STRESS_CAPACITY_STAGE")
            .ok()
            .and_then(|value| Self::parse(&value))
    }
}

/// How the existing publish scheduler is configured for a capacity cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadModel {
    /// Total deltas scale with instrument count: `symbols * rate`.
    FixedPerInstrument,
    /// A fixed total is offered regardless of instrument count.
    FixedAggregate,
    /// A fixed total routed 80/20 to a deterministic 5% hot set.
    SkewedHotQuiet,
}

impl WorkloadModel {
    /// Parse the manifest/CLI spelling of a workload model.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fixed_per_instrument" | "per_instrument" => Some(Self::FixedPerInstrument),
            "fixed_aggregate" | "aggregate" => Some(Self::FixedAggregate),
            "skewed_hot_quiet" | "hot_quiet" => Some(Self::SkewedHotQuiet),
            _ => None,
        }
    }

    /// Model selected by `STRESS_WORKLOAD_MODEL`; skewed hot/quiet is the
    /// default for the short opt-in correctness cells.
    #[must_use]
    pub fn from_env_or_default() -> Self {
        std::env::var("STRESS_WORKLOAD_MODEL")
            .ok()
            .and_then(|value| Self::parse(&value))
            .unwrap_or(Self::SkewedHotQuiet)
    }

    /// Stable manifest spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixedPerInstrument => "fixed_per_instrument",
            Self::FixedAggregate => "fixed_aggregate",
            Self::SkewedHotQuiet => "skewed_hot_quiet",
        }
    }
}

/// One capacity characterization selection: a stage plus a workload model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityCell {
    pub stage: CapacityStage,
    pub model: WorkloadModel,
}

impl CapacityCell {
    /// Exact number of logical deltas this selection will offer.
    #[must_use]
    pub fn expected_events(self) -> usize {
        match self.model {
            WorkloadModel::FixedPerInstrument => {
                self.stage.symbols.saturating_mul(PER_INSTRUMENT_EVENTS)
            }
            WorkloadModel::FixedAggregate | WorkloadModel::SkewedHotQuiet => AGGREGATE_EVENTS,
        }
    }

    /// Translate the selection into the existing stress-runner configuration.
    ///
    /// Shards are derived (`None`) so the production capacity planner scales
    /// the topology with the instrument count instead of hard-coding it. The
    /// hot/quiet model reuses the exact-quota role assignment already exercised
    /// by the 4k reference cells.
    #[must_use]
    pub fn cell_config(self, seed: u64) -> CellConfig {
        let skewed = self.model == WorkloadModel::SkewedHotQuiet;
        CellConfig {
            seed,
            symbols: self.stage.symbols,
            decoy_fraction_ppm: 20_000,
            shards: None,
            replicas: 2,
            lifecycle_replicas: 1,
            events: self.expected_events(),
            hot_pair: false,
            uniform_spread: !skewed,
            hot_fraction_ppm: if skewed { HOT_FRACTION_PPM } else { 0 },
            hot_event_share_pct: if skewed { HOT_EVENT_SHARE_PCT } else { 0 },
            hot_selection: if skewed {
                HotSelection::ExactQuota
            } else {
                HotSelection::Weighted
            },
            burst: None,
            journal: true,
            // Anchoring and draining a 50k universe take longer than the 4k
            // smoke budget; these are opt-in cells, never part of default CI.
            // Keep the sum below the runner's 120 s collector safety timeout.
            limits: Some((75, 15)),
            force_capacity_reject: false,
            fail_inject: None,
            lifecycle_scenario: LifecycleScenario::None,
            writer_queue: None,
            worktree_dir: None,
            require_clean_worktree: None,
            fault_schedule: None,
            capacity: Some(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_parsing_covers_the_full_progression() {
        for (value, expected) in [
            ("4k", CapacityStage::FOUR_K),
            ("4000", CapacityStage::FOUR_K),
            ("10k", CapacityStage::TEN_K),
            ("10000", CapacityStage::TEN_K),
            ("25k", CapacityStage::TWENTY_FIVE_K),
            ("25000", CapacityStage::TWENTY_FIVE_K),
            ("50k", CapacityStage::FIFTY_K),
            ("50000", CapacityStage::FIFTY_K),
        ] {
            assert_eq!(CapacityStage::parse(value), Some(expected), "{value}");
        }
        assert_eq!(CapacityStage::parse("7k"), None);
        assert_eq!(CapacityStage::parse(""), None);
        assert_eq!(CapacityStage::ALL.len(), 4);
    }

    #[test]
    fn model_parsing_and_manifest_spelling_round_trip() {
        for (value, expected) in [
            ("fixed_per_instrument", WorkloadModel::FixedPerInstrument),
            ("per_instrument", WorkloadModel::FixedPerInstrument),
            ("fixed_aggregate", WorkloadModel::FixedAggregate),
            ("aggregate", WorkloadModel::FixedAggregate),
            ("skewed_hot_quiet", WorkloadModel::SkewedHotQuiet),
            ("hot_quiet", WorkloadModel::SkewedHotQuiet),
        ] {
            assert_eq!(WorkloadModel::parse(value), Some(expected), "{value}");
            assert!(expected.as_str().contains(value), "{value}");
        }
        assert_eq!(WorkloadModel::parse("burst"), None);
    }
}
