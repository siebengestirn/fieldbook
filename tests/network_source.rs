//! Deterministic loopback evidence for the generic network source.
//!
//! A local TCP server speaks the source handshake, serves declared JSONL
//! lines and closes the connection on a fault boundary. The client reconnects
//! from the durable checkpoint, so recovery uses only the generic application
//! boundary: no market-data collector, venue adapter or simulator change.

#![expect(
    clippy::expect_used,
    reason = "integration tests assert deterministic store state and need named failures"
)]

use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use engineering_challenge::application::{
    durable::{CommitStore, load_state, run_streaming_pipeline},
    file_source::{CheckpointBinding, DurableCheckpoint, SourceIdentity},
    network_source,
    postgres::PostgresTarget,
    transform_observation,
};

const TOKEN: &str = "declared-stream-token-v1";

fn record(index: usize) -> String {
    format!(
        "{{\"id\":\"net-{index}\",\"payload\":{{\"entity_id\":\"acct\",\"kind\":\"credit\",\"value\":{index},\"observed_at\":{index}}}}}"
    )
}

fn lines(count: usize) -> Vec<String> {
    (0..count).map(record).collect()
}

struct MockServer {
    address: String,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockServer {
    fn start(all_lines: Vec<String>, first_limit: usize, malformed_at: Option<usize>) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let address = listener.local_addr().expect("local address").to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let handle = std::thread::spawn(move || {
            let mut connections = 0usize;
            while !stop_for_thread.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(_) => break,
                };
                // A client may stop reading early. Bound every socket wait so
                // server teardown can never block on an abandoned connection.
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let mut hello = [0_u8; 64];
                let _ = stream.read(&mut hello);
                let _ = stream.write_all(format!("TOKEN {TOKEN}\n").as_bytes());
                // The server always replays the declared stream from line one.
                // The first connection stops early to create a clean disconnect;
                // the client discards the committed prefix on every reconnect.
                let count = if connections == 0 {
                    first_limit.min(all_lines.len())
                } else {
                    all_lines.len()
                };
                for (index, line) in all_lines.iter().take(count).enumerate() {
                    let line = if malformed_at == Some(index) {
                        "{not json}"
                    } else {
                        line.as_str()
                    };
                    let _ = stream.write_all(format!("{line}\n").as_bytes());
                    let _ = stream.flush();
                }
                connections += 1;
            }
        });
        Self {
            address,
            stop,
            handle: Some(handle),
        }
    }

    fn address(&self) -> &str {
        &self.address
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

fn store_at(directory: &std::path::Path, name: &str, target_id: &str) -> CommitStore {
    let identity = SourceIdentity::register_token(name, TOKEN);
    CommitStore::open(directory, CheckpointBinding::new(&identity, target_id)).expect("open store")
}

fn anchor_for(name: &str, target_id: &str) -> DurableCheckpoint {
    let identity = SourceIdentity::register_token(name, TOKEN);
    DurableCheckpoint::new(&CheckpointBinding::new(&identity, target_id))
}

#[test]
fn network_source_resumes_after_disconnect_without_duplicates() {
    let name = "network-resume";
    let target_id = "pg-loopback";
    let directory = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start(lines(6), 3, None);

    let mut anchor = anchor_for(name, target_id);
    let mut total_delivered = 0u64;
    for _round in 0..2 {
        let mut source = network_source::resume(server.address(), name, TOKEN, &anchor, target_id)
            .expect("connect and resume");
        let mut target = PostgresTarget::new(target_id);
        let mut store = store_at(directory.path(), name, target_id);
        let result = run_streaming_pipeline(
            &mut source,
            transform_observation,
            &mut target,
            &anchor,
            &mut store,
        );
        assert_eq!(result.report.accounted(), result.report.total_read);
        assert_eq!(result.report.uncertain, 0);
        assert_eq!(result.report.duplicates, 0);
        total_delivered += result.report.delivered;
        anchor = result.checkpoint;
    }
    assert_eq!(total_delivered, 6);
    assert_eq!(anchor.last_committed_sequence, 6);
    assert!(anchor.last_committed_offset > 0);

    let identity = SourceIdentity::register_token(name, TOKEN);
    let binding = CheckpointBinding::new(&identity, target_id);
    let state = load_state(directory.path(), Some(&binding)).expect("reopen store");
    assert_eq!(
        state
            .checkpoint()
            .expect("checkpoint")
            .last_committed_sequence,
        6
    );
}

#[test]
fn network_source_quarantines_malformed_lines_and_still_accounts_every_line() {
    let name = "network-malformed";
    let target_id = "pg-loopback";
    let directory = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start(lines(5), 5, Some(2));
    let anchor = anchor_for(name, target_id);

    let mut source =
        network_source::connect(server.address(), name, TOKEN).expect("connect network source");
    let mut target = PostgresTarget::new(target_id);
    let mut store = store_at(directory.path(), name, target_id);
    let result = run_streaming_pipeline(
        &mut source,
        transform_observation,
        &mut target,
        &anchor,
        &mut store,
    );

    assert_eq!(result.report.total_read, 5);
    assert_eq!(result.report.delivered, 4);
    assert_eq!(result.report.rejected, 1);
    assert_eq!(result.report.quarantined, 1);
    assert_eq!(result.report.accounted(), result.report.total_read);
    assert_eq!(result.checkpoint.last_committed_sequence, 5);
}

#[test]
fn network_source_soak_replays_many_lines_across_four_disconnects() {
    let name = "network-soak";
    let target_id = "pg-loopback";
    let directory = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start(lines(2_000), 500, None);
    let mut anchor = anchor_for(name, target_id);
    let mut delivered = 0u64;

    for _round in 0..4 {
        if anchor.last_committed_sequence == 2_000 {
            break;
        }
        let mut source = network_source::resume(server.address(), name, TOKEN, &anchor, target_id)
            .expect("reconnect");
        let mut target = PostgresTarget::new(target_id);
        let mut store = store_at(directory.path(), name, target_id);
        let result = run_streaming_pipeline(
            &mut source,
            transform_observation,
            &mut target,
            &anchor,
            &mut store,
        );
        assert_eq!(result.report.accounted(), result.report.total_read);
        assert_eq!(result.report.uncertain, 0);
        assert_eq!(result.report.duplicates, 0);
        delivered += result.report.delivered;
        anchor = result.checkpoint;
    }
    assert_eq!(delivered, 2_000);
    assert_eq!(anchor.last_committed_sequence, 2_000);
}
