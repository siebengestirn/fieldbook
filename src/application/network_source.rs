//! Generic line-oriented network source for the non-market-data application.
//!
//! This source is independent of the market-data collector: it speaks a tiny
//! local protocol (`HELLO <name>` / `TOKEN <declared token>`) over TCP and then
//! reuses the same [`FileSource`] line parser, duplicate index and resume
//! anchor used by the file source. The declared token identifies the stream;
//! it is not a whole-stream hash, so a resume anchor binds to the token rather
//! than claiming content completeness.
//!
//! No credential, proxy, DNS override or venue protocol is involved. Callers
//! supply an address and a token; tests use a loopback listener.

use std::io::BufReader;

use thiserror::Error;

use crate::application::file_source::{DurableCheckpoint, FileSource, SourceIdentity};

/// Errors from opening or handshaking a network source.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NetworkSourceError {
    #[error("connecting to network source `{address}`: {message}")]
    Connect { address: String, message: String },
    #[error("network handshake with `{address}`: {message}")]
    Handshake { address: String, message: String },
    #[error("network source `{address}` declared token `{found}`, expected `{expected}`")]
    TokenMismatch {
        address: String,
        expected: String,
        found: String,
    },
}

fn handshake(
    stream: &mut std::net::TcpStream,
    name: &str,
    address: &str,
    expected_token: &str,
) -> Result<(), NetworkSourceError> {
    use std::io::{Read, Write};

    let hello = format!("HELLO {name}\n");
    stream
        .write_all(hello.as_bytes())
        .map_err(|error| NetworkSourceError::Handshake {
            address: address.to_owned(),
            message: error.to_string(),
        })?;
    let mut reply = Vec::new();
    let mut byte = [0_u8; 1];
    while reply.len() < 4096 {
        let read = stream
            .read(&mut byte)
            .map_err(|error| NetworkSourceError::Handshake {
                address: address.to_owned(),
                message: error.to_string(),
            })?;
        if read == 0 {
            return Err(NetworkSourceError::Handshake {
                address: address.to_owned(),
                message: "connection closed before token".to_owned(),
            });
        }
        if byte[0] == b'\n' {
            break;
        }
        reply.push(byte[0]);
    }
    let text = String::from_utf8(reply).map_err(|error| NetworkSourceError::Handshake {
        address: address.to_owned(),
        message: format!("token is not UTF-8: {error}"),
    })?;
    let found = text
        .strip_prefix("TOKEN ")
        .ok_or_else(|| NetworkSourceError::Handshake {
            address: address.to_owned(),
            message: format!("unexpected handshake reply `{text}`"),
        })?;
    if found != expected_token {
        return Err(NetworkSourceError::TokenMismatch {
            address: address.to_owned(),
            expected: expected_token.to_owned(),
            found: found.to_owned(),
        });
    }
    Ok(())
}

/// Open a fresh network source positioned at the first line.
pub fn connect(
    address: &str,
    name: &str,
    token: &str,
) -> Result<FileSource<BufReader<std::net::TcpStream>>, NetworkSourceError> {
    let mut stream =
        std::net::TcpStream::connect(address).map_err(|error| NetworkSourceError::Connect {
            address: address.to_owned(),
            message: error.to_string(),
        })?;
    handshake(&mut stream, name, address, token)?;
    let identity = SourceIdentity::register_token(name, token);
    Ok(FileSource::new(BufReader::new(stream), identity))
}

/// Reconnect and resume from a verified anchor. The already-committed prefix
/// is discarded locally from the new stream; the server always sends the whole
/// declared stream, so no server-side cursor state is trusted.
pub fn resume(
    address: &str,
    name: &str,
    token: &str,
    anchor: &DurableCheckpoint,
    target_id: &str,
) -> Result<FileSource<BufReader<std::net::TcpStream>>, NetworkSourceError> {
    let mut stream =
        std::net::TcpStream::connect(address).map_err(|error| NetworkSourceError::Connect {
            address: address.to_owned(),
            message: error.to_string(),
        })?;
    handshake(&mut stream, name, address, token)?;
    let identity = SourceIdentity::register_token(name, token);
    FileSource::with_resume(BufReader::new(stream), identity, anchor, target_id).map_err(|error| {
        NetworkSourceError::Handshake {
            address: address.to_owned(),
            message: error.to_string(),
        }
    })
}
