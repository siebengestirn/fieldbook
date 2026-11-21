//! Universal typed event model for all financial information domains.
//!
//! The envelope carries provenance, multiple timestamp roles and typed domain
//! payloads. It does not reinterpret venue symbols as permanent identity and
//! does not collapse different information categories into one generic price
//! update.

use serde::Deserialize;
use serde::Serialize;

use crate::{
    canonical::EventId, identity::InstrumentId, quality::QualityState, reference_data::EntityId,
};

/// Role of one recorded timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampRole {
    ExchangeEvent,
    ExchangePublication,
    NetworkReceive,
    ApplicationReceive,
    Normalization,
    DurableWrite,
}

impl TimestampRole {
    /// Return the stable contract string for this timestamp role.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExchangeEvent => "exchange_event",
            Self::ExchangePublication => "exchange_publication",
            Self::NetworkReceive => "network_receive",
            Self::ApplicationReceive => "application_receive",
            Self::Normalization => "normalization",
            Self::DurableWrite => "durable_write",
        }
    }
}

/// One timestamp with explicit clock, precision and uncertainty metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedTimestamp {
    pub role: TimestampRole,
    pub value_us: i64,
    pub unit: String,
    pub clock_source: String,
    pub precision_us: u32,
    pub uncertainty_us: Option<u32>,
    pub utc_offset_minutes: Option<i16>,
}

impl ObservedTimestamp {
    /// Build a timestamp in microseconds with no claimed uncertainty.
    pub fn new(role: TimestampRole, value_us: i64, clock_source: &str) -> Self {
        Self {
            role,
            value_us,
            unit: "microseconds".to_owned(),
            clock_source: clock_source.to_owned(),
            precision_us: 1,
            uncertainty_us: None,
            utc_offset_minutes: Some(0),
        }
    }
}

/// Link from a normalized event back to immutable source evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub raw_record_id: String,
    pub source_id: String,
    pub feed_id: Option<String>,
    pub venue: Option<String>,
    pub decoder_version: String,
    pub license_reference: Option<String>,
}

/// Order lifecycle action where an order-level feed provides one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderAction {
    Add,
    Modify,
    Cancel,
    Execute,
}

/// Strongly typed payload for one financial information domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "payload_type", rename_all = "snake_case")]
pub enum DomainPayload {
    EquityQuote {
        bid_mantissa: i64,
        ask_mantissa: i64,
        bid_quantity: i64,
        ask_quantity: i64,
        price_scale: u8,
        quantity_scale: u8,
    },
    OrderEvent {
        order_id: String,
        action: OrderAction,
        side: String,
        price_mantissa: i64,
        quantity_mantissa: i64,
    },
    Trade {
        trade_id: String,
        price_mantissa: i64,
        quantity_mantissa: i64,
    },
    BondQuote {
        yield_basis_points: i64,
        clean_price_mantissa: i64,
        accrued_interest_mantissa: i64,
    },
    FxQuote {
        bid_mantissa: i64,
        ask_mantissa: i64,
        value_date_us: i64,
    },
    FutureQuote {
        bid_mantissa: i64,
        ask_mantissa: i64,
        open_interest: u64,
    },
    OptionQuote {
        strike_mantissa: i64,
        expiry_us: i64,
        implied_volatility_basis_points: i64,
    },
    CommodityAssessment {
        location: String,
        grade: String,
        price_mantissa: i64,
    },
    EconomicObservation {
        release_id: String,
        value_mantissa: i64,
        revision: u32,
    },
    CorporateAction {
        action_type: String,
        effective_us: i64,
    },
    NewsEvent {
        document_sha256: String,
        published_us: i64,
    },
    BlockchainEvent {
        chain: String,
        block_number: u64,
        transaction_hash: Option<String>,
    },
}

impl DomainPayload {
    /// Return the stable contract string for this payload.
    pub fn payload_type(&self) -> &'static str {
        match self {
            Self::EquityQuote { .. } => "equity_quote",
            Self::OrderEvent { .. } => "order_event",
            Self::Trade { .. } => "trade",
            Self::BondQuote { .. } => "bond_quote",
            Self::FxQuote { .. } => "fx_quote",
            Self::FutureQuote { .. } => "future_quote",
            Self::OptionQuote { .. } => "option_quote",
            Self::CommodityAssessment { .. } => "commodity_assessment",
            Self::EconomicObservation { .. } => "economic_observation",
            Self::CorporateAction { .. } => "corporate_action",
            Self::NewsEvent { .. } => "news_event",
            Self::BlockchainEvent { .. } => "blockchain_event",
        }
    }
}

/// Universal event envelope with typed payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UniversalEnvelope<T> {
    pub event_id: EventId,
    pub instrument_id: Option<InstrumentId>,
    pub entity_id: Option<EntityId>,
    pub event_type: String,
    pub timestamps: Vec<ObservedTimestamp>,
    pub sequence: Option<u64>,
    pub source_schema_version: u32,
    pub normalizer_version: String,
    pub quality: QualityState,
    pub provenance: Provenance,
    pub payload: T,
}

impl<T> UniversalEnvelope<T> {
    /// Validate the provenance and timestamp contract.
    pub fn validate(&self) -> Result<(), EventModelError> {
        if self.event_id.as_str().is_empty() {
            return Err(EventModelError::MissingEventId);
        }
        if self.provenance.raw_record_id.is_empty() || self.provenance.source_id.is_empty() {
            return Err(EventModelError::MissingProvenance);
        }
        let has_receive = self.timestamps.iter().any(|timestamp| {
            matches!(
                timestamp.role,
                TimestampRole::NetworkReceive | TimestampRole::ApplicationReceive
            )
        });
        if !has_receive {
            return Err(EventModelError::MissingReceiveTimestamp);
        }
        if self
            .timestamps
            .iter()
            .any(|timestamp| timestamp.unit != "microseconds")
        {
            return Err(EventModelError::UnknownTimestampUnit);
        }
        Ok(())
    }
}

/// Failure in the universal event contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventModelError {
    /// Event identity is empty.
    #[error("event identity is required")]
    MissingEventId,
    /// Raw record or source identity is empty.
    #[error("provenance is required")]
    MissingProvenance,
    /// No network or application receive timestamp was recorded.
    #[error("receive timestamp is required")]
    MissingReceiveTimestamp,
    /// A timestamp declared an unsupported unit.
    #[error("timestamp unit must be microseconds")]
    UnknownTimestampUnit,
}

/// Specialized market-data envelope.
pub type MarketEnvelope = UniversalEnvelope<DomainPayload>;

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> MarketEnvelope {
        UniversalEnvelope {
            event_id: EventId::derive("test", "source", "event-1"),
            instrument_id: None,
            entity_id: None,
            event_type: "equity_quote".to_owned(),
            timestamps: vec![ObservedTimestamp::new(
                TimestampRole::NetworkReceive,
                1_700_000_000_000_000,
                "host_clock",
            )],
            sequence: Some(1),
            source_schema_version: 1,
            normalizer_version: "test".to_owned(),
            quality: QualityState::RawCaptured,
            provenance: Provenance {
                raw_record_id: "record-1".to_owned(),
                source_id: "source".to_owned(),
                feed_id: None,
                venue: None,
                decoder_version: "test".to_owned(),
                license_reference: None,
            },
            payload: DomainPayload::EquityQuote {
                bid_mantissa: 100,
                ask_mantissa: 101,
                bid_quantity: 10,
                ask_quantity: 20,
                price_scale: 6,
                quantity_scale: 6,
            },
        }
    }

    #[test]
    fn envelope_validates_receive_and_provenance() {
        let event = envelope();
        assert!(event.validate().is_ok(), "complete envelope must validate");
        let mut missing = event.clone();
        missing.timestamps.clear();
        assert_eq!(
            missing.validate(),
            Err(EventModelError::MissingReceiveTimestamp),
            "receive timestamp is mandatory"
        );
    }

    #[test]
    fn payload_types_remain_distinct() {
        let event = envelope();
        assert_eq!(
            event.payload.payload_type(),
            "equity_quote",
            "payload type must be stable"
        );
        let text = serde_json::to_string(&event.payload).expect("serialize payload");
        assert!(
            text.contains("\"payload_type\":\"equity_quote\""),
            "serde tag must be present"
        );
    }
}
