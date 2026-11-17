//! Kraken session stub for H0.
//!
//! H0 opens no websocket and produces no market events. The stub exists so the
//! exchange adapter satisfies the composition seam without pretending that
//! book or telemetry feeds are implemented. Later stages replace it with a
//! real session while keeping the normalized core unchanged.

use async_trait::async_trait;

use crate::{
    model::SourceId,
    session::{
        HeartbeatRequest, HeartbeatSpec, MarketDataSession, SessionFactory, SessionFailure,
        SessionFrame,
    },
};

/// H0 no-network session.
pub struct KrakenSession;

#[async_trait]
impl MarketDataSession for KrakenSession {
    fn heartbeat_spec(&self) -> HeartbeatSpec {
        HeartbeatSpec::disabled()
    }

    async fn connect(&mut self) -> Result<(), SessionFailure> {
        Ok(())
    }

    async fn apply_subscriptions(
        &mut self,
        _desired: &std::collections::BTreeSet<String>,
    ) -> Result<(), SessionFailure> {
        Ok(())
    }

    async fn next_frame(&mut self) -> Result<SessionFrame, SessionFailure> {
        Ok(SessionFrame::idle(0))
    }

    async fn send_keepalive(&mut self) -> Result<HeartbeatRequest, SessionFailure> {
        Ok(HeartbeatRequest::default())
    }

    async fn request_snapshot(&mut self, _symbols: &[String]) -> Result<(), SessionFailure> {
        Ok(())
    }

    async fn close(&mut self) {}
}

/// Factory for H0 sessions.
pub struct KrakenSessionFactory;

#[async_trait]
impl SessionFactory for KrakenSessionFactory {
    async fn create_session(
        &self,
        _source: SourceId,
    ) -> anyhow::Result<Box<dyn MarketDataSession>> {
        Ok(Box::new(KrakenSession))
    }
}
