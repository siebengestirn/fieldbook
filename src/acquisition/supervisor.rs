// SPDX-License-Identifier: Apache-2.0
//! Typed connection supervision with bounded recovery decisions.

use serde::{Deserialize, Serialize};

/// Observable state of one source connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Degraded,
    Reconnecting,
    Backoff,
    Stopped,
}

impl ConnectionState {
    /// Return the stable contract string for this state.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Degraded => "degraded",
            Self::Reconnecting => "reconnecting",
            Self::Backoff => "backoff",
            Self::Stopped => "stopped",
        }
    }

    /// Check whether the state permits message processing.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Connected | Self::Degraded)
    }
}

/// Bounded retry and backoff policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorPolicy {
    pub max_attempts: u32,
    pub backoff_base_ms: u64,
    pub backoff_max_ms: u64,
    pub healthy_after_ms: u64,
}

impl SupervisorPolicy {
    /// Validate policy bounds.
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        anyhow::ensure!(self.max_attempts > 0, "max_attempts must be positive");
        anyhow::ensure!(self.backoff_base_ms > 0, "backoff base must be positive");
        anyhow::ensure!(
            self.backoff_max_ms >= self.backoff_base_ms,
            "backoff maximum must not be below its base"
        );
        anyhow::ensure!(self.healthy_after_ms > 0, "healthy window must be positive");
        Ok(())
    }

    /// Return the bounded delay for an attempt number.
    pub fn delay_ms(&self, attempt: u32) -> u64 {
        let shift = attempt.saturating_sub(1).min(20);
        self.backoff_base_ms
            .saturating_mul(1u64 << shift)
            .min(self.backoff_max_ms)
    }
}

/// Action produced by a supervision event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisionDecision {
    NoAction,
    Reconnect { delay_ms: u64 },
    Stop { reason: String },
}

/// State machine for one supervised source connection.
#[derive(Debug, Clone)]
pub struct ConnectionSupervisor {
    state: ConnectionState,
    attempts: u32,
    policy: SupervisorPolicy,
    last_success_us: Option<i64>,
}

impl ConnectionSupervisor {
    /// Create a disconnected supervisor under a validated policy.
    pub fn new(policy: SupervisorPolicy) -> Result<Self, anyhow::Error> {
        policy.validate()?;
        Ok(Self {
            state: ConnectionState::Disconnected,
            attempts: 0,
            policy,
            last_success_us: None,
        })
    }

    /// Return the current state.
    pub const fn state(&self) -> ConnectionState {
        self.state
    }

    /// Return the current consecutive failure count.
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Begin a connection attempt.
    pub fn begin_connect(&mut self) -> SupervisionDecision {
        self.state = ConnectionState::Connecting;
        SupervisionDecision::NoAction
    }

    /// Record a successful connection.
    pub fn on_connected(&mut self, at_us: i64) -> SupervisionDecision {
        self.state = ConnectionState::Connected;
        self.attempts = 0;
        self.last_success_us = Some(at_us);
        SupervisionDecision::NoAction
    }

    /// Record degraded operation without reconnecting.
    pub fn on_degraded(&mut self) -> SupervisionDecision {
        if self.state.is_active() {
            self.state = ConnectionState::Degraded;
        }
        SupervisionDecision::NoAction
    }

    /// Decide the next action after a failure with a machine-readable reason.
    pub fn on_failure(&mut self, reason: &str) -> SupervisionDecision {
        self.attempts = self.attempts.saturating_add(1);
        if self.attempts >= self.policy.max_attempts {
            self.state = ConnectionState::Stopped;
            return SupervisionDecision::Stop {
                reason: reason.to_owned(),
            };
        }
        self.state = ConnectionState::Backoff;
        SupervisionDecision::Reconnect {
            delay_ms: self.policy.delay_ms(self.attempts),
        }
    }

    /// Decide whether a stale connection should be renewed.
    pub fn on_stale(&mut self, now_us: i64) -> SupervisionDecision {
        let window_us =
            i64::try_from(self.policy.healthy_after_ms.saturating_mul(1000)).unwrap_or(i64::MAX);
        let stale = self
            .last_success_us
            .is_none_or(|last| last.saturating_add(window_us) <= now_us);
        if stale && self.state.is_active() {
            self.state = ConnectionState::Reconnecting;
            SupervisionDecision::Reconnect {
                delay_ms: self.policy.backoff_base_ms,
            }
        } else {
            SupervisionDecision::NoAction
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> SupervisorPolicy {
        SupervisorPolicy {
            max_attempts: 3,
            backoff_base_ms: 10,
            backoff_max_ms: 100,
            healthy_after_ms: 1000,
        }
    }

    #[test]
    fn policy_rejects_invalid_bounds() {
        let mut invalid = policy();
        invalid.backoff_max_ms = 1;
        assert!(invalid.validate().is_err(), "invalid backoff must fail");
        assert!(policy().validate().is_ok(), "valid policy must pass");
    }

    #[test]
    fn failures_are_bounded() {
        let mut supervisor = ConnectionSupervisor::new(policy()).expect("supervisor");
        supervisor.begin_connect();
        supervisor.on_connected(0);
        assert_eq!(
            supervisor.state(),
            ConnectionState::Connected,
            "connect must succeed"
        );
        assert_eq!(
            supervisor.on_failure("drop"),
            SupervisionDecision::Reconnect { delay_ms: 10 },
            "first failure must reconnect"
        );
        supervisor.on_failure("drop");
        assert_eq!(
            supervisor.on_failure("drop"),
            SupervisionDecision::Stop {
                reason: "drop".to_owned()
            },
            "exhausted attempts must stop"
        );
        assert_eq!(
            supervisor.state(),
            ConnectionState::Stopped,
            "stopped state must be observable"
        );
    }

    #[test]
    fn stale_connection_is_renewed() {
        let mut supervisor = ConnectionSupervisor::new(policy()).expect("supervisor");
        supervisor.on_connected(0);
        assert_eq!(
            supervisor.on_stale(500_000),
            SupervisionDecision::NoAction,
            "fresh connection must not reconnect"
        );
        assert_eq!(
            supervisor.on_stale(2_000_000),
            SupervisionDecision::Reconnect { delay_ms: 10 },
            "stale connection must reconnect"
        );
    }
}
