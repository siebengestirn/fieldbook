//! H2 Hyperliquid selected-book evidence against a loopback websocket mock.
//!
//! These tests drive the real supervised worker and a real v2 journal. They
//! prove: exact selected coverage (`expected == acknowledged == anchored`),
//! valid `completeness`/`verify`/`journal verify`, zero `LevelChange` from the
//! venue, recovery re-anchoring in a new connection epoch, and trade telemetry
//! that produces no anchors. Nothing here is a live-venue, latency or capacity
//! claim.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    reason = "integration tests drive failure paths with unwrap/expect/panic and print diagnostics"
)]
#![allow(
    clippy::allow_attributes,
    reason = "test crates relax the production lint set wholesale; `expect` cannot express lints that may not fire"
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

use fieldbook::{
    collector::{CollectorCore, CoreConfig},
    completeness,
    config::{ExchangeKind, HyperliquidSettings, NetworkSettings, Settings},
    coverage, exchange,
    journal::{
        self, DecodeOutcome, JournalConfig, JournalHandle, JournalMeta, SyncMode, read_records,
    },
    metrics::{CollectorStats, WriterStats},
    model::{Control, ExchangeEvent, MarketInfo, NumericSpec, PriceUnit, QuantityUnit, SourceId},
    protocol::SessionSignal,
    report::{MarketMetadata, ReportConfig, RunReport},
    session::{SessionFactory, WorkerConfig, WorkerEvent, run_worker},
    verify,
    writer::{DirectWriter, WriterOptions},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
    net::TcpListener,
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const CHUNK_SIZE: usize = 4096;
const CHUNK_STR: &str = "4096";
const SELECTED_COINS: [&str; 10] = [
    "BTC", "ETH", "SOL", "ARB", "OP", "AVAX", "LINK", "DOGE", "ADA", "SUI",
];
const TRADES: &str = include_str!("../fixtures/hyperliquid/trades.json");

fn native(coin: &str) -> String {
    exchange::hyperliquid::books::native_symbol("", coin)
}

fn book_frame(coin: &str, time_ms: i64, bids: &[(&str, &str)], asks: &[(&str, &str)]) -> String {
    let to_levels = |levels: &[(&str, &str)]| -> Vec<Value> {
        levels
            .iter()
            .map(|(px, sz)| json!({"px": px, "sz": sz, "n": 1}))
            .collect()
    };
    json!({
        "channel": "l2Book",
        "data": {
            "coin": coin,
            "time": time_ms,
            "levels": [to_levels(bids), to_levels(asks)],
        }
    })
    .to_string()
}

/// Ten selected books spanning two-sided, one-sided, empty and deep snapshots.
/// The exact bytes are retained by the journal, so the mock sends the same
/// strings the test later asserts against.
fn selected_book_frames() -> Vec<String> {
    SELECTED_COINS
        .iter()
        .enumerate()
        .map(|(index, coin)| {
            let time = 1_700_000_000_000_i64 + index as i64;
            match index {
                0 => book_frame(
                    coin,
                    time,
                    &[("67890.0", "1.5"), ("67889.5", "0.25")],
                    &[("67891.0", "2.0")],
                ),
                1 => book_frame(coin, time, &[("3500.25", "4")], &[]),
                2 => book_frame(coin, time, &[], &[]),
                _ => book_frame(
                    coin,
                    time,
                    &[("1.1", "10.5"), ("1.09", "0.5"), ("1.08", "0.000001")],
                    &[("1.11", "7"), ("1.12", "8.25"), ("1.13", "0.000001")],
                ),
            }
        })
        .collect()
}

/// A minimal loopback Hyperliquid websocket. Each connection records the exact
/// subscribe/unsubscribe commands it received, then pushes a scripted list of
/// application frames. No HTTP server is needed: H0 discovery is fixture-driven.
struct HyperliquidMock {
    addr: SocketAddr,
    commands: Arc<Mutex<Vec<Vec<Value>>>>,
    connections: Arc<AtomicUsize>,
    shutdown: broadcast::Sender<()>,
    tasks: Vec<JoinHandle<()>>,
}

impl HyperliquidMock {
    async fn start(scripts: Vec<Vec<String>>) -> Self {
        let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.expect("bind ws"));
        let addr = listener.local_addr().expect("ws address");
        let commands = Arc::new(Mutex::new(Vec::<Vec<Value>>::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let scripts = Arc::new(Mutex::new(scripts));
        let (shutdown, _) = broadcast::channel(1);
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let listener = Arc::clone(&listener);
            let commands = Arc::clone(&commands);
            let connections = Arc::clone(&connections);
            let scripts = Arc::clone(&scripts);
            let mut shutdown_rx = shutdown.subscribe();
            let shutdown_tx = shutdown.clone();
            tasks.push(tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown_rx.recv() => break,
                        accepted = listener.accept() => {
                            let Ok((socket, _)) = accepted else { break };
                            let id = connections.fetch_add(1, Ordering::SeqCst);
                            {
                                let mut commands = commands.lock().expect("commands lock");
                                while commands.len() <= id {
                                    commands.push(Vec::new());
                                }
                            }
                            let script = scripts
                                .lock()
                                .expect("scripts lock")
                                .get(id)
                                .cloned()
                                .unwrap_or_default();
                            let commands = Arc::clone(&commands);
                            let mut connection_shutdown = shutdown_tx.subscribe();
                            tokio::spawn(async move {
                                serve_connection(id, socket, script, commands, &mut connection_shutdown).await;
                            });
                        }
                    }
                }
            }));
        }
        Self {
            addr,
            commands,
            connections,
            shutdown,
            tasks,
        }
    }

    fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    fn commands_for(&self, id: usize) -> Vec<Value> {
        self.commands
            .lock()
            .expect("commands lock")
            .get(id)
            .cloned()
            .unwrap_or_default()
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

async fn serve_connection(
    id: usize,
    socket: tokio::net::TcpStream,
    script: Vec<String>,
    commands: Arc<Mutex<Vec<Vec<Value>>>>,
    shutdown: &mut broadcast::Receiver<()>,
) {
    let Ok(websocket) = accept_async(socket).await else {
        return;
    };
    let (mut sink, mut stream) = websocket.split();
    // Read all subscription commands until the client goes quiet, then push
    // this connection's scripted application frames.
    loop {
        tokio::select! {
            _ = shutdown.recv() => return,
            message = tokio::time::timeout(Duration::from_millis(300), stream.next()) => {
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
    for frame in script {
        tokio::select! {
            _ = shutdown.recv() => return,
            result = sink.send(Message::Text(frame.into())) => {
                if result.is_err() {
                    return;
                }
            }
        }
    }
    // Keep the socket open while the worker reads the scripted frames.
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
        queue_capacity: 64,
        enqueue_timeout: Duration::from_millis(250),
        flush_interval: Duration::from_millis(10),
        sync_interval: Duration::from_millis(20),
        segment_max_bytes: 0,
        segment_max_seconds: 0,
        sync_mode: SyncMode::Interval,
    }
}

fn worker_config(journal: JournalHandle) -> WorkerConfig {
    WorkerConfig {
        status_interval: Duration::from_millis(50),
        idle_timeout: Duration::from_secs(60),
        reconnect_after: Duration::from_secs(60),
        reconnect_base_backoff: Duration::from_millis(10),
        reconnect_max_backoff: Duration::from_millis(50),
        connect_permits: Arc::new(tokio::sync::Semaphore::new(4)),
        journal: Some(journal),
    }
}

fn selected_settings(addr: SocketAddr, coins: &[&str]) -> HyperliquidSettings {
    HyperliquidSettings {
        ws_url: format!("ws://{addr}"),
        books: fieldbook::config::HyperliquidBookSettings {
            perpetuals: coins.iter().map(|coin| (*coin).to_owned()).collect(),
            trades_telemetry: true,
        },
        ..HyperliquidSettings::default()
    }
}

async fn recv_until(
    events: &mut mpsc::Receiver<WorkerEvent>,
    mut done: impl FnMut(&[WorkerEvent]) -> bool,
) -> Vec<WorkerEvent> {
    let mut seen = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while !done(&seen) {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = tokio::time::timeout(remaining, events.recv())
            .await
            .expect("timed out waiting for worker event")
            .expect("worker event channel closed");
        seen.push(event);
    }
    seen
}

fn market(coin: &str) -> MarketInfo {
    MarketInfo {
        exchange: "hyperliquid".to_owned(),
        native_symbol: native(coin),
        symbol: native(coin),
        title: Some(coin.to_owned()),
        outcome: None,
        exchange_market_id: None,
        close_time_us: None,
        tick_size_us: None,
        price_ranges: Vec::new(),
        price_level_structure: None,
        numeric_spec: NumericSpec {
            price_unit: PriceUnit::Currency("USDC".to_owned()),
            quantity_unit: QuantityUnit::Contracts,
            ..NumericSpec::default()
        },
    }
}

fn write_capture(directory: &Path, messages: Vec<fieldbook::model::OutputMessage>) -> PathBuf {
    let path = directory.join("capture.sbe");
    let mut writer = DirectWriter::create(WriterOptions {
        path: path.to_str().unwrap().to_owned(),
        chunk_size: CHUNK_SIZE,
        queue: 64,
        sync_on_close: false,
        overwrite: false,
        rotate_bytes: 0,
        rotate_secs: 0,
        min_free_bytes: 0,
        fsync_interval_secs: 0,
    })
    .unwrap();
    for message in messages {
        writer.write(&message).unwrap();
    }
    writer.shutdown().unwrap();
    path
}

fn write_sidecars(
    directory: &Path,
    markets: &[MarketInfo],
    acknowledged: &[String],
    writer_messages: usize,
) {
    MarketMetadata::new("hyperliquid", markets.to_vec())
        .write(&directory.join("markets.json"))
        .unwrap();
    let settings = Settings::for_exchange(ExchangeKind::Hyperliquid);
    let mut report = RunReport::new(
        "test",
        "hyperliquid",
        "public",
        1_000_000,
        2_000_000,
        "test scenario complete",
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
    report.write(&directory.join("report.json")).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_l2book_anchors_exact_coverage_with_valid_evidence() {
    let frames = selected_book_frames();
    let mut script = frames.clone();
    script.push(TRADES.to_owned());
    let mock = HyperliquidMock::start(vec![script]).await;

    let temp = TempDir::new().expect("tempdir");
    let journal = JournalHandle::start(
        journal_config(&temp.path().join("journal")),
        JournalMeta::new("hyperliquid", "test", "test", None),
    )
    .expect("journal start");
    let run_directory = PathBuf::from(journal.status().run_directory.clone());

    let factory: Arc<dyn SessionFactory> = Arc::new(
        exchange::hyperliquid::session::HyperliquidSessionFactory::new(
            selected_settings(mock.addr, &SELECTED_COINS),
            NetworkSettings::default(),
        ),
    );
    let (cmd_tx, cmd_rx) = mpsc::channel::<Control>(16);
    let (evt_tx, mut evt_rx) = mpsc::channel::<WorkerEvent>(128);
    let (stop_tx, stop_rx) = broadcast::channel(1);
    let worker = tokio::spawn(run_worker(
        factory,
        SourceId::new(0, 0),
        cmd_rx,
        evt_tx,
        worker_config(journal.clone()),
        stop_rx,
    ));
    let desired: BTreeSet<String> = SELECTED_COINS.iter().map(|coin| native(coin)).collect();
    cmd_tx
        .send(Control::SetSubscriptions(desired.clone()))
        .await
        .unwrap();

    let seen = recv_until(&mut evt_rx, |events| {
        let messages = events
            .iter()
            .rev()
            .find_map(|event| match event {
                WorkerEvent::Status(status) => Some(status.messages),
                _ => None,
            })
            .unwrap_or(0);
        messages >= (frames.len() + 1) as u64
    })
    .await;
    let _ = stop_tx.send(());
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("worker did not stop")
        .expect("worker task failed");
    drop(cmd_tx);

    // Zero LevelChange from this venue, ever. Only l2Book snapshots are events.
    let mut snapshots = Vec::new();
    for event in &seen {
        if let WorkerEvent::Events { events, .. } = event {
            for normalized in events {
                assert!(
                    !matches!(normalized, ExchangeEvent::LevelChange { .. }),
                    "Hyperliquid produced a LevelChange: {normalized:?}"
                );
                if let ExchangeEvent::Snapshot { .. } = normalized {
                    snapshots.push(normalized.clone());
                }
            }
        }
    }
    assert_eq!(
        snapshots.len(),
        SELECTED_COINS.len(),
        "every selected coin anchored"
    );
    let mut anchored = BTreeSet::new();
    for snapshot in &snapshots {
        let ExchangeEvent::Snapshot {
            native_symbol,
            book,
            meta,
        } = snapshot
        else {
            unreachable!("filtered snapshots only");
        };
        assert!(
            anchored.insert(native_symbol.clone()),
            "duplicate snapshot for {native_symbol}"
        );
        assert!(
            meta.received_at_us > 0,
            "exact receive timestamp is captured"
        );
        assert!(
            meta.seq.is_none() && meta.clock.is_none(),
            "no venue ordering clock"
        );
        assert_eq!(meta.connection_epoch, 1);
        assert!(
            meta.receive_seq >= 1,
            "supervisor stamped the receive fence"
        );
        // A feed timestamp is advisory: it must not replace the receive time.
        assert!(meta.exchange_ts_us.is_some_and(|ts| ts > 0));
        assert!(book.bids.len() + book.asks.len() <= 6);
    }
    assert_eq!(anchored, desired, "expected == acknowledged anchored set");

    // Exact wire reconciliation: only telemetry + selected books/trades.
    let commands = mock.commands_for(0);
    let mut command_set = BTreeSet::new();
    for command in &commands {
        let method = command["method"].as_str().unwrap_or("").to_owned();
        let kind = command["subscription"]["type"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        let coin = command["subscription"]["coin"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        assert_eq!(method, "subscribe", "steady state has no unsubscribes");
        command_set.insert((kind, coin));
    }
    let mut expected_commands = BTreeSet::new();
    expected_commands.insert(("allMids".to_owned(), String::new()));
    expected_commands.insert(("fastAssetCtxs".to_owned(), String::new()));
    for coin in SELECTED_COINS {
        expected_commands.insert(("l2Book".to_owned(), coin.to_owned()));
        expected_commands.insert(("trades".to_owned(), coin.to_owned()));
    }
    assert_eq!(
        command_set, expected_commands,
        "exact selected subscription set"
    );

    // Journal: exact bytes, MarketData for books, ProtocolControl for trades.
    let summary = journal
        .shutdown(Duration::from_secs(2))
        .expect("journal shutdown");
    assert_eq!(summary.record_count, (frames.len() + 1) as u64);
    let records = read_records(&run_directory).expect("read journal records");
    assert_eq!(records.len(), frames.len() + 1);
    for (index, frame) in frames.iter().enumerate() {
        assert_eq!(
            records[index].payload,
            frame.as_bytes(),
            "exact l2Book bytes"
        );
        assert!(matches!(
            records[index].outcome,
            DecodeOutcome::Decoded { events: 1 }
        ));
    }
    let trade_record = records.last().expect("trades record");
    assert_eq!(
        trade_record.payload,
        TRADES.as_bytes(),
        "exact trades bytes"
    );
    assert!(matches!(
        trade_record.outcome,
        DecodeOutcome::Ignored { .. }
    ));

    // Signals are typed at the observation boundary: books are MarketData,
    // trades are ProtocolControl. Native controls never advance receive_seq.
    let observations = journal::read_observations(&run_directory).expect("read observations");
    let data: Vec<_> = observations
        .iter()
        .filter(|observation| observation.receive_seq.is_some())
        .collect();
    assert_eq!(data.len(), frames.len() + 1);
    for observation in &data[..frames.len()] {
        assert_eq!(observation.signal, Some(SessionSignal::MarketData));
    }
    assert_eq!(
        data.last().expect("trades observation").signal,
        Some(SessionSignal::ProtocolControl),
        "trades are telemetry, not market data"
    );
    let verification = journal::verify_run(&run_directory).expect("journal verify");
    assert!(verification.complete, "journal verify is complete");
    assert!(
        verification.errors.is_empty(),
        "journal verify errors: {:?}",
        verification.errors
    );
    assert_eq!(verification.record_count, (frames.len() + 1) as u64);

    // Build the SBE capture through the real CollectorCore and certify it with
    // the three independent evidence tools.
    let mut settings = Settings::for_exchange(ExchangeKind::Hyperliquid);
    settings.collector.shards = 1;
    settings.collector.replicas = 1;
    let config = CoreConfig::from_settings(&settings, 1, exchange::hyperliquid::capabilities())
        .expect("core config");
    let mut core = CollectorCore::new(config).expect("collector core");
    let markets: Vec<MarketInfo> = SELECTED_COINS.iter().map(|coin| market(coin)).collect();
    core.apply_discovery(1_000_000, markets.clone())
        .expect("discovery");
    let mut outputs = Vec::new();
    for snapshot in snapshots {
        outputs.extend(core.handle_event(SourceId::new(0, 0), snapshot));
    }
    let writer_messages = outputs.len();
    let capture = write_capture(temp.path(), outputs);
    write_sidecars(
        temp.path(),
        &markets,
        &SELECTED_COINS
            .iter()
            .map(|coin| native(coin))
            .collect::<Vec<_>>(),
        writer_messages,
    );

    let coverage = coverage::run(
        &capture,
        &temp.path().join("markets.json"),
        &temp.path().join("report.json"),
        CHUNK_STR,
    )
    .expect("coverage run");
    assert!(coverage.valid, "coverage: {coverage:?}");
    assert_eq!(coverage.expected, SELECTED_COINS.len());
    assert_eq!(coverage.acknowledged, SELECTED_COINS.len());
    assert_eq!(coverage.anchored, SELECTED_COINS.len());

    let completeness = completeness::run(&capture, CHUNK_STR, false).expect("completeness run");
    assert!(completeness.valid, "completeness: {completeness:?}");
    assert_eq!(completeness.updates, 0, "Hyperliquid contributes no deltas");
    assert_eq!(completeness.deltas_before_snapshot, 0);

    let verify = verify::run(&capture, CHUNK_STR).expect("verify run");
    assert!(verify.valid, "verify: {verify:?}");
    assert_eq!(verify.price_level_updates, 0, "no deltas to reconstruct");
    assert_eq!(verify.snapshot_mismatches, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recovery_reanchors_with_a_fresh_l2book_in_a_new_connection_epoch() {
    let first = book_frame(
        "BTC",
        1_700_000_000_000,
        &[("100.0", "1")],
        &[("101.0", "2")],
    );
    let second = book_frame(
        "BTC",
        1_700_000_000_001,
        &[("200.0", "3")],
        &[("201.0", "4")],
    );
    let mock = HyperliquidMock::start(vec![
        vec![first.clone()],
        vec![second.clone(), TRADES.to_owned()],
    ])
    .await;

    let temp = TempDir::new().expect("tempdir");
    let journal = JournalHandle::start(
        journal_config(&temp.path().join("journal")),
        JournalMeta::new("hyperliquid", "test", "test", None),
    )
    .expect("journal start");

    let factory: Arc<dyn SessionFactory> = Arc::new(
        exchange::hyperliquid::session::HyperliquidSessionFactory::new(
            selected_settings(mock.addr, &["BTC"]),
            NetworkSettings::default(),
        ),
    );
    let (cmd_tx, cmd_rx) = mpsc::channel::<Control>(16);
    let (evt_tx, mut evt_rx) = mpsc::channel::<WorkerEvent>(128);
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
        .send(Control::SetSubscriptions(BTreeSet::from([native("BTC")])))
        .await
        .unwrap();

    let _ = recv_until(&mut evt_rx, |events| {
        events.iter().any(|event| {
            matches!(event, WorkerEvent::Events { events, .. }
            if events.iter().any(|normalized| matches!(normalized, ExchangeEvent::Snapshot { .. })))
        })
    })
    .await;

    cmd_tx
        .send(Control::Recover {
            generation: 9,
            symbols: vec![native("BTC")],
            reconnect: true,
        })
        .await
        .unwrap();
    let after_recover = recv_until(&mut evt_rx, |events| {
        events.iter().any(|event| match event {
            WorkerEvent::Events { events, .. } => events.iter().any(|normalized| {
                matches!(normalized, ExchangeEvent::Snapshot { meta, .. }
                    if meta.connection_epoch == 2)
            }),
            _ => false,
        })
    })
    .await;

    let ack = after_recover
        .iter()
        .find_map(|event| match event {
            WorkerEvent::RecoveryReady {
                generation,
                connection_epoch,
                receive_seq,
                targets,
                ..
            } if *generation == 9 => Some((*connection_epoch, *receive_seq, targets.clone())),
            _ => None,
        })
        .expect("recovery acknowledgement");
    assert_eq!(ack.0, 1, "ack fences the old connection epoch");
    assert_eq!(
        ack.2,
        vec![native("BTC")],
        "ack enumerates the exact target"
    );
    assert_eq!(
        mock.connection_count(),
        2,
        "a new connection epoch was opened"
    );

    let epoch_two: Vec<&ExchangeEvent> = after_recover
        .iter()
        .filter_map(|event| {
            match event {
            WorkerEvent::Events { events, .. } => events.iter().find(|event| {
                matches!(event, ExchangeEvent::Snapshot { meta, .. } if meta.connection_epoch == 2)
            }),
            _ => None,
        }
        })
        .collect();
    assert!(!epoch_two.is_empty(), "fresh snapshot in the new epoch");
    for event in &epoch_two {
        match event {
            ExchangeEvent::Snapshot { book, meta, .. } => {
                assert!(
                    meta.receive_seq > ack.1 || meta.connection_epoch > ack.0,
                    "recovery snapshot is read strictly after the acknowledgement fence"
                );
                assert_eq!(book.bids.len(), 1);
                assert_eq!(book.bids[0].price.raw(), 200_000_000);
                assert_eq!(book.asks[0].price.raw(), 201_000_000);
            }
            other => panic!("no non-snapshot event may be promoted after recovery: {other:?}"),
        }
    }
    // A trades frame in the new epoch is still telemetry, with no anchor. The
    // only promoted normalized events are the l2Book snapshot and the worker's
    // local StreamReset on connect; a trade can never be a LevelChange.
    let mut promoted_snapshots = 0usize;
    for event in &after_recover {
        if let WorkerEvent::Events { events, .. } = event {
            for normalized in events {
                assert!(
                    !matches!(normalized, ExchangeEvent::LevelChange { .. }),
                    "a trade produced a LevelChange after recovery: {normalized:?}"
                );
                if matches!(normalized, ExchangeEvent::Snapshot { .. }) {
                    promoted_snapshots += 1;
                }
            }
        }
    }
    assert_eq!(
        promoted_snapshots, 1,
        "only the fresh l2Book anchored data in the new epoch"
    );

    let commands = mock.commands_for(1);
    let kinds: Vec<&str> = commands
        .iter()
        .map(|command| command["subscription"]["type"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"l2Book"),
        "new epoch re-subscribed the book"
    );
    assert!(
        kinds.contains(&"trades"),
        "new epoch re-subscribed trades telemetry"
    );

    let _ = stop_tx.send(());
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("worker did not stop")
        .expect("worker task failed");
    drop(cmd_tx);
    let _ = journal.shutdown(Duration::from_secs(2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_l2book_is_rejected_with_exact_raw_bytes_and_no_event() {
    const MALFORMED: &str = include_str!("../fixtures/hyperliquid/l2_book_malformed.json");
    let mock = HyperliquidMock::start(vec![vec![MALFORMED.to_owned()]]).await;
    let temp = TempDir::new().expect("tempdir");
    let journal = JournalHandle::start(
        journal_config(&temp.path().join("journal")),
        JournalMeta::new("hyperliquid", "test", "test", None),
    )
    .expect("journal start");
    let run_directory = PathBuf::from(journal.status().run_directory.clone());
    let factory: Arc<dyn SessionFactory> = Arc::new(
        exchange::hyperliquid::session::HyperliquidSessionFactory::new(
            selected_settings(mock.addr, &["BTC"]),
            NetworkSettings::default(),
        ),
    );
    let (cmd_tx, cmd_rx) = mpsc::channel::<Control>(16);
    let (evt_tx, mut evt_rx) = mpsc::channel::<WorkerEvent>(64);
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
        .send(Control::SetSubscriptions(BTreeSet::from([native("BTC")])))
        .await
        .unwrap();
    let seen = recv_until(&mut evt_rx, |events| {
        events
            .iter()
            .rev()
            .find_map(|event| match event {
                WorkerEvent::Status(status) => Some(status.decode_errors),
                _ => None,
            })
            .unwrap_or(0)
            >= 1
    })
    .await;
    let _ = stop_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(5), worker).await;
    drop(cmd_tx);
    assert!(
        seen.iter()
            .all(|event| !matches!(event, WorkerEvent::Events { .. })),
        "a malformed frame produced no book event"
    );
    let summary = journal.shutdown(Duration::from_secs(2)).expect("shutdown");
    assert_eq!(summary.record_count, 1);
    let records = read_records(&run_directory).expect("read records");
    assert_eq!(records[0].payload, MALFORMED.as_bytes());
    assert!(matches!(records[0].outcome, DecodeOutcome::Rejected { .. }));
}
