//! Hyperliquid telemetry codec (H1).
//!
//! Decodes the venue-wide `allMids` and `fastAssetCtxs` channels into typed,
//! validated observations. These frames are telemetry only: no price level,
//! depth, or book anchor is derived from them and no `ExchangeEvent` is ever
//! produced here. Numeric strings are parsed exactly at the canonical SBE
//! scale and rejected when inexact; unknown additive fields are ignored.
//!
//! `assetCtxs` is not subscribed in H1: no checked-in golden
//! fixture pins its wire shape. Adding it is a later, fixture-gated change.

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::fixed;

/// Canonical SBE decimal scale. Mirrors `fixed::DECIMALS`; the assertion in
/// the test module keeps the two in lockstep.
const SCALE: u8 = 6;

/// Venue channel carrying a map of coin to mid price.
pub const CHANNEL_ALL_MIDS: &str = "allMids";
/// Venue channel carrying per-coin statistics at high frequency.
pub const CHANNEL_FAST_ASSET_CTXS: &str = "fastAssetCtxs";
/// Documented as not subscribed in H1 (no golden fixture). Listed so the
/// omission is explicit rather than an accidental oversight.
pub const CHANNEL_ASSET_CTXS_NOT_SUBSCRIBED: &str = "assetCtxs";

/// One coin's mid price from an `allMids` frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mid {
    pub coin: String,
    /// Exact coefficient at [`SCALE`] decimal places.
    pub mid: i64,
}

/// Per-coin statistics from a `fastAssetCtxs` frame. Only exact price fields
/// are retained; funding/open-interest/volume-style fields are additive
/// telemetry that H1 ignores (and must not round).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetCtx {
    pub coin: String,
    pub mark_px: Option<i64>,
    pub mid_px: Option<i64>,
    pub oracle_px: Option<i64>,
}

/// A decoded telemetry frame. `Ignored` is returned for an additive channel
/// this build does not recognise; it is never a decode error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryFrame {
    AllMids(Vec<Mid>),
    FastAssetCtxs(Vec<AssetCtx>),
    Ignored(String),
}

/// Build the wire subscription request for a telemetry channel.
///
/// Only the two fixture-proven H1 channels are allowed. `assetCtxs` and any
/// other channel are rejected so an operator cannot silently subscribe to a
/// feed whose wire shape has not been pinned by a golden fixture.
pub fn subscription_request(channel: &str) -> anyhow::Result<Value> {
    match channel {
        CHANNEL_ALL_MIDS | CHANNEL_FAST_ASSET_CTXS => Ok(json!({
            "method": "subscribe",
            "subscription": { "type": channel },
        })),
        other => bail!(
            "Hyperliquid channel `{other}` is not subscribed in H1; only `allMids` and \
             `fastAssetCtxs` are supported (`assetCtxs` has no golden fixture)"
        ),
    }
}

/// Parse one optional numeric string field exactly. `null` and absent fields
/// are treated as absent; a present non-string or inexact value is rejected.
fn numeric_field(value: &Value, field: &str, context: &str) -> anyhow::Result<Option<i64>> {
    let Some(raw) = value.get(field) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let text = raw
        .as_str()
        .with_context(|| format!("`{field}` in {context} is not a string"))?;
    let parsed = fixed::parse_scaled(text, SCALE)
        .with_context(|| format!("parsing `{field}`=`{text}` in {context}"))?;
    Ok(Some(parsed))
}

fn coin_field(value: &Value, context: &str) -> anyhow::Result<String> {
    let coin = value
        .get("coin")
        .and_then(Value::as_str)
        .with_context(|| format!("{context} has no string `coin`"))?;
    anyhow::ensure!(!coin.is_empty(), "{context} has an empty `coin`");
    Ok(coin.to_owned())
}

fn parse_all_mids(data: &Value) -> anyhow::Result<TelemetryFrame> {
    // Canonical shape is `data.mids`; accept a bare coin map as well so a
    // venue variation does not force a lossy fallback. Both are telemetry.
    let mids = data
        .get("mids")
        .and_then(Value::as_object)
        .or_else(|| data.as_object())
        .context("allMids data has no `mids` object")?;
    let mut out = Vec::with_capacity(mids.len());
    for (coin, value) in mids {
        anyhow::ensure!(!coin.is_empty(), "allMids has an empty coin");
        let text = value
            .as_str()
            .with_context(|| format!("allMids mid for `{coin}` is not a string"))?;
        let mid = fixed::parse_scaled(text, SCALE)
            .with_context(|| format!("parsing allMids mid for `{coin}`"))?;
        out.push(Mid {
            coin: coin.clone(),
            mid,
        });
    }
    out.sort_by(|left, right| left.coin.cmp(&right.coin));
    Ok(TelemetryFrame::AllMids(out))
}

fn parse_fast_asset_ctxs(data: &Value) -> anyhow::Result<TelemetryFrame> {
    let entries = data
        .as_array()
        .context("fastAssetCtxs data is not an array")?;
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let context = "fastAssetCtxs entry";
        let coin = coin_field(entry, context)?;
        let mark_px = numeric_field(entry, "markPx", context)?;
        let mid_px = numeric_field(entry, "midPx", context)?;
        let oracle_px = numeric_field(entry, "oraclePx", context)?;
        out.push(AssetCtx {
            coin,
            mark_px,
            mid_px,
            oracle_px,
        });
    }
    Ok(TelemetryFrame::FastAssetCtxs(out))
}

/// Decode one inbound text payload as a Hyperliquid telemetry frame.
///
/// Unknown additive fields at every level are ignored. A malformed numeric
/// value in a field this codec recognises rejects the whole frame, which the
/// session reports as a decode failure with the exact raw bytes retained.
pub fn decode_telemetry(text: &str) -> anyhow::Result<TelemetryFrame> {
    let value: Value = serde_json::from_str(text).context("parsing Hyperliquid telemetry JSON")?;
    let channel = value
        .get("channel")
        .and_then(Value::as_str)
        .context("Hyperliquid frame has no string `channel`")?;
    let data = value
        .get("data")
        .context("Hyperliquid frame has no `data`")?;
    match channel {
        CHANNEL_ALL_MIDS => parse_all_mids(data),
        CHANNEL_FAST_ASSET_CTXS => parse_fast_asset_ctxs(data),
        other => Ok(TelemetryFrame::Ignored(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_MIDS: &str = include_str!("../../../fixtures/hyperliquid/all_mids.json");
    const ALL_MIDS_MALFORMED: &str =
        include_str!("../../../fixtures/hyperliquid/all_mids_malformed.json");
    const FAST_CTX: &str = include_str!("../../../fixtures/hyperliquid/fast_asset_ctxs.json");
    const FAST_CTX_MALFORMED: &str =
        include_str!("../../../fixtures/hyperliquid/fast_asset_ctxs_malformed.json");

    #[test]
    fn canonical_scale_matches_the_fixed_point_pipeline() {
        assert_eq!(usize::from(SCALE), fixed::DECIMALS);
    }

    #[test]
    fn all_mids_decodes_exact_values_and_ignores_additive_fields() {
        let frame = decode_telemetry(ALL_MIDS).expect("valid allMids fixture");
        assert_eq!(
            frame,
            TelemetryFrame::AllMids(vec![
                Mid {
                    coin: "BTC".to_owned(),
                    mid: 67_890_125_000
                },
                Mid {
                    coin: "ETH".to_owned(),
                    mid: 3_512_500_000
                },
            ])
        );
    }

    #[test]
    fn fast_asset_ctxs_decodes_exact_values_and_ignores_additive_fields() {
        let frame = decode_telemetry(FAST_CTX).expect("valid fastAssetCtxs fixture");
        assert_eq!(
            frame,
            TelemetryFrame::FastAssetCtxs(vec![
                AssetCtx {
                    coin: "BTC".to_owned(),
                    mark_px: Some(67_890_000_000),
                    mid_px: Some(67_890_125_000),
                    oracle_px: Some(67_800_000_000),
                },
                AssetCtx {
                    coin: "ETH".to_owned(),
                    mark_px: None,
                    mid_px: Some(3_512_500_000),
                    oracle_px: None,
                },
            ])
        );
    }

    #[test]
    fn malformed_numeric_strings_are_rejected_not_rounded() {
        // Both fixtures carry an otherwise valid envelope with one inexact
        // numeric value; neither may be silently rounded or accepted.
        assert!(decode_telemetry(ALL_MIDS_MALFORMED).is_err());
        assert!(decode_telemetry(FAST_CTX_MALFORMED).is_err());
        for bad in [
            r#"{"channel":"allMids","data":{"mids":{"BTC":"1.1234567"}}}"#,
            r#"{"channel":"allMids","data":{"mids":{"BTC":"not-a-number"}}}"#,
            r#"{"channel":"allMids","data":{"mids":{"BTC":"1e3"}}}"#,
            r#"{"channel":"fastAssetCtxs","data":[{"coin":"BTC","markPx":123}]}"#,
        ] {
            assert!(decode_telemetry(bad).is_err(), "expected rejection: {bad}");
        }
    }

    #[test]
    fn unknown_additive_fields_are_ignored_not_rejected() {
        let frame = decode_telemetry(
            r#"{
                "channel":"fastAssetCtxs",
                "futureEnvelopeField":{"anything":true},
                "data":[{
                    "coin":"BTC",
                    "midPx":"1.000001",
                    "funding":"0.0000125",
                    "impactPxs":["1.0","1.1"],
                    "openInterest":"123.456789",
                    "aFieldFromTheFuture":"not even numeric"
                }]
            }"#,
        )
        .expect("additive fields must not reject the frame");
        assert_eq!(
            frame,
            TelemetryFrame::FastAssetCtxs(vec![AssetCtx {
                coin: "BTC".to_owned(),
                mark_px: None,
                mid_px: Some(1_000_001),
                oracle_px: None,
            }])
        );
    }

    #[test]
    fn unknown_channel_is_ignored_not_rejected() {
        let frame = decode_telemetry(r#"{"channel":"newFancyFeed","data":{"x":1}}"#)
            .expect("unknown additive channel is ignored");
        assert_eq!(frame, TelemetryFrame::Ignored("newFancyFeed".to_owned()));
    }

    #[test]
    fn subscription_requests_cover_exactly_the_two_supported_channels() {
        assert_eq!(
            subscription_request(CHANNEL_ALL_MIDS).expect("allMids subscription"),
            json!({"method":"subscribe","subscription":{"type":"allMids"}})
        );
        assert_eq!(
            subscription_request(CHANNEL_FAST_ASSET_CTXS).expect("fastAssetCtxs subscription"),
            json!({"method":"subscribe","subscription":{"type":"fastAssetCtxs"}})
        );
        assert!(
            subscription_request(CHANNEL_ASSET_CTXS_NOT_SUBSCRIBED).is_err(),
            "assetCtxs is not subscribed in H1 without a golden fixture"
        );
    }
}
