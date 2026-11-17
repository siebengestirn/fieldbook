//! Typed lifecycle-churn scenarios for the deterministic stress laboratory.
//!
//! A scenario is a *selector* plus a deterministic schedule: the root seed and
//! an explicit, ordered event list (including venue timestamps) are the whole
//! workload. Catalog mutations are applied to the mock at the moment each
//! event is published, so discovery always sees the authoritative universe
//! after a retirement or an addition. Nothing here calls production code
//! directly; the schedule drives the collector through the mock wire.
//!
//! Each event records the effect the current producer contract requires:
//! `Applied` (one `core.lifecycle_events`), `Duplicate` (identity dedup),
//! `Stale` (monotonic high-water mark) or `Unknown` (an event for a market the
//! catalog does not contain). The runner certifies the observed counters against
//! this expectation before it can publish a `success` manifest.

use std::collections::BTreeSet;

use serde_json::{Value, json};

/// Version of the scenario/schedule semantics, recorded through the runner's
/// `scenario_version` and in the manifest scenario block.
pub const LIFECYCLE_SCHEDULE_VERSION: u32 = 1;

/// Typed selector stored in `CellConfig`. `None` keeps every existing cell on
/// the historical workload; the other ten variants are the L1-L10 scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LifecycleScenario {
    #[default]
    None,
    /// L1: a new market is created and an active market is activated.
    AddActivate,
    /// L2: lifecycle replicas subscribe to `market_lifecycle_v2`.
    Subscribe,
    /// L3: the added market is snapshot-anchored before deltas reach it.
    SnapshotAnchor,
    /// L4: mutable metadata changes on a tracked market.
    MetadataChange,
    /// L5: a terminal `settled` retires a market exactly once.
    RetireSettle,
    /// L6: an identical redelivery is deduplicated.
    Duplicate,
    /// L7: an older nonterminal notification is dropped.
    OutOfOrder,
    /// L8: two different payloads with the same kind/timestamp both apply.
    ConflictingReplica,
    /// L9: reappearance after retirement is governed by the discovery round's
    /// start time (a stale round is vetoed, a later round re-adds).
    Reappearance,
    /// L10: lifecycle churn interleaved with the full delta workload.
    ChurnUnderLoad,
}

impl LifecycleScenario {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AddActivate => "add_activate",
            Self::Subscribe => "subscribe",
            Self::SnapshotAnchor => "snapshot_anchor",
            Self::MetadataChange => "metadata_change",
            Self::RetireSettle => "retire_settle",
            Self::Duplicate => "duplicate",
            Self::OutOfOrder => "out_of_order",
            Self::ConflictingReplica => "conflicting_replica",
            Self::Reappearance => "reappearance",
            Self::ChurnUnderLoad => "churn_under_load",
        }
    }

    #[must_use]
    pub const fn scenario_id(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::AddActivate => Some("L1"),
            Self::Subscribe => Some("L2"),
            Self::SnapshotAnchor => Some("L3"),
            Self::MetadataChange => Some("L4"),
            Self::RetireSettle => Some("L5"),
            Self::Duplicate => Some("L6"),
            Self::OutOfOrder => Some("L7"),
            Self::ConflictingReplica => Some("L8"),
            Self::Reappearance => Some("L9"),
            Self::ChurnUnderLoad => Some("L10"),
        }
    }
}

/// How an event changes the authoritative REST catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogEffect {
    None,
    Add,
    Retire,
}

/// Observed counter effect required by the current state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEffect {
    Applied,
    Duplicate,
    Stale,
}

/// Market addressed by a schedule event, resolved once against the synthetic
/// universe so every event carries a concrete, reproducible ticker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketRef {
    Eligible(usize),
    Added,
    Retired,
}

/// One explicit schedule entry. `after_deltas` places the event in the delta
/// stream; `exchange_ts_ms` pins dedup and ordering independently of scheduling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEvent {
    pub step: u32,
    pub epoch: u32,
    pub after_deltas: usize,
    pub event_type: &'static str,
    pub ticker: String,
    pub exchange_ts_ms: i64,
    pub extra: Value,
    pub catalog_effect: CatalogEffect,
    pub expected_effect: LifecycleEffect,
}

/// One universe epoch. `expected` is the set that must be acknowledged and
/// snapshot-anchored at that epoch; `required_anchors` is the same set spelled
/// out separately so manifests can pin both hashes. `retired` is the set no
/// longer a coverage obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniverseEpoch {
    pub index: u32,
    pub label: &'static str,
    pub expected: BTreeSet<String>,
    pub required_anchors: BTreeSet<String>,
    pub retired: BTreeSet<String>,
    pub added: BTreeSet<String>,
}

/// A fully resolved, deterministic scenario schedule.
#[derive(Debug, Clone)]
pub struct LifecycleSchedule {
    pub scenario: LifecycleScenario,
    pub seed: u64,
    pub base_ts_ms: i64,
    pub events: Vec<LifecycleEvent>,
    pub epochs: Vec<UniverseEpoch>,
    pub added_ticker: Option<String>,
    pub retired_ticker: Option<String>,
}

impl LifecycleSchedule {
    /// Number of events the collector must apply (`core.lifecycle_events`).
    #[must_use]
    pub fn expected_applied(&self) -> u64 {
        self.events
            .iter()
            .filter(|event| event.expected_effect == LifecycleEffect::Applied)
            .count() as u64
    }

    /// Number of events classified as cross-replica duplicates.
    #[must_use]
    pub fn expected_duplicates(&self) -> u64 {
        self.events
            .iter()
            .filter(|event| event.expected_effect == LifecycleEffect::Duplicate)
            .count() as u64
    }

    /// Number of events rejected by the monotonic high-water mark.
    #[must_use]
    pub fn expected_stale(&self) -> u64 {
        self.events
            .iter()
            .filter(|event| event.expected_effect == LifecycleEffect::Stale)
            .count() as u64
    }

    /// REST record for the lifecycle-added market, served by the mock catalog
    /// when the schedule's first `CatalogEffect::Add` event is published.
    #[must_use]
    pub fn added_record(&self) -> Option<Value> {
        self.added_ticker.as_ref().map(|ticker| {
            json!({
                "ticker": ticker,
                "event_ticker": format!("STRESS-LIFE-EV-{:04x}", self.seed),
                "title": format!("Lifecycle-added market {ticker}"),
                "status": "active",
                "close_time": "2027-01-01T00:00:00Z",
                "price_ranges": [{"start": "0", "end": "1", "step": "0.01"}],
                "price_level_structure": "linear_cent",
            })
        })
    }

    /// Final epoch's expected set.
    #[must_use]
    pub fn final_expected(&self) -> &BTreeSet<String> {
        &self
            .epochs
            .last()
            .expect("a schedule always has an initial epoch")
            .expected
    }

    /// Build a schedule from a seed, the initial eligible universe and the
    /// number of deltas the cell will publish. No wall-clock input is used.
    #[must_use]
    pub fn build(
        scenario: LifecycleScenario,
        seed: u64,
        eligible: &[String],
        delta_events: usize,
    ) -> Self {
        let base_ts_ms = 1_700_000_000_000 + (seed % 1_000_000) as i64;
        let added_ticker = (scenario != LifecycleScenario::None).then(|| {
            format!(
                "STRESS-LIFE-{seed:04x}-{:06}",
                eligible.len() + LIFECYCLE_SCHEDULE_VERSION as usize
            )
        });
        let retired_ticker = eligible.first().cloned();
        let initial: BTreeSet<String> = eligible.iter().cloned().collect();
        let mut schedule = Self {
            scenario,
            seed,
            base_ts_ms,
            events: Vec::new(),
            epochs: vec![UniverseEpoch {
                index: 0,
                label: "initial",
                expected: initial.clone(),
                required_anchors: initial,
                retired: BTreeSet::new(),
                added: BTreeSet::new(),
            }],
            added_ticker: added_ticker.clone(),
            retired_ticker: retired_ticker.clone(),
        };
        if scenario == LifecycleScenario::None {
            return schedule;
        }

        let ticker = |reference: &MarketRef| -> String {
            match reference {
                MarketRef::Eligible(index) => eligible
                    .get(*index)
                    .cloned()
                    .unwrap_or_else(|| eligible.first().cloned().unwrap_or_default()),
                MarketRef::Added => added_ticker.clone().unwrap_or_default(),
                MarketRef::Retired => retired_ticker.clone().unwrap_or_default(),
            }
        };
        let created_extra = |ticker: &str| -> Value {
            json!({
                "event_type": "created",
                "close_ts": 1_798_521_600i64,
                "additional_metadata": {"title": format!("Lifecycle-added market {ticker}")},
                "price_ranges": [{"start": "0", "end": "1", "step": "0.01"}],
                "price_level_structure": "linear_cent",
            })
        };
        // `publish_lifecycle_at` fills `market_ticker` from the resolved target,
        // so extras only carry the payload-specific fields.
        let title_extra = |title: &str| -> Value {
            json!({
                "event_type": "metadata_updated",
                "additional_metadata": {"title": title},
            })
        };

        let push = |event_type: &'static str,
                    reference: MarketRef,
                    ts_offset: i64,
                    after_deltas: usize,
                    extra: Value,
                    catalog_effect: CatalogEffect,
                    expected_effect: LifecycleEffect,
                    schedule: &mut Self| {
            let step = schedule.events.len() as u32;
            let epoch = schedule.epochs.last().map_or(0, |epoch| epoch.index);
            let event_ticker = ticker(&reference);
            schedule.events.push(LifecycleEvent {
                step,
                epoch,
                after_deltas,
                event_type,
                ticker: event_ticker.clone(),
                exchange_ts_ms: base_ts_ms + ts_offset,
                extra,
                catalog_effect,
                expected_effect,
            });
            if catalog_effect != CatalogEffect::None {
                let mut next = schedule.epochs.last().expect("initial epoch").clone();
                next.index = step + 1;
                if catalog_effect == CatalogEffect::Add {
                    next.expected.insert(event_ticker.clone());
                    next.required_anchors.insert(event_ticker.clone());
                    next.retired.remove(&event_ticker);
                    next.added.insert(event_ticker);
                    next.label = "added";
                } else {
                    next.expected.remove(&event_ticker);
                    next.required_anchors.remove(&event_ticker);
                    next.retired.insert(event_ticker.clone());
                    next.added.remove(&event_ticker);
                    next.label = "retired";
                }
                schedule.epochs.push(next);
            }
        };

        let added = MarketRef::Added;
        let first = MarketRef::Eligible(0);
        match scenario {
            LifecycleScenario::None => {}
            LifecycleScenario::AddActivate => {
                push(
                    "created",
                    added,
                    1_000,
                    0,
                    created_extra(&ticker(&added)),
                    CatalogEffect::Add,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
                push(
                    "activated",
                    first,
                    2_000,
                    0,
                    json!({"event_type": "activated"}),
                    CatalogEffect::None,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
            }
            LifecycleScenario::Subscribe | LifecycleScenario::SnapshotAnchor => {
                push(
                    "created",
                    added,
                    1_000,
                    0,
                    created_extra(&ticker(&added)),
                    CatalogEffect::Add,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
            }
            LifecycleScenario::MetadataChange => {
                push(
                    "metadata_updated",
                    first,
                    1_000,
                    0,
                    title_extra("Lifecycle metadata title"),
                    CatalogEffect::None,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
            }
            LifecycleScenario::RetireSettle => {
                push(
                    "settled",
                    MarketRef::Retired,
                    1_000,
                    usize::MAX,
                    json!({"event_type": "settled"}),
                    CatalogEffect::Retire,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
            }
            LifecycleScenario::Duplicate => {
                let extra = title_extra("Duplicate payload");
                for step in 0..2 {
                    push(
                        "metadata_updated",
                        first,
                        1_000,
                        0,
                        extra.clone(),
                        CatalogEffect::None,
                        if step == 0 {
                            LifecycleEffect::Applied
                        } else {
                            LifecycleEffect::Duplicate
                        },
                        &mut schedule,
                    );
                }
            }
            LifecycleScenario::OutOfOrder => {
                push(
                    "metadata_updated",
                    first,
                    2_000,
                    0,
                    title_extra("Newer metadata title"),
                    CatalogEffect::None,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
                push(
                    "metadata_updated",
                    first,
                    -2_000,
                    0,
                    title_extra("Older metadata title"),
                    CatalogEffect::None,
                    LifecycleEffect::Stale,
                    &mut schedule,
                );
            }
            LifecycleScenario::ConflictingReplica => {
                push(
                    "metadata_updated",
                    first,
                    1_000,
                    0,
                    title_extra("First conflicting payload"),
                    CatalogEffect::None,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
                push(
                    "metadata_updated",
                    first,
                    1_000,
                    0,
                    title_extra("Second conflicting payload"),
                    CatalogEffect::None,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
            }
            LifecycleScenario::Reappearance => {
                push(
                    "settled",
                    MarketRef::Retired,
                    1_000,
                    0,
                    json!({"event_type": "settled"}),
                    CatalogEffect::Retire,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
                push(
                    "created",
                    MarketRef::Retired,
                    3_000,
                    delta_events,
                    created_extra(&ticker(&MarketRef::Retired)),
                    CatalogEffect::Add,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
            }
            LifecycleScenario::ChurnUnderLoad => {
                push(
                    "created",
                    added,
                    1_000,
                    0,
                    created_extra(&ticker(&added)),
                    CatalogEffect::Add,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
                push(
                    "metadata_updated",
                    first,
                    2_000,
                    delta_events / 4,
                    title_extra("Churn metadata title"),
                    CatalogEffect::None,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
                let duplicate = title_extra("Churn duplicate payload");
                push(
                    "metadata_updated",
                    first,
                    3_000,
                    delta_events / 2,
                    duplicate.clone(),
                    CatalogEffect::None,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
                push(
                    "metadata_updated",
                    first,
                    3_000,
                    delta_events / 2,
                    duplicate,
                    CatalogEffect::None,
                    LifecycleEffect::Duplicate,
                    &mut schedule,
                );
                push(
                    "metadata_updated",
                    first,
                    -4_000,
                    delta_events * 3 / 4,
                    title_extra("Churn stale payload"),
                    CatalogEffect::None,
                    LifecycleEffect::Stale,
                    &mut schedule,
                );
                // Retirement is last so the add-triggered discovery round has
                // completed: the current state machine has no cross-round veto
                // when a venue timestamp predates the round start.
                push(
                    "settled",
                    MarketRef::Retired,
                    4_000,
                    usize::MAX,
                    json!({"event_type": "settled"}),
                    CatalogEffect::Retire,
                    LifecycleEffect::Applied,
                    &mut schedule,
                );
            }
        }
        schedule
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_is_reproducible_under_a_fixed_seed() {
        let eligible: Vec<String> = (0..32).map(|index| format!("STRESS-{index:06}")).collect();
        let a = LifecycleSchedule::build(LifecycleScenario::ChurnUnderLoad, 7, &eligible, 400);
        let b = LifecycleSchedule::build(LifecycleScenario::ChurnUnderLoad, 7, &eligible, 400);
        assert_eq!(a.events, b.events);
        assert_eq!(a.epochs, b.epochs);
    }

    #[test]
    fn epochs_track_retirement_and_readdition() {
        let eligible: Vec<String> = (0..8).map(|index| format!("S-{index}")).collect();
        let schedule = LifecycleSchedule::build(LifecycleScenario::Reappearance, 3, &eligible, 10);
        assert_eq!(schedule.epochs.len(), 3);
        assert_eq!(schedule.epochs[0].expected.len(), 8);
        assert!(schedule.epochs[1].retired.contains(&eligible[0]));
        assert_eq!(schedule.epochs[2].expected, schedule.epochs[0].expected);
    }

    #[test]
    fn expected_counters_distinguish_duplicate_and_stale_events() {
        let eligible: Vec<String> = (0..8).map(|index| format!("S-{index}")).collect();
        let schedule =
            LifecycleSchedule::build(LifecycleScenario::ChurnUnderLoad, 3, &eligible, 400);
        assert_eq!(schedule.expected_applied(), 4);
        assert_eq!(schedule.expected_duplicates(), 1);
        assert_eq!(schedule.expected_stale(), 1);
        assert_eq!(schedule.events.len(), 6);
    }

    #[test]
    fn duplicate_events_share_a_venue_timestamp_and_out_of_order_is_older() {
        let eligible: Vec<String> = (0..8).map(|index| format!("S-{index}")).collect();
        let duplicate = LifecycleSchedule::build(LifecycleScenario::Duplicate, 5, &eligible, 20);
        assert_eq!(
            duplicate.events[0].exchange_ts_ms,
            duplicate.events[1].exchange_ts_ms
        );
        let out_of_order =
            LifecycleSchedule::build(LifecycleScenario::OutOfOrder, 5, &eligible, 20);
        assert!(
            out_of_order.events[1].exchange_ts_ms < out_of_order.events[0].exchange_ts_ms,
            "the rejected event must carry a strictly older venue timestamp"
        );
    }
}
