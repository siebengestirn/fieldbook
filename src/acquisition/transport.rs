// SPDX-License-Identifier: Apache-2.0
//! Tokio-based transport primitives for the acquisition plane.
//!
//! Transport kinds and feed configurations describe connector endpoints.
//! [`recv_udp_packet`] reads one datagram into a bounded buffer under a
//! timeout and stamps it with a wall-clock receive time. Multicast membership
//! is configured separately by [`join_multicast`] so the receive path stays
//! usable for unicast sockets.

use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use serde::Deserialize;
use serde::Serialize;

use crate::time::now_us;
use crate::util::sha256_hex;

/// Acquisition transport kinds with stable wire strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// Reliable byte stream over TCP.
    Tcp,
    /// Unicast UDP datagrams.
    Udp,
    /// UDP datagrams delivered through a multicast group.
    UdpMulticast,
    /// Full-duplex messages over a WebSocket upgrade.
    WebSocket,
    /// Request or streaming transfer over HTTP.
    Http,
    /// Replay from a local file.
    File,
    /// Local inter-process communication channel.
    Ipc,
}

impl TransportKind {
    /// Return the stable wire string for this transport.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::UdpMulticast => "udp_multicast",
            Self::WebSocket => "web_socket",
            Self::Http => "http",
            Self::File => "file",
            Self::Ipc => "ipc",
        }
    }
}

/// Configuration for a UDP feed endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdpFeedConfig {
    /// Local address used to bind the receiving socket.
    pub bind_addr: String,
    /// Optional multicast group address.
    pub group: Option<String>,
    /// UDP port of the feed.
    pub port: u16,
    /// Optional local interface used for multicast membership.
    pub interface: Option<String>,
    /// Largest accepted datagram, in bytes.
    pub max_datagram_bytes: usize,
    /// Receive deadline in milliseconds.
    pub receive_timeout_ms: u64,
}

impl UdpFeedConfig {
    /// Check that required fields are present and numeric bounds hold.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.bind_addr.trim().is_empty(),
            "UDP bind address must not be empty"
        );
        anyhow::ensure!(self.port > 0, "UDP port must be greater than zero");
        anyhow::ensure!(
            (256..=65_535).contains(&self.max_datagram_bytes),
            "max_datagram_bytes must be in 256..=65535, got {}",
            self.max_datagram_bytes
        );
        anyhow::ensure!(
            self.receive_timeout_ms >= 1,
            "receive_timeout_ms must be at least 1"
        );
        Ok(())
    }
}

/// Configuration for a TCP feed endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpFeedConfig {
    /// Remote endpoint in `host:port` form.
    pub endpoint: String,
    /// Delay before reconnecting, in milliseconds.
    pub reconnect_after_ms: u64,
    /// Largest accepted frame, in bytes.
    pub max_frame_bytes: usize,
}

impl TcpFeedConfig {
    /// Check that the endpoint is present and numeric bounds hold.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.endpoint.trim().is_empty(),
            "TCP endpoint must not be empty"
        );
        anyhow::ensure!(
            self.reconnect_after_ms >= 1,
            "reconnect_after_ms must be at least 1"
        );
        anyhow::ensure!(
            self.max_frame_bytes >= 1,
            "max_frame_bytes must be at least 1"
        );
        Ok(())
    }
}

/// One received feed packet together with its capture metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedPacket {
    /// Logical channel that carried the packet.
    pub channel: String,
    /// Feed source associated with the packet.
    pub source: String,
    /// Optional sequence assigned by the feed.
    pub sequence: Option<u64>,
    /// Original packet bytes.
    pub payload: Vec<u8>,
    /// Receive time in microseconds since the Unix epoch.
    pub received_at_us: i64,
    /// Local socket address, when available.
    pub local_addr: Option<String>,
    /// Peer socket address, when available.
    pub peer_addr: Option<String>,
}

impl FeedPacket {
    /// Build a packet from its payload and capture metadata.
    pub fn new(
        channel: impl Into<String>,
        source: impl Into<String>,
        sequence: Option<u64>,
        payload: Vec<u8>,
        received_at_us: i64,
        local_addr: Option<String>,
        peer_addr: Option<String>,
    ) -> Self {
        Self {
            channel: channel.into(),
            source: source.into(),
            sequence,
            payload,
            received_at_us,
            local_addr,
            peer_addr,
        }
    }

    /// Return the lowercase SHA-256 digest of the original payload bytes.
    #[must_use]
    pub fn payload_sha256(&self) -> String {
        sha256_hex(&self.payload)
    }
}

/// Receive one datagram and attach local capture metadata.
///
/// The receive deadline comes from `receive_timeout_ms`. Multicast membership
/// is not changed by this function; call [`join_multicast`] beforehand when
/// the socket receives a multicast group.
pub async fn recv_udp_packet(
    socket: &tokio::net::UdpSocket,
    config: &UdpFeedConfig,
) -> anyhow::Result<FeedPacket> {
    let mut buffer = vec![0_u8; config.max_datagram_bytes];
    let deadline = Duration::from_millis(config.receive_timeout_ms);
    let (length, peer) = tokio::time::timeout(deadline, socket.recv_from(&mut buffer))
        .await
        .context("UDP receive timed out")?
        .context("UDP recv_from failed")?;
    let received_at_us = now_us().context("receive timestamp outside i64 range")?;
    let local: SocketAddr = socket
        .local_addr()
        .context("local UDP address unavailable")?;
    buffer.truncate(length);
    Ok(FeedPacket::new(
        "udp",
        config.bind_addr.clone(),
        None,
        buffer,
        received_at_us,
        Some(local.to_string()),
        Some(peer.to_string()),
    ))
}

/// Join the configured multicast group on an existing UDP socket.
///
/// An IPv4 group uses `join_multicast_v4` with the configured interface, or
/// the unspecified address when no interface is set. An IPv6 group uses
/// `join_multicast_v6` with interface index zero when no interface is set.
/// A group that parses as neither address family returns an error.
pub fn join_multicast(
    socket: &tokio::net::UdpSocket,
    config: &UdpFeedConfig,
) -> anyhow::Result<()> {
    let group_text = config
        .group
        .as_deref()
        .context("multicast group is not configured")?;
    match group_text.parse::<Ipv4Addr>() {
        Ok(group) => {
            let interface = match config.interface.as_deref() {
                Some(text) => text
                    .parse::<Ipv4Addr>()
                    .with_context(|| format!("invalid IPv4 multicast interface `{text}`"))?,
                None => Ipv4Addr::UNSPECIFIED,
            };
            socket
                .join_multicast_v4(group, interface)
                .context("joining IPv4 multicast group")?;
        }
        Err(ipv4_error) => match group_text.parse::<Ipv6Addr>() {
            Ok(group) => {
                let interface = match config.interface.as_deref() {
                    Some(text) => text.parse::<u32>().with_context(|| {
                        format!("invalid IPv6 multicast interface index `{text}`")
                    })?,
                    None => 0,
                };
                socket
                    .join_multicast_v6(&group, interface)
                    .context("joining IPv6 multicast group")?;
            }
            Err(ipv6_error) => anyhow::bail!(
                "invalid multicast group `{group_text}`: IPv4: {ipv4_error}; IPv6: {ipv6_error}"
            ),
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn udp_config() -> UdpFeedConfig {
        UdpFeedConfig {
            bind_addr: "127.0.0.1".to_owned(),
            group: None,
            port: 9_001,
            interface: None,
            max_datagram_bytes: 2_048,
            receive_timeout_ms: 50,
        }
    }

    fn tcp_config() -> TcpFeedConfig {
        TcpFeedConfig {
            endpoint: "127.0.0.1:9002".to_owned(),
            reconnect_after_ms: 250,
            max_frame_bytes: 65_536,
        }
    }

    #[test]
    fn transport_kind_strings_are_stable() {
        assert_eq!(TransportKind::Tcp.as_str(), "tcp", "TCP string is stable");
        assert_eq!(TransportKind::Udp.as_str(), "udp", "UDP string is stable");
        assert_eq!(
            TransportKind::UdpMulticast.as_str(),
            "udp_multicast",
            "multicast UDP string is stable"
        );
        assert_eq!(
            TransportKind::WebSocket.as_str(),
            "web_socket",
            "WebSocket string is stable"
        );
        assert_eq!(
            TransportKind::Http.as_str(),
            "http",
            "HTTP string is stable"
        );
        assert_eq!(
            TransportKind::File.as_str(),
            "file",
            "file string is stable"
        );
        assert_eq!(TransportKind::Ipc.as_str(), "ipc", "IPC string is stable");
    }

    #[test]
    fn udp_config_validate_accepts_bounds_and_rejects_bad_fields() {
        assert!(udp_config().validate().is_ok(), "valid UDP config passes");
        let mut config = udp_config();
        config.bind_addr = String::new();
        assert!(config.validate().is_err(), "empty bind address is rejected");
        config = udp_config();
        config.port = 0;
        assert!(config.validate().is_err(), "zero port is rejected");
        config = udp_config();
        config.max_datagram_bytes = 255;
        assert!(config.validate().is_err(), "undersized buffer is rejected");
        config = udp_config();
        config.max_datagram_bytes = 65_536;
        assert!(config.validate().is_err(), "oversized buffer is rejected");
        config = udp_config();
        config.receive_timeout_ms = 0;
        assert!(config.validate().is_err(), "zero timeout is rejected");
    }

    #[test]
    fn tcp_config_validate_accepts_bounds_and_rejects_bad_fields() {
        assert!(tcp_config().validate().is_ok(), "valid TCP config passes");
        let mut config = tcp_config();
        config.endpoint = "  ".to_owned();
        assert!(config.validate().is_err(), "blank endpoint is rejected");
        config = tcp_config();
        config.reconnect_after_ms = 0;
        assert!(
            config.validate().is_err(),
            "zero reconnect delay is rejected"
        );
        config = tcp_config();
        config.max_frame_bytes = 0;
        assert!(config.validate().is_err(), "zero frame limit is rejected");
    }

    #[test]
    fn payload_digest_uses_original_payload_bytes() {
        let packet = FeedPacket::new(
            "udp",
            "127.0.0.1",
            Some(7),
            vec![0, 1, 2, 3],
            1_700_000_000_000_000,
            None,
            None,
        );
        assert_eq!(
            packet.payload_sha256(),
            sha256_hex(&[0, 1, 2, 3]),
            "digest matches the original payload"
        );
    }

    #[tokio::test]
    async fn invalid_multicast_group_is_rejected() {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind loopback UDP socket");
        let mut config = udp_config();
        config.group = Some("not-an-address".to_owned());
        assert!(
            join_multicast(&socket, &config).is_err(),
            "bad group is rejected"
        );
    }

    #[tokio::test]
    async fn recv_udp_packet_receives_loopback_datagram() {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind receiving UDP socket");
        let local: SocketAddr = socket.local_addr().expect("read receiving socket address");
        let mut config = udp_config();
        config.port = local.port();
        let sender = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind sending UDP socket");
        sender
            .send_to(b"loopback-packet", local)
            .await
            .expect("send loopback datagram");
        let packet = recv_udp_packet(&socket, &config)
            .await
            .expect("receive loopback datagram");
        assert_eq!(
            packet.payload,
            b"loopback-packet".to_vec(),
            "payload matches the sent datagram"
        );
        assert!(packet.received_at_us > 0, "receive timestamp is populated");
        let expected_local = local.to_string();
        assert_eq!(
            packet.local_addr.as_deref(),
            Some(expected_local.as_str()),
            "local address matches the bound socket"
        );
        assert!(packet.peer_addr.is_some(), "peer address is populated");
    }
}
