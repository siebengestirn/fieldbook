//! Coinbase Exchange reference-data discovery (H0).
//!
//! Discovery is deliberately separate from the websocket path: H0 opens no
//! socket. Reference data comes either from an explicitly configured offline
//! `products` fixture or from the public `/products` REST endpoint.

use std::time::Instant;

use anyhow::Context as _;
use reqwest::Client;
use serde_json::Value;

use crate::{
    capacity::count_selected_markets, config::CoinbaseSettings, exchange::DiscoveryResult,
    model::MarketInfo, time::now_us,
};

use super::codec::parse_reference_document;

/// Fetch the public products document from the configured REST endpoint.
async fn fetch_products(client: &Client, settings: &CoinbaseSettings) -> anyhow::Result<Value> {
    let url = format!("{}/products", settings.rest_url.trim_end_matches('/'));
    client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("fetching Coinbase products from `{url}`"))?
        .error_for_status()
        .with_context(|| format!("Coinbase products status from `{url}`"))?
        .json::<Value>()
        .await
        .context("parsing Coinbase products response")
}

/// Discover the configured reference universe.
pub async fn discover(
    settings: &CoinbaseSettings,
    client: &Client,
    max_markets: usize,
) -> anyhow::Result<Vec<MarketInfo>> {
    Ok(discover_with_stats(settings, client, max_markets)
        .await?
        .markets)
}

/// Discover with page/record telemetry. H0 has a single products document, so
/// `pages` is always one; malformed and retry counters stay zero because a bad
/// record rejects the round rather than being dropped.
pub async fn discover_with_stats(
    settings: &CoinbaseSettings,
    client: &Client,
    max_markets: usize,
) -> anyhow::Result<DiscoveryResult> {
    let started = Instant::now();
    let started_at_us = now_us().unwrap_or(0);
    let document = if let Some(path) = &settings.reference_fixture {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading Coinbase reference fixture `{}`", path.display()))?;
        serde_json::from_str(&text).context("parsing Coinbase reference fixture")?
    } else {
        fetch_products(client, settings).await?
    };
    let mut markets = parse_reference_document(&document, settings.require_online)?;
    markets.sort_by(|left, right| left.native_symbol.cmp(&right.native_symbol));
    if max_markets > 0 && markets.len() > max_markets {
        markets.truncate(max_markets);
    }
    let selected_markets = count_selected_markets(&markets) as u64;
    let outcome_symbols = markets.len() as u64;
    Ok(DiscoveryResult {
        markets,
        stats: crate::exchange::DiscoveryStats {
            pages: 1,
            selected_markets,
            outcome_symbols,
            malformed_records: 0,
            retries: 0,
            duration_ms: started.elapsed().as_millis() as u64,
            started_at_us,
            finished_at_us: now_us().unwrap_or(started_at_us),
        },
    })
}
