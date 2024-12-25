use crate::message::message_parser::{ErrorCode, MessageParser, MessageParserEvent};
use crate::message::message_test::{
    create_test_message, MessageStructuredData, TestMessageBase, TestObjectDatagramMessage,
    TestObjectStreamMessage, TestStreamHeaderGroupMessage, TestStreamHeaderTrackMessage,
    TestStreamMiddlerGroupMessage, TestStreamMiddlerTrackMessage, TestSubscribeDoneMessage,
    TestSubscribeOkMessage,
};
use crate::message::object::ObjectHeader;
use crate::message::{ControlMessage, FilterType, MessageType, MAX_MESSSAGE_HEADER_SIZE};
use crate::{Error, Result, Serializer};
use bytes::Bytes;
use rstest::rstest;
use std::fmt::{Display, Formatter};

struct TestParserParams {
    message_type: MessageType,
    uses_web_transport: bool,
}

impl Display for TestParserParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}_{}",
            self.message_type,
            if self.uses_web_transport {
                "WebTransport"
            } else {
                "QUIC"
            }
        )
    }
}

impl TestParserParams {
    fn new(message_type: MessageType, uses_web_transport: bool) -> Self {
        Self {
            message_type,
            uses_web_transport,
        }
    }
}

struct TestParserVisitor {
    object_payload: Option<Bytes>,
    end_of_message: bool,
    parsing_error: Option<String>,
    parsing_error_code: ErrorCode,
    messages_received: u64,
    last_message: Option<MessageStructuredData>,
}

impl TestParserVisitor {
    fn new() -> Self {
        Self {
            object_payload: None,
            end_of_message: false,
            parsing_error: None,
            parsing_error_code: ErrorCode::NoError,
            messages_received: 0,
            last_message: None,
        }
    }

    fn handle_event(&mut self, event: MessageParserEvent) {
        match event {
            MessageParserEvent::ParsingError(code, reason) => self.on_parsing_error(code, reason),
            MessageParserEvent::ObjectMessage(message, payload, end_of_message) => {
                self.on_object_message(message, payload, end_of_message)
            }
            MessageParserEvent::ControlMessage(message) => self.on_control_message(message),
        }
    }

    fn on_parsing_error(&mut self, code: ErrorCode, reason: String) {
        self.parsing_error = Some(reason);
        self.parsing_error_code = code;
    }

    fn on_object_message(&mut self, message: ObjectHeader, payload: Bytes, end_of_message: bool) {
        self.object_payload = Some(payload);
        self.end_of_message = end_of_message;
        self.messages_received += 1;
        self.last_message = Some(MessageStructuredData::Object(message));
    }

    fn on_control_message(&mut self, message: ControlMessage) {
        self.end_of_message = true;
        self.messages_received += 1;
        self.last_message = Some(MessageStructuredData::Control(message));
    }
}

struct TestParser {
    visitor: TestParserVisitor,
    message_type: MessageType,
    uses_web_transport: bool,
    parser: MessageParser,
}

impl TestParser {
    fn new(params: &TestParserParams) -> Self {
        Self {
            visitor: TestParserVisitor::new(),
            message_type: params.message_type,
            uses_web_transport: params.uses_web_transport,
            parser: MessageParser::new(params.uses_web_transport),
        }
    }

    fn make_message(&self) -> Box<dyn TestMessageBase> {
        create_test_message(self.message_type, self.uses_web_transport)
    }
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_parse_one_message(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    tester
        .parser
        .process_data(&mut message.packet_sample(), true);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(
        1, tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_message() {
        // Check payload message.
        assert!(
            tester.visitor.object_payload.is_some(),
            "message type {:?}",
            tester.message_type
        );
        assert_eq!(
            "foo",
            tester.visitor.object_payload.as_ref().unwrap(),
            "message type {:?}",
            tester.message_type
        );
    }

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_one_message_with_long_varints(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let mut message = tester.make_message();
    assert!(
        message.expand_varints().is_ok(),
        "message type {:?}",
        tester.message_type
    );
    tester
        .parser
        .process_data(&mut message.packet_sample(), true);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(
        1, tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_message() {
        // Check payload message.
        assert_eq!(
            "foo",
            tester.visitor.object_payload.as_ref().unwrap(),
            "message type {:?}",
            tester.message_type
        );
    }
    assert!(
        !tester.visitor.parsing_error.is_some(),
        "message type {:?}",
        tester.message_type
    );

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_two_part_message(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    let total_message_size = message.packet_sample().len();
    // The test Object message has payload for less then half the message length,
    // so splitting the message in half will prevent the first half from being
    // processed.
    let mut first_data_size = total_message_size / 2;
    if tester.message_type == MessageType::StreamHeaderTrack {
        // The boundary happens to fall right after the stream header, so move it.
        first_data_size += 1;
    }
    tester
        .parser
        .process_data(&mut &message.packet_sample()[..first_data_size], false);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(
        0, tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    tester.parser.process_data(
        &mut &message.packet_sample()[first_data_size..total_message_size],
        true,
    );
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(
        1, tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );

    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_message() {
        assert_eq!(
            "foo",
            tester.visitor.object_payload.as_ref().unwrap(),
            "message type {:?}",
            tester.message_type
        );
    }
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    assert!(
        !tester.visitor.parsing_error.is_some(),
        "message type {:?}",
        tester.message_type
    );

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_one_byte_at_atime(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    let total_message_size = message.packet_sample().len();
    let object_payload_size = 3;
    for i in 0..total_message_size {
        if !tester.message_type.is_object_message() {
            assert_eq!(
                0, tester.visitor.messages_received,
                "message type {:?} at {}-th byte of {} bytes",
                tester.message_type, i, total_message_size
            );
        }
        assert!(
            !tester.visitor.end_of_message,
            "message type {:?}",
            tester.message_type
        );
        tester
            .parser
            .process_data(&mut &message.packet_sample()[i..i + 1], false);
        while let Some(event) = tester.parser.poll_event() {
            tester.visitor.handle_event(event);
        }
    }
    assert_eq!(
        if tester.message_type.is_object_message() {
            object_payload_size + 1
        } else {
            1
        },
        tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_without_payload_length() {
        assert!(
            !tester.visitor.end_of_message,
            "message type {:?}",
            tester.message_type
        );
        let empty: Vec<u8> = vec![];
        tester.parser.process_data(&mut &empty[..], true); // Needs the FIN
        while let Some(event) = tester.parser.poll_event() {
            tester.visitor.handle_event(event);
        }
        assert_eq!(
            object_payload_size + 2,
            tester.visitor.messages_received,
            "message type {:?}",
            tester.message_type
        );
    }
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    assert!(
        !tester.visitor.parsing_error.is_some(),
        "message type {:?}",
        tester.message_type
    );

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_one_byte_at_a_time_longer_varints(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let mut message = tester.make_message();
    assert!(
        message.expand_varints().is_ok(),
        "message type {:?}",
        tester.message_type
    );

    let total_message_size = message.packet_sample().len();
    let object_payload_size = 3;
    for i in 0..total_message_size {
        if !tester.message_type.is_object_message() {
            assert_eq!(
                0, tester.visitor.messages_received,
                "message type {:?}",
                tester.message_type
            );
        }
        assert!(
            !tester.visitor.end_of_message,
            "message type {:?}",
            tester.message_type
        );
        tester
            .parser
            .process_data(&mut &message.packet_sample()[i..i + 1], false);
        while let Some(event) = tester.parser.poll_event() {
            tester.visitor.handle_event(event);
        }
    }
    assert_eq!(
        if tester.message_type.is_object_message() {
            object_payload_size + 1
        } else {
            1
        },
        tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_without_payload_length() {
        assert!(
            !tester.visitor.end_of_message,
            "message type {:?}",
            tester.message_type
        );
        let empty: Vec<u8> = vec![];
        tester.parser.process_data(&mut &empty[..], true); // Needs the FIN
        while let Some(event) = tester.parser.poll_event() {
            tester.visitor.handle_event(event);
        }
        assert_eq!(
            object_payload_size + 2,
            tester.visitor.messages_received,
            "message type {:?}",
            tester.message_type
        );
    }
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    assert!(
        !tester.visitor.parsing_error.is_some(),
        "message type {:?}",
        tester.message_type
    );

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_early_fin(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    let total_message_size = message.packet_sample().len();
    let mut first_data_size = total_message_size / 2;
    if tester.message_type == MessageType::StreamHeaderTrack {
        // The boundary happens to fall right after the stream header, so move it.
        first_data_size += 1;
    }
    tester
        .parser
        .process_data(&mut &message.packet_sample()[..first_data_size], true);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(0, tester.visitor.messages_received);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("FIN after incomplete message".to_string())
    );
    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_separate_early_fin(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    let total_message_size = message.packet_sample().len();
    let mut first_data_size = total_message_size / 2;
    if tester.message_type == MessageType::StreamHeaderTrack {
        // The boundary happens to fall right after the stream header, so move it.
        first_data_size += 1;
    }
    tester
        .parser
        .process_data(&mut &message.packet_sample()[..first_data_size], false);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    let empty: Vec<u8> = vec![];
    tester.parser.process_data(&mut &empty[..], true);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("End of stream before complete message".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );
    Ok(())
}

const K_WEB_TRANS: bool = true;
const K_RAW_QUIC: bool = false;

// Tests for message-specific error cases, and behaviors for a single message
// type.
struct TestMessageSpecific {
    visitor: TestParserVisitor,
}

impl TestMessageSpecific {
    fn new() -> Self {
        Self {
            visitor: TestParserVisitor::new(),
        }
    }
}

#[test]
fn test_object_stream_separate_fin() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    // OBJECT can return on an unknown-length message even without receiving a
    // FIN.
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let message = TestObjectStreamMessage::new();
    parser.process_data(&mut message.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"foo"))
    );
    assert!(!tester.visitor.end_of_message);

    let empty: Vec<u8> = vec![];
    parser.process_data(&mut &empty[..], true); // send the FIN
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 2);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(tester.visitor.object_payload, Some(Bytes::from_static(b"")));
    assert!(tester.visitor.end_of_message);
    assert!(!tester.visitor.parsing_error.is_some());
    Ok(())
}

// Send the header + some payload, pure payload, then pure payload to end the
// message.
#[test]
fn test_three_part_object() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let message = TestObjectStreamMessage::new();
    parser.process_data(&mut message.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(!tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"foo"))
    );

    // second part
    parser.process_data(&mut Bytes::from_static(b"bar"), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 2);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(!tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"bar"))
    );

    // third part includes FIN
    parser.process_data(&mut Bytes::from_static(b"deadbeef"), true);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 3);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"deadbeef"))
    );
    assert!(!tester.visitor.parsing_error.is_some());

    Ok(())
}

// Send the part of header, rest of header + payload, plus payload.
#[test]
fn test_three_part_object_first_incomplete() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let mut message = TestObjectStreamMessage::new();

    // first part
    parser.process_data(&mut &message.packet_sample()[0..4], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);

    // second part. Add padding to it.
    message.set_wire_image_size(100);
    parser.process_data(
        &mut &message.packet_sample()[4..message.total_message_size()],
        false,
    );
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(!tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    // The value "93" is the overall wire image size of 100 minus the non-payload
    // part of the message.
    assert_eq!(tester.visitor.object_payload.as_ref().unwrap().len(), 93);

    // third part includes FIN
    parser.process_data(&mut Bytes::from_static(b"bar"), true);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 2);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"bar"))
    );
    assert!(!tester.visitor.parsing_error.is_some());

    Ok(())
}

#[test]
fn test_stream_header_group_follow_on() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    // first part
    let message1 = TestStreamHeaderGroupMessage::new();
    parser.process_data(&mut message1.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message1.equal_field_values(last_message));
    assert!(tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"foo"))
    );
    assert!(!tester.visitor.parsing_error.is_some());
    // second part
    let message2 = TestStreamMiddlerGroupMessage::new();
    parser.process_data(&mut message2.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 2);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message2.equal_field_values(last_message));
    assert!(tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"bar"))
    );
    assert!(!tester.visitor.parsing_error.is_some());

    Ok(())
}

#[test]
fn test_stream_header_track_follow_on() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    // first part
    let message1 = TestStreamHeaderTrackMessage::new();
    parser.process_data(&mut message1.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message1.equal_field_values(last_message));
    assert!(tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"foo"))
    );
    assert!(!tester.visitor.parsing_error.is_some());
    // second part
    let message2 = TestStreamMiddlerTrackMessage::new();
    parser.process_data(&mut message2.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 2);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message2.equal_field_values(last_message));
    assert!(tester.visitor.end_of_message);
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"bar"))
    );
    assert!(!tester.visitor.parsing_error.is_some());

    Ok(())
}

#[test]
fn test_client_setup_role_is_invalid() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x40, 0x02, 0x01, 0x02, // versions
        0x02, // 2 params
        0x00, 0x01, 0x04, // role = invalid
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Invalid ROLE parameter".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_server_setup_role_is_invalid() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x41, 0x01, 0x02, // 2 param
        0x00, 0x01, 0x04, // role = invalid
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Invalid ROLE parameter".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_setup_role_appears_twice() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x40, 0x02, 0x01, 0x02, // versions
        0x03, // 3 params
        0x00, 0x01, 0x03, // role = PubSub
        0x00, 0x01, 0x03, // role = PubSub
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("ROLE parameter appears twice in SETUP".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_client_setup_role_is_missing() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x40, 0x02, 0x01, 0x02, // versions = 1, 2
        0x01, // 1 param
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("ROLE parameter missing from CLIENT_SETUP message".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_server_setup_role_is_missing() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x41, 0x01, 0x00, // 1 param
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("ROLE parameter missing from SERVER_SETUP message".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_setup_role_varint_length_is_wrong() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x40, // type
        0x02, 0x01, 0x02, // versions
        0x02, // 2 parameters
        0x00, 0x02, 0x03, // role = PubSub, but length is 2
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Parameter length does not match varint encoding".to_string())
    );

    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ParameterLengthMismatch
    );

    Ok(())
}

#[test]
fn test_setup_path_from_server() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x41, 0x01, // version = 1
        0x01, // 1 param
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("PATH parameter in SERVER_SETUP".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_setup_path_appears_twice() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x40, 0x02, 0x01, 0x02, // versions = 1, 2
        0x03, // 3 params
        0x00, 0x01, 0x03, // role = PubSub
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("PATH parameter appears twice in CLIENT_SETUP".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_setup_path_over_webtrans() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_WEB_TRANS);
    let setup = vec![
        0x40, 0x40, 0x02, 0x01, 0x02, // versions = 1, 2
        0x02, // 2 params
        0x00, 0x01, 0x03, // role = PubSub
        0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("WebTransport connection is using PATH parameter in SETUP".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_setup_path_missing() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let setup = vec![
        0x40, 0x40, 0x02, 0x01, 0x02, // versions = 1, 2
        0x01, // 1 param
        0x00, 0x01, 0x03, // role = PubSub
    ];
    parser.process_data(&mut &setup[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("PATH SETUP parameter missing from Client message over QUIC".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_subscribe_authorization_info_twice() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_WEB_TRANS);
    let subscribe = vec![
        0x03, 0x01, 0x02, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x02, // filter_type = kLatestObject
        0x02, // two params
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("AUTHORIZATION_INFO parameter appears twice in SUBSCRIBE".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_subscribe_update_authorization_info_twice() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_WEB_TRANS);
    let subscribe_update = vec![
        0x02, 0x02, 0x03, 0x01, 0x05, 0x06, // start and end sequences
        0x02, // 2 parameters
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe_update[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("AUTHORIZATION_INFO parameter appears twice in SUBSCRIBE_UPDATE".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_announce_authorization_info_twice() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_WEB_TRANS);
    let announce = vec![
        0x06, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x02, // 2 params
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &announce[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("AUTHORIZATION_INFO parameter appears twice in ANNOUNCE".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_fin_mid_payload() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let message = TestStreamHeaderGroupMessage::new();
    parser.process_data(
        &mut &message.packet_sample()[..message.total_message_size() - 1],
        true,
    );
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Received FIN mid-payload".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_partial_payload_then_fin() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let message = TestStreamHeaderTrackMessage::new();
    parser.process_data(
        &mut &message.packet_sample()[..message.total_message_size() - 1],
        false,
    );
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    parser.process_data(&mut Bytes::new(), true);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("End of stream before complete OBJECT PAYLOAD".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_data_after_fin() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    parser.process_data(&mut Bytes::new(), true); // Find FIN
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    parser.process_data(&mut Bytes::from_static(b"foo"), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Data after end of stream".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_non_normal_object_has_payload() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let object_stream = vec![
        0x00, 0x03, 0x04, 0x05, 0x06, 0x07, 0x02, // varints
        0x66, 0x6f, 0x6f, // payload = "foo"
    ];
    parser.process_data(&mut &object_stream[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Object with non-normal status has payload".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_invalid_object_status() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let object_stream = vec![
        0x00, 0x03, 0x04, 0x05, 0x06, 0x07, 0x06, // varints
        0x66, 0x6f, 0x6f, // payload = "foo"
    ];
    parser.process_data(&mut &object_stream[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Invalid object status".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ErrorCode::ProtocolViolation
    );

    Ok(())
}

#[test]
fn test_setup2kb() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let mut writer = vec![];
    (MessageType::ServerSetup as u64).serialize(&mut writer)?;
    0x1u64.serialize(&mut writer)?; // version
    0x1u64.serialize(&mut writer)?; // num_params
    0xbeefu64.serialize(&mut writer)?; // unknown param
    MAX_MESSSAGE_HEADER_SIZE.serialize(&mut writer)?; // very long parameter
    writer.append(&mut vec![0x04u8; MAX_MESSSAGE_HEADER_SIZE]);

    // Send incomplete message
    parser.process_data(&mut &writer[..writer.len() - 1], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Cannot parse non-OBJECT messages > 2KB".to_string())
    );
    assert_eq!(tester.visitor.parsing_error_code, ErrorCode::InternalError);

    Ok(())
}

#[test]
fn test_unknown_message_type() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let mut writer = vec![];
    0xbeefu64.serialize(&mut writer)?; // unknown message type
    parser.process_data(&mut &writer[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("Unknown message type 0xbeef".to_string())
    );

    Ok(())
}

#[test]
fn test_latest_group() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe = vec![
        0x03, 0x01, 0x02, // id and alias
        0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x01, // filter_type = kLatestGroup
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    assert!(tester.visitor.last_message.is_some());
    let message = if let Some(MessageStructuredData::Control(ControlMessage::Subscribe(message))) =
        tester.visitor.last_message
    {
        message
    } else {
        assert!(false);
        return Err(Error::ErrInvalidMessageType(0));
    };
    if let FilterType::LatestGroup = message.filter_type {
        assert!(true);
    } else {
        assert!(false);
    }

    Ok(())
}

#[test]
fn test_latest_object() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe = vec![
        0x03, 0x01, 0x02, // id and alias
        0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x02, // filter_type = kLatestObject
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    assert!(!tester.visitor.parsing_error.is_some());
    let message = if let Some(MessageStructuredData::Control(ControlMessage::Subscribe(message))) =
        tester.visitor.last_message
    {
        message
    } else {
        assert!(false);
        return Err(Error::ErrInvalidMessageType(0));
    };
    if let FilterType::LatestObject = message.filter_type {
        assert!(true);
    } else {
        assert!(false);
    }

    Ok(())
}

#[test]
fn test_absolute_start() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe = vec![
        0x03, 0x01, 0x02, // id and alias
        0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x03, // filter_type = kAbsoluteStart
        0x04, // start_group = 4
        0x01, // start_object = 1
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    assert!(!tester.visitor.parsing_error.is_some());
    let message = if let Some(MessageStructuredData::Control(ControlMessage::Subscribe(message))) =
        tester.visitor.last_message
    {
        message
    } else {
        assert!(false);
        return Err(Error::ErrInvalidMessageType(0));
    };
    if let FilterType::AbsoluteStart(start) = message.filter_type {
        assert_eq!(start.group_id, 4);
        assert_eq!(start.object_id, 1);
    } else {
        assert!(false);
    }

    Ok(())
}

#[test]
fn test_absolute_range_explicit_end_object() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe = vec![
        0x03, 0x01, 0x02, // id and alias
        0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x04, // filter_type = kAbsoluteStart
        0x04, // start_group = 4
        0x01, // start_object = 1
        0x07, // end_group = 7
        0x03, // end_object = 2
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    assert!(!tester.visitor.parsing_error.is_some());
    let message = if let Some(MessageStructuredData::Control(ControlMessage::Subscribe(message))) =
        tester.visitor.last_message
    {
        message
    } else {
        assert!(false);
        return Err(Error::ErrInvalidMessageType(0));
    };
    if let FilterType::AbsoluteRange(start, end) = message.filter_type {
        assert_eq!(start.group_id, 4);
        assert_eq!(start.object_id, 1);
        assert_eq!(end.group_id, 7);
        assert_eq!(end.object_id, 2);
    } else {
        assert!(false);
    }

    Ok(())
}

#[test]
fn test_absolute_range_whole_end_group() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe = vec![
        0x03, 0x01, 0x02, // id and alias
        0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x04, // filter_type = kAbsoluteRange
        0x04, // start_group = 4
        0x01, // start_object = 1
        0x07, // end_group = 7
        0x00, // end whole group
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    assert!(!tester.visitor.parsing_error.is_some());
    let message = if let Some(MessageStructuredData::Control(ControlMessage::Subscribe(message))) =
        tester.visitor.last_message
    {
        message
    } else {
        assert!(false);
        return Err(Error::ErrInvalidMessageType(0));
    };
    if let FilterType::AbsoluteRange(start, end) = message.filter_type {
        assert_eq!(start.group_id, 4);
        assert_eq!(start.object_id, 1);
        assert_eq!(end.group_id, 7);
        assert_eq!(end.object_id, u64::MAX);
    } else {
        assert!(false);
    }

    Ok(())
}

#[test]
fn test_absolute_range_end_group_too_low() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe = vec![
        0x03, 0x01, 0x02, // id and alias
        0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x04, // filter_type = kAbsoluteRange
        0x04, // start_group = 4
        0x01, // start_object = 1
        0x03, // end_group = 3
        0x00, // end whole group
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("End group is less than start group".to_string())
    );

    Ok(())
}

#[test]
fn test_absolute_range_exactly_one_object() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe = vec![
        0x03, 0x01, 0x02, // id and alias
        0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x04, // filter_type = kAbsoluteRange
        0x04, // start_group = 4
        0x01, // start_object = 1
        0x04, // end_group = 4
        0x02, // end object = 1
        0x00, // no parameters
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);

    Ok(())
}

#[test]
fn test_subscribe_update_exactly_one_object() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe_update = vec![
        0x02, 0x02, 0x03, 0x01, 0x04, 0x07, // start and end sequences
        0x00, // No parameters
    ];
    parser.process_data(&mut &subscribe_update[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);

    Ok(())
}

#[test]
fn test_subscribe_update_end_group_too_low() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe_update = vec![
        0x02, 0x02, 0x03, 0x01, 0x03, 0x06, // start and end sequences
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe_update[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("End group is less than start group".to_string())
    );

    Ok(())
}

#[test]
fn test_absolute_range_end_object_too_low() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe = vec![
        0x03, 0x01, 0x02, // id and alias
        0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        0x04, // filter_type = kAbsoluteRange
        0x04, // start_group = 4
        0x01, // start_object = 1
        0x04, // end_group = 4
        0x01, // end_object = 0
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("End object comes before start object".to_string())
    );

    Ok(())
}

#[test]
fn test_subscribe_update_end_object_too_low() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe_update = vec![
        0x02, 0x02, 0x03, 0x02, 0x04, 0x01, // start and end sequences
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe_update[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("End object comes before start object".to_string())
    );

    Ok(())
}

#[test]
fn test_subscribe_update_no_end_group() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let subscribe_update = vec![
        0x02, 0x02, 0x03, 0x02, 0x00, 0x01, // start and end sequences
        0x01, // 1 parameter
        0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
    ];
    parser.process_data(&mut &subscribe_update[..], false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("SUBSCRIBE_UPDATE has end_object but no end_group".to_string())
    );

    Ok(())
}

static TEST_MESSAGE_TYPES: &[MessageType] = &[
    MessageType::ObjectStream, // kObjectDatagram is a unique set of tests.
    MessageType::Subscribe,
    MessageType::SubscribeOk,
    MessageType::SubscribeError,
    MessageType::SubscribeUpdate,
    MessageType::UnSubscribe,
    MessageType::SubscribeDone,
    MessageType::AnnounceCancel,
    MessageType::TrackStatusRequest,
    MessageType::TrackStatus,
    MessageType::Announce,
    MessageType::AnnounceOk,
    MessageType::AnnounceError,
    MessageType::UnAnnounce,
    MessageType::ClientSetup,
    MessageType::ServerSetup,
    MessageType::StreamHeaderTrack,
    MessageType::StreamHeaderGroup,
    MessageType::GoAway,
];

#[test]
fn test_all_messages_together() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let mut buffer = vec![0u8; 5000];
    let mut write = 0;
    let mut read = 0;
    let mut fully_received = 0;
    let mut prev_message: Option<Box<dyn TestMessageBase>> = None;
    for &message_type in TEST_MESSAGE_TYPES {
        // Each iteration, process from the halfway point of one message to the
        // halfway point of the next.
        if message_type.is_object_message() {
            continue; // Objects cannot share a stream with other messages.
        }
        let message = create_test_message(message_type, K_RAW_QUIC);
        let sample = message.packet_sample();
        let total_message_size = sample.len();
        buffer[write..write + total_message_size].copy_from_slice(sample);
        let new_read = write + total_message_size / 2;
        parser.process_data(&mut &buffer[read..new_read], false);
        while let Some(event) = parser.poll_event() {
            tester.visitor.handle_event(event);
        }
        assert_eq!(tester.visitor.messages_received, fully_received);
        if let Some(prev_message) = prev_message.as_ref() {
            let last_message = tester.visitor.last_message.as_ref().unwrap();
            assert!(prev_message.equal_field_values(last_message));
        }
        fully_received += 1;
        read = new_read;
        write += total_message_size;
        prev_message = Some(message);
    }
    // Deliver the rest
    parser.process_data(&mut &buffer[read..write], true);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, fully_received);
    if let Some(prev_message) = prev_message.as_ref() {
        let last_message = tester.visitor.last_message.as_ref().unwrap();
        assert!(prev_message.equal_field_values(last_message));
    } else {
        assert!(false);
    }
    assert!(!tester.visitor.parsing_error.is_some());

    Ok(())
}

#[test]
fn test_datagram_successful() -> Result<()> {
    let message = TestObjectDatagramMessage::new();
    let (object_header, payload) = MessageParser::process_datagram(&mut message.packet_sample())?;
    let object_metadata = MessageStructuredData::Object(object_header);
    assert!(message.equal_field_values(&object_metadata));
    assert_eq!(payload, "foo");

    Ok(())
}

#[test]
fn test_wrong_message_in_datagram() -> Result<()> {
    let message = TestObjectStreamMessage::new();
    let result = MessageParser::process_datagram(&mut message.packet_sample());
    assert!(result.is_err());
    assert_eq!(
        Err(Error::ErrParseError(
            ErrorCode::ProtocolViolation,
            "invalid datagram".to_string(),
        )),
        result
    );

    Ok(())
}

#[test]
fn test_truncated_datagram() -> Result<()> {
    let mut message = TestObjectDatagramMessage::new();
    message.set_wire_image_size(4);
    let result = MessageParser::process_datagram(&mut message.packet_sample());
    assert!(result.is_err());
    assert_eq!(Err(Error::ErrUnexpectedEnd), result);

    Ok(())
}

#[test]
fn test_very_truncated_datagram() -> Result<()> {
    let message = vec![0x40];
    let result = MessageParser::process_datagram(&mut &message[..]);
    assert!(result.is_err());
    assert_eq!(Err(Error::ErrUnexpectedEnd), result);

    Ok(())
}

#[test]
fn test_subscribe_ok_invalid_content_exists() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let mut subscribe_ok = TestSubscribeOkMessage::new();
    subscribe_ok.set_invalid_content_exists();
    parser.process_data(&mut subscribe_ok.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("SUBSCRIBE_OK ContentExists has invalid value 2".to_string())
    );

    Ok(())
}

#[test]
fn test_subscribe_done_invalid_content_exists() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let mut subscribe_done = TestSubscribeDoneMessage::new();
    subscribe_done.set_invalid_content_exists();
    parser.process_data(&mut subscribe_done.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("SUBSCRIBE_DONE ContentExists has invalid value 2".to_string())
    );

    Ok(())
}
