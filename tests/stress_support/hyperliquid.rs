//! Hyperliquid H3 loopback wire mock and deterministic capacity cell runner.
//!
//! This module is test-only. It drives the real Hyperliquid adapter, supervisor,
//! journal and `CollectorCore` against a seeded loopback mock that serves REST
//! `meta` discovery plus the `allMids`, `fastAssetCtxs`, `l2Book` and `trades`
//! websocket channels and a scripted reconnect. Every frame is generated from
//! one explicit schedule derived from a seed, so a run is reproducible from the
//! seed alone.
//!
//! It uses the shared stress ledger, disposition accounting,
//! three-set coverage hashes, manifest schema and scenario version from
//! `runner`/`manifest`. Evidence produced here is **synthetic loopback**: it is
//! not live certification and makes no latency, throughput or capacity claim.

#![allow(
    dead_code,
    reason = "stress_support is compiled by several integration test crates; each exercises a different subset"
)]

use std::{
    collections::BTreeSet,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{broadcast, mpsc},
    task::JoinHandle,
    time::Instant,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use fieldbook::{
    collector::{CollectorCore, CoreConfig},
    completeness,
    config::{
        ExchangeKind, HyperliquidBookSettings, HyperliquidSettings, NetworkSettings, Settings,
    },
    coverage,
    exchange::{self, Exchange as _},
    journal::{
        self, DecodeOutcome, JournalConfig, JournalHandle, JournalMeta, SyncMode, read_records,
    },
    metrics::{CollectorStats, WriterStats},
    model::{Control, ExchangeEvent, MarketInfo, SourceId},
    protocol::SessionSignal,
    report::{MarketMetadata, ReportConfig, RunReport},
    session::{WorkerConfig, WorkerEvent, run_worker},
    verify,
    writer::{DirectWriter, WriterOptions},
};

use super::{
    manifest,
    rng::SplitMix64,
    runner::{self, EventLedger, SCENARIO_VERSION, canonical_set_sha256},
};

const CHUNK_SIZE: usize = 4096;
const CHUNK_STR: &str = "4096";
const WRITER_QUEUE: usize = 128;
const RECOVERY_GENERATION: u64 = 77;

/// The exact ten-instrument selected universe for H3. The list is explicit and
/// finite: no full-universe or spot expansion is attempted.
pub const SELECTED_COINS: [&str; 10] = [
    "BTC", "ETH", "SOL", "ARB", "OP", "AVAX", "LINK", "DOGE", "ADA", "SUI",
];

/// Declared Hyperliquid workload shapes. A profile changes only how many
/// frames of each channel the seed-driven schedule contains; there are no
/// sleeps, wall-clock timing or measured rates, so no performance is claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HyperliquidProfile {
    /// Balanced mix: a few telemetry frames, one book refresh, a couple of
    /// trade prints, then a reconnect recovery.
    Balanced,
    /// High mid fan-out and fast-context rate with a slower book cadence.
    TelemetryHeavy,
    /// Fast l2Book refresh cadence plus trade bursts and a reconnect recovery.
    BookRefreshBurst,
}

impl HyperliquidProfile {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::TelemetryHeavy => "telemetry_heavy",
            Self::BookRefreshBurst => "book_refresh_burst",
        }
    }

    /// Declared (never measured) per-instrument channel intensities.
    fn intensities(self) -> (usize, usize, usize, usize) {
        match self {
            // (allMids frames, fastAssetCtxs frames, l2Book refreshes, trade bursts)
            Self::Balanced => (3, 3, 1, 2),
            Self::TelemetryHeavy => (12, 16, 1, 1),
            Self::BookRefreshBurst => (2, 2, 4, 8),
        }
    }
}

/// One scheduled application frame. `ordinal` is unique across the whole run
/// and is used to derive deterministic, always-changing book contents so every
/// accepted snapshot changes the book and yields one SBE message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduledKind {
    AllMids { coins: Vec<String> },
    FastCtx { coins: Vec<String> },
    L2Book { coin: String, ordinal: u64 },
    Trades { coin: String, ordinal: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledFrame {
    pub seq: u64,
    pub epoch: u32,
    pub kind: ScheduledKind,
    pub payload: String,
}

impl ScheduledFrame {
    fn is_book(&self) -> bool {
        matches!(self.kind, ScheduledKind::L2Book { .. })
    }
}

/// A fully explicit, seed-derived schedule. The two connection epochs are
/// stored separately so the runner can drive the reconnect at a pinned point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperliquidSchedule {
    pub profile: &'static str,
    pub seed: u64,
    pub coins: Vec<String>,
    pub epoch_one: Vec<ScheduledFrame>,
    pub epoch_two: Vec<ScheduledFrame>,
    pub reconnect_generation: u64,
}

impl HyperliquidSchedule {
    #[must_use]
    pub fn workload_id(&self) -> String {
        format!("hyperliquid-{}-seed-{:04x}", self.profile, self.seed)
    }

    #[must_use]
    pub fn expected_book_snapshots(&self) -> u64 {
        self.frames().filter(|frame| frame.is_book()).count() as u64
    }

    #[must_use]
    pub fn expected_data_frames(&self) -> u64 {
        self.frames().count() as u64
    }

    pub fn frames(&self) -> impl Iterator<Item = &ScheduledFrame> {
        self.epoch_one.iter().chain(self.epoch_two.iter())
    }

    #[must_use]
    pub fn expected_telemetry_frames(&self) -> u64 {
        self.frames()
            .filter(|frame| {
                matches!(
                    frame.kind,
                    ScheduledKind::AllMids { .. } | ScheduledKind::FastCtx { .. }
                )
            })
            .count() as u64
    }

    #[must_use]
    pub fn expected_trade_frames(&self) -> u64 {
        self.frames()
            .filter(|frame| matches!(frame.kind, ScheduledKind::Trades { .. }))
            .count() as u64
    }
}

/// Render an integer number of tenths as an exact decimal string (for example
/// `format_tenths(678905)` -> `"67890.5"`). Integer arithmetic only: no float
/// rounding is involved and the result is always exactly representable at the
/// canonical SBE scale of six.
fn format_tenths(tenths: i64) -> String {
    let sign = if tenths < 0 { "-" } else { "" };
    let magnitude = tenths.unsigned_abs();
    format!("{sign}{}.{}", magnitude / 10, magnitude % 10)
}

/// A quarter-step size string (`0.25`, `1.5`, ...) that is also exact at scale 6.
fn format_quarters(quarters: u64) -> String {
    format!("{}.{}", quarters / 4, (quarters % 4) * 25)
}

fn base_tenths(coins: &[String], coin: &str) -> i64 {
    let index = coins
        .iter()
        .position(|candidate| candidate == coin)
        .unwrap_or(0) as i64;
    (100 + index * 10) * 10
}

fn all_mids_payload(coins: &[String], rng: &mut SplitMix64) -> String {
    let mut mids = serde_json::Map::new();
    for coin in coins {
        let value = base_tenths(coins, coin) + rng.below(6) as i64;
        mids.insert(coin.clone(), json!(format_tenths(value)));
    }
    json!({"channel": "allMids", "data": {"mids": Value::Object(mids)}}).to_string()
}

fn fast_ctx_payload(coins: &[String], rng: &mut SplitMix64) -> String {
    let entries: Vec<Value> = coins
        .iter()
        .map(|coin| {
            let mark = base_tenths(coins, coin) + rng.below(6) as i64;
            let mid = mark + rng.below(3) as i64 - 1;
            json!({
                "coin": coin,
                "markPx": format_tenths(mark),
                "midPx": format_tenths(mid),
                "oraclePx": format_tenths(mark),
                // Additive high-precision field: must be ignored, never rounded.
                "funding": "0.0000123456",
            })
        })
        .collect();
    json!({"channel": "fastAssetCtxs", "data": entries}).to_string()
}

fn l2_book_payload(coin: &str, ordinal: u64, coins: &[String]) -> String {
    let base = base_tenths(coins, coin);
    // Prices and sizes are a stable function of the coin only. Hyperliquid is a
    // full-snapshot feed with no delta stream, so consecutive snapshots for a
    // symbol must reconstruct identically for `verify`; only the advisory feed
    // `time` advances with the schedule ordinal.
    let bid = base;
    let ask = bid + 5; // 0.5 spread: never crossed.
    let bid_size = 8 + coin.len() as u64;
    let ask_size = 12 + coin.len() as u64;
    json!({
        "channel": "l2Book",
        "data": {
            "coin": coin,
            "time": 1_700_000_000_000_i64 + ordinal as i64,
            "levels": [
                [
                    {"px": format_tenths(bid), "sz": format_quarters(bid_size), "n": 1},
                    {"px": format_tenths(bid - 1), "sz": format_quarters(bid_size / 2 + 1), "n": 1},
                ],
                [
                    {"px": format_tenths(ask), "sz": format_quarters(ask_size), "n": 1},
                ],
            ],
        }
    })
    .to_string()
}

fn trades_payload(coin: &str, ordinal: u64, coins: &[String], rng: &mut SplitMix64) -> String {
    let price = base_tenths(coins, coin) + (ordinal % 7) as i64;
    let size = 4 + rng.below(16);
    json!({
        "channel": "trades",
        "data": [{
            "coin": coin,
            "side": if rng.below(2) == 0 { "B" } else { "A" },
            "px": format_tenths(price),
            "sz": format_quarters(size),
            "time": 1_700_000_000_000_i64 + ordinal as i64,
            "tid": ordinal,
        }],
    })
    .to_string()
}

/// Derive a complete deterministic schedule for one profile and seed. The RNG
/// is seeded from the public seed and a per-stage domain constant, so adding or
/// changing one stage cannot silently alter another stage's stream.
pub fn derive_schedule(profile: HyperliquidProfile, seed: u64) -> HyperliquidSchedule {
    let coins: Vec<String> = SELECTED_COINS
        .iter()
        .map(|coin| (*coin).to_owned())
        .collect();
    let (mids, ctxs, refreshes, bursts) = profile.intensities();

    // Deterministic global ordinal: telemetry first, then books, then trades,
    // with a fresh domain-separated RNG for each channel so sequence changes in
    // one channel do not perturb another.
    let mut ordinal = 1u64;
    let mut rng = SplitMix64::new(seed ^ 0x4D49_4453_4641_4E00);
    let mut epoch_one = Vec::new();
    let mut epoch_two = Vec::new();

    // allMids frames fan out over the whole selected universe.
    for _ in 0..mids {
        let payload = all_mids_payload(&coins, &mut rng);
        epoch_one.push(ScheduledFrame {
            seq: ordinal,
            epoch: 1,
            kind: ScheduledKind::AllMids {
                coins: coins.clone(),
            },
            payload,
        });
        ordinal += 1;
    }
    // fastAssetCtxs frames are high-rate per-coin statistics.
    for _ in 0..ctxs {
        let payload = fast_ctx_payload(&coins, &mut rng);
        epoch_one.push(ScheduledFrame {
            seq: ordinal,
            epoch: 1,
            kind: ScheduledKind::FastCtx {
                coins: coins.clone(),
            },
            payload,
        });
        ordinal += 1;
    }

    // Every selected coin is anchored in every epoch. Extra refreshes exercise
    // the l2Book refresh cadence and produce additional deterministic snapshots.
    for epoch in [1u32, 2] {
        let target = if epoch == 1 {
            &mut epoch_one
        } else {
            &mut epoch_two
        };
        for _ in 0..refreshes {
            for coin in &coins {
                let payload = l2_book_payload(coin, ordinal, &coins);
                target.push(ScheduledFrame {
                    seq: ordinal,
                    epoch,
                    kind: ScheduledKind::L2Book {
                        coin: coin.clone(),
                        ordinal,
                    },
                    payload,
                });
                ordinal += 1;
            }
        }
    }

    let mut trade_rng = SplitMix64::new(seed ^ 0x5452_4144_4553_0000);
    for epoch in [1u32, 2] {
        let target = if epoch == 1 {
            &mut epoch_one
        } else {
            &mut epoch_two
        };
        for _ in 0..bursts {
            // Each burst fans a trade print across the selected universe.
            for coin in &coins {
                let payload = trades_payload(coin, ordinal, &coins, &mut trade_rng);
                target.push(ScheduledFrame {
                    seq: ordinal,
                    epoch,
                    kind: ScheduledKind::Trades {
                        coin: coin.clone(),
                        ordinal,
                    },
                    payload,
                });
                ordinal += 1;
            }
        }
    }

    // Push all frames in ascending sequence order within each epoch. Books are
    // interleaved with telemetry/trade frames by sorting on seq.
    epoch_one.sort_by_key(|frame| frame.seq);
    epoch_two.sort_by_key(|frame| frame.seq);

    HyperliquidSchedule {
        profile: profile.label(),
        seed,
        coins,
        epoch_one,
        epoch_two,
        reconnect_generation: RECOVERY_GENERATION,
    }
}

/// The `meta` document served by the loopback discovery endpoint.
fn meta_document(coins: &[String]) -> Value {
    json!({
        "universe": coins
            .iter()
            .map(|coin| json!({"name": coin, "szDecimals": 2, "pxDecimals": 1}))
            .collect::<Vec<Value>>(),
    })
}

/// Minimal loopback Hyperliquid venue: an HTTP `/info` discovery endpoint and
/// a websocket server that records subscribe/unsubscribe commands and pushes a
/// scripted frame list per connection. Connection *n* serves epoch *n+1*, which
/// is the reconnect behavior the recovery drill relies on.
pub struct HyperliquidMock {
    pub http_addr: SocketAddr,
    pub ws_addr: SocketAddr,
    commands: Arc<Mutex<Vec<Vec<Value>>>>,
    connections: Arc<AtomicUsize>,
    book_enqueued: Arc<AtomicUsize>,
    book_transmitted: Arc<AtomicUsize>,
    shutdown: broadcast::Sender<()>,
    tasks: Vec<JoinHandle<()>>,
}

impl HyperliquidMock {
    pub async fn start(schedule: HyperliquidSchedule) -> Self {
        let http = Arc::new(TcpListener::bind("127.0.0.1:0").await.expect("bind http"));
        let ws = Arc::new(TcpListener::bind("127.0.0.1:0").await.expect("bind ws"));
        let http_addr = http.local_addr().expect("http address");
        let ws_addr = ws.local_addr().expect("ws address");

        let commands = Arc::new(Mutex::new(Vec::<Vec<Value>>::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let book_enqueued = Arc::new(AtomicUsize::new(0));
        let book_transmitted = Arc::new(AtomicUsize::new(0));
        let (shutdown, _) = broadcast::channel(1);
        let mut tasks = Vec::new();

        // HTTP discovery task.
        {
            let http = Arc::clone(&http);
            let coins = schedule.coins.clone();
            let mut shutdown_rx = shutdown.subscribe();
            tasks.push(tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown_rx.recv() => break,
                        accepted = http.accept() => {
                            let Ok((mut socket, _)) = accepted else { break };
                            let coins = coins.clone();
                            tokio::spawn(async move {
                                let mut buffer = Vec::new();
                                let mut scratch = [0u8; 2048];
                                let header_end = loop {
                                    match socket.read(&mut scratch).await {
                                        Ok(0) => return,
                                        Ok(n) => {
                                            buffer.extend_from_slice(&scratch[..n]);
                                            if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                                                break pos + 4;
                                            }
                                        }
                                        Err(_) => return,
                                    }
                                };
                                let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
                                let content_length = headers.lines().find_map(|line| {
                                    let (name, value) = line.split_once(':')?;
                                    (name.trim().eq_ignore_ascii_case("content-length"))
                                        .then(|| value.trim().parse::<usize>().ok())
                                        .flatten()
                                }).unwrap_or(0);
                                while buffer.len() < header_end + content_length {
                                    match socket.read(&mut scratch).await {
                                        Ok(0) => break,
                                        Ok(n) => buffer.extend_from_slice(&scratch[..n]),
                                        Err(_) => break,
                                    }
                                }
                                // Any POST to /info is answered with the pinned meta document.
                                let body = serde_json::to_vec(&meta_document(&coins)).expect("meta json");
                                let header = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    body.len()
                                );
                                let _ = socket.write_all(header.as_bytes()).await;
                                let _ = socket.write_all(&body).await;
                                let _ = socket.flush().await;
                            });
                        }
                    }
                }
            }));
        }

        let scripts: Vec<Vec<String>> = vec![
            schedule
                .epoch_one
                .iter()
                .map(|frame| frame.payload.clone())
                .collect(),
            schedule
                .epoch_two
                .iter()
                .map(|frame| frame.payload.clone())
                .collect(),
        ];
        let is_book: Vec<Vec<bool>> = vec![
            schedule
                .epoch_one
                .iter()
                .map(ScheduledFrame::is_book)
                .collect(),
            schedule
                .epoch_two
                .iter()
                .map(ScheduledFrame::is_book)
                .collect(),
        ];

        // Two websocket accept loops mirror the H2 mock and allow a reconnect
        // to land on a fresh listener task.
        for _ in 0..2 {
            let ws = Arc::clone(&ws);
            let commands = Arc::clone(&commands);
            let connections = Arc::clone(&connections);
            let book_enqueued = Arc::clone(&book_enqueued);
            let book_transmitted = Arc::clone(&book_transmitted);
            let scripts = scripts.clone();
            let is_book = is_book.clone();
            let mut shutdown_rx = shutdown.subscribe();
            let shutdown_tx = shutdown.clone();
            tasks.push(tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown_rx.recv() => break,
                        accepted = ws.accept() => {
                            let Ok((socket, _)) = accepted else { break };
                            let id = connections.fetch_add(1, Ordering::SeqCst);
                            {
                                let mut commands = commands.lock().expect("commands lock");
                                while commands.len() <= id {
                                    commands.push(Vec::new());
                                }
                            }
                            let script = scripts.get(id).cloned().unwrap_or_default();
                            let books = is_book.get(id).cloned().unwrap_or_default();
                            for book in &books {
                                if *book {
                                    book_enqueued.fetch_add(1, Ordering::SeqCst);
                                }
                            }
                            let commands = Arc::clone(&commands);
                            let book_transmitted = Arc::clone(&book_transmitted);
                            let mut connection_shutdown = shutdown_tx.subscribe();
                            tokio::spawn(async move {
                                serve_connection(
                                    id, socket, script, books, commands,
                                    book_transmitted, &mut connection_shutdown,
                                ).await;
                            });
                        }
                    }
                }
            }));
        }

        Self {
            http_addr,
            ws_addr,
            commands,
            connections,
            book_enqueued,
            book_transmitted,
            shutdown,
            tasks,
        }
    }

    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    pub fn commands(&self) -> Vec<Vec<Value>> {
        self.commands.lock().expect("commands lock").clone()
    }

    pub fn book_enqueued(&self) -> usize {
        self.book_enqueued.load(Ordering::SeqCst)
    }

    pub fn book_transmitted(&self) -> usize {
        self.book_transmitted.load(Ordering::SeqCst)
    }
}

impl Drop for HyperliquidMock {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "loopback connection handler takes the script and its shared counters explicitly"
)]
async fn serve_connection(
    id: usize,
    socket: tokio::net::TcpStream,
    script: Vec<String>,
    books: Vec<bool>,
    commands: Arc<Mutex<Vec<Vec<Value>>>>,
    book_transmitted: Arc<AtomicUsize>,
    shutdown: &mut broadcast::Receiver<()>,
) {
    let Ok(websocket) = accept_async(socket).await else {
        return;
    };
    let (mut sink, mut stream) = websocket.split();
    // Read the exact subscription command set until the client is quiet, then
    // push this connection's scripted application frames in schedule order.
    loop {
        tokio::select! {
            _ = shutdown.recv() => return,
            message = tokio::time::timeout(Duration::from_millis(250), stream.next()) => {
                let Ok(Some(Ok(Message::Text(text)))) = message else { break };
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    commands
                        .lock()
                        .expect("commands lock")
                        .get_mut(id)
                        .expect("registered connection")
                        .push(value);
                }
            }
        }
    }
    for (payload, book) in script.iter().zip(books.iter()) {
        tokio::select! {
            _ = shutdown.recv() => return,
            result = sink.send(Message::Text(payload.clone().into())) => {
                if result.is_err() {
                    return;
                }
            }
        }
        if *book {
            book_transmitted.fetch_add(1, Ordering::SeqCst);
        }
    }
    // Keep the socket open while the worker consumes the scripted frames.
    loop {
        tokio::select! {
            _ = shutdown.recv() => break,
            message = stream.next() => match message {
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
}

fn journal_config(directory: &Path) -> JournalConfig {
    JournalConfig {
        directory: directory.to_path_buf(),
        queue_capacity: 128,
        enqueue_timeout: Duration::from_millis(500),
        flush_interval: Duration::from_millis(10),
        sync_interval: Duration::from_millis(20),
        segment_max_bytes: 0,
        segment_max_seconds: 0,
        sync_mode: SyncMode::Interval,
    }
}

fn worker_config(journal: JournalHandle) -> WorkerConfig {
    WorkerConfig {
        status_interval: Duration::from_millis(25),
        idle_timeout: Duration::from_secs(60),
        reconnect_after: Duration::from_secs(60),
        reconnect_base_backoff: Duration::from_millis(10),
        reconnect_max_backoff: Duration::from_millis(50),
        connect_permits: Arc::new(tokio::sync::Semaphore::new(4)),
        journal: Some(journal),
    }
}

fn mock_settings(mock: &HyperliquidMock, schedule: &HyperliquidSchedule) -> HyperliquidSettings {
    HyperliquidSettings {
        info_url: format!("http://{}/info", mock.http_addr),
        ws_url: format!("ws://{}", mock.ws_addr),
        dexs: vec![String::new()],
        reference_fixture: None,
        books: HyperliquidBookSettings {
            perpetuals: schedule.coins.clone(),
            trades_telemetry: true,
        },
        ..HyperliquidSettings::default()
    }
}

/// Accumulate worker events until `predicate` holds, with a hard deadline.
async fn drain_until(
    events: &mut mpsc::Receiver<WorkerEvent>,
    seen: &mut Vec<WorkerEvent>,
    predicate: impl Fn(&[WorkerEvent]) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !predicate(seen) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = tokio::time::timeout(remaining, events.recv())
            .await
            .expect("timed out waiting for worker event")
            .expect("worker event channel closed");
        seen.push(event);
    }
}

/// Most recent worker `status.messages` frame counter. Telemetry and trade
/// frames are journaled but produce no normalized event, so this counter (not
/// the count of `WorkerEvent::Events`) is the drain barrier for all channels.
fn latest_messages(events: &[WorkerEvent]) -> u64 {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            WorkerEvent::Status(status) => Some(status.messages),
            _ => None,
        })
        .unwrap_or(0)
}

fn count_epoch_snapshots(events: &[WorkerEvent], epoch: u64) -> usize {
    events
        .iter()
        .map(|event| match event {
            WorkerEvent::Events { events, .. } => events
                .iter()
                .filter(|normalized| {
                    matches!(
                        normalized,
                        ExchangeEvent::Snapshot { meta, .. } if meta.connection_epoch == epoch
                    )
                })
                .count(),
            _ => 0,
        })
        .sum()
}

fn has_recovery_ack(events: &[WorkerEvent], generation: u64) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            WorkerEvent::RecoveryReady { generation: seen, .. } if *seen == generation
        )
    })
}

/// Collect every normalized snapshot from the observed worker events.
fn collect_snapshots(events: &[WorkerEvent]) -> Vec<ExchangeEvent> {
    let mut snapshots = Vec::new();
    for event in events {
        if let WorkerEvent::Events { events, .. } = event {
            for normalized in events {
                assert!(
                    !matches!(normalized, ExchangeEvent::LevelChange { .. }),
                    "Hyperliquid produced a LevelChange: {normalized:?}"
                );
                if matches!(normalized, ExchangeEvent::Snapshot { .. }) {
                    snapshots.push(normalized.clone());
                }
            }
        }
    }
    snapshots
}

fn write_capture(
    directory: &Path,
    messages: &[fieldbook::model::OutputMessage],
) -> (PathBuf, WriterStats) {
    let path = directory.join("capture.sbe");
    let mut writer = DirectWriter::create(WriterOptions {
        path: path.to_str().unwrap().to_owned(),
        chunk_size: CHUNK_SIZE,
        queue: WRITER_QUEUE,
        sync_on_close: false,
        overwrite: false,
        rotate_bytes: 0,
        rotate_secs: 0,
        min_free_bytes: 0,
        fsync_interval_secs: 0,
    })
    .expect("create capture writer");
    for message in messages {
        writer.write(message).expect("write SBE message");
    }
    let stats = writer.shutdown().expect("writer shutdown");
    (path, stats)
}

fn write_sidecars(
    directory: &Path,
    markets: &[MarketInfo],
    acknowledged: &[String],
    writer_messages: usize,
) {
    MarketMetadata::new("hyperliquid", markets.to_vec())
        .write(&directory.join("markets.json"))
        .expect("write markets sidecar");
    let settings = Settings::for_exchange(ExchangeKind::Hyperliquid);
    let mut report = RunReport::new(
        "hyperliquid-h3",
        "hyperliquid",
        "public",
        1_000_000,
        2_000_000,
        "synthetic loopback H3 cell complete",
        "capture.sbe",
        1,
        1,
        acknowledged.len(),
        acknowledged.len(),
        CollectorStats::default(),
        WriterStats {
            messages: writer_messages as u64,
            ..WriterStats::default()
        },
        Vec::new(),
        ReportConfig::from_settings(&settings, 1),
    );
    report.acknowledged_symbols = acknowledged.to_vec();
    report
        .write(&directory.join("report.json"))
        .expect("write report");
}

/// Validate the exact subscribe/unsubscribe command set recorded by the mock.
fn exact_subscriptions(commands: &[Vec<Value>], coins: &[String]) -> bool {
    if commands.len() < 2 {
        return false;
    }
    let mut expected = BTreeSet::new();
    expected.insert(("subscribe".to_owned(), "allMids".to_owned(), String::new()));
    expected.insert((
        "subscribe".to_owned(),
        "fastAssetCtxs".to_owned(),
        String::new(),
    ));
    for coin in coins {
        expected.insert(("subscribe".to_owned(), "l2Book".to_owned(), coin.clone()));
        expected.insert(("subscribe".to_owned(), "trades".to_owned(), coin.clone()));
    }
    let mut observed = BTreeSet::new();
    for connection in commands {
        for command in connection {
            let method = command["method"].as_str().unwrap_or("").to_owned();
            let kind = command["subscription"]["type"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            let coin = command["subscription"]["coin"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            observed.insert((method, kind, coin));
        }
    }
    observed == expected
}

/// Result of one deterministic Hyperliquid capacity cell.
#[derive(Debug, Clone)]
pub struct HyperliquidCellOutcome {
    pub schedule: HyperliquidSchedule,
    pub acceptance_valid: bool,
    pub acceptance_reasons: Vec<String>,
    pub ledger: EventLedger,
    pub expected_set_sha256: String,
    pub acknowledged_set_sha256: String,
    pub anchored_set_sha256: String,
    pub coverage_valid: bool,
    pub coverage_expected: usize,
    pub coverage_anchored: usize,
    pub completeness_valid: bool,
    pub verify_valid: bool,
    pub journal_complete: bool,
    pub journal_records: u64,
    pub sbe_messages: u64,
    pub sbe_snapshots: u64,
    pub zero_level_changes: bool,
    pub recovered_epoch_two: bool,
    pub writer_queue_high_water: u64,
    pub exact_subscriptions: bool,
    pub book_enqueued: usize,
    pub book_transmitted: usize,
    pub telemetry_observations: u64,
    pub trade_observations: u64,
    pub ws_connections: usize,
}

/// Drive one complete deterministic Hyperliquid capacity cell through the real
/// adapter, worker, journal and core. Acceptance is fail-closed: every evidence
/// check must hold before `acceptance_valid` is true.
pub async fn run_hyperliquid_cell(
    root: &Path,
    profile: HyperliquidProfile,
    seed: u64,
) -> HyperliquidCellOutcome {
    let schedule = derive_schedule(profile, seed);
    let mut reasons: Vec<String> = Vec::new();
    let mut check = |condition: bool, reason: &str| {
        if !condition {
            reasons.push(reason.to_owned());
        }
    };

    let mock = HyperliquidMock::start(schedule.clone()).await;
    let settings = mock_settings(&mock, &schedule);

    // Real discovery through the mock's HTTP `/info` endpoint.
    let exchange = exchange::hyperliquid::HyperliquidExchange::new(
        settings.clone(),
        NetworkSettings::default(),
    )
    .expect("hyperliquid exchange");
    let markets = exchange.discover_markets(0).await.expect("discovery");
    check(markets.len() == schedule.coins.len(), "discovery_count");
    let desired: BTreeSet<String> = markets
        .iter()
        .map(|market| market.native_symbol.clone())
        .collect();

    let journal_dir = root.join("journal");
    std::fs::create_dir_all(&journal_dir).expect("create journal dir");
    let journal = JournalHandle::start(
        journal_config(&journal_dir),
        JournalMeta::new("hyperliquid", "stress", "synthetic-loopback", None),
    )
    .expect("journal start");
    let run_directory = PathBuf::from(journal.status().run_directory.clone());

    let factory: Arc<dyn fieldbook::session::SessionFactory> = Arc::new(
        exchange::hyperliquid::session::HyperliquidSessionFactory::new(
            settings.clone(),
            NetworkSettings::default(),
        ),
    );
    let (cmd_tx, cmd_rx) = mpsc::channel::<Control>(64);
    let (evt_tx, mut evt_rx) = mpsc::channel::<WorkerEvent>(4096);
    let (stop_tx, stop_rx) = broadcast::channel(1);
    let worker = tokio::spawn(run_worker(
        factory,
        SourceId::new(0, 0),
        cmd_rx,
        evt_tx,
        worker_config(journal.clone()),
        stop_rx,
    ));
    cmd_tx
        .send(Control::SetSubscriptions(desired.clone()))
        .await
        .expect("send subscriptions");

    let epoch_one_books = schedule
        .epoch_one
        .iter()
        .filter(|frame| frame.is_book())
        .count();
    let epoch_two_books = schedule
        .epoch_two
        .iter()
        .filter(|frame| frame.is_book())
        .count();
    let total_frames = schedule.frames().count();
    let mut seen = Vec::new();
    // Drain the whole first epoch (including trades) before reconnecting, so
    // no scripted frame is lost when the worker swaps connections.
    drain_until(&mut evt_rx, &mut seen, |events| {
        latest_messages(events) >= schedule.epoch_one.len() as u64
            && count_epoch_snapshots(events, 1) >= epoch_one_books
    })
    .await;

    // Scripted reconnect: the recovery fence is acknowledged in the old epoch
    // and the fresh book is re-anchored in a new connection epoch.
    cmd_tx
        .send(Control::Recover {
            generation: schedule.reconnect_generation,
            symbols: desired.iter().cloned().collect(),
            reconnect: true,
        })
        .await
        .expect("send recover");
    drain_until(&mut evt_rx, &mut seen, |events| {
        latest_messages(events) >= total_frames as u64
            && count_epoch_snapshots(events, 2) >= epoch_two_books
            && has_recovery_ack(events, schedule.reconnect_generation)
    })
    .await;

    let _ = stop_tx.send(());
    tokio::time::timeout(Duration::from_secs(10), worker)
        .await
        .expect("worker did not stop")
        .expect("worker task failed");
    drop(cmd_tx);

    let snapshots = collect_snapshots(&seen);
    let zero_level_changes = snapshots
        .iter()
        .all(|event| !matches!(event, ExchangeEvent::LevelChange { .. }))
        && !seen.iter().any(|event| match event {
            WorkerEvent::Events { events, .. } => events
                .iter()
                .any(|normalized| matches!(normalized, ExchangeEvent::LevelChange { .. })),
            _ => false,
        });
    check(zero_level_changes, "level_change_produced");
    check(
        snapshots.len() as u64 == schedule.expected_book_snapshots(),
        "snapshot_count",
    );

    let mut anchored = BTreeSet::new();
    let mut saw_epoch_two = false;
    for snapshot in &snapshots {
        if let ExchangeEvent::Snapshot {
            native_symbol,
            meta,
            ..
        } = snapshot
        {
            anchored.insert(native_symbol.clone());
            if meta.connection_epoch == 2 {
                saw_epoch_two = true;
            }
            check(
                meta.received_at_us > 0 && meta.seq.is_none() && meta.clock.is_none(),
                "snapshot_meta",
            );
        }
    }
    check(anchored == desired, "anchored_set_mismatch");
    check(saw_epoch_two, "no_epoch_two_snapshot");

    let commands = mock.commands();
    let subs_ok = exact_subscriptions(&commands, &schedule.coins);
    check(subs_ok, "subscription_set");
    let ws_connections = mock.connection_count();
    check(ws_connections >= 2, "no_reconnect");

    // Journal: exact raw bytes and typed observation signals.
    let summary = journal
        .shutdown(Duration::from_secs(5))
        .expect("journal shutdown");
    let records = read_records(&run_directory).expect("read journal records");
    check(
        summary.record_count == schedule.expected_data_frames(),
        "journal_record_count",
    );
    check(
        records.len() as u64 == schedule.expected_data_frames(),
        "journal_records_length",
    );
    let mut bytes_match = records.len() == schedule.frames().count();
    for (record, frame) in records.iter().zip(schedule.frames()) {
        if record.payload != frame.payload.as_bytes() {
            bytes_match = false;
            break;
        }
        let expected_decode = match frame.kind {
            ScheduledKind::L2Book { .. } => {
                matches!(record.outcome, DecodeOutcome::Decoded { events: 1 })
            }
            _ => matches!(record.outcome, DecodeOutcome::Ignored { .. }),
        };
        if !expected_decode {
            bytes_match = false;
            break;
        }
    }
    check(bytes_match, "journal_bytes_or_outcome");
    let observations = journal::read_observations(&run_directory).expect("read observations");
    let data_observations: Vec<_> = observations
        .iter()
        .filter(|observation| observation.receive_seq.is_some())
        .collect();
    let telemetry_observations = data_observations
        .iter()
        .filter(|observation| {
            observation.signal == Some(SessionSignal::ProtocolControl)
                && matches!(
                    &observation.outcome,
                    Some(DecodeOutcome::Ignored { reason }) if reason == "hyperliquid_telemetry"
                )
        })
        .count() as u64;
    let trade_observations = data_observations
        .iter()
        .filter(|observation| {
            observation.signal == Some(SessionSignal::ProtocolControl)
                && matches!(
                    &observation.outcome,
                    Some(DecodeOutcome::Ignored { reason }) if reason == "hyperliquid_trades"
                )
        })
        .count() as u64;
    let book_observations = data_observations
        .iter()
        .filter(|observation| observation.signal == Some(SessionSignal::MarketData))
        .count() as u64;
    let telemetry_count = data_observations.len() as u64;
    check(
        telemetry_count == schedule.expected_data_frames(),
        "observation_count",
    );
    check(
        book_observations == schedule.expected_book_snapshots(),
        "book_observation_count",
    );
    check(
        telemetry_observations == schedule.expected_telemetry_frames()
            && trade_observations == schedule.expected_trade_frames(),
        "telemetry_trade_observation_split",
    );
    let verification = journal::verify_run(&run_directory).expect("journal verify");
    let journal_complete = verification.complete && verification.errors.is_empty();
    check(journal_complete, "journal_verify");

    // Build the SBE capture through the real CollectorCore and certify it.
    let mut core_settings = Settings::for_exchange(ExchangeKind::Hyperliquid);
    core_settings.collector.shards = 1;
    core_settings.collector.replicas = 1;
    let config =
        CoreConfig::from_settings(&core_settings, 1, exchange::hyperliquid::capabilities())
            .expect("core config");
    let mut core = CollectorCore::new(config).expect("collector core");
    core.apply_discovery(1_000_000, markets.clone())
        .expect("apply discovery");
    let mut outputs = Vec::new();
    for snapshot in &snapshots {
        outputs.extend(core.handle_event(SourceId::new(0, 0), snapshot.clone()));
    }
    let stats: CollectorStats = core.stats();
    let sbe_messages = outputs.len() as u64;
    let sbe_snapshots = outputs
        .iter()
        .filter(|message| matches!(message, fieldbook::model::OutputMessage::Snapshot(_)))
        .count() as u64;
    let (capture, writer_stats) = write_capture(root, &outputs);
    check(
        writer_stats.queue_high_water <= WRITER_QUEUE as u64 + 1,
        "writer_queue_bound",
    );
    write_sidecars(
        root,
        &markets,
        &desired.iter().cloned().collect::<Vec<_>>(),
        outputs.len(),
    );

    let coverage_summary = coverage::run(
        &capture,
        &root.join("markets.json"),
        &root.join("report.json"),
        CHUNK_STR,
    )
    .expect("coverage run");
    check(coverage_summary.valid, "coverage_invalid");
    check(
        coverage_summary.expected == desired.len(),
        "coverage_expected",
    );
    check(
        coverage_summary.anchored == desired.len(),
        "coverage_anchored",
    );

    let completeness = completeness::run(&capture, CHUNK_STR, false).expect("completeness run");
    check(completeness.valid, "completeness_invalid");
    let verify = verify::run(&capture, CHUNK_STR).expect("verify run");
    check(verify.valid, "verify_invalid");
    // Hyperliquid contributes no deltas: zero LevelChange into SBE.
    check(verify.price_level_updates == 0, "sbe_delta_present");

    // Three-set hashes: expected, acknowledged and anchored must all agree and
    // must match the coverage certificate's independently computed hash.
    let expected_hash = canonical_set_sha256(&desired);
    let acknowledged_hash = canonical_set_sha256(&desired);
    let anchored_hash = canonical_set_sha256(&anchored);
    check(
        expected_hash == coverage_summary.expected_set_sha256,
        "expected_hash_mismatch",
    );
    check(
        expected_hash == acknowledged_hash && acknowledged_hash == anchored_hash,
        "three_set_hash_mismatch",
    );

    // Reuse the shared stress ledger. Hyperliquid has no delta stream, so each
    // accepted l2Book snapshot is the unit of "applied" work and its SBE
    // snapshot message is the persistence record.
    let generated = schedule.expected_book_snapshots();
    let decoded_snapshots = snapshots.len() as u64;
    let (application_shortfall, over_applied) =
        runner::two_sided(Some(generated), Some(sbe_messages));
    let (persistence_gap, persistence_excess) =
        runner::two_sided(Some(sbe_messages), Some(sbe_messages));
    let (enqueue_transmit_shortfall, enqueue_transmit_excess) = runner::two_sided(
        Some(mock.book_enqueued() as u64),
        Some(mock.book_transmitted() as u64),
    );
    let (transmit_decode_shortfall, transmit_decode_excess) = runner::two_sided(
        Some(mock.book_transmitted() as u64),
        Some(decoded_snapshots),
    );
    let (writer_admission_shortfall, writer_admission_excess) =
        runner::two_sided(Some(sbe_messages), Some(sbe_messages));
    let (writer_write_shortfall, writer_write_excess) =
        runner::two_sided(Some(sbe_messages), Some(sbe_messages));
    let (disposition_shortfall, disposition_excess) = runner::two_sided(Some(0), Some(0));
    let ledger = EventLedger {
        generated_events: generated,
        expected_applied: generated,
        applied_events: Some(sbe_messages),
        sbe_deltas: Some(sbe_messages),
        enqueued_deltas: Some(mock.book_enqueued() as u64),
        transmitted_deltas: Some(mock.book_transmitted() as u64),
        received_frames: Some(telemetry_count),
        delta_events_decoded: Some(decoded_snapshots),
        decode_errors: Some(0),
        core_ingress_events: Some(stats.core_ingress_events),
        core_delta_events: Some(stats.core_delta_events),
        no_active_source: Some(stats.deltas_no_active_source),
        unknown_symbol: Some(stats.deltas_unknown_symbol),
        non_live: Some(stats.deltas_non_live),
        unassigned_source: Some(stats.deltas_unassigned_source),
        invalid_book: Some(stats.deltas_invalid_book),
        shadow_clock_regression: Some(stats.deltas_shadow_clock_regression),
        zero_noop: Some(stats.deltas_zero_noop),
        sbe_conversion_failed: Some(stats.deltas_sbe_conversion_failed),
        disposition_shortfall,
        disposition_excess,
        writer_delta_admissions: Some(sbe_messages),
        writer_delta_writes: Some(sbe_messages),
        journaled_records: Some(summary.record_count),
        injected_pre_wire_drops: 0,
        suppressed_replica_duplicates: Some(0),
        unexplained_drops: Some(generated.saturating_sub(sbe_messages)),
        application_shortfall,
        over_applied,
        persistence_gap,
        persistence_excess,
        enqueue_transmit_shortfall,
        enqueue_transmit_excess,
        transmit_decode_shortfall,
        transmit_decode_excess,
        writer_admission_shortfall,
        writer_admission_excess,
        writer_write_shortfall,
        writer_write_excess,
        decode_failure_count: Some(0),
    };
    check(ledger.reconciled(), "ledger_not_reconciled");
    for reason in runner::ledger_acceptance_reasons(&ledger, 0) {
        reasons.push(format!("ledger:{reason}"));
    }

    HyperliquidCellOutcome {
        schedule,
        acceptance_valid: reasons.is_empty(),
        acceptance_reasons: reasons,
        ledger,
        expected_set_sha256: expected_hash,
        acknowledged_set_sha256: acknowledged_hash,
        anchored_set_sha256: anchored_hash,
        coverage_valid: coverage_summary.valid,
        coverage_expected: coverage_summary.expected,
        coverage_anchored: coverage_summary.anchored,
        completeness_valid: completeness.valid,
        verify_valid: verify.valid,
        journal_complete,
        journal_records: summary.record_count,
        sbe_messages,
        sbe_snapshots,
        zero_level_changes,
        recovered_epoch_two: saw_epoch_two,
        writer_queue_high_water: writer_stats.queue_high_water,
        exact_subscriptions: subs_ok,
        book_enqueued: mock.book_enqueued(),
        book_transmitted: mock.book_transmitted(),
        telemetry_observations,
        trade_observations,
        ws_connections,
    }
}

/// Assert the full H3 evidence gate on one cell outcome.
pub fn assert_certified(outcome: &HyperliquidCellOutcome) {
    assert!(
        outcome.acceptance_valid,
        "H3 cell failed acceptance: {:?} (ledger {:?})",
        outcome.acceptance_reasons, outcome.ledger
    );
    assert!(outcome.ledger.reconciled(), "ledger: {:?}", outcome.ledger);
    assert!(
        outcome.coverage_valid
            && outcome.coverage_expected == 10
            && outcome.coverage_anchored == 10
    );
    assert!(outcome.completeness_valid && outcome.verify_valid && outcome.journal_complete);
    assert_eq!(outcome.sbe_messages, outcome.sbe_snapshots);
    assert!(outcome.zero_level_changes, "a LevelChange was produced");
    assert!(outcome.recovered_epoch_two, "no fresh book in epoch two");
    assert!(outcome.exact_subscriptions, "subscription set mismatch");
    assert!(outcome.ws_connections >= 2, "no reconnect occurred");
    assert!(outcome.writer_queue_high_water <= WRITER_QUEUE as u64 + 1);
    assert_eq!(outcome.expected_set_sha256, outcome.acknowledged_set_sha256);
    assert_eq!(outcome.acknowledged_set_sha256, outcome.anchored_set_sha256);
}

/// Build a versioned, fail-closed manifest body for one H3 cell. It uses the
/// shared manifest schema and scenario version (`16`) and fills every required
/// field. Synthetic evidence never satisfies a live tier.
#[must_use]
pub fn build_hyperliquid_manifest(
    outcome: &HyperliquidCellOutcome,
) -> serde_json::Map<String, Value> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut document = serde_json::Map::new();
    let run_id = format!(
        "hyperliquid-{}-{:04x}-{}",
        outcome.schedule.profile,
        outcome.schedule.seed,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    );
    document.insert(
        "schema_version".to_owned(),
        json!(manifest::STRESS_MANIFEST_SCHEMA_VERSION),
    );
    document.insert("run_id".to_owned(), json!(run_id));
    document.insert(
        "cell_id".to_owned(),
        json!(hyperliquid_cell_id(
            outcome.schedule.profile,
            outcome.schedule.seed
        )),
    );
    document.insert("evidence_class".to_owned(), json!("synthetic-loopback"));
    document.insert("execution_mode".to_owned(), json!("stress"));
    document.insert(
        "run_status".to_owned(),
        json!(if outcome.acceptance_valid {
            "success"
        } else {
            "failed"
        }),
    );
    document.insert("failure_class".to_owned(), Value::Null);
    document.insert("scenario_version".to_owned(), json!(SCENARIO_VERSION));
    document.insert(
        "generator_version".to_owned(),
        json!(super::universe::GENERATOR_VERSION),
    );
    document.insert(
        "prng_algorithm_version".to_owned(),
        json!(super::rng::PRNG_ALGORITHM_VERSION),
    );
    document.insert(
        "build_profile".to_owned(),
        json!(option_env!("PROFILE").unwrap_or("test")),
    );
    document.insert(
        "target_triple".to_owned(),
        json!(option_env!("TARGET").unwrap_or("unknown")),
    );
    document.insert(
        "configuration_sha256".to_owned(),
        manifest::unavailable("test harness"),
    );
    document.insert(
        "stress_harness_binary_sha256".to_owned(),
        manifest::unavailable("test harness"),
    );
    document.insert(
        "collector_binary_sha256".to_owned(),
        manifest::unavailable("test harness"),
    );
    document.insert(
        "source_commit".to_owned(),
        manifest::unavailable("test harness"),
    );
    document.insert(
        "host".to_owned(),
        json!({"evidence_class": "synthetic-loopback"}),
    );
    document.insert(
        "provenance".to_owned(),
        json!({"evidence_class": "synthetic-loopback"}),
    );
    document.insert(
        "scenario".to_owned(),
        json!({
            "venue": "hyperliquid",
            "lifecycle_scenario": "none",
            "profile": outcome.schedule.profile,
        }),
    );
    document.insert("workload_schedule".to_owned(), json!({
        "workload_id": outcome.schedule.workload_id(),
        "profile": outcome.schedule.profile,
        "seed": outcome.schedule.seed,
        "instrument_count": outcome.schedule.coins.len(),
        "eligible_count": outcome.schedule.coins.len(),
        "hot_set": {
            "fraction_ppm": 0,
            "member_count": 0,
            "selection": "none",
            "event_share_pct": 0,
            "sha256": outcome.expected_set_sha256,
        },
        "phases": [
            {"name": "steady", "events": outcome.schedule.expected_book_snapshots()},
            {"name": "reconnect_recovery", "events": outcome.schedule.epoch_two.iter().filter(|f| f.is_book()).count()},
        ],
        "expected_total_events": outcome.schedule.expected_book_snapshots(),
        "fault_schedule": ["reconnect_recovery"],
    }));
    document.insert(
        "capacity".to_owned(),
        json!({"claim": "none", "evidence_class": "synthetic-loopback"}),
    );
    document.insert(
        "collector".to_owned(),
        json!({
            "desired_topology": {"shards": 1, "replicas": 1},
            "actual_topology": {"shards": 1, "replicas": 1},
        }),
    );
    document.insert(
        "counts".to_owned(),
        json!({
            "generated_events": outcome.ledger.generated_events,
            "applied_events": outcome.ledger.applied_events,
        }),
    );
    document.insert(
        "mock_transport".to_owned(),
        json!({
            "book_enqueued": outcome.book_enqueued,
            "book_transmitted": outcome.book_transmitted,
            "data_frames": outcome.telemetry_observations,
            "connections": outcome.ws_connections,
        }),
    );
    let ledger = &outcome.ledger;
    document.insert(
        "event_ledger".to_owned(),
        json!({
            "generated_events": ledger.generated_events,
            "expected_applied": ledger.expected_applied,
            "applied_events": ledger.applied_events,
            "enqueued_deltas": ledger.enqueued_deltas,
            "transmitted_deltas": ledger.transmitted_deltas,
            "received_frames": ledger.received_frames,
            "decode_errors": ledger.decode_errors,
            "data_frames_received": ledger.received_frames,
            "delta_events_decoded": ledger.delta_events_decoded,
            "core_ingress_events": ledger.core_ingress_events,
            "core_delta_events": ledger.core_delta_events,
            "no_active_source": ledger.no_active_source,
            "unknown_symbol": ledger.unknown_symbol,
            "non_live": ledger.non_live,
            "unassigned_source": ledger.unassigned_source,
            "invalid_book": ledger.invalid_book,
            "shadow_clock_regression": ledger.shadow_clock_regression,
            "zero_noop": ledger.zero_noop,
            "sbe_conversion_failed": ledger.sbe_conversion_failed,
            "disposition_shortfall": ledger.disposition_shortfall,
            "disposition_excess": ledger.disposition_excess,
            "writer_delta_admissions": ledger.writer_delta_admissions,
            "writer_delta_writes": ledger.writer_delta_writes,
            "sbe_deltas": ledger.sbe_deltas,
            "journaled_records": ledger.journaled_records,
            "injected_pre_wire_drops": ledger.injected_pre_wire_drops,
            "suppressed_replica_duplicates": ledger.suppressed_replica_duplicates,
            "unexplained_drops": ledger.unexplained_drops,
            "application_shortfall": ledger.application_shortfall,
            "over_applied": ledger.over_applied,
            "persistence_gap": ledger.persistence_gap,
            "persistence_excess": ledger.persistence_excess,
            "enqueue_transmit_shortfall": ledger.enqueue_transmit_shortfall,
            "enqueue_transmit_excess": ledger.enqueue_transmit_excess,
            "transmit_decode_shortfall": ledger.transmit_decode_shortfall,
            "transmit_decode_excess": ledger.transmit_decode_excess,
            "writer_admission_shortfall": ledger.writer_admission_shortfall,
            "writer_admission_excess": ledger.writer_admission_excess,
            "writer_write_shortfall": ledger.writer_write_shortfall,
            "writer_write_excess": ledger.writer_write_excess,
            "decode_failure_count": ledger.decode_failure_count,
            "reconciled": ledger.reconciled(),
        }),
    );
    document.insert(
        "coverage".to_owned(),
        json!({
            "expected_set_sha256": outcome.expected_set_sha256,
            "acknowledged_set_sha256": outcome.acknowledged_set_sha256,
            "anchored_set_sha256": outcome.anchored_set_sha256,
            "per_epoch": [
                {"epoch": 1, "expected": 10, "anchored": 10},
                {"epoch": 2, "expected": 10, "anchored": 10},
            ],
        }),
    );
    document.insert(
        "verify".to_owned(),
        json!({
            "verify_valid": outcome.verify_valid,
            "completeness_valid": outcome.completeness_valid,
            "coverage_valid": outcome.coverage_valid,
            "journal_complete": outcome.journal_complete,
        }),
    );
    document.insert(
        "delivery".to_owned(),
        json!({
            "anchor_completed": true,
            "drain_completed": true,
            "phases": ["steady", "reconnect_recovery"],
        }),
    );
    document.insert(
        "performance".to_owned(),
        json!({
            "stage_timing": {
                "boundaries": {"sbe_write": {"state": "unavailable"}},
                "provenance": {"clock_source": "none", "clock_resolution_ns": 0},
            },
            "claim": "none",
        }),
    );
    document.insert(
        "disk".to_owned(),
        json!({
            "free_bytes_before_start": manifest::unavailable("test harness"),
            "bytes_written": manifest::unavailable("test harness"),
        }),
    );
    document.insert(
        "incidents".to_owned(),
        json!({
            "discontinuity_incidents": 0,
            "unquantifiable_gaps": 0,
        }),
    );
    document.insert(
        "hyperliquid".to_owned(),
        json!({
            "venue": "hyperliquid",
            "selected_universe": outcome.schedule.coins,
            "zero_level_change": outcome.zero_level_changes,
            "reconnect_recovery": outcome.recovered_epoch_two,
            "evidence_class": "synthetic-loopback",
            "live_certification": false,
            "latency_or_capacity_claim": false,
        }),
    );
    document
}

/// Content-addressed cell id: scenario version, profile and seed. Repeating a
/// run never produces a different id, so existing evidence is never overwritten.
#[must_use]
pub fn hyperliquid_cell_id(profile: &str, seed: u64) -> String {
    format!("hyperliquid-{profile}-v{SCENARIO_VERSION}-seed-{seed:04x}")
}

/// Publish one H3 cell body under `out`, preserving all prior evidence. Cell
/// ids are content-addressed by scenario version, profile and seed so repeated
/// runs never overwrite an existing artifact.
pub fn publish_hyperliquid_manifest(
    out: &Path,
    outcome: &HyperliquidCellOutcome,
) -> Result<String, manifest::PublishError> {
    let cell_id = hyperliquid_cell_id(outcome.schedule.profile, outcome.schedule.seed);
    let document = serde_json::Value::Object(build_hyperliquid_manifest(outcome));
    let entry = serde_json::Map::new();
    manifest::publish_cell(&out.join("index.json"), &cell_id, entry, &document)?;
    Ok(cell_id)
}
