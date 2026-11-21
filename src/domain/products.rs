//! Typed data-product and source-capability contracts (Platform M3, Phase 1).
//!
//! These contracts coexist with `MarketDataSession::apply_subscriptions`
//! still takes `&BTreeSet<String>`, and `ExchangeCapabilities::new(bool)` still
//! reports the legacy sequence-clock flag. Nothing here changes the SBE,
//! journal, replay or catalog formats, and no adapter behavior is altered.
//!
//! A [`DataProduct`] names *what* a source can deliver. A [`ProductKind`] is
//! its stable discriminant and supplies deterministic subscription keys so a
//! legacy native-symbol set can be converted to a [`SubscriptionSet`] without
//! migrating `MarketInfo.symbol` or `native_symbol`. A [`SourceCapabilities`]
//! declaration describes *how* a source delivers those products. A
//! [`SubscriptionLease`] tracks the local lifecycle of one subscription and
//! refuses to treat a socket write or a stale connection epoch as a venue
//! acknowledgement.

use std::{
    collections::BTreeSet,
    fmt::{self, Display, Formatter},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Scope token for products that are venue- or feed-global rather than bound
/// to a single instrument. It is part of the stable key vocabulary.
pub const GLOBAL_SCOPE: &str = "global";

/// Stable discriminant of a [`DataProduct`].
///
/// The `as_str` values are the serialized serde names and the key prefixes.
/// They are part of the contract: changing one changes every subscription key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ProductKind {
    #[serde(rename = "orderbook_l2")]
    OrderBookL2,
    #[serde(rename = "trades")]
    Trades,
    #[serde(rename = "best_bid_offer")]
    BestBidOffer,
    #[serde(rename = "asset_context")]
    AssetContext,
    #[serde(rename = "venue_lifecycle")]
    VenueLifecycle,
    #[serde(rename = "global_statistics")]
    GlobalStatistics,
}

impl ProductKind {
    /// Stable serde and key prefix for this kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderBookL2 => "orderbook_l2",
            Self::Trades => "trades",
            Self::BestBidOffer => "best_bid_offer",
            Self::AssetContext => "asset_context",
            Self::VenueLifecycle => "venue_lifecycle",
            Self::GlobalStatistics => "global_statistics",
        }
    }

    /// Every declared kind, in a fixed order for deterministic iteration.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::OrderBookL2,
            Self::Trades,
            Self::BestBidOffer,
            Self::AssetContext,
            Self::VenueLifecycle,
            Self::GlobalStatistics,
        ]
    }

    /// Deterministic subscription key for one scope. `scope` may itself
    /// contain separators (native symbols are namespaced); the fixed prefix
    /// keeps different kinds distinct.
    #[must_use]
    pub fn subscription_key(self, scope: &str) -> String {
        format!("{}:{}", self.as_str(), scope)
    }
}

impl Display for ProductKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One typed product a source can deliver.
///
/// Instrument fields are plain `String`s so legacy native symbols remain the
/// only migration seam; a future `InstrumentId` adapter can feed the same
/// strings without changing any key. `Display` equals [`DataProduct::key`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataProduct {
    #[serde(rename = "orderbook_l2")]
    OrderBookL2 { instrument: String },
    #[serde(rename = "trades")]
    Trades { instrument: String },
    #[serde(rename = "best_bid_offer")]
    BestBidOffer { instrument: String },
    #[serde(rename = "asset_context")]
    AssetContext { instrument: Option<String> },
    #[serde(rename = "venue_lifecycle")]
    VenueLifecycle { scope: String },
    #[serde(rename = "global_statistics")]
    GlobalStatistics { scope: String },
}

impl DataProduct {
    /// Build the variant for `kind` with a single scope/instrument value. This
    /// is the bridge used when converting legacy native symbols.
    #[must_use]
    pub fn for_scope(kind: ProductKind, scope: impl Into<String>) -> Self {
        let scope = scope.into();
        match kind {
            ProductKind::OrderBookL2 => Self::OrderBookL2 { instrument: scope },
            ProductKind::Trades => Self::Trades { instrument: scope },
            ProductKind::BestBidOffer => Self::BestBidOffer { instrument: scope },
            ProductKind::AssetContext => Self::AssetContext {
                instrument: Some(scope),
            },
            ProductKind::VenueLifecycle => Self::VenueLifecycle { scope },
            ProductKind::GlobalStatistics => Self::GlobalStatistics { scope },
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ProductKind {
        match self {
            Self::OrderBookL2 { .. } => ProductKind::OrderBookL2,
            Self::Trades { .. } => ProductKind::Trades,
            Self::BestBidOffer { .. } => ProductKind::BestBidOffer,
            Self::AssetContext { .. } => ProductKind::AssetContext,
            Self::VenueLifecycle { .. } => ProductKind::VenueLifecycle,
            Self::GlobalStatistics { .. } => ProductKind::GlobalStatistics,
        }
    }

    /// Instrument for per-instrument products, `global` for a product without
    /// an instrument, and the explicit scope for venue/feed-scoped products.
    #[must_use]
    pub fn scope(&self) -> &str {
        match self {
            Self::OrderBookL2 { instrument }
            | Self::Trades { instrument }
            | Self::BestBidOffer { instrument } => instrument.as_str(),
            Self::AssetContext { instrument } => instrument.as_deref().unwrap_or(GLOBAL_SCOPE),
            Self::VenueLifecycle { scope } | Self::GlobalStatistics { scope } => scope.as_str(),
        }
    }

    /// Stable subscription key: `<kind>:<scope>`.
    #[must_use]
    pub fn key(&self) -> String {
        self.kind().subscription_key(self.scope())
    }
}

impl Display for DataProduct {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.kind(), self.scope())
    }
}

/// Deterministic set of subscription keys ready for the existing
/// `apply_subscriptions(&BTreeSet<String>)` seam.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubscriptionSet {
    keys: BTreeSet<String>,
}

impl SubscriptionSet {
    /// Convert a legacy native-symbol set into one product kind's keys.
    /// Insertion order cannot affect the result, and the keys are directly
    /// usable as the `BTreeSet<String>` the session trait already accepts.
    #[must_use]
    pub fn from_native_symbols(symbols: &BTreeSet<String>, kind: ProductKind) -> Self {
        Self {
            keys: symbols
                .iter()
                .map(|symbol| kind.subscription_key(symbol))
                .collect(),
        }
    }

    #[must_use]
    pub fn from_products(products: impl IntoIterator<Item = DataProduct>) -> Self {
        Self {
            keys: products.into_iter().map(|product| product.key()).collect(),
        }
    }

    #[must_use]
    pub fn keys(&self) -> &BTreeSet<String> {
        &self.keys
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn insert(&mut self, product: DataProduct) {
        self.keys.insert(product.key());
    }
}

impl FromIterator<DataProduct> for SubscriptionSet {
    fn from_iter<T: IntoIterator<Item = DataProduct>>(iter: T) -> Self {
        Self::from_products(iter)
    }
}

/// How a source publishes updates for a product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateModel {
    #[serde(rename = "snapshot_and_delta")]
    SnapshotAndDelta,
    #[serde(rename = "snapshot_only")]
    SnapshotOnly,
    #[serde(rename = "telemetry_only")]
    TelemetryOnly,
    #[serde(rename = "reference_data")]
    ReferenceData,
}

/// Which authority orders frames for this source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderingScope {
    #[serde(rename = "receive_fence")]
    ReceiveFence,
    #[serde(rename = "per_subscription")]
    PerSubscription,
    #[serde(rename = "global_sequence")]
    GlobalSequence,
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

/// Declared recovery strategy after a gap or transport loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryMode {
    #[serde(rename = "resubscribe_and_reanchor")]
    ResubscribeAndReanchor,
    #[serde(rename = "sequence_gap_resubscribe")]
    SequenceGapResubscribe,
    #[serde(rename = "full_reseed")]
    FullReseed,
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

/// Venue heartbeat mechanism the session supervisor must execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeartbeatMechanism {
    #[serde(rename = "native_ping_pong")]
    NativePingPong,
    #[serde(rename = "application_text")]
    ApplicationText,
    #[serde(rename = "disabled")]
    Disabled,
}

/// Whether a subscription is addressed per instrument or to one global channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionTopology {
    #[serde(rename = "per_instrument")]
    PerInstrument,
    #[serde(rename = "global")]
    Global,
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

/// Numeric contract a source's frames obey before normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrecisionModel {
    #[serde(rename = "exact_decimal")]
    ExactDecimal,
    #[serde(rename = "fixed_point")]
    FixedPoint,
    #[serde(rename = "integer_ticks")]
    IntegerTicks,
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

/// Declared behavior when a bounded queue or sink is saturated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackpressureBehavior {
    #[serde(rename = "bounded_fail_closed")]
    BoundedFailClosed,
    #[serde(rename = "bounded_blocking")]
    BoundedBlocking,
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

/// Typed declaration of what and how a source delivers.
///
/// These are declarations consumed by callers, not instructions that change
/// any adapter's runtime behavior. `has_sequence_clock` mirrors the legacy
/// [`crate::exchange::ExchangeCapabilities`] flag; the core keeps reading that
/// flag unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCapabilities {
    /// Live products this source can deliver. Empty means no live feed.
    pub products: BTreeSet<ProductKind>,
    /// The source supplies reference data (discovery-time records).
    pub provides_reference_data: bool,
    pub update_model: UpdateModel,
    pub ordering_scope: OrderingScope,
    pub recovery_mode: RecoveryMode,
    pub heartbeat: HeartbeatMechanism,
    pub subscription_topology: SubscriptionTopology,
    /// Topology of telemetry channels such as venue-wide statistics.
    pub telemetry_topology: SubscriptionTopology,
    pub precision: PrecisionModel,
    pub backpressure: BackpressureBehavior,
    pub has_sequence_clock: bool,
}

impl SourceCapabilities {
    /// Conservative projection of the legacy boolean capability flag. It
    /// declares no typed products: a legacy adapter made no product claim.
    #[must_use]
    pub fn from_legacy(has_sequence_clock: bool) -> Self {
        Self {
            products: BTreeSet::new(),
            provides_reference_data: false,
            update_model: UpdateModel::SnapshotAndDelta,
            ordering_scope: if has_sequence_clock {
                OrderingScope::GlobalSequence
            } else {
                OrderingScope::ReceiveFence
            },
            recovery_mode: if has_sequence_clock {
                RecoveryMode::SequenceGapResubscribe
            } else {
                RecoveryMode::ResubscribeAndReanchor
            },
            heartbeat: HeartbeatMechanism::Disabled,
            subscription_topology: SubscriptionTopology::PerInstrument,
            telemetry_topology: SubscriptionTopology::NotApplicable,
            precision: PrecisionModel::FixedPoint,
            backpressure: BackpressureBehavior::BoundedFailClosed,
            has_sequence_clock,
        }
    }

    #[must_use]
    pub fn supports(&self, kind: ProductKind) -> bool {
        self.products.contains(&kind)
    }
}

/// Local lifecycle of one declared subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SubscriptionState {
    #[serde(rename = "desired")]
    Desired,
    #[serde(rename = "sent")]
    Sent,
    #[serde(rename = "acknowledged")]
    Acknowledged,
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "removing")]
    Removing,
    #[serde(rename = "removed")]
    Removed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "uncertain")]
    Uncertain,
}

impl SubscriptionState {
    /// Explicit transition table. A socket write (`Sent`) is not
    /// an acknowledgement, and only a venue acknowledgement can produce
    /// `Acknowledged`.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Desired, Self::Sent | Self::Removed)
                | (
                    Self::Sent,
                    Self::Acknowledged | Self::Failed | Self::Uncertain | Self::Removed
                )
                | (
                    Self::Acknowledged,
                    Self::Active | Self::Removing | Self::Uncertain
                )
                | (Self::Active, Self::Removing | Self::Uncertain)
                | (
                    Self::Removing,
                    Self::Removed | Self::Active | Self::Uncertain
                )
                | (Self::Removed, Self::Desired)
                | (Self::Failed, Self::Desired | Self::Removed)
                | (
                    Self::Uncertain,
                    Self::Desired | Self::Acknowledged | Self::Removed
                )
        )
    }

    pub fn transition(self, next: Self) -> Result<Self, InvalidSubscriptionTransition> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(InvalidSubscriptionTransition {
                from: self,
                to: next,
            })
        }
    }
}

impl Display for SubscriptionState {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Desired => "desired",
            Self::Sent => "sent",
            Self::Acknowledged => "acknowledged",
            Self::Active => "active",
            Self::Removing => "removing",
            Self::Removed => "removed",
            Self::Failed => "failed",
            Self::Uncertain => "uncertain",
        };
        formatter.write_str(text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid subscription transition: {from} -> {to}")]
pub struct InvalidSubscriptionTransition {
    pub from: SubscriptionState,
    pub to: SubscriptionState,
}

/// Connection generation a subscription acknowledgement belongs to. It
/// advances on every disconnect; an ack carrying an older epoch is stale.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SubscriptionEpoch(pub u64);

impl SubscriptionEpoch {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl Display for SubscriptionEpoch {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SubscriptionError {
    #[error(transparent)]
    Transition(#[from] InvalidSubscriptionTransition),
    #[error("stale subscription epoch: expected {expected}, observed {observed}")]
    StaleEpoch {
        expected: SubscriptionEpoch,
        observed: SubscriptionEpoch,
    },
}

/// One subscription bound to the connection epoch in which it was declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionLease {
    pub state: SubscriptionState,
    pub epoch: SubscriptionEpoch,
}

impl SubscriptionLease {
    #[must_use]
    pub const fn desired(epoch: SubscriptionEpoch) -> Self {
        Self {
            state: SubscriptionState::Desired,
            epoch,
        }
    }

    /// Record a successful local socket write. This is transport progress
    /// only: it never sets `Acknowledged` and never consults a stale epoch.
    pub fn socket_write(&mut self) -> Result<(), SubscriptionError> {
        self.state = self.state.transition(SubscriptionState::Sent)?;
        Ok(())
    }

    /// Accept a venue acknowledgement for the current epoch only.
    pub fn acknowledge(&mut self, epoch: SubscriptionEpoch) -> Result<(), SubscriptionError> {
        self.require_epoch(epoch)?;
        self.state = self.state.transition(SubscriptionState::Acknowledged)?;
        Ok(())
    }

    /// Accept observed activity for the current epoch only.
    pub fn activate(&mut self, epoch: SubscriptionEpoch) -> Result<(), SubscriptionError> {
        self.require_epoch(epoch)?;
        self.state = self.state.transition(SubscriptionState::Active)?;
        Ok(())
    }

    fn require_epoch(&self, observed: SubscriptionEpoch) -> Result<(), SubscriptionError> {
        if observed == self.epoch {
            Ok(())
        } else {
            Err(SubscriptionError::StaleEpoch {
                expected: self.epoch,
                observed,
            })
        }
    }

    /// A transport disconnect invalidates every assumption tied to the old
    /// epoch, regardless of the prior state: the venue-side subscription is no
    /// longer known and must be re-declared in the returned new epoch.
    pub fn disconnect(&mut self) -> SubscriptionEpoch {
        self.state = SubscriptionState::Uncertain;
        self.epoch = self.epoch.next();
        self.epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_keys_are_stable_and_scope_separated() {
        let book = DataProduct::OrderBookL2 {
            instrument: "BTC".to_owned(),
        };
        let trades = DataProduct::Trades {
            instrument: "BTC".to_owned(),
        };
        let bbo = DataProduct::BestBidOffer {
            instrument: "BTC".to_owned(),
        };
        let asset = DataProduct::AssetContext {
            instrument: Some("BTC".to_owned()),
        };
        let global_asset = DataProduct::AssetContext { instrument: None };
        let lifecycle = DataProduct::VenueLifecycle {
            scope: "kalshi".to_owned(),
        };
        let stats = DataProduct::GlobalStatistics {
            scope: "hyperliquid".to_owned(),
        };

        assert_eq!(book.key(), "orderbook_l2:BTC");
        assert_eq!(trades.key(), "trades:BTC");
        assert_eq!(bbo.key(), "best_bid_offer:BTC");
        assert_eq!(asset.key(), "asset_context:BTC");
        assert_eq!(global_asset.key(), "asset_context:global");
        assert_eq!(lifecycle.key(), "venue_lifecycle:kalshi");
        assert_eq!(stats.key(), "global_statistics:hyperliquid");

        assert_eq!(book.to_string(), book.key());
        assert_eq!(global_asset.scope(), GLOBAL_SCOPE);
        assert_eq!(lifecycle.scope(), "kalshi");
        assert_eq!(stats.kind(), ProductKind::GlobalStatistics);

        let encoded = serde_json::to_string(&book).expect("serializable product");
        assert_eq!(encoded, r#"{"orderbook_l2":{"instrument":"BTC"}}"#);
        let decoded: DataProduct = serde_json::from_str(&encoded).expect("deserializable product");
        assert_eq!(decoded, book);
    }

    #[test]
    fn product_kind_keys_match_variant_keys() {
        let scope = "coinbase:spot:BTC/USD".to_owned();
        for kind in ProductKind::all() {
            let product = DataProduct::for_scope(*kind, scope.clone());
            assert_eq!(product.kind(), *kind);
            assert_eq!(product.scope(), scope);
            assert_eq!(product.key(), kind.subscription_key(&scope));
            assert_eq!(
                serde_json::to_string(kind).expect("serializable kind"),
                format!("\"{}\"", kind.as_str())
            );
        }
    }

    #[test]
    fn legacy_native_symbols_convert_to_deterministic_subscription_keys() {
        let first: BTreeSet<String> = ["kalshi:KX-B", "kalshi:KX-A"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let second: BTreeSet<String> = ["kalshi:KX-A", "kalshi:KX-B"]
            .into_iter()
            .map(str::to_owned)
            .collect();

        let books = SubscriptionSet::from_native_symbols(&first, ProductKind::OrderBookL2);
        let reordered = SubscriptionSet::from_native_symbols(&second, ProductKind::OrderBookL2);
        assert_eq!(books, reordered);
        assert_eq!(books.len(), 2);
        assert_eq!(
            books.keys(),
            &BTreeSet::from([
                "orderbook_l2:kalshi:KX-A".to_owned(),
                "orderbook_l2:kalshi:KX-B".to_owned(),
            ])
        );

        let trades = SubscriptionSet::from_native_symbols(&first, ProductKind::Trades);
        assert_ne!(books.keys(), trades.keys());
        assert!(trades.keys().iter().all(|key| key.starts_with("trades:")));

        // The result is the exact type the unchanged session API accepts.
        let legacy_api_set: BTreeSet<String> = books.keys().clone();
        assert_eq!(legacy_api_set.len(), 2);

        let empty: BTreeSet<String> = BTreeSet::new();
        assert!(SubscriptionSet::from_native_symbols(&empty, ProductKind::OrderBookL2).is_empty());

        let mut collected: SubscriptionSet = ProductKind::all()
            .iter()
            .map(|kind| DataProduct::for_scope(*kind, "X"))
            .collect();
        collected.insert(DataProduct::AssetContext { instrument: None });
        assert_eq!(collected.len(), ProductKind::all().len() + 1);
    }

    #[test]
    fn subscription_transition_table_is_explicit() {
        use SubscriptionState as S;
        assert!(S::Desired.can_transition_to(S::Sent));
        assert!(!S::Sent.can_transition_to(S::Active));
        assert!(!S::Desired.can_transition_to(S::Acknowledged));
        assert!(!S::Removed.can_transition_to(S::Active));
        assert!(!S::Desired.can_transition_to(S::Desired));

        let error = S::Sent
            .transition(S::Active)
            .expect_err("invalid transition");
        assert_eq!(
            error,
            InvalidSubscriptionTransition {
                from: S::Sent,
                to: S::Active
            }
        );
        assert_eq!(
            error.to_string(),
            "invalid subscription transition: sent -> active"
        );

        let all = [
            S::Desired,
            S::Sent,
            S::Acknowledged,
            S::Active,
            S::Removing,
            S::Removed,
            S::Failed,
            S::Uncertain,
        ];
        let allowed: BTreeSet<(S, S)> = all
            .iter()
            .flat_map(|from| {
                all.iter()
                    .filter(move |to| from.can_transition_to(**to))
                    .map(move |to| (*from, *to))
            })
            .collect();
        assert_eq!(allowed.len(), 20);
        assert!(allowed.contains(&(S::Uncertain, S::Acknowledged)));
        assert!(!allowed.contains(&(S::Removed, S::Sent)));
    }

    #[test]
    fn socket_write_does_not_imply_acknowledgement() {
        let epoch = SubscriptionEpoch::new(7);
        let mut lease = SubscriptionLease::desired(epoch);
        assert_eq!(lease.state, SubscriptionState::Desired);

        lease.socket_write().expect("socket write recorded");
        assert_eq!(lease.state, SubscriptionState::Sent);
        assert_ne!(lease.state, SubscriptionState::Acknowledged);
        assert!(!SubscriptionState::Sent.can_transition_to(SubscriptionState::Active));

        // An ack from another epoch cannot turn the write into a venue ack.
        assert_eq!(
            lease.acknowledge(epoch.next()),
            Err(SubscriptionError::StaleEpoch {
                expected: epoch,
                observed: epoch.next(),
            })
        );
        assert_eq!(lease.state, SubscriptionState::Sent);

        lease.acknowledge(epoch).expect("current-epoch ack");
        assert_eq!(lease.state, SubscriptionState::Acknowledged);
    }

    #[test]
    fn disconnect_invalidates_epoch_assumptions() {
        let epoch = SubscriptionEpoch::new(4);
        let mut lease = SubscriptionLease::desired(epoch);
        lease.socket_write().expect("write");
        lease.acknowledge(epoch).expect("ack");
        lease.activate(epoch).expect("activity");
        assert_eq!(lease.state, SubscriptionState::Active);

        let new_epoch = lease.disconnect();
        assert_eq!(new_epoch, epoch.next());
        assert_eq!(lease.state, SubscriptionState::Uncertain);
        assert_eq!(lease.epoch, new_epoch);

        // Every old-epoch assumption is now rejected, including activity that
        // may have been in flight when the socket was lost.
        assert!(matches!(
            lease.acknowledge(epoch),
            Err(SubscriptionError::StaleEpoch { .. })
        ));
        assert!(matches!(
            lease.activate(epoch),
            Err(SubscriptionError::StaleEpoch { .. })
        ));
        assert_eq!(lease.state, SubscriptionState::Uncertain);

        // Re-declaration belongs to the new epoch; an old-epoch ack still fails.
        lease.state = lease
            .state
            .transition(SubscriptionState::Desired)
            .expect("re-declare");
        lease.socket_write().expect("new-epoch write");
        assert!(lease.acknowledge(epoch).is_err());
        lease.acknowledge(new_epoch).expect("new-epoch ack");
        assert_eq!(lease.state, SubscriptionState::Acknowledged);
    }

    #[test]
    fn source_capabilities_from_legacy_preserves_clock_flag() {
        let clocked = SourceCapabilities::from_legacy(true);
        assert!(clocked.has_sequence_clock);
        assert_eq!(clocked.ordering_scope, OrderingScope::GlobalSequence);
        assert_eq!(clocked.recovery_mode, RecoveryMode::SequenceGapResubscribe);
        assert!(clocked.products.is_empty());
        assert!(!clocked.provides_reference_data);
        assert!(!clocked.supports(ProductKind::OrderBookL2));

        let clockless = SourceCapabilities::from_legacy(false);
        assert!(!clockless.has_sequence_clock);
        assert_eq!(clockless.ordering_scope, OrderingScope::ReceiveFence);
        assert_eq!(
            clockless.recovery_mode,
            RecoveryMode::ResubscribeAndReanchor
        );
    }

    #[test]
    fn source_capabilities_serde_round_trip_is_stable() {
        let mut source = SourceCapabilities::from_legacy(false);
        source.products = BTreeSet::from([ProductKind::OrderBookL2, ProductKind::GlobalStatistics]);
        source.telemetry_topology = SubscriptionTopology::Global;
        let encoded = serde_json::to_string(&source).expect("serializable capabilities");
        let decoded: SourceCapabilities =
            serde_json::from_str(&encoded).expect("deserializable capabilities");
        assert_eq!(decoded, source);
    }
}
