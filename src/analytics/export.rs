//! Streaming NDJSON export of one verified capture.
//!
//! Rust owns input verification and typed row generation. Parquet and q are
//! separate downstream stages. This module writes only into a staging
//! directory chosen by the caller; publication is a later decision.

use std::{
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::Path,
};

use anyhow::Context;
use anyhow::ensure;
use serde::Serialize;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;

use crate::{
    journal::{self, DecodeOutcome, JournalEntry},
    report::{MarketMetadata, RunReport},
    util::{
        AlignedSbeReader, OrderbookSchema,
        dtos::orderbook::{AnyMessage, PriceLevelUpdateDomain, SnapshotDomain},
    },
};

use super::EXPORT_SCHEMA_VERSION;
use super::verify_capture;

const CAPTURE_ID_DOMAIN: &[u8] = b"fieldbook.export.v1";

#[derive(Debug, Clone, Serialize)]
struct JournalObservationRow {
    capture_id: String,
    run_id: String,
    journal_schema_version: u32,
    segment_index: u64,
    segment_file: String,
    segment_record_ordinal: u64,
    record_region_offset: u64,
    record_byte_length: u64,
    segment_file_sha256: String,
    source: String,
    shard: u16,
    replica: u16,
    connection_epoch: u64,
    observation_seq: u64,
    receive_seq: Option<u64>,
    control_seq: Option<u64>,
    clock_source: String,
    direction: String,
    record_kind: String,
    frame_kind: Option<String>,
    control_opcode: Option<String>,
    signal: Option<String>,
    signal_correlation: Option<Vec<u8>>,
    heartbeat_mechanism: Option<String>,
    received_at_us: i64,
    date: i32,
    payload_length: u64,
    payload_sha256: String,
    decode_status: Option<String>,
    decode_events: Option<u32>,
    decode_reason: Option<String>,
    decode_category: Option<String>,
    send_status: Option<String>,
    send_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SbeMessageRow {
    capture_id: String,
    sbe_file_sha256: String,
    message_ordinal: u64,
    message_kind: String,
    schema_id: u16,
    schema_version: u16,
    serialization_symbol: String,
    native_symbol: Option<String>,
    mapping_status: String,
    received_at_us: i64,
    exchange_timestamp_us: i64,
    date: i32,
    side: Option<String>,
    price_mantissa_sbe: Option<i64>,
    quantity_mantissa_sbe: Option<i64>,
    quantity_semantics: String,
    snapshot_bid_levels: u32,
    snapshot_ask_levels: u32,
}

#[derive(Debug, Clone, Serialize)]
struct SbeBookLevelRow {
    capture_id: String,
    sbe_file_sha256: String,
    message_ordinal: u64,
    serialization_symbol: String,
    side: String,
    level_ordinal: u32,
    price_mantissa_sbe: i64,
    quantity_mantissa_sbe: i64,
    received_at_us: i64,
    date: i32,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureInstrumentRow {
    capture_id: String,
    exchange: String,
    native_symbol: String,
    symbol: String,
    title: Option<String>,
    outcome: Option<String>,
    exchange_market_id: Option<String>,
    close_time_us: Option<i64>,
    tick_size_us: Option<i64>,
    price_level_structure: Option<String>,
    price_scale: u8,
    quantity_scale: u8,
    price_unit_kind: String,
    price_unit_code: Option<String>,
    quantity_unit_kind: String,
    quantity_unit_code: Option<String>,
    quantity_step: Option<i64>,
    lot_size: Option<i64>,
    contract_multiplier_num: Option<i64>,
    contract_multiplier_den: Option<i64>,
    metadata_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureChildRow {
    capture_id: String,
    symbol: String,
    ordinal: u32,
    start: i64,
    end: i64,
    step: i64,
    origin: i64,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureRunRow {
    capture_id: String,
    venue: String,
    environment: String,
    started_at_us: i64,
    ended_at_us: i64,
    duration_us: i64,
    journal_run_id: String,
    journal_schema_version: u32,
    journal_code_commit: String,
    journal_provenance_status: String,
    collector_version: String,
    sbe_schema_id: u16,
    sbe_schema_version: u16,
    expected_symbols: usize,
    acknowledged_symbols: usize,
    anchored_symbols: usize,
    exchange_snapshots: u64,
    sbe_messages: u64,
    journal_records: u64,
    capture_verification_status: String,
    export_verification_status: String,
    date: i32,
}

fn date_days(received_at_us: i64) -> i32 {
    let days = received_at_us.div_euclid(86_400_000_000);
    i32::try_from(days).unwrap_or(i32::MAX)
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn json_number(value: &Value, key: &str) -> Option<serde_json::Number> {
    value.get(key).and_then(Value::as_number).cloned()
}

fn decode_fields(
    outcome: Option<&DecodeOutcome>,
) -> (Option<String>, Option<u32>, Option<String>, Option<String>) {
    let Some(outcome) = outcome else {
        return (None, None, None, None);
    };
    let Ok(value) = serde_json::to_value(outcome) else {
        return (None, None, None, None);
    };
    let events = json_number(&value, "events")
        .and_then(|number| number.as_u64())
        .and_then(|v| u32::try_from(v).ok());
    (
        json_string(&value, "status"),
        events,
        json_string(&value, "reason"),
        json_string(&value, "category"),
    )
}

fn send_fields(result: Option<&crate::protocol::SendResult>) -> (Option<String>, Option<String>) {
    let Some(result) = result else {
        return (None, None);
    };
    let Ok(value) = serde_json::to_value(result) else {
        return (None, None);
    };
    (json_string(&value, "status"), json_string(&value, "error"))
}

fn unit_parts(value: &impl Serialize) -> (String, Option<String>) {
    match serde_json::to_value(value) {
        Ok(Value::String(text)) => (text, None),
        Ok(Value::Object(map)) => {
            let kind = map.keys().next().cloned().unwrap_or_default();
            let code = map.get(&kind).and_then(|entry| match entry {
                Value::String(text) => Some(text.clone()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            });
            (kind, code)
        }
        _ => ("unknown".to_owned(), None),
    }
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    hasher.update(length.to_le_bytes());
    hasher.update(bytes);
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("opening `{}` for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .context("reading export for hashing")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push_str(&format!("{byte:02x}"));
    }
    Ok(text)
}

fn write_row<W: Write, T: Serialize>(writer: &mut W, row: &T) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(row).context("serializing export row")?;
    writer.write_all(&bytes).context("writing export row")?;
    writer
        .write_all(b"\n")
        .context("writing export row terminator")?;
    Ok(())
}

struct JournalRowWriter<'a> {
    writer: BufWriter<File>,
    capture_id: &'a str,
    run_id: &'a str,
    segment_hashes: &'a std::collections::BTreeMap<String, String>,
    rows: u64,
}

impl journal::JournalVisitor for JournalRowWriter<'_> {
    fn visit(&mut self, entry: &JournalEntry<'_>) -> anyhow::Result<()> {
        let classification = serde_json::to_value(entry.classification)
            .context("serializing record classification")?;
        let (decode_status, decode_events, decode_reason, decode_category) =
            decode_fields(entry.outcome);
        let (send_status, send_error) = send_fields(entry.send_result);
        let signal_value =
            serde_json::to_value(entry.signal).context("serializing session signal")?;
        let row = JournalObservationRow {
            capture_id: self.capture_id.to_owned(),
            run_id: self.run_id.to_owned(),
            journal_schema_version: entry.schema_version,
            segment_index: entry.position.segment_index,
            segment_file: entry.position.segment_file.clone(),
            segment_record_ordinal: entry.position.segment_record_ordinal,
            record_region_offset: entry.position.record_byte_offset,
            record_byte_length: entry.position.record_byte_length,
            segment_file_sha256: self
                .segment_hashes
                .get(&entry.position.segment_file)
                .cloned()
                .unwrap_or_default(),
            source: entry.source.to_owned(),
            shard: entry.shard,
            replica: entry.replica,
            connection_epoch: entry.connection_epoch,
            observation_seq: entry.observation_seq,
            receive_seq: entry.receive_seq,
            control_seq: entry.control_seq,
            clock_source: entry.clock_source.to_owned(),
            direction: serde_json::to_value(entry.direction)
                .and_then(serde_json::from_value)
                .unwrap_or_default(),
            record_kind: json_string(&classification, "record_kind").unwrap_or_default(),
            frame_kind: json_string(&classification, "frame_kind"),
            control_opcode: json_string(&classification, "opcode"),
            signal: json_string(&signal_value, "signal"),
            signal_correlation: signal_value
                .get("correlation")
                .and_then(|value| serde_json::from_value(value.clone()).ok()),
            heartbeat_mechanism: serde_json::to_value(entry.mechanism)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned)),
            received_at_us: entry.received_at_us,
            date: date_days(entry.received_at_us),
            payload_length: u64::try_from(entry.payload.len())?,
            payload_sha256: entry.payload_sha256.clone(),
            decode_status,
            decode_events,
            decode_reason,
            decode_category,
            send_status,
            send_error,
        };
        write_row(&mut self.writer, &row)?;
        self.rows = self
            .rows
            .checked_add(1)
            .context("journal row count overflow")?;
        Ok(())
    }
}

fn build_symbol_map(markets: &MarketMetadata) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut map: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for market in &markets.markets {
        map.entry(market.symbol.clone())
            .or_default()
            .push(market.native_symbol.clone());
    }
    map
}

fn resolve_symbol(
    map: &std::collections::BTreeMap<String, Vec<String>>,
    symbol: &str,
) -> (Option<String>, &'static str) {
    match map.get(symbol) {
        None => (None, "unresolved"),
        Some(values) if values.len() == 1 => (Some(values[0].clone()), "resolved"),
        Some(_) => (None, "ambiguous"),
    }
}

fn export_sbe(
    capture_id: &str,
    sbe_path: &Path,
    sbe_sha256: &str,
    staging: &Path,
    markets: &MarketMetadata,
) -> anyhow::Result<(u64, u64, u64, u64)> {
    let symbol_map = build_symbol_map(markets);
    let mut messages = BufWriter::new(
        File::create(staging.join("sbe_message.ndjson")).context("creating sbe_message.ndjson")?,
    );
    let mut levels = BufWriter::new(
        File::create(staging.join("sbe_book_level.ndjson"))
            .context("creating sbe_book_level.ndjson")?,
    );
    let mut reader =
        AlignedSbeReader::<OrderbookSchema>::open_auto(sbe_path).context("opening SBE capture")?;
    let mut ordinal = 0u64;
    let mut message_rows = 0u64;
    let mut level_rows = 0u64;
    let mut unknown_rows = 0u64;
    while let Some(message) = reader.read_msg().context("reading SBE message")? {
        ordinal = ordinal.checked_add(1).context("SBE ordinal overflow")?;
        match message {
            AnyMessage::Snapshot(decoder) => {
                let snapshot =
                    SnapshotDomain::try_from_decoder(decoder).context("decoding SBE snapshot")?;
                let (native_symbol, mapping_status) = resolve_symbol(&symbol_map, &snapshot.symbol);
                let bid_count = u32::try_from(snapshot.bids.len())?;
                let ask_count = u32::try_from(snapshot.asks.len())?;
                let row = SbeMessageRow {
                    capture_id: capture_id.to_owned(),
                    sbe_file_sha256: sbe_sha256.to_owned(),
                    message_ordinal: ordinal,
                    message_kind: "snapshot".to_owned(),
                    schema_id: crate::util::dtos::orderbook::SnapshotSchema::SCHEMA_ID,
                    schema_version: crate::util::dtos::orderbook::SnapshotSchema::SCHEMA_VERSION,
                    serialization_symbol: snapshot.symbol.clone(),
                    native_symbol,
                    mapping_status: mapping_status.to_owned(),
                    received_at_us: snapshot.received_at.timestamp_micros(),
                    exchange_timestamp_us: snapshot.exchange_timestamp.timestamp_micros(),
                    date: date_days(snapshot.received_at.timestamp_micros()),
                    side: None,
                    price_mantissa_sbe: None,
                    quantity_mantissa_sbe: None,
                    quantity_semantics: "snapshot_absolute".to_owned(),
                    snapshot_bid_levels: bid_count,
                    snapshot_ask_levels: ask_count,
                };
                write_row(&mut messages, &row)?;
                message_rows = message_rows.checked_add(1).context("SBE row overflow")?;
                for (index, entry) in snapshot.bids.iter().enumerate() {
                    let level = SbeBookLevelRow {
                        capture_id: capture_id.to_owned(),
                        sbe_file_sha256: sbe_sha256.to_owned(),
                        message_ordinal: ordinal,
                        serialization_symbol: snapshot.symbol.clone(),
                        side: "bid".to_owned(),
                        level_ordinal: u32::try_from(index)?,
                        price_mantissa_sbe: entry.price_level,
                        quantity_mantissa_sbe: entry.quantity,
                        received_at_us: snapshot.received_at.timestamp_micros(),
                        date: date_days(snapshot.received_at.timestamp_micros()),
                    };
                    write_row(&mut levels, &level)?;
                    level_rows = level_rows.checked_add(1).context("level row overflow")?;
                }
                for (index, entry) in snapshot.asks.iter().enumerate() {
                    let level = SbeBookLevelRow {
                        capture_id: capture_id.to_owned(),
                        sbe_file_sha256: sbe_sha256.to_owned(),
                        message_ordinal: ordinal,
                        serialization_symbol: snapshot.symbol.clone(),
                        side: "ask".to_owned(),
                        level_ordinal: u32::try_from(index)?,
                        price_mantissa_sbe: entry.price_level,
                        quantity_mantissa_sbe: entry.quantity,
                        received_at_us: snapshot.received_at.timestamp_micros(),
                        date: date_days(snapshot.received_at.timestamp_micros()),
                    };
                    write_row(&mut levels, &level)?;
                    level_rows = level_rows.checked_add(1).context("level row overflow")?;
                }
            }
            AnyMessage::PriceLevelUpdate(decoder) => {
                let update = PriceLevelUpdateDomain::try_from_decoder(decoder)
                    .context("decoding SBE price level update")?;
                let (native_symbol, mapping_status) = resolve_symbol(&symbol_map, &update.symbol);
                let side = serde_json::to_value(update.side)
                    .and_then(serde_json::from_value)
                    .unwrap_or_else(|_| "unknown".to_owned());
                let row = SbeMessageRow {
                    capture_id: capture_id.to_owned(),
                    sbe_file_sha256: sbe_sha256.to_owned(),
                    message_ordinal: ordinal,
                    message_kind: "price_level_update".to_owned(),
                    schema_id: crate::util::dtos::orderbook::PriceLevelUpdateSchema::SCHEMA_ID,
                    schema_version:
                        crate::util::dtos::orderbook::PriceLevelUpdateSchema::SCHEMA_VERSION,
                    serialization_symbol: update.symbol.clone(),
                    native_symbol,
                    mapping_status: mapping_status.to_owned(),
                    received_at_us: update.received_at.timestamp_micros(),
                    exchange_timestamp_us: update.exchange_timestamp.timestamp_micros(),
                    date: date_days(update.received_at.timestamp_micros()),
                    side: Some(side),
                    price_mantissa_sbe: Some(update.price_level),
                    quantity_mantissa_sbe: Some(update.quantity),
                    quantity_semantics: "absolute_post_apply".to_owned(),
                    snapshot_bid_levels: 0,
                    snapshot_ask_levels: 0,
                };
                write_row(&mut messages, &row)?;
                message_rows = message_rows.checked_add(1).context("SBE row overflow")?;
            }
            AnyMessage::Unknown { .. } => {
                unknown_rows = unknown_rows
                    .checked_add(1)
                    .context("unknown count overflow")?;
                let row = SbeMessageRow {
                    capture_id: capture_id.to_owned(),
                    sbe_file_sha256: sbe_sha256.to_owned(),
                    message_ordinal: ordinal,
                    message_kind: "unknown".to_owned(),
                    schema_id: 0,
                    schema_version: 0,
                    serialization_symbol: String::new(),
                    native_symbol: None,
                    mapping_status: "unresolved".to_owned(),
                    received_at_us: 0,
                    exchange_timestamp_us: 0,
                    date: 0,
                    side: None,
                    price_mantissa_sbe: None,
                    quantity_mantissa_sbe: None,
                    quantity_semantics: "unsupported".to_owned(),
                    snapshot_bid_levels: 0,
                    snapshot_ask_levels: 0,
                };
                write_row(&mut messages, &row)?;
                message_rows = message_rows.checked_add(1).context("SBE row overflow")?;
            }
        }
    }
    messages.flush().context("flushing SBE messages")?;
    levels.flush().context("flushing SBE levels")?;
    Ok((message_rows, level_rows, unknown_rows, ordinal))
}

#[derive(Debug, Clone, Serialize)]
struct InstrumentCounts {
    instruments: u64,
    ranges: u64,
    bands: u64,
}

fn export_instruments(
    capture_id: &str,
    markets: &MarketMetadata,
    staging: &Path,
) -> anyhow::Result<InstrumentCounts> {
    let mut instruments = BufWriter::new(
        File::create(staging.join("capture_instrument.ndjson"))
            .context("creating capture_instrument.ndjson")?,
    );
    let mut ranges = BufWriter::new(
        File::create(staging.join("capture_price_range.ndjson"))
            .context("creating capture_price_range.ndjson")?,
    );
    let mut bands = BufWriter::new(
        File::create(staging.join("capture_tick_band.ndjson"))
            .context("creating capture_tick_band.ndjson")?,
    );
    let mut counts = InstrumentCounts {
        instruments: 0,
        ranges: 0,
        bands: 0,
    };
    for market in &markets.markets {
        let bytes = serde_json::to_vec(market).context("serializing market metadata")?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let metadata_sha256 = format!("{:x}", hasher.finalize());
        let (price_kind, price_code) = unit_parts(&market.numeric_spec.price_unit);
        let (quantity_kind, quantity_code) = unit_parts(&market.numeric_spec.quantity_unit);
        let (num, den) = match &market.numeric_spec.contract_multiplier {
            Some(value) => {
                let json =
                    serde_json::to_value(value).context("serializing contract multiplier")?;
                (
                    json.get("numerator").and_then(Value::as_i64),
                    json.get("denominator").and_then(Value::as_i64),
                )
            }
            None => (None, None),
        };
        let row = CaptureInstrumentRow {
            capture_id: capture_id.to_owned(),
            exchange: market.exchange.clone(),
            native_symbol: market.native_symbol.clone(),
            symbol: market.symbol.clone(),
            title: market.title.clone(),
            outcome: market.outcome.clone(),
            exchange_market_id: market.exchange_market_id.clone(),
            close_time_us: market.close_time_us,
            tick_size_us: market.tick_size_us,
            price_level_structure: market.price_level_structure.clone(),
            price_scale: market.numeric_spec.price_scale,
            quantity_scale: market.numeric_spec.quantity_scale,
            price_unit_kind: price_kind,
            price_unit_code: price_code,
            quantity_unit_kind: quantity_kind,
            quantity_unit_code: quantity_code,
            quantity_step: market
                .numeric_spec
                .quantity_step
                .and_then(|value| serde_json::to_value(value).ok())
                .and_then(|value| value.as_i64()),
            lot_size: market
                .numeric_spec
                .lot_size
                .and_then(|value| serde_json::to_value(value).ok())
                .and_then(|value| value.as_i64()),
            contract_multiplier_num: num,
            contract_multiplier_den: den,
            metadata_sha256,
        };
        write_row(&mut instruments, &row)?;
        counts.instruments = counts
            .instruments
            .checked_add(1)
            .context("instrument count overflow")?;
        for (ordinal, range) in market.price_ranges.iter().enumerate() {
            let child = CaptureChildRow {
                capture_id: capture_id.to_owned(),
                symbol: market.symbol.clone(),
                ordinal: u32::try_from(ordinal)?,
                start: range.start_us,
                end: range.end_us,
                step: range.step_us,
                origin: 0,
            };
            write_row(&mut ranges, &child)?;
            counts.ranges = counts
                .ranges
                .checked_add(1)
                .context("range count overflow")?;
        }
        for (ordinal, band) in market.numeric_spec.tick_schedule.iter().enumerate() {
            let json = serde_json::to_value(band).context("serializing tick band")?;
            let child = CaptureChildRow {
                capture_id: capture_id.to_owned(),
                symbol: market.symbol.clone(),
                ordinal: u32::try_from(ordinal)?,
                start: json
                    .get("start")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                end: json.get("end").and_then(Value::as_i64).unwrap_or_default(),
                step: json.get("step").and_then(Value::as_i64).unwrap_or_default(),
                origin: json
                    .get("origin")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
            };
            write_row(&mut bands, &child)?;
            counts.bands = counts.bands.checked_add(1).context("band count overflow")?;
        }
    }
    instruments.flush().context("flushing instruments")?;
    ranges.flush().context("flushing ranges")?;
    bands.flush().context("flushing bands")?;
    Ok(counts)
}

/// Result of one staging-only export.
#[derive(Debug, Clone, Serialize)]
pub struct ExportSummary {
    pub export_schema_version: u32,
    pub capture_id: String,
    pub export_id: String,
    pub staging_directory: String,
    pub venue: String,
    pub journal_provenance_status: String,
    pub row_counts: std::collections::BTreeMap<String, u64>,
    pub logical_content_sha256: std::collections::BTreeMap<String, String>,
    pub input_sha256: std::collections::BTreeMap<String, String>,
    pub verification_results: std::collections::BTreeMap<String, bool>,
}

/// Verify one capture and write typed NDJSON rows into a staging directory.
pub fn export_capture(capture_root: &Path, staging: &Path) -> anyhow::Result<ExportSummary> {
    let verification = verify_capture(capture_root)?;
    ensure!(!staging.exists(), "staging directory already exists");
    fs::create_dir_all(staging).context("creating staging directory")?;

    let sbe_path = capture_root.join("orderbook.sbe");
    let markets_path = capture_root.join("markets.json");
    let report_path = capture_root.join("report.json");
    let markets: MarketMetadata =
        serde_json::from_slice(&fs::read(&markets_path)?).context("parsing markets.json")?;
    let report: RunReport =
        serde_json::from_slice(&fs::read(&report_path)?).context("parsing report.json")?;

    let sbe_sha256 = verification
        .artifact_sha256
        .get("orderbook.sbe")
        .cloned()
        .context("missing SBE digest")?;
    let mut journal_writer = JournalRowWriter {
        writer: BufWriter::new(
            File::create(staging.join("journal_observation.ndjson"))
                .context("creating journal_observation.ndjson")?,
        ),
        capture_id: &verification.capture_id,
        run_id: &verification.journal_code_commit,
        segment_hashes: &verification.journal_segment_sha256,
        rows: 0,
    };
    let stream = journal::visit_observations(
        Path::new(&verification.journal_directory),
        &mut journal_writer,
    )?;
    journal_writer
        .writer
        .flush()
        .context("flushing journal rows")?;
    let journal_rows = journal_writer.rows;

    let (sbe_messages, sbe_levels, unknown_messages, sbe_ordinals) = export_sbe(
        &verification.capture_id,
        &sbe_path,
        &sbe_sha256,
        staging,
        &markets,
    )?;
    let instruments = export_instruments(&verification.capture_id, &markets, staging)?;

    let mut run_rows = BufWriter::new(
        File::create(staging.join("capture_run.ndjson")).context("creating capture_run.ndjson")?,
    );
    let duration = report.ended_at_us.saturating_sub(report.started_at_us);
    let run_row = CaptureRunRow {
        capture_id: verification.capture_id.clone(),
        venue: report.exchange.clone(),
        environment: report.environment.clone(),
        started_at_us: report.started_at_us,
        ended_at_us: report.ended_at_us,
        duration_us: duration,
        journal_run_id: stream.run_id.clone(),
        journal_schema_version: 2,
        journal_code_commit: verification.journal_code_commit.clone(),
        journal_provenance_status: verification.journal_provenance_status.clone(),
        collector_version: report.version.clone(),
        sbe_schema_id: 1,
        sbe_schema_version: 2,
        expected_symbols: verification.instrument_count,
        acknowledged_symbols: report.acknowledged_symbols.len(),
        anchored_symbols: verification.instrument_count,
        exchange_snapshots: report.core.exchange_snapshots,
        sbe_messages: verification.sbe_messages,
        journal_records: verification.journal_records,
        capture_verification_status: "pass".to_owned(),
        export_verification_status: if unknown_messages == 0 {
            "pending".to_owned()
        } else {
            "unsupported_messages".to_owned()
        },
        date: date_days(report.started_at_us),
    };
    write_row(&mut run_rows, &run_row)?;
    run_rows.flush().context("flushing capture run")?;

    let files = [
        "journal_observation.ndjson",
        "sbe_message.ndjson",
        "sbe_book_level.ndjson",
        "capture_instrument.ndjson",
        "capture_price_range.ndjson",
        "capture_tick_band.ndjson",
        "capture_run.ndjson",
    ];
    let mut row_counts = std::collections::BTreeMap::new();
    let mut logical_content_sha256 = std::collections::BTreeMap::new();
    row_counts.insert("journal_observation".to_owned(), journal_rows);
    row_counts.insert("sbe_message".to_owned(), sbe_messages);
    row_counts.insert("sbe_book_level".to_owned(), sbe_levels);
    row_counts.insert("capture_instrument".to_owned(), instruments.instruments);
    row_counts.insert("capture_price_range".to_owned(), instruments.ranges);
    row_counts.insert("capture_tick_band".to_owned(), instruments.bands);
    row_counts.insert("capture_run".to_owned(), 1);
    for file in files {
        logical_content_sha256.insert(
            file.trim_end_matches(".ndjson").to_owned(),
            sha256_file(&staging.join(file))?,
        );
    }

    let mut hasher = Sha256::new();
    hash_field(&mut hasher, CAPTURE_ID_DOMAIN);
    hash_field(&mut hasher, verification.capture_id.as_bytes());
    hash_field(&mut hasher, &EXPORT_SCHEMA_VERSION.to_be_bytes());
    for digest in logical_content_sha256.values() {
        hash_field(&mut hasher, digest.as_bytes());
    }
    let export_id = format!("{:x}", hasher.finalize());

    let mut input_sha256 = verification.artifact_sha256.clone();
    input_sha256.insert(
        "journal_manifest.json".to_owned(),
        verification.journal_manifest_sha256.clone(),
    );
    let mut verification_results = std::collections::BTreeMap::new();
    verification_results.insert("journal".to_owned(), verification.journal_complete);
    verification_results.insert("sbe".to_owned(), verification.sbe_valid);
    verification_results.insert("completeness".to_owned(), verification.completeness_valid);
    verification_results.insert("coverage".to_owned(), verification.coverage_valid);
    verification_results.insert("no_unknown_sbe".to_owned(), unknown_messages == 0);
    verification_results.insert(
        "sbe_ordinals_match_rows".to_owned(),
        sbe_ordinals == sbe_messages,
    );
    verification_results.insert(
        "journal_rows_match_stream".to_owned(),
        journal_rows == stream.records,
    );

    let summary = ExportSummary {
        export_schema_version: EXPORT_SCHEMA_VERSION,
        capture_id: verification.capture_id.clone(),
        export_id,
        staging_directory: staging.display().to_string(),
        venue: report.exchange.clone(),
        journal_provenance_status: verification.journal_provenance_status.clone(),
        row_counts,
        logical_content_sha256,
        input_sha256,
        verification_results,
    };
    let bytes = serde_json::to_vec_pretty(&summary).context("serializing export manifest")?;
    fs::write(staging.join("export-manifest.json"), bytes).context("writing export manifest")?;
    Ok(summary)
}

#[cfg(test)]
mod tests;
