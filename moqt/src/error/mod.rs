use crate::message::message_parser::ErrorCode;
use std::string::FromUtf8Error;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    #[error("value too large for varint encoding")]
    ErrVarIntBoundsExceeded,
    #[error("unexpected buffer end")]
    ErrUnexpectedEnd,
    #[error("malformed varint")]
    ErrMalformedVarInt,
    #[error("buffer is too short")]
    ErrBufferTooShort,
    #[error("duplicate parameter")]
    ErrDuplicateParameter,
    #[error("missing parameter")]
    ErrMissingParameter,
    #[error("unsupported parameter: {0}")]
    ErrUnsupportedParameter(u64),
    #[error("invalid message type: {0}")]
    ErrInvalidMessageType(u64),
    #[error("invalid filter type: {0}")]
    ErrInvalidFilterType(u64),
    #[error("invalid boolean value: {0}")]
    ErrInvalidBooleanValue(u8),
    #[error("unsupported version: {0}")]
    ErrUnsupportedVersion(u64),
    #[error("invalid role: {0}")]
    ErrInvalidRole(u64),
    #[error("invalid object type due to {0}")]
    ErrInvalidObjectType(String),
    #[error("track or group forward preference requires length")]
    ErrTrackGroupForwardPreferenceRequiresLength,
    #[error("object status must be kNormal if payload is non-empty")]
    ErrNonEmptyPayloadMustBeWithNormalObjectStatus,
    #[error("parse error with code: {0} and reason: {1}")]
    ErrParseError(ErrorCode, String),
    #[error("frame error with reason: {0}")]
    ErrFrameError(String),
    #[error("stream error with code: {0} and reason: {1}")]
    ErrStreamError(ErrorCode, String),
    #[error("{0}")]
    ErrOther(String),
    #[error("stream not exist")]
    ErrStreamNotExisted,
    #[error("stream closed")]
    ErrStreamClosed,

    #[error("invalid string")]
    ErrInvalidString(#[from] FromUtf8Error),
}
