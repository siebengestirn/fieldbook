//! H1 Hyperliquid telemetry evidence.
//!
//! Drives the real session supervisor against a loopback websocket with a real
//! v2 journal. The gate this test enforces: telemetry frames are journaled
//! with their exact bytes and typed `ProtocolControl` observations, and the
//! worker emits zero book events (so the collector can create zero anchors).
//! Nothing here is a live-venue claim.

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
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use tempfile::TempDir;
use tokio::{
    net::TcpListener,
    sync::{broadcast, mpsc},
};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

use fieldbook::{
    config::{HyperliquidSettings, NetworkSettings},
    exchange::hyperliquid::session::HyperliquidSessionFactory,
    journal::{
        DecodeOutcome, JournalConfig, JournalHandle, JournalMeta, SyncMode, read_observations,
        read_records,
    },
    model::{Control, SourceId},
    protocol::{FrameClassification, FrameDirection, SessionSignal},
    session::{SessionFactory, WorkerConfig, WorkerEvent, run_worker},
};

type ServerWs = WebSocketStream<tokio::net::TcpStream>;

const ALL_MIDS: &str = include_str!("../fixtures/hyperliquid/all_mids.json");
const FAST_CTX: &str = include_str!("../fixtures/hyperliquid/fast_asset_ctxs.json");

fn journal_config(directory: &Path) -> JournalConfig {
    JournalConfig {
        directory: directory.to_path_buf(),
        queue_capacity: 16,
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

async fn read_subscribe(ws: &mut ServerWs, expected: &str) {
    let message = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("server read timed out")
        .expect("server stream ended")
        .expect("server websocket error");
    let Message::Text(text) = message else {
        panic!("expected a subscription text frame, got {message:?}");
    };
    let value: serde_json::Value = serde_json::from_str(&text).expect("subscription is JSON");
    assert_eq!(value["method"], "subscribe");
    assert_eq!(value["subscription"]["type"], expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn telemetry_journals_exact_bytes_with_zero_book_events_and_anchors() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        read_subscribe(&mut ws, "allMids").await;
        read_subscribe(&mut ws, "fastAssetCtxs").await;
        ws.send(Message::Text(ALL_MIDS.to_owned().into()))
            .await
            .unwrap();
        ws.send(Message::Text(FAST_CTX.to_owned().into()))
            .await
            .unwrap();
        while let Some(message) = ws.next().await {
            if message.is_err() {
                break;
            }
        }
    });

    let temp = TempDir::new().expect("tempdir");
    let journal = JournalHandle::start(
        journal_config(&temp.path().join("journal")),
        JournalMeta::new("hyperliquid", "test", "test", None),
    )
    .expect("journal start");
    let run_directory = PathBuf::from(journal.status().run_directory.clone());

    let settings = HyperliquidSettings {
        ws_url: format!("ws://{address}"),
        ..HyperliquidSettings::default()
    };
    let factory: Arc<dyn SessionFactory> = Arc::new(HyperliquidSessionFactory::new(
        settings,
        NetworkSettings::default(),
    ));

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

    // Wait until both telemetry payloads have been received and accounted for.
    let mut seen = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = tokio::time::timeout(remaining, evt_rx.recv())
            .await
            .expect("timed out waiting for telemetry status")
            .expect("event channel closed");
        seen.push(event);
        if let Some(WorkerEvent::Status(status)) = seen.last()
            && status.messages >= 2
        {
            break;
        }
    }
    let _ = stop_tx.send(());
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("worker did not stop")
        .expect("worker task failed");
    drop(cmd_tx);

    // Zero book events: no Snapshot or LevelChange ever reached the core, so
    // no source-local book could be anchored by telemetry.
    for event in &seen {
        assert!(
            !matches!(event, WorkerEvent::Events { .. }),
            "a telemetry frame produced a book event: {event:?}"
        );
    }
    while let Ok(event) = evt_rx.try_recv() {
        assert!(
            !matches!(event, WorkerEvent::Events { .. }),
            "a telemetry frame produced a book event: {event:?}"
        );
    }

    let summary = journal
        .shutdown(Duration::from_secs(2))
        .expect("journal shutdown");
    assert_eq!(
        summary.record_count, 2,
        "every telemetry payload is journaled"
    );

    let records = read_records(&run_directory).expect("read journal records");
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0].payload,
        ALL_MIDS.as_bytes(),
        "exact allMids bytes"
    );
    assert_eq!(
        records[1].payload,
        FAST_CTX.as_bytes(),
        "exact fastAssetCtxs bytes"
    );
    for record in &records {
        assert!(
            matches!(record.outcome, DecodeOutcome::Ignored { .. }),
            "telemetry is accepted but produces no normalized event: {:?}",
            record.outcome
        );
    }

    let observations = read_observations(&run_directory).expect("read journal observations");
    let data: Vec<_> = observations
        .iter()
        .filter(|observation| observation.receive_seq.is_some())
        .collect();
    assert_eq!(data.len(), 2, "two inbound data observations");
    for observation in data {
        assert_eq!(observation.direction, FrameDirection::Inbound);
        assert!(matches!(
            observation.classification,
            FrameClassification::Data { .. }
        ));
        assert_eq!(
            observation.signal,
            Some(SessionSignal::ProtocolControl),
            "telemetry is typed as protocol control, not market data or a heartbeat"
        );
    }
    server.await.unwrap();
}
