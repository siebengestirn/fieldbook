//! End-to-end evidence that the generic runner uses the segmented store.
//!
//! Both the file adapter and the loopback TCP adapter are driven through the
//! same `DurableStore` abstraction as the legacy commit store.

#![expect(
    clippy::expect_used,
    reason = "integration tests assert deterministic durable state"
)]

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use engineering_challenge::application::{
    durable::{StreamingLimits, run_streaming_pipeline, run_streaming_pipeline_bounded},
    durable_segmented::{SegmentedStore, StoreLimits, SyncPolicy},
    file_source::{CheckpointBinding, DurableCheckpoint, FileSource, SourceIdentity},
    network_source,
    postgres::PostgresSimulator,
    transform_observation,
};

fn record(index: usize) -> String {
    format!(
        "{{\"id\":\"seg-run-{index}\",\"payload\":{{\"entity_id\":\"acct\",\"kind\":\"credit\",\"value\":{index},\"observed_at\":{index}}}}}"
    )
}

fn segmented(
    directory: &std::path::Path,
    name: &str,
    bytes: &[u8],
    target: &str,
) -> SegmentedStore {
    let binding = CheckpointBinding::new(&SourceIdentity::register(name, bytes), target);
    SegmentedStore::open(directory, binding, SyncPolicy::Full, StoreLimits::default())
        .expect("open segmented store")
}

#[test]
fn file_source_delivers_through_segmented_store() {
    let root = tempfile::tempdir().expect("tempdir");
    let name = "segmented-file-source";
    let bytes = format!("{}\n", (0..5).map(record).collect::<Vec<_>>().join("\n"));
    let input = root.path().join("input.jsonl");
    std::fs::write(&input, &bytes).expect("write input");
    let identity = SourceIdentity::register(name, bytes.as_bytes());
    let binding = CheckpointBinding::new(&identity, "seg-file-target");
    let anchor = DurableCheckpoint::new(&binding);
    let mut store = segmented(
        &root.path().join("store"),
        name,
        bytes.as_bytes(),
        "seg-file-target",
    );
    let mut source = FileSource::open(&input, name).expect("open source");
    let mut target = PostgresSimulator::new("seg-file-target");
    let result = run_streaming_pipeline_bounded(
        &mut source,
        transform_observation,
        &mut target,
        &anchor,
        &mut store,
        StreamingLimits::new(2, usize::MAX, 1),
    );
    assert_eq!(result.report.delivered, 5, "all records delivered");
    assert_eq!(
        result.checkpoint.last_committed_sequence, 5,
        "frontier advanced"
    );
    store.verify().expect("segmented store verifies");
}

struct MockServer {
    address: String,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockServer {
    fn start(lines: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let address = listener.local_addr().expect("local address").to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let handle = thread::spawn(move || {
            while !stop_for_thread.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(_) => break,
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                let mut hello = [0_u8; 64];
                let _ = stream.read(&mut hello);
                let _ = stream.write_all(b"TOKEN seg-network-token\n");
                for line in &lines {
                    let _ = stream.write_all(format!("{line}\n").as_bytes());
                    let _ = stream.flush();
                }
            }
        });
        Self {
            address,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[test]
fn tcp_source_delivers_through_segmented_store() {
    let server = MockServer::start((0..4).map(record).collect());
    let root = tempfile::tempdir().expect("tempdir");
    let name = "segmented-tcp-source";
    let token = "seg-network-token";
    let bytes = (0..4).map(record).collect::<Vec<_>>().join("\n");
    let identity = SourceIdentity::register_token(name, token);
    let binding = CheckpointBinding::new(&identity, "seg-tcp-target");
    let anchor = DurableCheckpoint::new(&binding);
    let mut store = segmented(
        &root.path().join("store"),
        name,
        bytes.as_bytes(),
        "seg-tcp-target",
    );
    let mut source = network_source::connect(&server.address, name, token).expect("connect source");
    let mut target = PostgresSimulator::new("seg-tcp-target");
    let result = run_streaming_pipeline(
        &mut source,
        transform_observation,
        &mut target,
        &anchor,
        &mut store,
    );
    assert_eq!(result.report.delivered, 4, "all TCP records delivered");
    assert_eq!(
        result.checkpoint.last_committed_sequence, 4,
        "frontier advanced"
    );
    store.verify().expect("segmented store verifies");
}
