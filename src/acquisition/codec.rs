// SPDX-License-Identifier: Apache-2.0
//! Acquisition feed decoding primitives.
//!
//! A [`CodecRegistry`] maps a `(source, schema_version)` pair to a boxed
//! [`FeedCodec`] and partitions decoded observations by outcome. Decoding is
//! fail-closed: a packet without a registered codec produces an error rather
//! than a partial batch.

use std::collections::BTreeMap;

use anyhow::Context as _;
use serde::{
    Deserialize, //
    Serialize,   //
};
use sha2::{
    Digest, //
    Sha256, //
};

/// Decode outcome attached to one observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecodeStatus {
    /// Payload produced a structured observation.
    Decoded,
    /// Payload was recognized and produced no observation.
    Ignored,
    /// Payload failed validation and was refused.
    Rejected,
    /// Payload uses a schema version outside the codec support range.
    Unsupported,
}

impl DecodeStatus {
    /// Stable lowercase label for the status.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Decoded => "decoded",
            Self::Ignored => "ignored",
            Self::Rejected => "rejected",
            Self::Unsupported => "unsupported",
        }
    }
}

/// One structured observation emitted by a feed codec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedObservation {
    /// Identifier of the packet that carried the observation.
    pub packet_id: String,
    /// Source feed that produced the packet.
    pub source: String,
    /// Schema version of the codec that decoded the packet.
    pub schema_version: u32,
    /// Observation kind, such as `trade` or `book`.
    pub kind: String,
    /// Observation body encoded as JSON text.
    pub payload_json: String,
    /// Decode outcome attached to the observation.
    pub status: DecodeStatus,
    /// Reason text for a status other than [`DecodeStatus::Decoded`].
    pub reason: Option<String>,
}

impl DecodedObservation {
    /// Compute a `Sha256` digest over the stable fields of the observation.
    ///
    /// Field boundaries carry a unit separator so distinct field splits
    /// cannot produce the same digest.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.packet_id.as_bytes());
        hasher.update([0x1f]);
        hasher.update(self.source.as_bytes());
        hasher.update([0x1f]);
        hasher.update(self.schema_version.to_le_bytes());
        hasher.update([0x1f]);
        hasher.update(self.kind.as_bytes());
        hasher.update([0x1f]);
        hasher.update(self.payload_json.as_bytes());
        hasher.update([0x1f]);
        hasher.update(self.status.as_str().as_bytes());
        hasher.update([0x1f]);
        match &self.reason {
            Some(reason) => hasher.update(reason.as_bytes()),
            None => hasher.update([0x00]),
        }
        let digest = hasher.finalize();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&digest);
        bytes
    }
}

/// Schema-versioned decoder for one feed source.
pub trait FeedCodec: Send + Sync {
    /// Decode a raw payload into observations.
    ///
    /// # Errors
    /// Returns an error when the payload is malformed beyond per-observation
    /// rejection.
    fn decode(&self, payload: &[u8]) -> anyhow::Result<Vec<DecodedObservation>>;

    /// Schema version understood by the codec.
    fn schema_version(&self) -> u32;
}

/// Observations partitioned by decode outcome.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DecodeBatch {
    /// Observations carrying [`DecodeStatus::Decoded`] or [`DecodeStatus::Ignored`].
    pub accepted: Vec<DecodedObservation>,
    /// Observations carrying [`DecodeStatus::Rejected`].
    pub rejected: Vec<DecodedObservation>,
    /// Observations carrying [`DecodeStatus::Unsupported`].
    pub unsupported: Vec<DecodedObservation>,
}

/// Registry of boxed codecs keyed by source and schema version.
#[derive(Default)]
pub struct CodecRegistry {
    codecs: BTreeMap<(String, u32), Box<dyn FeedCodec>>,
}

impl CodecRegistry {
    /// Construct an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a codec under its source and reported schema version.
    pub fn register(&mut self, source: &str, codec: Box<dyn FeedCodec>) {
        let key = (source.to_owned(), codec.schema_version());
        self.codecs.insert(key, codec);
    }

    /// Resolve the codec registered for a source and schema version.
    #[must_use]
    pub fn resolve(&self, source: &str, schema_version: u32) -> Option<&dyn FeedCodec> {
        self.codecs
            .get(&(source.to_owned(), schema_version))
            .map(Box::as_ref)
    }

    /// Decode one packet with the codec registered for its envelope.
    ///
    /// Packet id, source and schema version are stamped onto every
    /// observation returned by the codec. A missing codec fails closed.
    ///
    /// # Errors
    /// Returns an error when no codec is registered or the codec fails.
    pub fn decode(
        &self,
        packet_id: &str,
        source: &str,
        schema_version: u32,
        payload: &[u8],
    ) -> anyhow::Result<DecodeBatch> {
        let codec = self.resolve(source, schema_version).with_context(|| {
            format!("no codec registered for source `{source}` schema version {schema_version}")
        })?;
        let mut observations = codec.decode(payload).with_context(|| {
            format!("decoding packet `{packet_id}` from source `{source}` schema {schema_version}")
        })?;
        for observation in &mut observations {
            observation.packet_id = packet_id.to_owned();
            observation.source = source.to_owned();
            observation.schema_version = schema_version;
        }
        let mut batch = DecodeBatch::default();
        for observation in observations {
            match observation.status {
                DecodeStatus::Decoded | DecodeStatus::Ignored => batch.accepted.push(observation),
                DecodeStatus::Rejected => batch.rejected.push(observation),
                DecodeStatus::Unsupported => batch.unsupported.push(observation),
            }
        }
        Ok(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::CodecRegistry;
    use super::DecodeBatch;
    use super::DecodeStatus;
    use super::DecodedObservation;
    use super::FeedCodec;

    struct LengthPrefixedCodec {
        version: u32,
    }

    impl FeedCodec for LengthPrefixedCodec {
        fn decode(&self, payload: &[u8]) -> anyhow::Result<Vec<DecodedObservation>> {
            let (kind_len, rest) = payload
                .split_first()
                .ok_or_else(|| anyhow::anyhow!("empty payload"))?;
            let kind_len = usize::from(*kind_len);
            if rest.len() < kind_len {
                anyhow::bail!("truncated kind field");
            }
            let (kind, body) = rest.split_at(kind_len);
            Ok(vec![DecodedObservation {
                packet_id: String::new(),
                source: String::new(),
                schema_version: 0,
                kind: String::from_utf8(kind.to_vec())?,
                payload_json: String::from_utf8(body.to_vec())?,
                status: DecodeStatus::Decoded,
                reason: None,
            }])
        }

        fn schema_version(&self) -> u32 {
            self.version
        }
    }

    struct FixedStatusCodec {
        version: u32,
        status: DecodeStatus,
    }

    impl FeedCodec for FixedStatusCodec {
        fn decode(&self, _payload: &[u8]) -> anyhow::Result<Vec<DecodedObservation>> {
            Ok(vec![DecodedObservation {
                packet_id: String::new(),
                source: String::new(),
                schema_version: 0,
                kind: "status".to_owned(),
                payload_json: "{}".to_owned(),
                status: self.status,
                reason: None,
            }])
        }

        fn schema_version(&self) -> u32 {
            self.version
        }
    }

    fn encoded(kind: &str, body: &str) -> Vec<u8> {
        let mut payload = vec![u8::try_from(kind.len()).unwrap_or(u8::MAX)];
        payload.extend_from_slice(kind.as_bytes());
        payload.extend_from_slice(body.as_bytes());
        payload
    }

    #[test]
    fn length_prefixed_decode_stamps_envelope() {
        let mut registry = CodecRegistry::new();
        registry.register("venue-a", Box::new(LengthPrefixedCodec { version: 3 }));
        let payload = encoded("trade", "{\"price\":10}");
        let batch: DecodeBatch = registry
            .decode("packet-7", "venue-a", 3, &payload)
            .expect("registered codec decodes payload");
        assert_eq!(batch.accepted.len(), 1, "one accepted observation");
        let observation = &batch.accepted[0];
        assert_eq!(observation.packet_id, "packet-7", "packet id stamped");
        assert_eq!(observation.source, "venue-a", "source stamped");
        assert_eq!(observation.schema_version, 3, "schema version stamped");
        assert_eq!(observation.kind, "trade", "kind from length prefix");
        assert_eq!(
            observation.payload_json, "{\"price\":10}",
            "body from remainder"
        );
        assert_eq!(
            observation.status,
            DecodeStatus::Decoded,
            "decoded status kept"
        );
        assert!(batch.rejected.is_empty(), "no rejected observations");
        assert!(batch.unsupported.is_empty(), "no unsupported observations");
    }

    #[test]
    fn missing_codec_fails_closed() {
        let registry = CodecRegistry::new();
        let result = registry.decode("packet-1", "venue-x", 1, b"payload");
        assert!(result.is_err(), "unregistered source returns an error");
        let message = format!("{:#}", result.expect_err("error present"));
        assert!(
            message.contains("no codec registered"),
            "error names missing codec: {message}"
        );
    }

    #[test]
    fn decode_partitions_by_status() {
        let mut registry = CodecRegistry::new();
        registry.register(
            "reject-venue",
            Box::new(FixedStatusCodec {
                version: 1,
                status: DecodeStatus::Rejected,
            }),
        );
        registry.register(
            "unsupported-venue",
            Box::new(FixedStatusCodec {
                version: 2,
                status: DecodeStatus::Unsupported,
            }),
        );
        registry.register(
            "ignored-venue",
            Box::new(FixedStatusCodec {
                version: 3,
                status: DecodeStatus::Ignored,
            }),
        );
        let rejected = registry
            .decode("p1", "reject-venue", 1, b"x")
            .expect("reject codec registered");
        assert_eq!(rejected.rejected.len(), 1, "rejected observation bucketed");
        assert!(rejected.accepted.is_empty(), "no accepted observations");
        let unsupported = registry
            .decode("p2", "unsupported-venue", 2, b"x")
            .expect("unsupported codec registered");
        assert_eq!(
            unsupported.unsupported.len(),
            1,
            "unsupported observation bucketed"
        );
        let ignored = registry
            .decode("p3", "ignored-venue", 3, b"x")
            .expect("ignored codec registered");
        assert_eq!(ignored.accepted.len(), 1, "ignored observation accepted");
    }

    #[test]
    fn digest_is_stable_and_field_sensitive() {
        let observation = DecodedObservation {
            packet_id: "packet-9".to_owned(),
            source: "venue-a".to_owned(),
            schema_version: 2,
            kind: "trade".to_owned(),
            payload_json: "{\"qty\":3}".to_owned(),
            status: DecodeStatus::Decoded,
            reason: None,
        };
        let copy = observation.clone();
        assert_eq!(
            observation.digest(),
            copy.digest(),
            "identical observations share a digest"
        );
        let mut changed_body = observation.clone();
        changed_body.payload_json.push(' ');
        assert_ne!(
            observation.digest(),
            changed_body.digest(),
            "digest changes with payload body"
        );
        let mut changed_source = observation.clone();
        changed_source.source = "venue-b".to_owned();
        assert_ne!(
            observation.digest(),
            changed_source.digest(),
            "digest changes with source"
        );
    }

    #[test]
    fn status_labels_are_stable() {
        assert_eq!(DecodeStatus::Decoded.as_str(), "decoded", "decoded label");
        assert_eq!(DecodeStatus::Ignored.as_str(), "ignored", "ignored label");
        assert_eq!(
            DecodeStatus::Rejected.as_str(),
            "rejected",
            "rejected label"
        );
        assert_eq!(
            DecodeStatus::Unsupported.as_str(),
            "unsupported",
            "unsupported label"
        );
    }
}
