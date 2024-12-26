use crate::moqt_messages::*;
use crate::moqt_priority::MoqtDeliveryOrder;
use crate::serde::{data_writer::*, wire_serialization::*};
use crate::{compute_length_on_wire, serialize_into_buffer, serialize_into_writer};
use bytes::{Bytes, BytesMut};
use log::error;
use std::io::Error;
use std::io::ErrorKind;

// Encoding for string parameters as described in
// https://moq-wg.github.io/moq-transport/draft-ietf-moq-transport.html#name-parameters
pub struct StringParameter {
    enum_type: u64,
    data: String,
}

impl StringParameter {
    pub fn new(enum_type: u64, data: String) -> Self {
        Self { enum_type, data }
    }
}

pub struct WireStringParameter<'a>(pub &'a StringParameter);

impl WireType for WireStringParameter<'_> {
    fn get_length_on_wire(&self) -> usize {
        compute_length_on_wire!(
            WireVarInt62(self.0.enum_type),
            WireStringWithVarInt62Length::new(self.0.data.as_str())
        )
    }
    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> Result<(), Error> {
        serialize_into_writer!(
            writer,
            WireVarInt62(self.0.enum_type),
            WireStringWithVarInt62Length::new(self.0.data.as_str())
        )
    }
}

impl<'a> RefWireType<'a, StringParameter> for WireStringParameter<'a> {
    fn from_ref(value: &'a StringParameter) -> Self {
        Self(value)
    }
}

// Encoding for integer parameters as described in
// https://moq-wg.github.io/moq-transport/draft-ietf-moq-transport.html#name-parameters
pub struct IntParameter {
    enum_type: u64,
    value: u64,
}

impl IntParameter {
    pub fn new(enum_type: u64, value: u64) -> Self {
        Self { enum_type, value }
    }
}

pub struct WireIntParameter<'a>(pub &'a IntParameter);

impl WireIntParameter<'_> {
    fn needed_var_int_len(value: u64) -> u64 {
        DataWriter::get_var_int62_len(value) as u64
    }
}

impl WireType for WireIntParameter<'_> {
    fn get_length_on_wire(&self) -> usize {
        compute_length_on_wire!(
            WireVarInt62(self.0.enum_type),
            WireVarInt62(WireIntParameter::needed_var_int_len(self.0.value)),
            WireVarInt62(self.0.value)
        )
    }

    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> Result<(), Error> {
        serialize_into_writer!(
            writer,
            WireVarInt62(self.0.enum_type),
            WireVarInt62(WireIntParameter::needed_var_int_len(self.0.value)),
            WireVarInt62(self.0.value)
        )
    }
}

impl<'a> RefWireType<'a, IntParameter> for WireIntParameter<'a> {
    fn from_ref(value: &'a IntParameter) -> Self {
        Self(value)
    }
}

pub struct WireSubscribeParameterList<'a>(pub &'a MoqtSubscribeParameters);

impl WireSubscribeParameterList<'_> {
    pub fn string_parameters(&self) -> Vec<StringParameter> {
        let mut result = vec![];
        if let Some(authorization_info) = &self.0.authorization_info {
            result.push(StringParameter::new(
                MoqtTrackRequestParameter::kAuthorizationInfo as u64,
                authorization_info.to_string(),
            ));
        }
        result
    }

    pub fn int_parameters(&self) -> Vec<IntParameter> {
        let mut result = vec![];
        if let Some(delivery_timeout) = &self.0.delivery_timeout {
            assert!(!delivery_timeout.is_zero());
            result.push(IntParameter::new(
                MoqtTrackRequestParameter::kDeliveryTimeout as u64,
                delivery_timeout.as_millis() as u64,
            ));
        }
        if let Some(max_cache_duration) = &self.0.max_cache_duration {
            assert!(!max_cache_duration.is_zero());
            result.push(IntParameter::new(
                MoqtTrackRequestParameter::kMaxCacheDuration as u64,
                max_cache_duration.as_millis() as u64,
            ));
        }
        if let Some(object_ack_window) = &self.0.object_ack_window {
            assert!(!object_ack_window.is_zero());
            result.push(IntParameter::new(
                MoqtTrackRequestParameter::kOackWindowSize as u64,
                object_ack_window.as_micros() as u64,
            ));
        }
        result
    }
}

impl WireType for WireSubscribeParameterList<'_> {
    fn get_length_on_wire(&self) -> usize {
        let string_parameters = self.string_parameters();
        let int_parameters = self.int_parameters();
        compute_length_on_wire!(
            WireVarInt62((string_parameters.len() + int_parameters.len()) as u64),
            WireSpan::<WireStringParameter<'_>, StringParameter>::new(&string_parameters),
            WireSpan::<WireIntParameter<'_>, IntParameter>::new(&int_parameters)
        )
    }

    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> Result<(), Error> {
        let string_parameters = self.string_parameters();
        let int_parameters = self.int_parameters();
        serialize_into_writer!(
            writer,
            WireVarInt62((string_parameters.len() + int_parameters.len()) as u64),
            WireSpan::<WireStringParameter<'_>, StringParameter>::new(&string_parameters),
            WireSpan::<WireIntParameter<'_>, IntParameter>::new(&int_parameters)
        )
    }
}

pub struct WireFullTrackName<'a> {
    name: &'a FullTrackName,
    includes_name: bool,
}

impl<'a> WireFullTrackName<'a> {
    /// If |includes_name| is true, the last element in the tuple is the track
    /// name and is therefore not counted in the prefix of the namespace tuple.
    pub fn new(name: &'a FullTrackName, includes_name: bool) -> Self {
        Self {
            name,
            includes_name,
        }
    }

    fn num_elements(&self) -> usize {
        if self.includes_name {
            self.name.tuple().len() - 1
        } else {
            self.name.tuple().len()
        }
    }
}

impl WireType for WireFullTrackName<'_> {
    fn get_length_on_wire(&self) -> usize {
        compute_length_on_wire!(
            WireVarInt62(self.num_elements() as u64),
            WireSpan::<WireStringWithVarInt62Length<'_>, String>::new(self.name.tuple())
        )
    }

    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> Result<(), Error> {
        serialize_into_writer!(
            writer,
            WireVarInt62(self.num_elements() as u64),
            WireSpan::<WireStringWithVarInt62Length<'_>, String>::new(self.name.tuple())
        )
    }
}

#[macro_export]
macro_rules! serialize {
    ($($data:expr),*) => {{
        serialize_into_buffer!($($data),*)
    }};
}

#[macro_export]
macro_rules! serialize_control_message {
    ($enum_type:expr, $($data:expr),*) => {{
        let message_type = WireVarInt62($enum_type as u64);
        let payload_size = compute_length_on_wire!($($data),*);
        let buffer_size = payload_size
            + compute_length_on_wire!(message_type, WireVarInt62(payload_size as u64));

        if buffer_size == 0 {
            return Ok(BytesMut::new());
        }

        let mut buffer = BytesMut::with_capacity(buffer_size);
        let mut writer = DataWriter::new(&mut buffer);

        serialize_into_writer!(
            &mut writer,
            message_type,
            WireVarInt62(payload_size as u64),
            $($data),*
        )?;
        if writer.remaining() != 0 {
            error!("Failed to serialize control message: {}", $enum_type as u64);
            return Err(Error::from(ErrorKind::InvalidData));
        }

        Ok(buffer)
    }};
}

pub fn wire_delivery_order(delivery_order: &Option<MoqtDeliveryOrder>) -> WireUint8 {
    if let Some(delivery_order) = delivery_order {
        match delivery_order {
            MoqtDeliveryOrder::kAscending => WireUint8::new(0x01),
            MoqtDeliveryOrder::kDescending => WireUint8::new(0x02),
        }
    } else {
        WireUint8::new(0x00)
    }
}

pub fn signed_var_int_serialized_form(value: i64) -> u64 {
    if value < 0 {
        (((-value) as u64) << 1) | 0x01
    } else {
        (value as u64) << 1
    }
}

/// Serialize structured message data into a wire image. When the message format
/// is different per |perspective| or |using_webtrans|, it will omit unnecessary
/// fields. However, it does not enforce the presence of parameters that are
/// required for a particular mode.
/// There can be one instance of this per session. This framer does not enforce
/// that these Serialize() calls are made in a logical order, as they can be on
/// different streams.
#[derive(Default, Copy, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtFramer {
    using_webtrans: bool,
}

impl MoqtFramer {
    pub fn new(using_webtrans: bool) -> Self {
        Self { using_webtrans }
    }

    /// Serialize functions. Takes structured data and serializes it into a
    /// QuicheBuffer for delivery to the stream.
    /// Serializes the header for an object, including the appropriate stream
    /// header if `is_first_in_stream` is set to true.
    pub fn serialize_object_header(
        &self,
        message: &MoqtObject,
        message_type: MoqtDataStreamType,
        is_first_in_stream: bool,
    ) -> Result<BytesMut, Error> {
        if !Self::validate_object_metadata(message, message_type) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Object metadata is invalid",
            ));
        }
        if message_type == MoqtDataStreamType::kObjectDatagram {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Datagrams use SerializeObjectDatagram()",
            ));
        }
        if !is_first_in_stream {
            match message_type {
                MoqtDataStreamType::kStreamHeaderSubgroup => {
                    if message.payload_length == 0 {
                        serialize!(
                            WireVarInt62(message.object_id),
                            WireVarInt62(message.payload_length),
                            WireVarInt62(message.object_status as u64)
                        )
                    } else {
                        serialize!(
                            WireVarInt62(message.object_id),
                            WireVarInt62(message.payload_length)
                        )
                    }
                }
                MoqtDataStreamType::kStreamHeaderFetch => {
                    if let Some(subgroup_id) = message.subgroup_id {
                        if message.payload_length == 0 {
                            serialize!(
                                WireVarInt62(message.group_id),
                                WireVarInt62(subgroup_id),
                                WireVarInt62(message.object_id),
                                WireUint8::new(message.publisher_priority),
                                WireVarInt62(message.payload_length),
                                WireVarInt62(message.object_status as u64)
                            )
                        } else {
                            serialize!(
                                WireVarInt62(message.group_id),
                                WireVarInt62(subgroup_id),
                                WireVarInt62(message.object_id),
                                WireUint8::new(message.publisher_priority),
                                WireVarInt62(message.payload_length)
                            )
                        }
                    } else {
                        Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Message subgroup_id is none",
                        ))
                    }
                }
                _ => Err(Error::from(ErrorKind::InvalidInput)),
            }
        } else {
            match message_type {
                MoqtDataStreamType::kStreamHeaderSubgroup => {
                    if let Some(subgroup_id) = message.subgroup_id {
                        if message.payload_length == 0 {
                            serialize!(
                                WireVarInt62(message_type as u64),
                                WireVarInt62(message.track_alias),
                                WireVarInt62(message.group_id),
                                WireVarInt62(subgroup_id),
                                WireUint8::new(message.publisher_priority),
                                WireVarInt62(message.object_id),
                                WireVarInt62(message.payload_length),
                                WireVarInt62(message.object_status as u64)
                            )
                        } else {
                            serialize!(
                                WireVarInt62(message_type as u64),
                                WireVarInt62(message.track_alias),
                                WireVarInt62(message.group_id),
                                WireVarInt62(subgroup_id),
                                WireUint8::new(message.publisher_priority),
                                WireVarInt62(message.object_id),
                                WireVarInt62(message.payload_length)
                            )
                        }
                    } else {
                        Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Message subgroup_id is none",
                        ))
                    }
                }

                MoqtDataStreamType::kStreamHeaderFetch => {
                    if let Some(subgroup_id) = message.subgroup_id {
                        if message.payload_length == 0 {
                            serialize!(
                                WireVarInt62(message_type as u64),
                                WireVarInt62(message.track_alias),
                                WireVarInt62(message.group_id),
                                WireVarInt62(subgroup_id),
                                WireVarInt62(message.object_id),
                                WireUint8::new(message.publisher_priority),
                                WireVarInt62(message.payload_length),
                                WireVarInt62(message.object_status as u64)
                            )
                        } else {
                            serialize!(
                                WireVarInt62(message_type as u64),
                                WireVarInt62(message.track_alias),
                                WireVarInt62(message.group_id),
                                WireVarInt62(subgroup_id),
                                WireVarInt62(message.object_id),
                                WireUint8::new(message.publisher_priority),
                                WireVarInt62(message.payload_length)
                            )
                        }
                    } else {
                        Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Message subgroup_id is none",
                        ))
                    }
                }
                _ => Err(Error::from(ErrorKind::InvalidInput)),
            }
        }
    }

    pub fn serialize_object_datagram(
        &self,
        message: &MoqtObject,
        payload: &Bytes,
    ) -> Result<BytesMut, Error> {
        if !Self::validate_object_metadata(message, MoqtDataStreamType::kObjectDatagram) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Object metadata is invalid",
            ));
        }
        if message.payload_length != payload.len() as u64 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Payload length does not match payload",
            ));
        }
        if message.payload_length == 0 {
            serialize!(
                WireVarInt62(MoqtDataStreamType::kObjectDatagram as u64),
                WireVarInt62(message.track_alias),
                WireVarInt62(message.group_id),
                WireVarInt62(message.object_id),
                WireUint8::new(message.publisher_priority),
                WireVarInt62(message.payload_length),
                WireVarInt62(message.object_status as u64)
            )
        } else {
            serialize!(
                WireVarInt62(MoqtDataStreamType::kObjectDatagram as u64),
                WireVarInt62(message.track_alias),
                WireVarInt62(message.group_id),
                WireVarInt62(message.object_id),
                WireUint8::new(message.publisher_priority),
                WireVarInt62(message.payload_length),
                WireBytes(payload)
            )
        }
    }

    pub fn serialize_client_setup(&self, message: &MoqtClientSetup) -> Result<BytesMut, Error> {
        let mut int_parameters = vec![];
        let mut string_parameters = vec![];
        if let Some(role) = message.role {
            int_parameters.push(IntParameter::new(
                MoqtSetupParameter::kRole as u64,
                role as u64,
            ));
        }
        if let Some(max_subscribe_id) = message.max_subscribe_id {
            int_parameters.push(IntParameter::new(
                MoqtSetupParameter::kMaxSubscribeId as u64,
                max_subscribe_id,
            ));
        }
        if message.supports_object_ack {
            int_parameters.push(IntParameter::new(
                MoqtSetupParameter::kSupportObjectAcks as u64,
                1,
            ));
        }
        if !self.using_webtrans {
            if let Some(path) = &message.path {
                string_parameters.push(StringParameter::new(
                    MoqtSetupParameter::kPath as u64,
                    path.to_string(),
                ));
            }
        }
        serialize_control_message!(
            MoqtMessageType::kClientSetup,
            WireVarInt62(message.supported_versions.len() as u64),
            WireSpan::<WireVarInt62, MoqtVersion>::new(&message.supported_versions),
            WireVarInt62((string_parameters.len() + int_parameters.len()) as u64),
            WireSpan::<WireIntParameter<'_>, IntParameter>::new(&int_parameters),
            WireSpan::<WireStringParameter<'_>, StringParameter>::new(&string_parameters)
        )
    }
    pub fn serialize_server_setup(&self, message: &MoqtServerSetup) -> Result<BytesMut, Error> {
        let mut int_parameters = vec![];
        if let Some(role) = message.role {
            int_parameters.push(IntParameter::new(
                MoqtSetupParameter::kRole as u64,
                role as u64,
            ));
        }
        if let Some(max_subscribe_id) = message.max_subscribe_id {
            int_parameters.push(IntParameter::new(
                MoqtSetupParameter::kMaxSubscribeId as u64,
                max_subscribe_id,
            ));
        }
        if message.supports_object_ack {
            int_parameters.push(IntParameter::new(
                MoqtSetupParameter::kSupportObjectAcks as u64,
                1,
            ));
        }
        serialize_control_message!(
            MoqtMessageType::kServerSetup,
            WireVarInt62(message.selected_version as u64),
            WireVarInt62(int_parameters.len() as u64),
            WireSpan::<WireIntParameter<'_>, IntParameter>::new(&int_parameters)
        )
    }
    // Returns an empty buffer if there is an illegal combination of locations.
    pub fn serialize_subscribe(&self, message: &MoqtSubscribe) -> Result<BytesMut, Error> {
        let filter_type = get_filter_type(message);
        if filter_type == MoqtFilterType::kNone {
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid object range"));
        }
        match filter_type {
            MoqtFilterType::kLatestGroup | MoqtFilterType::kLatestObject => {
                serialize_control_message!(
                    MoqtMessageType::kSubscribe,
                    WireVarInt62(message.subscribe_id),
                    WireVarInt62(message.track_alias),
                    WireFullTrackName::new(&message.full_track_name, true),
                    WireUint8::new(message.subscriber_priority),
                    wire_delivery_order(&message.group_order),
                    WireVarInt62(filter_type as u64),
                    WireSubscribeParameterList(&message.parameters)
                )
            }
            MoqtFilterType::kAbsoluteStart => {
                if let (Some(start_group), Some(start_object)) =
                    (message.start_group, message.start_object)
                {
                    serialize_control_message!(
                        MoqtMessageType::kSubscribe,
                        WireVarInt62(message.subscribe_id),
                        WireVarInt62(message.track_alias),
                        WireFullTrackName::new(&message.full_track_name, true),
                        WireUint8::new(message.subscriber_priority),
                        wire_delivery_order(&message.group_order),
                        WireVarInt62(filter_type as u64),
                        WireVarInt62(start_group),
                        WireVarInt62(start_object),
                        WireSubscribeParameterList(&message.parameters)
                    )
                } else {
                    Err(Error::new(ErrorKind::InvalidInput, "Subscribe framing error due to empty start group/object in MoqtFilterType::kAbsoluteStart"))
                }
            }
            MoqtFilterType::kAbsoluteRange => {
                if let (Some(start_group), Some(end_group), Some(start_object)) =
                    (message.start_group, message.end_group, message.start_object)
                {
                    serialize_control_message!(
                        MoqtMessageType::kSubscribe,
                        WireVarInt62(message.subscribe_id),
                        WireVarInt62(message.track_alias),
                        WireFullTrackName::new(&message.full_track_name, true),
                        WireUint8::new(message.subscriber_priority),
                        wire_delivery_order(&message.group_order),
                        WireVarInt62(filter_type as u64),
                        WireVarInt62(start_group),
                        WireVarInt62(start_object),
                        WireVarInt62(end_group),
                        WireVarInt62(if let Some(end_object) = message.end_object {
                            end_object + 1
                        } else {
                            0
                        }),
                        WireSubscribeParameterList(&message.parameters)
                    )
                } else {
                    Err(Error::new(ErrorKind::InvalidInput, "Subscribe framing error due to empty start group/object or end group in MoqtFilterType::kAbsoluteRange"))
                }
            }
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "Subscribe framing error.",
            )),
        }
    }
    pub fn serialize_subscribe_ok(&self, message: &MoqtSubscribeOk) -> Result<BytesMut, Error> {
        if message.parameters.authorization_info.is_some() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "SUBSCRIBE_OK with delivery timeout",
            ));
        }
        if let Some(largest_id) = &message.largest_id {
            serialize_control_message!(
                MoqtMessageType::kSubscribeOk,
                WireVarInt62(message.subscribe_id),
                WireVarInt62(message.expires.as_millis() as u64),
                wire_delivery_order(&Some(message.group_order)),
                WireUint8::new(1),
                WireVarInt62(largest_id.group),
                WireVarInt62(largest_id.object),
                WireSubscribeParameterList(&message.parameters)
            )
        } else {
            serialize_control_message!(
                MoqtMessageType::kSubscribeOk,
                WireVarInt62(message.subscribe_id),
                WireVarInt62(message.expires.as_millis() as u64),
                wire_delivery_order(&Some(message.group_order)),
                WireUint8::new(0),
                WireSubscribeParameterList(&message.parameters)
            )
        }
    }
    pub fn serialize_subscribe_error(
        &self,
        message: &MoqtSubscribeError,
    ) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kSubscribeError,
            WireVarInt62(message.subscribe_id),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str()),
            WireVarInt62(message.track_alias)
        )
    }
    pub fn serialize_unsubscribe(&self, message: &MoqtUnsubscribe) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kUnsubscribe,
            WireVarInt62(message.subscribe_id)
        )
    }
    pub fn serialize_subscribe_done(&self, message: &MoqtSubscribeDone) -> Result<BytesMut, Error> {
        if let Some(final_id) = &message.final_id {
            serialize_control_message!(
                MoqtMessageType::kSubscribeDone,
                WireVarInt62(message.subscribe_id),
                WireVarInt62(message.status_code as u64),
                WireStringWithVarInt62Length::new(message.reason_phrase.as_str()),
                WireUint8::new(1),
                WireVarInt62(final_id.group),
                WireVarInt62(final_id.object)
            )
        } else {
            serialize_control_message!(
                MoqtMessageType::kSubscribeDone,
                WireVarInt62(message.subscribe_id),
                WireVarInt62(message.status_code as u64),
                WireStringWithVarInt62Length::new(message.reason_phrase.as_str()),
                WireUint8::new(0)
            )
        }
    }
    pub fn serialize_subscribe_update(
        &self,
        message: &MoqtSubscribeUpdate,
    ) -> Result<BytesMut, Error> {
        if message.parameters.authorization_info.is_some() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "SUBSCRIBE_UPDATE with authorization info",
            ));
        }
        let end_group = if let Some(end_group) = message.end_group {
            end_group + 1
        } else {
            0
        };
        let end_object = if let Some(end_object) = message.end_object {
            end_object + 1
        } else {
            0
        };
        if end_group == 0 && end_object != 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid object range"));
        }
        serialize_control_message!(
            MoqtMessageType::kSubscribeUpdate,
            WireVarInt62(message.subscribe_id),
            WireVarInt62(message.start_group),
            WireVarInt62(message.start_object),
            WireVarInt62(end_group),
            WireVarInt62(end_object),
            WireUint8::new(message.subscriber_priority),
            WireSubscribeParameterList(&message.parameters)
        )
    }
    pub fn serialize_announce(&self, message: &MoqtAnnounce) -> Result<BytesMut, Error> {
        if message.parameters.delivery_timeout.is_some() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "ANNOUNCE with delivery timeout",
            ));
        }
        serialize_control_message!(
            MoqtMessageType::kAnnounce,
            WireFullTrackName::new(&message.track_namespace, false),
            WireSubscribeParameterList(&message.parameters)
        )
    }
    pub fn serialize_announce_ok(&self, message: &MoqtAnnounceOk) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kAnnounceOk,
            WireFullTrackName::new(&message.track_namespace, false)
        )
    }
    pub fn serialize_announce_error(&self, message: &MoqtAnnounceError) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kAnnounceError,
            WireFullTrackName::new(&message.track_namespace, false),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str())
        )
    }
    pub fn serialize_announce_cancel(
        &self,
        message: &MoqtAnnounceCancel,
    ) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kAnnounceCancel,
            WireFullTrackName::new(&message.track_namespace, false),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str())
        )
    }
    pub fn serialize_track_status_request(
        &self,
        message: &MoqtTrackStatusRequest,
    ) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kTrackStatusRequest,
            WireFullTrackName::new(&message.full_track_name, true)
        )
    }
    pub fn serialize_unannounce(&self, message: &MoqtUnannounce) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kUnannounce,
            WireFullTrackName::new(&message.track_namespace, false)
        )
    }
    pub fn serialize_track_status(&self, message: &MoqtTrackStatus) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kTrackStatus,
            WireFullTrackName::new(&message.full_track_name, true),
            WireVarInt62(message.status_code as u64),
            WireVarInt62(message.last_group),
            WireVarInt62(message.last_object)
        )
    }
    pub fn serialize_go_away(&self, message: &MoqtGoAway) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kGoAway,
            WireStringWithVarInt62Length::new(message.new_session_uri.as_str())
        )
    }
    pub fn serialize_subscribe_announces(
        &self,
        message: &MoqtSubscribeAnnounces,
    ) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kSubscribeAnnounces,
            WireFullTrackName::new(&message.track_namespace, false),
            WireSubscribeParameterList(&message.parameters)
        )
    }
    pub fn serialize_subscribe_announces_ok(
        &self,
        message: &MoqtSubscribeAnnouncesOk,
    ) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kSubscribeAnnouncesOk,
            WireFullTrackName::new(&message.track_namespace, false)
        )
    }
    pub fn serialize_subscribe_announces_error(
        &self,
        message: &MoqtSubscribeAnnouncesError,
    ) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kSubscribeAnnouncesError,
            WireFullTrackName::new(&message.track_namespace, false),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str())
        )
    }
    pub fn serialize_unsubscribe_announces(
        &self,
        message: &MoqtUnsubscribeAnnounces,
    ) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kUnsubscribeAnnounces,
            WireFullTrackName::new(&message.track_namespace, false)
        )
    }
    pub fn serialize_max_subscribe_id(
        &self,
        message: &MoqtMaxSubscribeId,
    ) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kMaxSubscribeId,
            WireVarInt62(message.max_subscribe_id)
        )
    }
    pub fn serialize_fetch(&self, message: &MoqtFetch) -> Result<BytesMut, Error> {
        if message.end_group < message.start_object.group
            || (message.end_group == message.start_object.group
                && message.end_object.is_some()
                && *message.end_object.as_ref().unwrap() < message.start_object.object)
        {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid FETCH object range",
            ));
        }
        serialize_control_message!(
            MoqtMessageType::kFetch,
            WireVarInt62(message.subscribe_id),
            WireFullTrackName::new(&message.full_track_name, true),
            WireUint8::new(message.subscriber_priority),
            wire_delivery_order(&message.group_order),
            WireVarInt62(message.start_object.group),
            WireVarInt62(message.start_object.object),
            WireVarInt62(message.end_group),
            WireVarInt62(if let Some(end_object) = message.end_object {
                end_object + 1
            } else {
                0
            }),
            WireSubscribeParameterList(&message.parameters)
        )
    }
    pub fn serialize_fetch_cancel(&self, message: &MoqtFetchCancel) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kFetchCancel,
            WireVarInt62(message.subscribe_id)
        )
    }
    pub fn serialize_fetch_ok(&self, message: &MoqtFetchOk) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kFetchOk,
            WireVarInt62(message.subscribe_id),
            wire_delivery_order(&Some(message.group_order)),
            WireVarInt62(message.largest_id.group),
            WireVarInt62(message.largest_id.object),
            WireSubscribeParameterList(&message.parameters)
        )
    }
    pub fn serialize_fetch_error(&self, message: &MoqtFetchError) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kFetchError,
            WireVarInt62(message.subscribe_id),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str())
        )
    }
    pub fn serialize_object_ack(&self, message: &MoqtObjectAck) -> Result<BytesMut, Error> {
        serialize_control_message!(
            MoqtMessageType::kObjectAck,
            WireVarInt62(message.subscribe_id),
            WireVarInt62(message.group_id),
            WireVarInt62(message.object_id),
            WireVarInt62(signed_var_int_serialized_form(
                message.delta_from_deadline.as_micros() as i64
            ))
        )
    }

    // Returns true if the metadata is internally consistent.
    fn validate_object_metadata(object: &MoqtObject, message_type: MoqtDataStreamType) -> bool {
        if object.object_status != MoqtObjectStatus::kNormal && object.payload_length > 0 {
            return false;
        }
        if (message_type == MoqtDataStreamType::kStreamHeaderSubgroup
            || message_type == MoqtDataStreamType::kStreamHeaderFetch)
            != object.subgroup_id.is_some()
        {
            return false;
        }
        true
    }
}
