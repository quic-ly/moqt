#![warn(rust_2018_idioms)]
#![allow(dead_code)]

pub mod moqt_framer;
pub mod moqt_messages;
pub mod moqt_priority;
pub mod quic_types;
pub mod serde;
pub mod webtransport;

/// match between client and server perspective, since there may be a proxy
/// between them.
pub type StreamId = u32;
