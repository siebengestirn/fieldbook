//! Generate a deterministic journal fixture for the checked-in compatibility
//! tests. Not part of the shipped binary.
//!
//! Usage: `cargo run --example write_fixture -- <output-root>`

#![allow(
    clippy::expect_used,
    reason = "an offline fixture generator must abort loudly if the journal rejects a record"
)]

use std::time::Duration;

use engineering_challenge::{
    journal::{
        DataRecord, DecodeOutcome, JournalConfig, JournalHandle, JournalMeta, ObservationRecord,
        RawFrame, SyncMode,
    },
    model::SourceId,
    protocol::{
        ControlOpcode, FrameClassification, FrameDirection, FrameKind, HeartbeatMechanism,
        SendResult, SessionSignal, TransportObservation,
    },
};

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: write_fixture <output-root>");
    let handle = JournalHandle::start(
        JournalConfig {
            directory: root.into(),
            queue_capacity: 16,
            enqueue_timeout: Duration::from_millis(250),
            flush_interval: Duration::from_millis(10),
            sync_interval: Duration::from_millis(20),
            segment_max_bytes: 0,
            segment_max_seconds: 0,
            sync_mode: SyncMode::Always,
        },
        JournalMeta {
            run_id: "golden".to_owned(),
            venue: "polymarket".to_owned(),
            code_commit: "fixture".to_owned(),
            config_sha256: "fixture".to_owned(),
            route: None,
        },
    )
    .expect("journal start");

    let source = SourceId::new(0, 0);
    let data = |connection_epoch, receive_seq, received_at_us, frame, outcome| DataRecord {
        source,
        connection_epoch,
        receive_seq,
        received_at_us,
        frame,
        outcome,
        signal: SessionSignal::MarketData,
        mechanism: HeartbeatMechanism::ApplicationText,
    };
    handle
        .record(data(
            1,
            1,
            1_700_000_000_000_000,
            RawFrame::text(
                r#"{"event_type":"book","asset_id":"0xabc","bids":[],"asks":[]}"#.to_owned(),
            ),
            DecodeOutcome::decoded(1),
        ))
        .expect("text record");
    handle
        .observe(ObservationRecord {
            source,
            connection_epoch: 1,
            received_at_us: 1_700_000_000_000_050,
            direction: FrameDirection::Inbound,
            classification: FrameClassification::Control {
                opcode: ControlOpcode::Ping,
            },
            signal: SessionSignal::PeerPing,
            mechanism: HeartbeatMechanism::ApplicationText,
            payload: b"server-ping".to_vec(),
            send_result: None,
        })
        .expect("inbound ping");
    handle
        .observe(ObservationRecord {
            source,
            connection_epoch: 1,
            received_at_us: 1_700_000_000_000_060,
            direction: FrameDirection::Outbound,
            classification: FrameClassification::Control {
                opcode: ControlOpcode::Pong,
            },
            signal: SessionSignal::PeerPong,
            mechanism: HeartbeatMechanism::ApplicationText,
            payload: b"server-ping".to_vec(),
            send_result: Some(SendResult::Sent),
        })
        .expect("outbound pong");
    handle
        .record(data(
            1,
            2,
            1_700_000_000_000_100,
            RawFrame::binary(vec![0, 159, 146, 150, 255]),
            DecodeOutcome::ignored("unknown_event_type"),
        ))
        .expect("binary record");
    // Polymarket's text PONG is a data frame on the wire, so it consumes the
    // receive fence even though it only acknowledges a heartbeat.
    let mut pong = data(
        1,
        3,
        1_700_000_000_000_150,
        RawFrame::text("PONG".to_owned()),
        DecodeOutcome::ignored("control_frame"),
    );
    pong.signal = SessionSignal::HeartbeatAck { correlation: None };
    handle.record(pong).expect("application pong");
    handle
        .record(data(
            1,
            4,
            1_700_000_000_000_200,
            RawFrame::text(r#"{"event_type":"future_event"}"#.to_owned()),
            DecodeOutcome::rejected("invalid_json"),
        ))
        .expect("rejected record");
    handle
        .shutdown(Duration::from_secs(5))
        .expect("journal shutdown");

    // Keep the transport vocabulary referenced so this generator documents
    // the shape of an outbound observation rather than drifting.
    let _ = TransportObservation::outbound_data(
        FrameKind::Text,
        b"PING".to_vec(),
        SessionSignal::HeartbeatRequest,
        HeartbeatMechanism::ApplicationText,
        SendResult::Sent,
    );
}
