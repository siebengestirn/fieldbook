// SPDX-License-Identifier: Apache-2.0
//! Dual-feed packet arbitration by sequence number.
//!
//! [`FeedArbiter`] tracks the next expected sequence, records sequences seen
//! on either channel, buffers packets that arrive ahead of the expected
//! sequence and drains them once the gap is filled.

use std::collections::BTreeMap;

use serde::{
    Deserialize, //
    Serialize,   //
};

/// Feed channel that delivered a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedChannel {
    /// Primary feed channel.
    Primary,
    /// Secondary feed channel.
    Secondary,
}

impl FeedChannel {
    /// Stable lowercase label for the channel.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }
}

/// Packet carrying sequence metadata through arbitration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequencedPacket {
    /// Channel that delivered the packet.
    pub channel: FeedChannel,
    /// Feed sequence number.
    pub sequence: u64,
    /// Raw packet body.
    pub payload: Vec<u8>,
    /// Arrival timestamp in microseconds.
    pub received_at_us: i64,
}

/// Verdict for one submitted packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArbiterVerdict {
    /// Packet advanced the expected sequence.
    Accepted,
    /// Packet sequence was seen on either channel.
    Duplicate,
    /// Packet arrived beyond a gap carrying the still-expected sequence.
    Gap(u64),
    /// Packet sequence is older than expected and unseen.
    Stale,
    /// Packet arrived ahead of expected and was buffered.
    OutOfOrder,
}

/// Arbiter reconciling primary and secondary feed sequences.
#[derive(Debug, Clone)]
pub struct FeedArbiter {
    /// Next sequence number required for in-order progress.
    pub expected: u64,
    /// Seen sequences mapped to channel and arrival timestamp.
    pub seen: BTreeMap<u64, (FeedChannel, i64)>,
    /// Count of gaps detected.
    pub gap_count: u64,
    buffered: BTreeMap<u64, SequencedPacket>,
}

impl FeedArbiter {
    /// Construct an arbiter starting at `expected`.
    #[must_use]
    pub fn new(expected: u64) -> Self {
        Self {
            expected,
            seen: BTreeMap::new(),
            gap_count: 0,
            buffered: BTreeMap::new(),
        }
    }

    fn record(&mut self, packet: &SequencedPacket) {
        self.seen
            .insert(packet.sequence, (packet.channel, packet.received_at_us));
    }

    /// Submit one packet for arbitration.
    ///
    /// A gap opens on the first packet beyond the expected sequence; that
    /// packet and later ones are buffered and released by [`Self::drain_available`].
    pub fn submit(&mut self, packet: SequencedPacket) -> anyhow::Result<ArbiterVerdict> {
        let sequence = packet.sequence;
        if self.seen.contains_key(&sequence) || self.buffered.contains_key(&sequence) {
            return Ok(ArbiterVerdict::Duplicate);
        }
        if sequence < self.expected {
            return Ok(ArbiterVerdict::Stale);
        }
        if sequence == self.expected {
            self.record(&packet);
            self.expected = sequence.saturating_add(1);
            return Ok(ArbiterVerdict::Accepted);
        }
        let gap_opened = self.buffered.is_empty();
        self.buffered.insert(sequence, packet);
        if gap_opened {
            self.gap_count = self.gap_count.saturating_add(1);
            return Ok(ArbiterVerdict::Gap(self.expected));
        }
        Ok(ArbiterVerdict::OutOfOrder)
    }

    /// Return buffered packets whose sequences are now in order.
    pub fn drain_available(&mut self) -> Vec<SequencedPacket> {
        let mut drained = Vec::new();
        while let Some(packet) = self.buffered.remove(&self.expected) {
            self.record(&packet);
            self.expected = self.expected.saturating_add(1);
            drained.push(packet);
        }
        drained
    }
}

#[cfg(test)]
mod tests {
    use super::{ArbiterVerdict, FeedArbiter, FeedChannel, SequencedPacket};

    fn packet(channel: FeedChannel, sequence: u64, body: &str, at: i64) -> SequencedPacket {
        SequencedPacket {
            channel,
            sequence,
            payload: body.as_bytes().to_vec(),
            received_at_us: at,
        }
    }

    #[test]
    fn in_order_packet_is_accepted() {
        let mut arbiter = FeedArbiter::new(1);
        let verdict = arbiter
            .submit(packet(FeedChannel::Primary, 1, "one", 100))
            .expect("submit succeeds");
        assert_eq!(
            verdict,
            ArbiterVerdict::Accepted,
            "expected packet accepted"
        );
        assert_eq!(arbiter.expected, 2, "expected sequence advanced");
        assert_eq!(
            arbiter.seen.get(&1).copied(),
            Some((FeedChannel::Primary, 100)),
            "seen entry records channel and arrival time"
        );
        assert_eq!(arbiter.gap_count, 0, "no gaps detected");
    }

    #[test]
    fn duplicate_across_channels_is_detected() {
        let mut arbiter = FeedArbiter::new(9);
        let first = arbiter
            .submit(packet(FeedChannel::Primary, 9, "a", 10))
            .expect("primary submit succeeds");
        assert_eq!(first, ArbiterVerdict::Accepted, "sequence accepted");
        let second = arbiter
            .submit(packet(FeedChannel::Secondary, 9, "a", 20))
            .expect("secondary submit succeeds");
        assert_eq!(
            second,
            ArbiterVerdict::Duplicate,
            "other channel reports duplicate"
        );
        assert_eq!(arbiter.seen.len(), 1, "single seen entry retained");
        assert_eq!(arbiter.expected, 10, "duplicate leaves expected unchanged");
    }

    #[test]
    fn gap_then_recovery_drains_buffered() {
        let mut arbiter = FeedArbiter::new(1);
        let first = arbiter
            .submit(packet(FeedChannel::Primary, 1, "1", 1))
            .expect("first submit succeeds");
        assert_eq!(first, ArbiterVerdict::Accepted, "first packet accepted");

        let gap = arbiter
            .submit(packet(FeedChannel::Primary, 4, "4", 4))
            .expect("gap submit succeeds");
        assert_eq!(gap, ArbiterVerdict::Gap(2), "gap reports expected sequence");
        assert_eq!(arbiter.expected, 2, "gap leaves expected unchanged");
        assert_eq!(arbiter.gap_count, 1, "gap counter incremented");

        let ahead = arbiter
            .submit(packet(FeedChannel::Secondary, 3, "3", 3))
            .expect("ahead submit succeeds");
        assert_eq!(ahead, ArbiterVerdict::OutOfOrder, "ahead packet buffered");

        let recovered = arbiter
            .submit(packet(FeedChannel::Primary, 2, "2", 2))
            .expect("recovery submit succeeds");
        assert_eq!(
            recovered,
            ArbiterVerdict::Accepted,
            "missing packet accepted"
        );

        let drained = arbiter.drain_available();
        let sequences: Vec<u64> = drained.iter().map(|item| item.sequence).collect();
        assert_eq!(sequences, vec![3, 4], "buffered packets drain in order");
        assert_eq!(
            arbiter.expected, 5,
            "expected advanced past drained packets"
        );
    }

    #[test]
    fn stale_packet_is_rejected() {
        let mut arbiter = FeedArbiter::new(5);
        let verdict = arbiter
            .submit(packet(FeedChannel::Secondary, 3, "old", 7))
            .expect("submit succeeds");
        assert_eq!(
            verdict,
            ArbiterVerdict::Stale,
            "older unseen packet is stale"
        );
        assert_eq!(
            arbiter.expected, 5,
            "stale packet leaves expected unchanged"
        );
        assert!(arbiter.seen.is_empty(), "stale packet is not recorded");
    }

    #[test]
    fn out_of_order_packets_drain_after_gap_fill() {
        let mut arbiter = FeedArbiter::new(1);
        let first = arbiter
            .submit(packet(FeedChannel::Primary, 3, "c", 3))
            .expect("first submit succeeds");
        assert_eq!(
            first,
            ArbiterVerdict::Gap(1),
            "first ahead packet opens gap"
        );
        let second = arbiter
            .submit(packet(FeedChannel::Primary, 2, "b", 2))
            .expect("second submit succeeds");
        assert_eq!(
            second,
            ArbiterVerdict::OutOfOrder,
            "second ahead packet buffered"
        );
        let fill = arbiter
            .submit(packet(FeedChannel::Secondary, 1, "a", 1))
            .expect("fill submit succeeds");
        assert_eq!(fill, ArbiterVerdict::Accepted, "missing packet accepted");

        let drained = arbiter.drain_available();
        assert_eq!(drained.len(), 2, "two buffered packets drained");
        assert_eq!(drained[0].sequence, 2, "lower sequence drains first");
        assert_eq!(drained[1].sequence, 3, "higher sequence drains second");
        assert!(arbiter.drain_available().is_empty(), "drain is repeatable");
    }

    #[test]
    fn channel_labels_are_stable() {
        assert_eq!(FeedChannel::Primary.as_str(), "primary", "primary label");
        assert_eq!(
            FeedChannel::Secondary.as_str(),
            "secondary",
            "secondary label"
        );
    }
}
