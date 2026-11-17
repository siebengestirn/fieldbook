//! Kraken adapter.
//!
//! H0 implements reference data only: deterministic discovery of active spot
//! instruments from an `AssetPairs` document. No websocket is opened and no
//! book or telemetry event is produced. Later stages extend this module
//! without changing the normalized core.

pub mod codec;
pub mod discovery;
pub mod session;

use std::sync::Arc;

use anyhow::Context as _;
use async_trait::async_trait;

use crate::{
    config::{ExchangeKind, KrakenSettings},
    exchange::{DiscoveryResult, Exchange, ExchangeCapabilities},
    model::MarketInfo,
    session::SessionFactory,
};

pub use codec::parse_reference_document;

/// Kraken exchange adapter (H0 reference data).
pub struct KrakenExchange {
    settings: KrakenSettings,
    client: reqwest::Client,
}

impl KrakenExchange {
    pub fn new(settings: KrakenSettings) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(
                settings.request_timeout_secs.max(1),
            ))
            .build()
            .context("building Kraken HTTP client")?;
        Ok(Self { settings, client })
    }

    #[must_use]
    pub fn settings(&self) -> &KrakenSettings {
        &self.settings
    }
}

#[async_trait]
impl Exchange for KrakenExchange {
    fn name(&self) -> &'static str {
        "kraken"
    }

    fn environment(&self) -> &'static str {
        "public"
    }

    fn capabilities(&self) -> ExchangeCapabilities {
        ExchangeCapabilities::new(false)
    }

    async fn discover_markets(&self, max_markets: usize) -> anyhow::Result<Vec<MarketInfo>> {
        discovery::discover(&self.settings, &self.client, max_markets).await
    }

    async fn discover_markets_with_stats(
        &self,
        max_markets: usize,
    ) -> anyhow::Result<DiscoveryResult> {
        discovery::discover_with_stats(&self.settings, &self.client, max_markets).await
    }

    fn session_factory(self: Arc<Self>) -> Arc<dyn SessionFactory> {
        Arc::new(session::KrakenSessionFactory)
    }
}

pub fn capabilities() -> ExchangeCapabilities {
    ExchangeCapabilities::new(false)
}

#[must_use]
pub const fn kind() -> ExchangeKind {
    ExchangeKind::Kraken
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    const REFERENCE: &str = include_str!("../../../fixtures/kraken/reference.json");

    #[test]
    fn h0_reference_document_produces_valid_namespaced_markets() {
        let value: serde_json::Value = serde_json::from_str(REFERENCE).expect("fixture JSON");
        let markets = parse_reference_document(&value).expect("valid reference document");
        assert_eq!(markets.len(), 5);
        for market in &markets {
            market.validate().expect("valid market");
            market.numeric_spec.validate().expect("valid numeric spec");
            assert_eq!(market.exchange, "kraken");
            assert_eq!(market.symbol, market.native_symbol);
            assert!(market.exchange_market_id.is_some());
            assert!(market.outcome.is_none());
        }
        assert!(
            markets
                .iter()
                .any(|market| market.native_symbol == "kraken:spot:XBT/USD")
        );
        assert!(
            markets
                .iter()
                .any(|market| market.native_symbol == "kraken:spot:XBT/EUR")
        );
        assert!(
            markets
                .iter()
                .any(|market| market.native_symbol == "kraken:spot:XRP/USD")
        );
    }

    #[tokio::test]
    async fn h0_discovery_is_offline_and_reconciles_stats() {
        let settings = KrakenSettings {
            reference_fixture: Some(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("fixtures/kraken/reference.json"),
            ),
            ..KrakenSettings::default()
        };
        let exchange = KrakenExchange::new(settings).unwrap();
        let result = exchange.discover_markets_with_stats(0).await.unwrap();
        assert_eq!(result.markets.len(), 5);
        assert_eq!(result.stats.selected_markets, 5);
        assert_eq!(result.stats.outcome_symbols, 5);
        assert_eq!(result.stats.pages, 1);
        assert_eq!(result.stats.malformed_records, 0);
        assert_eq!(result.stats.retries, 0);
    }

    #[test]
    fn h0_uses_clockless_capabilities_and_no_lifecycle_plane() {
        let exchange = KrakenExchange::new(KrakenSettings::default()).unwrap();
        assert_eq!(exchange.name(), "kraken");
        assert!(!exchange.capabilities().has_sequence_clock);
        assert_eq!(capabilities(), exchange.capabilities());
        assert_eq!(kind(), ExchangeKind::Kraken);
        let settings = Settings::for_exchange(ExchangeKind::Kraken);
        assert_eq!(settings.exchange, ExchangeKind::Kraken);
    }
}
