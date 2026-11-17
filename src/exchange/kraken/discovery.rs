//! Kraken reference-data discovery (H0).
//!
//! Discovery is separate from the websocket path: H0 opens no
//! socket. Reference data comes either from an explicitly configured offline
//! `AssetPairs`-style fixture or from a read-only GET against the public REST
//! endpoint.

use std::time::Instant;

use anyhow::{Context as _, bail};
use reqwest::Client;
use serde_json::Value;

use crate::{
    capacity::count_selected_markets, config::KrakenSettings, exchange::DiscoveryResult,
    model::MarketInfo, time::now_us,
};

use super::codec::parse_reference_document;

/// Fetch the `AssetPairs` document from the configured REST endpoint.
async fn fetch_asset_pairs(client: &Client, settings: &KrakenSettings) -> anyhow::Result<Value> {
    client
        .get(&settings.asset_pairs_url)
        .send()
        .await
        .context("fetching Kraken AssetPairs")?
        .error_for_status()
        .context("Kraken AssetPairs status")?
        .json::<Value>()
        .await
        .context("parsing Kraken AssetPairs response")
}

/// Apply the explicit pair selection, if any. A configured entry may be a
/// venue key (`XXBTZUSD`) or a canonical identity (`kraken:spot:XBT/USD`).
fn select_pairs(markets: Vec<MarketInfo>, pairs: &[String]) -> anyhow::Result<Vec<MarketInfo>> {
    if pairs.is_empty() {
        return Ok(markets);
    }
    let requested: std::collections::BTreeSet<&str> = pairs.iter().map(String::as_str).collect();
    let selected: Vec<MarketInfo> = markets
        .into_iter()
        .filter(|market| {
            requested.contains(market.native_symbol.as_str())
                || market
                    .exchange_market_id
                    .as_deref()
                    .is_some_and(|key| requested.contains(key))
        })
        .collect();
    if selected.is_empty() {
        bail!("no Kraken pair matched the configured `pairs` selection");
    }
    Ok(selected)
}

/// Discover the configured reference universe.
pub async fn discover(
    settings: &KrakenSettings,
    client: &Client,
    max_markets: usize,
) -> anyhow::Result<Vec<MarketInfo>> {
    Ok(discover_with_stats(settings, client, max_markets)
        .await?
        .markets)
}

/// Discover with page/record telemetry.
pub async fn discover_with_stats(
    settings: &KrakenSettings,
    client: &Client,
    max_markets: usize,
) -> anyhow::Result<DiscoveryResult> {
    let started = Instant::now();
    let started_at_us = now_us().unwrap_or(0);
    let mut markets = if let Some(path) = &settings.reference_fixture {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading Kraken reference fixture `{}`", path.display()))?;
        let document: Value =
            serde_json::from_str(&text).context("parsing Kraken reference fixture")?;
        parse_reference_document(&document)?
    } else {
        let document = fetch_asset_pairs(client, settings).await?;
        parse_reference_document(&document)?
    };
    markets = select_pairs(markets, &settings.pairs)?;
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
