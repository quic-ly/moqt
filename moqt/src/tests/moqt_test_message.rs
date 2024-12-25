use bytes::{Buf, BufMut, BytesMut};
use std::ops::{Deref, DerefMut};
use crate::moqt_messages::{kMaxMessageHeaderSize, MoqtMessageType};
use crate::serde::data_writer::DataWriter;

pub(crate) enum MessageStructuredData {
    MoqtClientSetup,
    MoqtServerSetup,
    MoqtObject,
    MoqtSubscribe,
    MoqtSubscribeOk,
    MoqtSubscribeError,
    MoqtUnsubscribe,
    MoqtSubscribeDone,
    MoqtSubscribeUpdate,
    MoqtAnnounce,
    MoqtAnnounceOk,
    MoqtAnnounceError,
    MoqtAnnounceCancel,
    MoqtTrackStatusRequest,
    MoqtUnannounce,
    MoqtTrackStatus,
    MoqtGoAway,
    MoqtSubscribeAnnounces,
    MoqtSubscribeAnnouncesOk,
    MoqtSubscribeAnnouncesError,
    MoqtUnsubscribeAnnounces,
    MoqtMaxSubscribeId,
    MoqtFetch,
    MoqtFetchCancel,
    MoqtFetchOk,
    MoqtFetchError,
    MoqtObjectAck,
}

// Base class containing a wire image and the corresponding structured
// representation of an example of each message. It allows parser and framer
// tests to iterate through all message types without much specialized code.
pub(crate) trait TestMessageBase {
    // Returns a copy of the structured data for the message.
    fn structured_data(&self) -> MessageStructuredData;

    // Compares |values| to the derived class's structured data to make sure
    // they are equal.
    fn equal_field_values(&self, values: &MessageStructuredData) -> bool;

    // Expand all varints in the message. This is pure virtual because each
    // message has a different layout of varints.
    fn expand_varints(&mut self) -> bool;
}

pub(crate) struct TestMessage {
    message_type: MoqtMessageType,
    wire_image: [u8; kMaxMessageHeaderSize + 20],
    wire_image_size: usize,
}

impl TestMessage {
    fn new(message_type: MoqtMessageType) -> Self {
        Self {
            message_type,
            wire_image: [0u8; kMaxMessageHeaderSize + 20],
            wire_image_size: 0,
        }
    }

    fn message_type(&self) -> MoqtMessageType {
        self.message_type
    }

    // The total actual size of the message.
    pub(crate) fn total_message_size(&self) -> usize {
        self.wire_image_size
    }

    fn packet_sample(&self) -> &[u8] {
        &self.wire_image[0..self.wire_image_size]
    }

    pub(crate) fn set_wire_image_size(&mut self, wire_image_size: usize) {
        self.wire_image_size = wire_image_size;
    }

    fn set_wire_image(&mut self, wire_image: &[u8], wire_image_size: usize) {
        self.wire_image[..wire_image_size].copy_from_slice(&wire_image[..wire_image_size]);
        self.wire_image_size = wire_image_size;
    }

    // This will cause a parsing error. Do not call this on Object Messages.
    fn decrease_payload_length_by_one(&mut self) {
        let length_offset =
            0x1usize << ((self.wire_image[0] & 0xc0) >> 6);
        self.wire_image[length_offset] -= 1;
    }
    fn increase_payload_length_by_one(&mut self) {
        let length_offset =
            0x1usize << ((self.wire_image[0] & 0xc0) >> 6);
        self.wire_image[length_offset] += 1;
        self.set_wire_image_size(self.wire_image_size + 1);
    }

    // Expands all the varints in the message, alternating between making them 2,
    // 4, and 8 bytes long. Updates length fields accordingly.
    // Each character in |varints| corresponds to a byte in the original message.
    // If there is a 'v', it is a varint that should be expanded. If '-', skip
    // to the next byte.
    fn expand_varints_impl(&mut self, varints: &[u8], is_control_message: bool) -> bool {
        let mut next_varint_len = 2;
        let mut new_wire_image = BytesMut::with_capacity(kMaxMessageHeaderSize + 1);
        let mut reader = &self.wire_image[..self.wire_image_size];
        let mut writer = DataWriter::new(&mut new_wire_image);
        let mut i = 0;
        let mut length_field = 0;
        if is_control_message {
            // the length will be a 16-bit varint.
            let mut nonvarint_type = false;
            while varints[i] == b'-' {
                i+=1;
                nonvarint_type = true;
                writer.write_uint8(reader.get_u8());
            }
            let mut value: u64;
            if !nonvarint_type {
                i+=1;
                reader.ReadVarInt62(&value);
                writer.WriteVarInt62WithForcedLength(
                    value, static_cast<quiche::QuicheVariableLengthIntegerLength>(
                        next_varint_len));
                next_varint_len *= 2;
                if (next_varint_len == 16) {
                    next_varint_len = 2;
                }
            }
            reader.ReadVarInt62(&value);
            ++i;
            length_field = writer.length();
            // Write in current length as a 2B placeholder.
            writer.WriteVarInt62WithForcedLength(
                value, static_cast<quiche::QuicheVariableLengthIntegerLength>(2));
        }
        while (!reader.IsDoneReading()) {
            if (i >= varints.length() || varints[i++] == '-') {
                uint8_t byte;
                reader.ReadUInt8(&byte);
                writer.WriteUInt8(byte);
                continue;
            }
            uint64_t value;
            reader.ReadVarInt62(&value);
            writer.WriteVarInt62WithForcedLength(
                value, static_cast<quiche::QuicheVariableLengthIntegerLength>(
                    next_varint_len));
            next_varint_len *= 2;
            if (next_varint_len == 16) {
                next_varint_len = 2;
            }
        }
        memcpy(wire_image_, new_wire_image, writer.length());
        wire_image_size_ = writer.length();
        if (is_control_message) {
            wire_image_[length_field + 1] =
                static_cast<uint8_t>(writer.length() - length_field - 2);
        }



        let mut i = 0;
        while reader.has_remaining() {
            if i >= varints.len()
                || varints[{
                i += 1;
                i - 1
            }] == b'-'
            {
                writer.put_u8(reader.get_u8());
                continue;
            }
            let (value, _) = u64::deserialize(&mut reader)?;
            let _ = TestMessage::write_var_int62with_forced_length(
                value,
                &mut writer,
                next_varint_len,
            )?;
            next_varint_len *= 2;
            if next_varint_len == 16 {
                next_varint_len = 2;
            }
        }
        self.wire_image[0..writer.len()].copy_from_slice(&writer[..]);
        self.wire_image_size = writer.len();
        Ok(())
    }
}

pub(crate) fn create_test_message(
    message_type: MoqtMessageType,
    uses_web_transport: bool,
) -> Box<dyn TestMessageBase> {
    match message_type {
        MoqtMessageType::ObjectStream => Box::new(TestObjectStreamMessage::new()),
        MoqtMessageType::ObjectDatagram => Box::new(TestObjectDatagramMessage::new()),
        MoqtMessageType::SubscribeUpdate => Box::new(TestSubscribeUpdateMessage::new()),
        MoqtMessageType::Subscribe => Box::new(TestSubscribeMessage::new()),
        MoqtMessageType::SubscribeOk => Box::new(TestSubscribeOkMessage::new()),
        MoqtMessageType::SubscribeError => Box::new(TestSubscribeErrorMessage::new()),
        MoqtMessageType::Announce => Box::new(TestAnnounceMessage::new()),
        MoqtMessageType::AnnounceOk => Box::new(TestAnnounceOkMessage::new()),
        MoqtMessageType::AnnounceError => Box::new(TestAnnounceErrorMessage::new()),
        MoqtMessageType::UnAnnounce => Box::new(TestUnAnnounceMessage::new()),
        MoqtMessageType::UnSubscribe => Box::new(TestUnSubscribeMessage::new()),
        MoqtMessageType::SubscribeDone => Box::new(TestSubscribeDoneMessage::new()),
        MoqtMessageType::AnnounceCancel => Box::new(TestAnnounceCancelMessage::new()),
        MoqtMessageType::TrackStatusRequest => Box::new(TestTrackStatusRequestMessage::new()),
        MoqtMessageType::TrackStatus => Box::new(TestTrackStatusMessage::new()),
        MoqtMessageType::GoAway => Box::new(TestGoAwayMessage::new()),
        MoqtMessageType::ClientSetup => Box::new(TestClientSetupMessage::new(uses_web_transport)),
        MoqtMessageType::ServerSetup => Box::new(TestServerSetupMessage::new()),
        MoqtMessageType::StreamHeaderTrack => Box::new(TestStreamHeaderTrackMessage::new()),
        MoqtMessageType::StreamHeaderGroup => Box::new(TestStreamHeaderGroupMessage::new()),
    }
}

// Base class for the two subtypes of Object Message.
pub(crate) struct TestObjectMessage {
    base: TestMessage,
    object_header: ObjectHeader,
}

impl TestObjectMessage {
    fn new(message_type: MoqtMessageType) -> Self {
        Self {
            base: TestMessage::new(message_type),
            object_header: ObjectHeader {
                subscribe_id: 3,
                track_alias: 4,
                group_id: 5,
                object_id: 6,
                object_send_order: 7,
                object_status: ObjectStatus::Normal,
                object_forwarding_preference: message_type
                    .get_object_forwarding_preference()
                    .unwrap(),
                object_payload_length: None,
            },
        }
    }
}

impl Deref for TestObjectMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestObjectMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestObjectMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Object(self.object_header)
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Object(object_header) = values {
            object_header
        } else {
            return false;
        };
        if cast.subscribe_id != self.object_header.subscribe_id {
            return false;
        }
        if cast.track_alias != self.object_header.track_alias {
            return false;
        }
        if cast.group_id != self.object_header.group_id {
            return false;
        }
        if cast.object_id != self.object_header.object_id {
            return false;
        }
        if cast.object_send_order != self.object_header.object_send_order {
            return false;
        }
        if cast.object_status != self.object_header.object_status {
            return false;
        }
        if cast.object_forwarding_preference != self.object_header.object_forwarding_preference {
            return false;
        }
        if cast.object_payload_length != self.object_header.object_payload_length {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        // do nothing.
        Ok(())
    }
}

pub(crate) struct TestObjectStreamMessage {
    pub(crate) base: TestObjectMessage,
    pub(crate) raw_packet: Vec<u8>,
}

impl TestObjectStreamMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestObjectMessage::new(MoqtMessageType::ObjectStream);
        let raw_packet = vec![
            0x00, 0x03, 0x04, 0x05, 0x06, 0x07, 0x00, // varints
            0x66, 0x6f, 0x6f, // payload = "foo"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());
        Self { base, raw_packet }
    }
}

impl Deref for TestObjectStreamMessage {
    type Target = TestObjectMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestObjectStreamMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestObjectStreamMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        self.base.structured_data()
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        self.base.equal_field_values(values)
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvvvvvv---".as_bytes()) // first six fields are varints
    }
}

pub(crate) struct TestObjectDatagramMessage {
    base: TestObjectMessage,
    raw_packet: Vec<u8>,
}

impl TestObjectDatagramMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestObjectMessage::new(MoqtMessageType::ObjectDatagram);
        let raw_packet = vec![
            0x01, 0x03, 0x04, 0x05, 0x06, 0x07, 0x00, // varints
            0x66, 0x6f, 0x6f, // payload = "foo"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());
        Self { base, raw_packet }
    }
}

impl Deref for TestObjectDatagramMessage {
    type Target = TestObjectMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestObjectDatagramMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestObjectDatagramMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        self.base.structured_data()
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        self.base.equal_field_values(values)
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvvvvvv---".as_bytes()) // first six fields are varints
    }
}

// Concatentation of the base header and the object-specific header. Follow-on
// object headers are handled in a different class.
pub(crate) struct TestStreamHeaderTrackMessage {
    base: TestObjectMessage,
    raw_packet: Vec<u8>,
}

impl TestStreamHeaderTrackMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestObjectMessage::new(MoqtMessageType::StreamHeaderTrack);
        // Some tests check that a FIN sent at the halfway point of a message results
        // in an error. Without the unnecessary expanded varint 0x0405, the halfway
        // point falls at the end of the Stream Header, which is legal. Expand the
        // varint so that the FIN would be illegal.
        let raw_packet = vec![
            0x40, 0x50, // two byte type field
            0x03, 0x04, 0x07, // varints
            0x05, 0x06, // object middler
            0x03, 0x66, 0x6f, 0x6f, // payload = "foo"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());
        base.object_header.object_payload_length = Some(3);

        Self { base, raw_packet }
    }
}

impl Deref for TestStreamHeaderTrackMessage {
    type Target = TestObjectMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestStreamHeaderTrackMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestStreamHeaderTrackMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        self.base.structured_data()
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        self.base.equal_field_values(values)
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("--vvvvvv".as_bytes()) // six one-byte varints
    }
}

pub(crate) struct TestStreamMiddlerTrackMessage {
    base: TestObjectMessage,
    raw_packet: Vec<u8>,
}

impl TestStreamMiddlerTrackMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestObjectMessage::new(MoqtMessageType::StreamHeaderTrack);
        let raw_packet = vec![
            0x09, 0x0a, // object middler
            0x03, 0x62, 0x61, 0x72, // payload = "bar"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());
        base.object_header.object_payload_length = Some(3);
        base.object_header.group_id = 9;
        base.object_header.object_id = 10;

        Self { base, raw_packet }
    }
}

impl Deref for TestStreamMiddlerTrackMessage {
    type Target = TestObjectMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestStreamMiddlerTrackMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestStreamMiddlerTrackMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        self.base.structured_data()
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        self.base.equal_field_values(values)
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvv".as_bytes())
    }
}

pub(crate) struct TestStreamHeaderGroupMessage {
    base: TestObjectMessage,
    raw_packet: Vec<u8>,
}

impl TestStreamHeaderGroupMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestObjectMessage::new(MoqtMessageType::StreamHeaderGroup);
        let raw_packet = vec![
            0x40, 0x51, // two-byte type field
            0x03, 0x04, 0x05, 0x07, // varints
            0x06, 0x03, 0x66, 0x6f, 0x6f, // object middler; payload = "foo"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());
        base.object_header.object_payload_length = Some(3);

        Self { base, raw_packet }
    }
}

impl Deref for TestStreamHeaderGroupMessage {
    type Target = TestObjectMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestStreamHeaderGroupMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestStreamHeaderGroupMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        self.base.structured_data()
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        self.base.equal_field_values(values)
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("--vvvvvv".as_bytes()) // six one-byte varints
    }
}

pub(crate) struct TestStreamMiddlerGroupMessage {
    base: TestObjectMessage,
    raw_packet: Vec<u8>,
}

impl TestStreamMiddlerGroupMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestObjectMessage::new(MoqtMessageType::StreamHeaderGroup);
        let raw_packet = vec![
            0x09, 0x03, 0x62, 0x61, 0x72, // object middler; payload = "bar"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());
        base.object_header.object_payload_length = Some(3);
        base.object_header.object_id = 9;

        Self { base, raw_packet }
    }
}

impl Deref for TestStreamMiddlerGroupMessage {
    type Target = TestObjectMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestStreamMiddlerGroupMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestStreamMiddlerGroupMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        self.base.structured_data()
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        self.base.equal_field_values(values)
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvv".as_bytes())
    }
}

pub(crate) struct TestClientSetupMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    client_setup: ClientSetup,
}

impl TestClientSetupMessage {
    pub(crate) fn new(webtrans: bool) -> Self {
        let mut base = TestMessage::new(MoqtMessageType::ClientSetup);
        let mut client_setup = ClientSetup {
            supported_versions: vec![Version::Unsupported(0x01), Version::Unsupported(0x02)],
            role: Some(Role::PubSub),
            path: Some("foo".to_string()),
            ..Default::default()
        };
        let mut raw_packet = vec![
            0x40, 0x40, // type
            0x02, 0x01, 0x02, // versions
            0x02, // 2 parameters
            0x00, 0x01, 0x03, // role = PubSub
            0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
        ];
        if webtrans {
            client_setup.path = None;
            raw_packet[5] = 0x01; // only one parameter
            base.set_wire_image(&raw_packet, raw_packet.len() - 5);
        } else {
            base.set_wire_image(&raw_packet, raw_packet.len());
        }

        Self {
            base,
            raw_packet,
            client_setup,
        }
    }
}

impl Deref for TestClientSetupMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestClientSetupMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestClientSetupMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::ClientSetup(self.client_setup.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::ClientSetup(cast)) = values
        {
            cast
        } else {
            return false;
        };
        if cast.supported_versions.len() != self.client_setup.supported_versions.len() {
            return false;
        }
        for i in 0..cast.supported_versions.len() {
            // Listed versions are 1 and 2, in that order.
            if cast.supported_versions[i] != self.client_setup.supported_versions[i] {
                return false;
            }
        }
        if cast.role != self.client_setup.role {
            return false;
        }
        if cast.path != self.client_setup.path {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        if self.client_setup.path.is_some() {
            self.expand_varints_impl("--vvvvvv-vv---".as_bytes())
            // first two bytes are already a 2B varint. Also, don't expand parameter
            // varints because that messes up the parameter length field.
        } else {
            self.expand_varints_impl("--vvvvvv-".as_bytes())
        }
    }
}

pub(crate) struct TestServerSetupMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    server_setup: ServerSetup,
}

impl TestServerSetupMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::ServerSetup);
        let server_setup = ServerSetup {
            supported_version: Version::Unsupported(0x01),
            role: Some(Role::PubSub),
        };
        let raw_packet = vec![
            0x40, 0x41, // type
            0x01, 0x01, // version, one param
            0x00, 0x01, 0x03, // role = PubSub
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            server_setup,
        }
    }
}

impl Deref for TestServerSetupMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestServerSetupMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestServerSetupMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::ServerSetup(self.server_setup.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::ServerSetup(cast)) = values
        {
            cast
        } else {
            return false;
        };
        if cast.supported_version != self.server_setup.supported_version {
            return false;
        }
        if cast.role != self.server_setup.role {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("--vvvv-".as_bytes())
    }
}

pub(crate) struct TestSubscribeMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    subscribe: Subscribe,
}

impl TestSubscribeMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::Subscribe);
        let subscribe = Subscribe {
            subscribe_id: 1,
            track_alias: 2,
            track_namespace: "foo".to_string(),
            track_name: "abcd".to_string(),
            filter_type: FilterType::AbsoluteStart(FullSequence {
                group_id: 4,
                object_id: 1,
            }),
            authorization_info: Some("bar".to_string()),
        };
        let raw_packet = vec![
            0x03, 0x01, 0x02, // id and alias
            0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
            0x03, // Filter type: Absolute Start
            0x04, // start_group = 4 (relative previous)
            0x01, // start_object = 1 (absolute)
            // No EndGroup or EndObject
            0x01, // 1 parameter
            0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            subscribe,
        }
    }
}

impl Deref for TestSubscribeMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestSubscribeMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestSubscribeMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::Subscribe(self.subscribe.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::Subscribe(cast)) = values {
            cast
        } else {
            return false;
        };
        if cast.subscribe_id != self.subscribe.subscribe_id {
            return false;
        }
        if cast.track_alias != self.subscribe.track_alias {
            return false;
        }
        if cast.track_namespace != self.subscribe.track_namespace {
            return false;
        }
        if cast.track_name != self.subscribe.track_name {
            return false;
        }
        if cast.filter_type != self.subscribe.filter_type {
            return false;
        }
        if cast.authorization_info != self.subscribe.authorization_info {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvvv---v----vvvvvv---".as_bytes())
    }
}

pub(crate) struct TestSubscribeOkMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    subscribe_ok: SubscribeOk,
}

impl TestSubscribeOkMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::SubscribeOk);
        let subscribe_ok = SubscribeOk {
            subscribe_id: 1,
            expires: 3,
            largest_group_object: Some(FullSequence {
                group_id: 12,
                object_id: 20,
            }),
        };
        let raw_packet = vec![
            0x04, 0x01, 0x03, // subscribe_id = 1, expires = 3
            0x01, 0x0c, 0x14, // largest_group_id = 12, largest_object_id = 20,
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            subscribe_ok,
        }
    }

    pub(crate) fn set_invalid_content_exists(&mut self) {
        self.raw_packet[3] = 0x02;
        let size = self.raw_packet.len();
        let raw = self.raw_packet.clone();
        self.wire_image[..size].copy_from_slice(&raw[..size]);
        self.wire_image_size = size;
    }
}

impl Deref for TestSubscribeOkMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestSubscribeOkMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestSubscribeOkMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::SubscribeOk(self.subscribe_ok.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::SubscribeOk(cast)) = values
        {
            cast
        } else {
            return false;
        };
        if cast.subscribe_id != self.subscribe_ok.subscribe_id {
            return false;
        }
        if cast.expires != self.subscribe_ok.expires {
            return false;
        }
        if cast.largest_group_object != self.subscribe_ok.largest_group_object {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvv-vv".as_bytes())
    }
}

pub(crate) struct TestSubscribeErrorMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    subscribe_error: SubscribeError,
}

impl TestSubscribeErrorMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::SubscribeError);
        let subscribe_error = SubscribeError {
            subscribe_id: 2,
            error_code: SubscribeErrorCode::InvalidRange as u64,
            reason_phrase: "bar".to_string(),
            track_alias: 4,
        };
        let raw_packet = vec![
            0x05, 0x02, // subscribe_id = 2
            0x01, // error_code = 1
            0x03, 0x62, 0x61, 0x72, // reason_phrase = "bar"
            0x04, // track_alias = 4,
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            subscribe_error,
        }
    }
}

impl Deref for TestSubscribeErrorMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestSubscribeErrorMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestSubscribeErrorMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::SubscribeError(self.subscribe_error.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast =
            if let MessageStructuredData::Control(ControlMessage::SubscribeError(cast)) = values {
                cast
            } else {
                return false;
            };
        if cast.subscribe_id != self.subscribe_error.subscribe_id {
            return false;
        }
        if cast.error_code != self.subscribe_error.error_code {
            return false;
        }
        if cast.reason_phrase != self.subscribe_error.reason_phrase {
            return false;
        }
        if cast.track_alias != self.subscribe_error.track_alias {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvvv---v".as_bytes())
    }
}

pub(crate) struct TestUnSubscribeMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    un_subscribe: UnSubscribe,
}

impl TestUnSubscribeMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::UnSubscribe);
        let un_subscribe = UnSubscribe { subscribe_id: 3 };
        let raw_packet = vec![
            0x0a, 0x03, // subscribe_id = 3
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            un_subscribe,
        }
    }
}

impl Deref for TestUnSubscribeMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestUnSubscribeMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestUnSubscribeMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::UnSubscribe(self.un_subscribe.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::UnSubscribe(cast)) = values
        {
            cast
        } else {
            return false;
        };
        if cast.subscribe_id != self.un_subscribe.subscribe_id {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv".as_bytes())
    }
}

pub(crate) struct TestSubscribeDoneMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    subscribe_done: SubscribeDone,
}

impl TestSubscribeDoneMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::SubscribeDone);
        let subscribe_done = SubscribeDone {
            subscribe_id: 2,
            status_code: 3,
            reason_phrase: "hi".to_string(),
            final_group_object: Some(FullSequence {
                group_id: 8,
                object_id: 12,
            }),
        };
        let raw_packet = vec![
            0x0b, 0x02, 0x03, // subscribe_id = 2, error_code = 3,
            0x02, 0x68, 0x69, // reason_phrase = "hi"
            0x01, 0x08, 0x0c, // final_id = (8,12)
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            subscribe_done,
        }
    }

    pub(crate) fn set_invalid_content_exists(&mut self) {
        self.raw_packet[6] = 0x02;
        let size = self.raw_packet.len();
        let raw = self.raw_packet.clone();
        self.wire_image[..size].copy_from_slice(&raw[..size]);
        self.wire_image_size = size;
    }
}

impl Deref for TestSubscribeDoneMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestSubscribeDoneMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestSubscribeDoneMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::SubscribeDone(self.subscribe_done.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast =
            if let MessageStructuredData::Control(ControlMessage::SubscribeDone(cast)) = values {
                cast
            } else {
                return false;
            };
        if cast.subscribe_id != self.subscribe_done.subscribe_id {
            return false;
        }
        if cast.status_code != self.subscribe_done.status_code {
            return false;
        }
        if cast.reason_phrase != self.subscribe_done.reason_phrase {
            return false;
        }
        if cast.final_group_object != self.subscribe_done.final_group_object {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvvv---vv".as_bytes())
    }
}

pub(crate) struct TestSubscribeUpdateMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    subscribe_update: SubscribeUpdate,
}

impl TestSubscribeUpdateMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::SubscribeUpdate);
        let subscribe_update = SubscribeUpdate {
            subscribe_id: 2,
            start_group_object: FullSequence {
                group_id: 3,
                object_id: 1,
            },
            end_group_object: Some(FullSequence {
                group_id: 4,
                object_id: 5,
            }),
            authorization_info: Some("bar".to_string()),
        };
        let raw_packet = vec![
            0x02, 0x02, 0x03, 0x01, 0x05, 0x06, // start and end sequences
            0x01, // 1 parameter
            0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            subscribe_update,
        }
    }
}

impl Deref for TestSubscribeUpdateMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestSubscribeUpdateMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestSubscribeUpdateMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::SubscribeUpdate(
            self.subscribe_update.clone(),
        ))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast =
            if let MessageStructuredData::Control(ControlMessage::SubscribeUpdate(cast)) = values {
                cast
            } else {
                return false;
            };
        if cast.subscribe_id != self.subscribe_update.subscribe_id {
            return false;
        }
        if cast.start_group_object != self.subscribe_update.start_group_object {
            return false;
        }
        if cast.end_group_object != self.subscribe_update.end_group_object {
            return false;
        }
        if cast.authorization_info != self.subscribe_update.authorization_info {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vvvvvvvvv---".as_bytes())
    }
}

pub(crate) struct TestAnnounceMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    announce: Announce,
}

impl TestAnnounceMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::Announce);
        let announce = Announce {
            track_namespace: "foo".to_string(),
            authorization_info: Some("bar".to_string()),
        };
        let raw_packet = vec![
            0x06, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x01, // 1 parameter
            0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            announce,
        }
    }
}

impl Deref for TestAnnounceMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestAnnounceMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestAnnounceMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::Announce(self.announce.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::Announce(cast)) = values {
            cast
        } else {
            return false;
        };
        if cast.track_namespace != self.announce.track_namespace {
            return false;
        }
        if cast.authorization_info != self.announce.authorization_info {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv---vvv---".as_bytes())
    }
}

pub(crate) struct TestAnnounceOkMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    announce_ok: AnnounceOk,
}

impl TestAnnounceOkMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::AnnounceOk);
        let announce_ok = AnnounceOk {
            track_namespace: "foo".to_string(),
        };
        let raw_packet = vec![
            0x07, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            announce_ok,
        }
    }
}

impl Deref for TestAnnounceOkMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestAnnounceOkMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestAnnounceOkMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::AnnounceOk(self.announce_ok.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::AnnounceOk(cast)) = values
        {
            cast
        } else {
            return false;
        };
        if cast.track_namespace != self.announce_ok.track_namespace {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv---".as_bytes())
    }
}

pub(crate) struct TestAnnounceErrorMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    announce_error: AnnounceError,
}

impl TestAnnounceErrorMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::AnnounceError);
        let announce_error = AnnounceError {
            track_namespace: "foo".to_string(),
            error_code: 1,
            reason_phrase: "bar".to_string(),
        };
        let raw_packet = vec![
            0x08, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x01, // error_code = 1
            0x03, 0x62, 0x61, 0x72, // reason_phrase = "bar"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            announce_error,
        }
    }
}

impl Deref for TestAnnounceErrorMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestAnnounceErrorMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestAnnounceErrorMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::AnnounceError(self.announce_error.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast =
            if let MessageStructuredData::Control(ControlMessage::AnnounceError(cast)) = values {
                cast
            } else {
                return false;
            };
        if cast.track_namespace != self.announce_error.track_namespace {
            return false;
        }
        if cast.error_code != self.announce_error.error_code {
            return false;
        }
        if cast.reason_phrase != self.announce_error.reason_phrase {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv---vv---".as_bytes())
    }
}

pub(crate) struct TestAnnounceCancelMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    announce_cancel: AnnounceCancel,
}

impl TestAnnounceCancelMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::AnnounceCancel);
        let announce_cancel = AnnounceCancel {
            track_namespace: "foo".to_string(),
        };
        let raw_packet = vec![
            0x0c, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            announce_cancel,
        }
    }
}

impl Deref for TestAnnounceCancelMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestAnnounceCancelMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestAnnounceCancelMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::AnnounceCancel(self.announce_cancel.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast =
            if let MessageStructuredData::Control(ControlMessage::AnnounceCancel(cast)) = values {
                cast
            } else {
                return false;
            };
        if cast.track_namespace != self.announce_cancel.track_namespace {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv---".as_bytes())
    }
}

pub(crate) struct TestUnAnnounceMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    un_announce: UnAnnounce,
}

impl TestUnAnnounceMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::UnAnnounce);
        let un_announce = UnAnnounce {
            track_namespace: "foo".to_string(),
        };
        let raw_packet = vec![
            0x09, 0x03, 0x66, 0x6f, 0x6f, // track_namespace
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            un_announce,
        }
    }
}

impl Deref for TestUnAnnounceMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestUnAnnounceMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestUnAnnounceMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::UnAnnounce(self.un_announce.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::UnAnnounce(cast)) = values
        {
            cast
        } else {
            return false;
        };
        if cast.track_namespace != self.un_announce.track_namespace {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv---".as_bytes())
    }
}

pub(crate) struct TestTrackStatusRequestMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    track_status_request: TrackStatusRequest,
}

impl TestTrackStatusRequestMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::TrackStatusRequest);
        let track_status_request = TrackStatusRequest {
            track_namespace: "foo".to_string(),
            track_name: "abcd".to_string(),
        };
        let raw_packet = vec![
            0x0d, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            track_status_request,
        }
    }
}

impl Deref for TestTrackStatusRequestMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestTrackStatusRequestMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestTrackStatusRequestMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::TrackStatusRequest(
            self.track_status_request.clone(),
        ))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::TrackStatusRequest(cast)) =
            values
        {
            cast
        } else {
            return false;
        };
        if cast.track_namespace != self.track_status_request.track_namespace {
            return false;
        }
        if cast.track_name != self.track_status_request.track_name {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv---v----".as_bytes())
    }
}

pub(crate) struct TestTrackStatusMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    track_status: TrackStatus,
}

impl TestTrackStatusMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::TrackStatus);
        let track_status = TrackStatus {
            track_namespace: "foo".to_string(),
            track_name: "abcd".to_string(),
            status_code: TrackStatusCode::InProgress as u64,
            last_group_object: FullSequence {
                group_id: 12,
                object_id: 20,
            },
        };
        let raw_packet = vec![
            0x0e, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
            0x00, 0x0c, 0x14, // status, last_group, last_object
        ];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            track_status,
        }
    }
}

impl Deref for TestTrackStatusMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestTrackStatusMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestTrackStatusMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::TrackStatus(self.track_status.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::TrackStatus(cast)) = values
        {
            cast
        } else {
            return false;
        };
        if cast.track_namespace != self.track_status.track_namespace {
            return false;
        }
        if cast.track_name != self.track_status.track_name {
            return false;
        }
        if cast.status_code != self.track_status.status_code {
            return false;
        }
        if cast.last_group_object != self.track_status.last_group_object {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv---v----vvv".as_bytes())
    }
}

pub(crate) struct TestGoAwayMessage {
    base: TestMessage,
    raw_packet: Vec<u8>,
    go_away: GoAway,
}

impl TestGoAwayMessage {
    pub(crate) fn new() -> Self {
        let mut base = TestMessage::new(MoqtMessageType::GoAway);
        let go_away = GoAway {
            new_session_uri: "foo".to_string(),
        };
        let raw_packet = vec![0x10, 0x03, 0x66, 0x6f, 0x6f];
        base.set_wire_image(&raw_packet, raw_packet.len());

        Self {
            base,
            raw_packet,
            go_away,
        }
    }
}

impl Deref for TestGoAwayMessage {
    type Target = TestMessage;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TestGoAwayMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl TestMessageBase for TestGoAwayMessage {
    fn packet_sample(&self) -> &[u8] {
        self.wire_image()
    }

    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Control(ControlMessage::GoAway(self.go_away.clone()))
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Control(ControlMessage::GoAway(cast)) = values {
            cast
        } else {
            return false;
        };
        if cast.new_session_uri != self.go_away.new_session_uri {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.expand_varints_impl("vv---".as_bytes())
    }
}
