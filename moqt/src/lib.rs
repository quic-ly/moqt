#![warn(rust_2018_idioms)]
#![allow(dead_code)]

mod connection;
mod error;
mod handler;
mod message;
mod moqt_framer;
pub mod moqt_messages;
pub mod moqt_priority;
pub mod quic_types;
mod serde;
mod session;
pub mod webtransport;

pub use error::{Error, Result};
pub use serde::{parameters::Parameters, varint::VarInt, Deserializer, Serializer};

/// match between client and server perspective, since there may be a proxy
/// between them.
pub type StreamId = u32;
