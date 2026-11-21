//! Universal entity, venue, listing and source reference graph.
//!
//! This module declares the identity layer required for instruments beyond one
//! venue or asset class. It reuses the opaque identifiers and resolution model
//! from [`crate::identity`] and keeps every mapping effective-dated.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use crate::{
    identity::{InstrumentId, OpaqueId, VenueId, VenueInstrumentMapping},
    taxonomy::{CountryCode, CurrencyCode},
};

/// Permanent legal or natural entity identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(OpaqueId);

impl EntityId {
    /// Derive an entity identity from stable namespace inputs.
    pub fn derive(namespace: &str, authority: &str, stable_key: &str) -> Self {
        Self(OpaqueId::derive("ent", namespace, authority, stable_key))
    }
}

/// Market or data product operated by a venue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MarketId(OpaqueId);

impl MarketId {
    /// Derive a market identity from stable namespace inputs.
    pub fn derive(namespace: &str, authority: &str, stable_key: &str) -> Self {
        Self(OpaqueId::derive("mkt", namespace, authority, stable_key))
    }
}

/// Outcome of a reference-data lookup with no silent coercion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReferenceResolution<T> {
    /// Exactly one record matched.
    Resolved(T),
    /// More than one candidate matched.
    Ambiguous,
    /// No record matched.
    Unresolved,
    /// Conflicting records claimed the same identity.
    Conflict,
}

/// Regulatory or operational status of a trading venue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueCategory {
    Exchange,
    MultilateralTradingFacility,
    AlternativeTradingSystem,
    DarkPool,
    BrokerInternalizer,
    DealerQuotationSystem,
    TradeReportingFacility,
    ContributedQuotationSystem,
}

impl VenueCategory {
    /// Return the stable contract string for this venue category.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exchange => "exchange",
            Self::MultilateralTradingFacility => "multilateral_trading_facility",
            Self::AlternativeTradingSystem => "alternative_trading_system",
            Self::DarkPool => "dark_pool",
            Self::BrokerInternalizer => "broker_internalizer",
            Self::DealerQuotationSystem => "dealer_quotation_system",
            Self::TradeReportingFacility => "trade_reporting_facility",
            Self::ContributedQuotationSystem => "contributed_quotation_system",
        }
    }
}

/// External identifier standard used in the reference graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentifierStandard {
    Isin,
    Figi,
    CompositeFigi,
    ShareClassFigi,
    Mic,
    Cfi,
    Fisn,
    Lei,
    Upi,
    Uti,
    Dti,
    Iso4217,
    VenueTicker,
}

impl IdentifierStandard {
    /// Return the stable contract string for this identifier standard.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Isin => "isin",
            Self::Figi => "figi",
            Self::CompositeFigi => "composite_figi",
            Self::ShareClassFigi => "share_class_figi",
            Self::Mic => "mic",
            Self::Cfi => "cfi",
            Self::Fisn => "fisn",
            Self::Lei => "lei",
            Self::Upi => "upi",
            Self::Uti => "uti",
            Self::Dti => "dti",
            Self::Iso4217 => "iso_4217",
            Self::VenueTicker => "venue_ticker",
        }
    }
}

/// Role of a Market Identifier Code in a venue record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicRole {
    Listing,
    Trading,
    TradeReporting,
}

impl MicRole {
    /// Return the stable contract string for this MIC role.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Listing => "listing",
            Self::Trading => "trading",
            Self::TradeReporting => "trade_reporting",
        }
    }
}

/// Effective-dated external identifier assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardIdentifier {
    pub standard: IdentifierStandard,
    pub value: String,
    pub valid_from_us: Option<i64>,
    pub valid_to_us: Option<i64>,
    pub source: String,
}

impl StandardIdentifier {
    /// Check whether this identifier applies at an observation time.
    pub fn is_valid_at(&self, observed_at_us: i64) -> bool {
        self.valid_from_us
            .is_none_or(|start| observed_at_us >= start)
            && self.valid_to_us.is_none_or(|end| observed_at_us <= end)
    }
}

/// Venue master record with jurisdiction and classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Venue {
    pub id: VenueId,
    pub name: String,
    pub mic: Option<String>,
    pub category: VenueCategory,
    pub jurisdiction: CountryCode,
    pub settlement_currency: Option<CurrencyCode>,
    pub mic_roles: Vec<MicRole>,
    pub identifiers: Vec<StandardIdentifier>,
}

/// Contract or instrument specification with effective dating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSpecification {
    pub instrument: InstrumentId,
    pub legal_name: String,
    pub issuer: Option<EntityId>,
    pub currency: Option<CurrencyCode>,
    pub multiplier: Option<i64>,
    pub expiry_us: Option<i64>,
    pub strike_mantissa: Option<i64>,
    pub valid_from_us: Option<i64>,
    pub valid_to_us: Option<i64>,
}

impl InstrumentSpecification {
    /// Check whether this specification applies at an observation time.
    pub fn is_valid_at(&self, observed_at_us: i64) -> bool {
        self.valid_from_us
            .is_none_or(|start| observed_at_us >= start)
            && self.valid_to_us.is_none_or(|end| observed_at_us <= end)
    }
}

/// Immutable edge in the entity and instrument graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: OpaqueId,
    pub to: OpaqueId,
    pub relation: EdgeRelation,
    pub valid_from_us: Option<i64>,
    pub valid_to_us: Option<i64>,
}

impl GraphEdge {
    /// Check whether this edge applies at an observation time.
    pub fn is_valid_at(&self, observed_at_us: i64) -> bool {
        self.valid_from_us
            .is_none_or(|start| observed_at_us >= start)
            && self.valid_to_us.is_none_or(|end| observed_at_us <= end)
    }
}

/// Typed relationship in the reference graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeRelation {
    IssuedBy,
    ListedOn,
    UnderlyingOf,
    OptionOf,
    DeliverableOf,
    SuccessorOf,
    ConstituentOf,
    VenueAliasOf,
}

impl EdgeRelation {
    /// Return the stable contract string for this graph relationship.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IssuedBy => "issued_by",
            Self::ListedOn => "listed_on",
            Self::UnderlyingOf => "underlying_of",
            Self::OptionOf => "option_of",
            Self::DeliverableOf => "deliverable_of",
            Self::SuccessorOf => "successor_of",
            Self::ConstituentOf => "constituent_of",
            Self::VenueAliasOf => "venue_alias_of",
        }
    }
}

/// Failure to register or resolve reference data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReferenceError {
    /// Two records claimed the same stable identity with different content.
    #[error("conflicting reference identity: {0}")]
    Conflict(String),
    /// A referenced identity was not registered.
    #[error("unresolved reference identity: {0}")]
    Unresolved(String),
}

/// Source-scoped reference catalog with effective-dated identity mappings.
#[derive(Debug, Default)]
pub struct ReferenceGraph {
    venues: BTreeMap<String, Venue>,
    edges: Vec<GraphEdge>,
    specifications: Vec<InstrumentSpecification>,
}

impl ReferenceGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a venue, rejecting conflicting records for the same identity.
    pub fn insert_venue(&mut self, venue: Venue) -> Result<(), ReferenceError> {
        let key = venue.id.to_string();
        match self.venues.get(&key) {
            Some(existing) if existing != &venue => Err(ReferenceError::Conflict(key)),
            Some(_) => Ok(()),
            None => {
                self.venues.insert(key, venue);
                Ok(())
            }
        }
    }

    /// Append a graph edge.
    pub fn insert_edge(&mut self, edge: GraphEdge) {
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }
    }

    /// Append an instrument specification.
    pub fn insert_specification(
        &mut self,
        specification: InstrumentSpecification,
    ) -> Result<(), ReferenceError> {
        if self.specifications.contains(&specification) {
            return Ok(());
        }
        if self.specifications.iter().any(|existing| {
            existing.instrument == specification.instrument
                && existing.is_valid_at(specification.valid_from_us.unwrap_or(i64::MIN))
        }) {
            return Err(ReferenceError::Conflict(
                specification.instrument.to_string(),
            ));
        }
        self.specifications.push(specification);
        Ok(())
    }

    /// Resolve one venue identifier at an observation time.
    pub fn resolve_venue(&self, id: &VenueId, observed_at_us: i64) -> ReferenceResolution<Venue> {
        let _ = observed_at_us;
        match self.venues.get(&id.to_string()) {
            Some(venue) => ReferenceResolution::Resolved(venue.clone()),
            None => ReferenceResolution::Unresolved,
        }
    }

    /// Resolve the specification active at an observation time.
    pub fn resolve_specification(
        &self,
        instrument: &InstrumentId,
        observed_at_us: i64,
    ) -> ReferenceResolution<InstrumentSpecification> {
        let matches: Vec<InstrumentSpecification> = self
            .specifications
            .iter()
            .filter(|spec| spec.instrument == *instrument && spec.is_valid_at(observed_at_us))
            .cloned()
            .collect();
        match matches.len() {
            0 => ReferenceResolution::Unresolved,
            1 => ReferenceResolution::Resolved(matches[0].clone()),
            _ => ReferenceResolution::Ambiguous,
        }
    }

    /// Resolve a venue symbol without inferring a cross-listing identity.
    pub fn resolve_venue_mapping(
        &self,
        mappings: &[VenueInstrumentMapping],
        observed_at_us: i64,
    ) -> ReferenceResolution<VenueInstrumentMapping> {
        let matches: Vec<VenueInstrumentMapping> = mappings
            .iter()
            .filter(|mapping| mapping.identifier.is_valid_at(observed_at_us))
            .cloned()
            .collect();
        match matches.len() {
            0 => ReferenceResolution::Unresolved,
            1 => ReferenceResolution::Resolved(matches[0].clone()),
            _ => ReferenceResolution::Ambiguous,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::IdentifierScheme;

    fn venue_id() -> VenueId {
        VenueId::derive("registry", "mic", "XABC")
    }

    fn venue() -> Venue {
        Venue {
            id: venue_id(),
            name: "Example Exchange".to_owned(),
            mic: Some("XABC".to_owned()),
            category: VenueCategory::Exchange,
            jurisdiction: CountryCode::new("US").expect("country code"),
            settlement_currency: Some(CurrencyCode::new("USD").expect("currency code")),
            mic_roles: vec![MicRole::Trading],
            identifiers: Vec::new(),
        }
    }

    #[test]
    fn venue_conflicts_are_rejected() {
        let mut graph = ReferenceGraph::new();
        graph.insert_venue(venue()).expect("first venue");
        let mut altered = venue();
        altered.name = "Other Name".to_owned();
        assert!(
            graph.insert_venue(altered).is_err(),
            "conflicting venue must be rejected"
        );
        graph
            .insert_venue(venue())
            .expect("duplicate is idempotent");
    }

    #[test]
    fn effective_dated_specifications_resolve_without_overwrite() {
        let instrument = InstrumentId::derive("registry", "isin", "US0000000001");
        let mut graph = ReferenceGraph::new();
        let first = InstrumentSpecification {
            instrument: instrument.clone(),
            legal_name: "Old Name".to_owned(),
            issuer: None,
            currency: None,
            multiplier: None,
            expiry_us: None,
            strike_mantissa: None,
            valid_from_us: Some(0),
            valid_to_us: Some(100),
        };
        let second = InstrumentSpecification {
            valid_from_us: Some(101),
            valid_to_us: None,
            legal_name: "New Name".to_owned(),
            ..first.clone()
        };
        graph
            .insert_specification(first.clone())
            .expect("first specification");
        graph
            .insert_specification(second)
            .expect("second specification");
        assert_eq!(
            graph.resolve_specification(&instrument, 50),
            ReferenceResolution::Resolved(first),
            "first specification must resolve"
        );
        assert!(
            matches!(
                graph.resolve_specification(&instrument, 150),
                ReferenceResolution::Resolved(_)
            ),
            "second specification must resolve"
        );
    }

    #[test]
    fn standard_identifiers_are_effective_dated() {
        let identifier = StandardIdentifier {
            standard: IdentifierStandard::Isin,
            value: "US0000000001".to_owned(),
            valid_from_us: Some(10),
            valid_to_us: Some(20),
            source: "gleif".to_owned(),
        };
        assert!(
            !identifier.is_valid_at(9),
            "earlier time is outside validity"
        );
        assert!(identifier.is_valid_at(15), "interior time is valid");
        assert!(
            !identifier.is_valid_at(21),
            "later time is outside validity"
        );
        assert_eq!(
            MicRole::TradeReporting.as_str(),
            "trade_reporting",
            "MIC role string must be stable"
        );
    }

    #[test]
    fn venue_mapping_ambiguity_is_not_collapsed() {
        let instrument = InstrumentId::derive("registry", "isin", "US0000000002");
        let identifier = crate::identity::InstrumentIdentifier {
            scheme: IdentifierScheme("ticker".to_owned()),
            assigning_authority: "XABC".to_owned(),
            identifier_value: "ABC".to_owned(),
            scope: "venue".to_owned(),
            valid_from: 0,
            valid_to: None,
            observed_at: 0,
            source_provenance: "adapter".to_owned(),
        };
        let mapping = VenueInstrumentMapping {
            instrument,
            venue: venue_id(),
            listing: crate::identity::ListingId::derive("registry", "XABC", "ABC"),
            segment: None,
            identifier,
            relationships: Vec::new(),
        };
        let graph = ReferenceGraph::new();
        assert_eq!(
            graph.resolve_venue_mapping(std::slice::from_ref(&mapping), 10),
            ReferenceResolution::Resolved(mapping.clone()),
            "single venue mapping must resolve"
        );
        assert_eq!(
            graph.resolve_venue_mapping(&[mapping.clone(), mapping], 10),
            ReferenceResolution::Ambiguous,
            "duplicate venue mapping must stay ambiguous"
        );
    }
}
