//! OKX adapter.
//!
//! H0 implements reference data only: deterministic decoding of an offline
//! `instruments`-style document into validated `MarketInfo` records with
//! explicit product-type-qualified identity and exact numeric scales. No
//! socket is opened, no subscription is sent and no book event is produced;
//! the session is an inert stub with a disabled heartbeat. H1 (planned) adds
//! REST discovery and a telemetry-only websocket, H2 (planned) adds a selected
//! book feed, and H3 (planned) adds capacity expansion behind the same seams.

pub mod codec;
pub mod discovery;
pub mod session;

use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    config::{ExchangeKind, OkxSettings},
    exchange::{DiscoveryResult, Exchange, ExchangeCapabilities},
    model::MarketInfo,
    session::SessionFactory,
};

pub use codec::parse_instruments_document;

/// OKX exchange adapter (H0 reference data only).
pub struct OkxExchange {
    settings: OkxSettings,
}

impl OkxExchange {
    #[must_use]
    pub fn new(settings: OkxSettings) -> Self {
        Self { settings }
    }

    #[must_use]
    pub fn settings(&self) -> &OkxSettings {
        &self.settings
    }
}

#[async_trait]
impl Exchange for OkxExchange {
    fn name(&self) -> &'static str {
        "okx"
    }

    fn environment(&self) -> &'static str {
        "public"
    }

    fn capabilities(&self) -> ExchangeCapabilities {
        ExchangeCapabilities::new(false)
    }

    async fn discover_markets(&self, max_markets: usize) -> anyhow::Result<Vec<MarketInfo>> {
        discovery::discover(&self.settings, max_markets).await
    }

    async fn discover_markets_with_stats(
        &self,
        max_markets: usize,
    ) -> anyhow::Result<DiscoveryResult> {
        discovery::discover_with_stats(&self.settings, max_markets).await
    }

    fn session_factory(self: Arc<Self>) -> Arc<dyn SessionFactory> {
        Arc::new(session::OkxSessionFactory)
    }

    // No separate lifecycle channel exists on OKX public feeds. The trait
    // default returns None; H0 states this explicitly for the capability map.
}

/// OKX declares no cross-replica comparable sequence clock. H0-H3 order by the
/// supervisor's receive fence, never by a venue field.
#[must_use]
pub fn capabilities() -> ExchangeCapabilities {
    ExchangeCapabilities::new(false)
}

#[must_use]
pub const fn kind() -> ExchangeKind {
    ExchangeKind::Okx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    const REFERENCE: &str = include_str!("../../../fixtures/okx/reference.json");

    fn scoped_settings() -> OkxSettings {
        OkxSettings {
            reference_fixture: Some(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("fixtures/okx/reference.json"),
            ),
            ..OkxSettings::default()
        }
    }

    #[test]
    fn h0_reference_document_produces_valid_namespaced_markets() {
        let value: serde_json::Value = serde_json::from_str(REFERENCE).expect("fixture JSON");
        let markets = parse_instruments_document(&value, &scoped_settings().inst_types)
            .expect("valid reference document");
        assert_eq!(markets.len(), 5);
        for market in &markets {
            market.validate().expect("valid market");
            market.numeric_spec.validate().expect("valid numeric spec");
            assert_eq!(market.exchange, "okx");
            assert_eq!(market.symbol, market.native_symbol);
            assert!(market.exchange_market_id.is_none());
            assert!(market.outcome.is_none());
        }
        assert!(
            markets
                .iter()
                .any(|market| market.native_symbol == "okx:spot:BTC/USDT")
        );
        assert!(
            markets
                .iter()
                .any(|market| market.native_symbol == "okx:swap:BTC/USDT")
        );
        let identities: std::collections::BTreeSet<&str> = markets
            .iter()
            .map(|market| market.native_symbol.as_str())
            .collect();
        assert_eq!(identities.len(), markets.len());
    }

    #[tokio::test]
    async fn h0_discovery_is_offline_and_reconciles_stats() {
        let exchange = OkxExchange::new(scoped_settings());
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
        let exchange = OkxExchange::new(OkxSettings::default());
        assert_eq!(exchange.name(), "okx");
        assert_eq!(exchange.environment(), "public");
        assert!(!exchange.capabilities().has_sequence_clock);
        assert_eq!(capabilities(), exchange.capabilities());
        assert_eq!(kind(), ExchangeKind::Okx);
        let adapter: Arc<dyn Exchange> = Arc::new(OkxExchange::new(OkxSettings::default()));
        assert!(adapter.lifecycle_session_factory().is_none());
    }

    #[test]
    fn h0_settings_defaults_are_backward_compatible_and_lowercase() {
        let old: OkxSettings = toml::from_str(
            r#"
            inst_types = ["SPOT", "SWAP"]
            request_timeout_secs = 5
            "#,
        )
        .expect("minimal OKX H0 configuration deserializes");
        assert_eq!(old.inst_types, vec!["SPOT", "SWAP"]);
        assert!(old.reference_fixture.is_none());
        assert_eq!(
            old.rest_url,
            "https://www.okx.com/api/v5/public/instruments"
        );
        let settings = Settings::for_exchange(ExchangeKind::Okx);
        assert_eq!(settings.exchange, ExchangeKind::Okx);
        assert_eq!(settings.exchange.to_string(), "okx");
    }
}
