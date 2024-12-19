#[allow(non_camel_case_types)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum Perspective {
    #[default]
    kClient,
    kServer,
}

/// A numeric ID uniquely identifying a WebTransport stream. Note that by design,
/// those IDs are not available in the Web API, and the IDs do not necessarily
/// match between client and server perspective, since there may be a proxy
/// between them.
pub type StreamId = u32;
/// Application-specific error code used for resetting either the read or the
/// write half of the stream.
pub type StreamErrorCode = u32;
/// Application-specific error code used for closing a WebTransport session.
pub type SessionErrorCode = u32;

/// WebTransport priority as defined in
/// https://w3c.github.io/webtransport/#webtransportsendstream-write
/// The rules are as follows:
/// - Streams with the same priority are handled in FIFO order.
/// - Streams with the same group_id but different send_order are handled
///   strictly in order.
/// - Different group_ids are handled in the FIFO order.
pub type SendGroupId = u32;
pub type SendOrder = i64;
