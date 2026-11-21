// SPDX-License-Identifier: Apache-2.0
//! Raw packet evidence and its mapping into the existing journal contract.
//!
//! Original payload bytes and packet boundaries are preserved before decode.
//! Journal integration uses an explicit source identity supplied by the
//! source manifest; no identity is inferred from packet content.

use serde::{Deserialize, Serialize};

use crate::{
    acquisition::arbiter::FeedChannel,
    journal::ObservationRecord,
    model::SourceId,
    protocol::{FrameClassification, FrameDirection, FrameKind, HeartbeatMechanism, SessionSignal},
    util::sha256_hex,
};

/// Immutable evidence for one received packet or application payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketEvidence {
    pub source: String,
    pub channel: FeedChannel,
    pub connection_epoch: u64,
    pub sequence: Option<u64>,
    pub protocol: String,
    pub payload: Vec<u8>,
    pub received_at_us: i64,
    pub capture_location: String,
    pub decoder_version: String,
}

impl PacketEvidence {
    /// Create evidence from a received payload.
    #[expect(
        clippy::too_many_arguments,
        reason = "packet evidence fields are explicit and caller-supplied"
    )]
    pub fn new(
        source: &str,
        channel: FeedChannel,
        connection_epoch: u64,
        sequence: Option<u64>,
        protocol: &str,
        payload: Vec<u8>,
        received_at_us: i64,
        capture_location: &str,
        decoder_version: &str,
    ) -> Self {
        Self {
            source: source.to_owned(),
            channel,
            connection_epoch,
            sequence,
            protocol: protocol.to_owned(),
            payload,
            received_at_us,
            capture_location: capture_location.to_owned(),
            decoder_version: decoder_version.to_owned(),
        }
    }

    /// Return the SHA-256 digest of the original payload bytes.
    pub fn payload_sha256(&self) -> String {
        sha256_hex(&self.payload)
    }

    /// Map to the existing journal observation record with an explicit source.
    pub fn to_observation(&self, source: SourceId) -> ObservationRecord {
        ObservationRecord {
            source,
            connection_epoch: self.connection_epoch,
            received_at_us: self.received_at_us,
            direction: FrameDirection::Inbound,
            classification: FrameClassification::Data {
                frame_kind: FrameKind::Binary,
            },
            signal: SessionSignal::MarketData,
            mechanism: HeartbeatMechanism::Disabled,
            payload: self.payload.clone(),
            send_result: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> PacketEvidence {
        PacketEvidence::new(
            "source-a",
            FeedChannel::Primary,
            1,
            Some(7),
            "itch",
            vec![1, 2, 3, 4],
            1_700_000_000_000_000,
            "lab",
            "decoder-1",
        )
    }

    #[test]
    fn payload_digest_is_stable() {
        let packet = evidence();
        assert_eq!(
            packet.payload_sha256(),
            sha256_hex(&[1, 2, 3, 4]),
            "digest must cover original bytes"
        );
    }

    #[test]
    fn journal_mapping_preserves_identity_and_payload() {
        let packet = evidence();
        let observation = packet.to_observation(SourceId::new(2, 3));
        assert_eq!(
            observation.source,
            SourceId::new(2, 3),
            "source is explicit"
        );
        assert_eq!(observation.connection_epoch, 1, "epoch must be retained");
        assert_eq!(
            observation.payload,
            vec![1, 2, 3, 4],
            "original bytes must survive"
        );
    }
}
