// SPDX-License-Identifier: Apache-2.0
//! Acquisition plane for the global data infrastructure.
//!
//! Transport acquisition, protocol decoding, feed arbitration, connection
//! supervision and packet evidence remain separate modules. The live exchange
//! collector is unchanged; these modules are additive and tested offline.

pub mod arbiter;
pub mod codec;
pub mod packet;
pub mod supervisor;
pub mod transport;
