//! Hyperliquid reference-data discovery (H0).
//!
//! Discovery is deliberately separate from the websocket path: H0 opens no
//! socket. Reference data comes either from an explicitly configured offline
//! fixture or from the `/info` REST endpoint.

use std::time::Instant;

use anyhow::{Context as _, bail};
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    capacity::count_selected_markets, config::HyperliquidSettings, exchange::DiscoveryResult,
    model::MarketInfo, time::now_us,
};

use super::codec::{parse_reference_document, parse_spot_meta};

/// Fetch one `meta` document from the configured REST endpoint.
async fn fetch_meta(
    client: &Client,
    settings: &HyperliquidSettings,
    dex: &str,
) -> anyhow::Result<Value> {
    let body = if dex.is_empty() {
        json!({"type": "meta"})
    } else {
        json!({"type": "meta", "dex": dex})
    };
    client
        .post(&settings.info_url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("fetching Hyperliquid meta for DEX `{dex}`"))?
        .error_for_status()
        .with_context(|| format!("Hyperliquid meta status for DEX `{dex}`"))?
        .json::<Value>()
        .await
        .context("parsing Hyperliquid meta response")
}

async fn fetch_spot_meta(client: &Client, settings: &HyperliquidSettings) -> anyhow::Result<Value> {
    client
        .post(&settings.info_url)
        .json(&json!({"type": "spotMeta"}))
        .send()
        .await
        .context("fetching Hyperliquid spotMeta")?
        .error_for_status()
        .context("Hyperliquid spotMeta status")?
        .json::<Value>()
        .await
        .context("parsing Hyperliquid spotMeta response")
}

/// Discover the configured reference universe.
pub async fn discover(
    settings: &HyperliquidSettings,
    client: &Client,
    max_markets: usize,
) -> anyhow::Result<Vec<MarketInfo>> {
    Ok(discover_with_stats(settings, client, max_markets)
        .await?
        .markets)
}

/// Discover with page/record telemetry.
pub async fn discover_with_stats(
    settings: &HyperliquidSettings,
    client: &Client,
    max_markets: usize,
) -> anyhow::Result<DiscoveryResult> {
    let started = Instant::now();
    let started_at_us = now_us().unwrap_or(0);
    if settings.dexs.is_empty() {
        bail!("Hyperliquid discovery requires at least one configured DEX");
    }
    let mut markets = Vec::new();
    let mut pages = 0u64;
    if let Some(path) = &settings.reference_fixture {
        let text = std::fs::read_to_string(path).with_context(|| {
            format!("reading Hyperliquid reference fixture `{}`", path.display())
        })?;
        let document: Value =
            serde_json::from_str(&text).context("parsing Hyperliquid reference fixture")?;
        markets.extend(parse_reference_document(
            &document,
            &settings.dexs,
            settings.spot_enabled,
        )?);
        pages += 1;
        if settings.spot_enabled {
            pages += 1;
        }
    } else {
        for dex in &settings.dexs {
            let meta = fetch_meta(client, settings, dex).await?;
            let universe = meta
                .get("universe")
                .and_then(Value::as_array)
                .with_context(|| format!("meta for DEX `{dex}` has no universe"))?;
            let mut document = serde_json::Map::new();
            document.insert("universe".to_owned(), Value::Array(universe.clone()));
            markets.extend(super::codec::parse_perp_universe(
                &Value::Object(document),
                dex,
            )?);
            pages += 1;
        }
        if settings.spot_enabled {
            let spot = fetch_spot_meta(client, settings).await?;
            markets.extend(parse_spot_meta(&spot)?);
            pages += 1;
        }
    }
    markets.sort_by(|left, right| left.native_symbol.cmp(&right.native_symbol));
    if max_markets > 0 && markets.len() > max_markets {
        markets.truncate(max_markets);
    }
    let selected_markets = count_selected_markets(&markets) as u64;
    let outcome_symbols = markets.len() as u64;
    Ok(DiscoveryResult {
        markets,
        stats: crate::exchange::DiscoveryStats {
            pages,
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
