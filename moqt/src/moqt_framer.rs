use crate::moqt_messages::*;
use crate::moqt_priority::MoqtDeliveryOrder;
use crate::serde::{data_writer::*, wire_serialization::*};
use crate::{compute_length_on_wire, serialize_into_buffer, serialize_into_writer};
use bytes::{Bytes, BytesMut};
use log::error;

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
    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
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

    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
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

    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
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

    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
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
            return BytesMut::new();
        }

        let mut buffer = BytesMut::with_capacity(buffer_size);
        let mut writer = DataWriter::new(&mut buffer);

        let result =  serialize_into_writer!(
            &mut writer,
            message_type,
            WireVarInt62(payload_size as u64),
            $($data),*
        );
        if !result || writer.remaining() != 0 {
            error!("Failed to serialize control message: {}", $enum_type as u64);
            return BytesMut::new();
        }

        buffer
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
    ) -> BytesMut {
        if !Self::validate_object_metadata(message, message_type) {
            error!("Object metadata is invalid");
            return BytesMut::new();
        }
        if message_type == MoqtDataStreamType::kObjectDatagram {
            error!("Datagrams use SerializeObjectDatagram()");
            return BytesMut::new();
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
                        error!("Message subgroup_id is none");
                        BytesMut::new()
                    }
                }
                _ => BytesMut::new(),
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
                        error!("Message subgroup_id is none");
                        BytesMut::new()
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
                        error!("Message subgroup_id is none");
                        BytesMut::new()
                    }
                }
                _ => BytesMut::new(),
            }
        }
    }

    pub fn serialize_object_datagram(&self, message: &MoqtObject, payload: &Bytes) -> BytesMut {
        if !Self::validate_object_metadata(message, MoqtDataStreamType::kObjectDatagram) {
            error!("Object metadata is invalid");
            return BytesMut::new();
        }
        if message.payload_length != payload.len() as u64 {
            error!("Payload length does not match payload");
            return BytesMut::new();
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

    pub fn serialize_client_setup(&self, message: &MoqtClientSetup) -> BytesMut {
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
    pub fn serialize_server_setup(&self, message: &MoqtServerSetup) -> BytesMut {
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
    pub fn serialize_subscribe(&self, message: &MoqtSubscribe) -> BytesMut {
        let filter_type = get_filter_type(message);
        if filter_type == MoqtFilterType::kNone {
            error!("Invalid object range");
            return BytesMut::new();
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
                    error!("Subscribe framing error due to empty start group/object in MoqtFilterType::kAbsoluteStart");
                    BytesMut::new()
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
                    error!("Subscribe framing error due to empty start group/object or end group in MoqtFilterType::kAbsoluteRange");
                    BytesMut::new()
                }
            }
            _ => {
                error!("Subscribe framing error.");
                BytesMut::new()
            }
        }
    }
    pub fn serialize_subscribe_ok(&self, message: &MoqtSubscribeOk) -> BytesMut {
        if message.parameters.authorization_info.is_some() {
            error!("SUBSCRIBE_OK with delivery timeout");
            return BytesMut::new();
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
    pub fn serialize_subscribe_error(&self, message: &MoqtSubscribeError) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kSubscribeError,
            WireVarInt62(message.subscribe_id),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str()),
            WireVarInt62(message.track_alias)
        )
    }
    pub fn serialize_unsubscribe(&self, message: &MoqtUnsubscribe) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kUnsubscribe,
            WireVarInt62(message.subscribe_id)
        )
    }
    pub fn serialize_subscribe_done(&self, message: &MoqtSubscribeDone) -> BytesMut {
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
    pub fn serialize_subscribe_update(&self, message: &MoqtSubscribeUpdate) -> BytesMut {
        if message.parameters.authorization_info.is_some() {
            error!("SUBSCRIBE_UPDATE with authorization info");
            return BytesMut::new();
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
            error!("Invalid object range");
            return BytesMut::new();
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
    pub fn serialize_announce(&self, message: &MoqtAnnounce) -> BytesMut {
        if message.parameters.delivery_timeout.is_some() {
            error!("ANNOUNCE with delivery timeout");
            return BytesMut::new();
        }
        serialize_control_message!(
            MoqtMessageType::kAnnounce,
            WireFullTrackName::new(&message.track_namespace, false),
            WireSubscribeParameterList(&message.parameters)
        )
    }
    pub fn serialize_announce_ok(&self, message: &MoqtAnnounceOk) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kAnnounceOk,
            WireFullTrackName::new(&message.track_namespace, false)
        )
    }
    pub fn serialize_announce_error(&self, message: &MoqtAnnounceError) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kAnnounceError,
            WireFullTrackName::new(&message.track_namespace, false),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str())
        )
    }
    pub fn serialize_announce_cancel(&self, message: &MoqtAnnounceCancel) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kAnnounceCancel,
            WireFullTrackName::new(&message.track_namespace, false),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str())
        )
    }
    pub fn serialize_track_status_request(&self, message: &MoqtTrackStatusRequest) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kTrackStatusRequest,
            WireFullTrackName::new(&message.full_track_name, true)
        )
    }
    pub fn serialize_unannounce(&self, message: &MoqtUnannounce) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kUnannounce,
            WireFullTrackName::new(&message.track_namespace, false)
        )
    }
    pub fn serialize_track_status(&self, message: &MoqtTrackStatus) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kTrackStatus,
            WireFullTrackName::new(&message.full_track_name, true),
            WireVarInt62(message.status_code as u64),
            WireVarInt62(message.last_group),
            WireVarInt62(message.last_object)
        )
    }
    pub fn serialize_go_away(&self, message: &MoqtGoAway) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kGoAway,
            WireStringWithVarInt62Length::new(message.new_session_uri.as_str())
        )
    }
    pub fn serialize_subscribe_announces(&self, message: &MoqtSubscribeAnnounces) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kSubscribeAnnounces,
            WireFullTrackName::new(&message.track_namespace, false),
            WireSubscribeParameterList(&message.parameters)
        )
    }
    pub fn serialize_subscribe_announces_ok(&self, message: &MoqtSubscribeAnnouncesOk) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kSubscribeAnnouncesOk,
            WireFullTrackName::new(&message.track_namespace, false)
        )
    }
    pub fn serialize_subscribe_announces_error(
        &self,
        message: &MoqtSubscribeAnnouncesError,
    ) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kSubscribeAnnouncesError,
            WireFullTrackName::new(&message.track_namespace, false),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str())
        )
    }
    pub fn serialize_unsubscribe_announces(&self, message: &MoqtUnsubscribeAnnounces) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kUnsubscribeAnnounces,
            WireFullTrackName::new(&message.track_namespace, false)
        )
    }
    pub fn serialize_max_subscribe_id(&self, message: &MoqtMaxSubscribeId) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kMaxSubscribeId,
            WireVarInt62(message.max_subscribe_id)
        )
    }
    pub fn serialize_fetch(&self, message: &MoqtFetch) -> BytesMut {
        if message.end_group < message.start_object.group
            || (message.end_group == message.start_object.group
                && message.end_object.is_some()
                && *message.end_object.as_ref().unwrap() < message.start_object.object)
        {
            error!("Invalid FETCH object range");
            return BytesMut::new();
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
    pub fn serialize_fetch_cancel(&self, message: &MoqtFetchCancel) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kFetchCancel,
            WireVarInt62(message.subscribe_id)
        )
    }
    pub fn serialize_fetch_ok(&self, message: &MoqtFetchOk) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kFetchOk,
            WireVarInt62(message.subscribe_id),
            wire_delivery_order(&Some(message.group_order)),
            WireVarInt62(message.largest_id.group),
            WireVarInt62(message.largest_id.object),
            WireSubscribeParameterList(&message.parameters)
        )
    }
    pub fn serialize_fetch_error(&self, message: &MoqtFetchError) -> BytesMut {
        serialize_control_message!(
            MoqtMessageType::kFetchError,
            WireVarInt62(message.subscribe_id),
            WireVarInt62(message.error_code as u64),
            WireStringWithVarInt62Length::new(message.reason_phrase.as_str())
        )
    }
    pub fn serialize_object_ack(&self, message: &MoqtObjectAck) -> BytesMut {
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

#[cfg(test)]
mod test {
    use super::*;
    use bytes::BufMut;
    //use rstest::rstest;

    struct MoqtFramerTestParams {
        message_type: MoqtMessageType,
        uses_web_transport: bool,
    }

    fn get_moqt_framer_test_params() -> Vec<MoqtFramerTestParams> {
        let mut params = vec![];
        let message_types = vec![
            MoqtMessageType::kSubscribe,
            MoqtMessageType::kSubscribeOk,
            MoqtMessageType::kSubscribeError,
            MoqtMessageType::kUnsubscribe,
            MoqtMessageType::kSubscribeDone,
            MoqtMessageType::kAnnounceCancel,
            MoqtMessageType::kTrackStatusRequest,
            MoqtMessageType::kTrackStatus,
            MoqtMessageType::kAnnounce,
            MoqtMessageType::kAnnounceOk,
            MoqtMessageType::kAnnounceError,
            MoqtMessageType::kUnannounce,
            MoqtMessageType::kGoAway,
            MoqtMessageType::kSubscribeAnnounces,
            MoqtMessageType::kSubscribeAnnouncesOk,
            MoqtMessageType::kSubscribeAnnouncesError,
            MoqtMessageType::kUnsubscribeAnnounces,
            MoqtMessageType::kMaxSubscribeId,
            MoqtMessageType::kFetch,
            MoqtMessageType::kFetchCancel,
            MoqtMessageType::kFetchOk,
            MoqtMessageType::kFetchError,
            MoqtMessageType::kObjectAck,
            MoqtMessageType::kClientSetup,
            MoqtMessageType::kServerSetup,
        ];
        for message_type in message_types {
            if message_type == MoqtMessageType::kClientSetup {
                for uses_web_transport in vec![false, true] {
                    params.push(MoqtFramerTestParams {
                        message_type,
                        uses_web_transport,
                    });
                }
            } else {
                // All other types are processed the same for either perspective or
                // transport.
                params.push(MoqtFramerTestParams {
                    message_type,
                    uses_web_transport: true,
                });
            }
        }
        params
    }

    fn param_name_formatter(param: &MoqtFramerTestParams) -> String {
        param.message_type.to_string()
            + "_"
            + if param.uses_web_transport {
                "WebTransport"
            } else {
                "QUIC"
            }
    }

    fn serialize_object(
        framer: &MoqtFramer,
        message: &MoqtObject,
        payload: &Bytes,
        stream_type: MoqtDataStreamType,
        is_first_in_stream: bool,
    ) -> BytesMut {
        let mut adjusted_message = message.clone();
        adjusted_message.payload_length = payload.len() as u64;
        let mut header = if stream_type == MoqtDataStreamType::kObjectDatagram {
            framer.serialize_object_datagram(&adjusted_message, payload)
        } else {
            framer.serialize_object_header(&adjusted_message, stream_type, is_first_in_stream)
        };
        if !header.is_empty() {
            header.put(&mut payload.clone());
        }
        header
    }

    /*
    class MoqtFramerTest
        : public quic::test::QuicTestWithParam<MoqtFramerTestParams> {
     public:
      MoqtFramerTest()
          : message_type_(GetParam().message_type),
            webtrans_(GetParam().uses_web_transport),
            buffer_allocator_(quiche::SimpleBufferAllocator::Get()),
            framer_(buffer_allocator_, GetParam().uses_web_transport) {}

      std::unique_ptr<TestMessageBase> MakeMessage(MoqtMessageType message_type) {
        return CreateTestMessage(message_type, webtrans_);
      }

      quiche::QuicheBuffer SerializeMessage(
          TestMessageBase::MessageStructuredData& structured_data) {
        switch (message_type_) {
          case MoqtMessageType::kSubscribe: {
            auto data = std::get<MoqtSubscribe>(structured_data);
            return framer_.SerializeSubscribe(data);
          }
          case MoqtMessageType::kSubscribeOk: {
            auto data = std::get<MoqtSubscribeOk>(structured_data);
            return framer_.SerializeSubscribeOk(data);
          }
          case MoqtMessageType::kSubscribeError: {
            auto data = std::get<MoqtSubscribeError>(structured_data);
            return framer_.SerializeSubscribeError(data);
          }
          case MoqtMessageType::kUnsubscribe: {
            auto data = std::get<MoqtUnsubscribe>(structured_data);
            return framer_.SerializeUnsubscribe(data);
          }
          case MoqtMessageType::kSubscribeDone: {
            auto data = std::get<MoqtSubscribeDone>(structured_data);
            return framer_.SerializeSubscribeDone(data);
          }
          case MoqtMessageType::kAnnounce: {
            auto data = std::get<MoqtAnnounce>(structured_data);
            return framer_.SerializeAnnounce(data);
          }
          case moqt::MoqtMessageType::kAnnounceOk: {
            auto data = std::get<MoqtAnnounceOk>(structured_data);
            return framer_.SerializeAnnounceOk(data);
          }
          case moqt::MoqtMessageType::kAnnounceError: {
            auto data = std::get<MoqtAnnounceError>(structured_data);
            return framer_.SerializeAnnounceError(data);
          }
          case moqt::MoqtMessageType::kAnnounceCancel: {
            auto data = std::get<MoqtAnnounceCancel>(structured_data);
            return framer_.SerializeAnnounceCancel(data);
          }
          case moqt::MoqtMessageType::kTrackStatusRequest: {
            auto data = std::get<MoqtTrackStatusRequest>(structured_data);
            return framer_.SerializeTrackStatusRequest(data);
          }
          case MoqtMessageType::kUnannounce: {
            auto data = std::get<MoqtUnannounce>(structured_data);
            return framer_.SerializeUnannounce(data);
          }
          case moqt::MoqtMessageType::kTrackStatus: {
            auto data = std::get<MoqtTrackStatus>(structured_data);
            return framer_.SerializeTrackStatus(data);
          }
          case moqt::MoqtMessageType::kGoAway: {
            auto data = std::get<MoqtGoAway>(structured_data);
            return framer_.SerializeGoAway(data);
          }
          case moqt::MoqtMessageType::kSubscribeAnnounces: {
            auto data = std::get<MoqtSubscribeAnnounces>(structured_data);
            return framer_.SerializeSubscribeAnnounces(data);
          }
          case moqt::MoqtMessageType::kSubscribeAnnouncesOk: {
            auto data = std::get<MoqtSubscribeAnnouncesOk>(structured_data);
            return framer_.SerializeSubscribeAnnouncesOk(data);
          }
          case moqt::MoqtMessageType::kSubscribeAnnouncesError: {
            auto data = std::get<MoqtSubscribeAnnouncesError>(structured_data);
            return framer_.SerializeSubscribeAnnouncesError(data);
          }
          case moqt::MoqtMessageType::kUnsubscribeAnnounces: {
            auto data = std::get<MoqtUnsubscribeAnnounces>(structured_data);
            return framer_.SerializeUnsubscribeAnnounces(data);
          }
          case moqt::MoqtMessageType::kMaxSubscribeId: {
            auto data = std::get<MoqtMaxSubscribeId>(structured_data);
            return framer_.SerializeMaxSubscribeId(data);
          }
          case moqt::MoqtMessageType::kFetch: {
            auto data = std::get<MoqtFetch>(structured_data);
            return framer_.SerializeFetch(data);
          }
          case moqt::MoqtMessageType::kFetchCancel: {
            auto data = std::get<MoqtFetchCancel>(structured_data);
            return framer_.SerializeFetchCancel(data);
          }
          case moqt::MoqtMessageType::kFetchOk: {
            auto data = std::get<MoqtFetchOk>(structured_data);
            return framer_.SerializeFetchOk(data);
          }
          case moqt::MoqtMessageType::kFetchError: {
            auto data = std::get<MoqtFetchError>(structured_data);
            return framer_.SerializeFetchError(data);
          }
          case moqt::MoqtMessageType::kObjectAck: {
            auto data = std::get<MoqtObjectAck>(structured_data);
            return framer_.SerializeObjectAck(data);
          }
          case MoqtMessageType::kClientSetup: {
            auto data = std::get<MoqtClientSetup>(structured_data);
            return framer_.SerializeClientSetup(data);
          }
          case MoqtMessageType::kServerSetup: {
            auto data = std::get<MoqtServerSetup>(structured_data);
            return framer_.SerializeServerSetup(data);
          }
          default:
            // kObjectDatagram is a totally different code path.
            return quiche::QuicheBuffer();
        }
      }

      MoqtMessageType message_type_;
      bool webtrans_;
      quiche::SimpleBufferAllocator* buffer_allocator_;
      MoqtFramer framer_;
    };*/

    /*
    INSTANTIATE_TEST_SUITE_P(MoqtFramerTests, MoqtFramerTest,
                             testing::ValuesIn(get_moqt_framer_test_params()),
                             param_name_formatter);

    TEST_P(MoqtFramerTest, OneMessage) {
      auto message = MakeMessage(message_type_);
      auto structured_data = message->structured_data();
      auto buffer = SerializeMessage(structured_data);
      EXPECT_EQ(buffer.size(), message->total_message_size());
      quiche::test::CompareCharArraysWithHexError(
          "frame encoding", buffer.data(), buffer.size(),
          message->PacketSample().data(), message->PacketSample().size());
    }

    class MoqtFramerSimpleTest : public quic::test::QuicTest {
     public:
      MoqtFramerSimpleTest()
          : buffer_allocator_(quiche::SimpleBufferAllocator::Get()),
            framer_(buffer_allocator_, /*web_transport=*/true) {}

      quiche::SimpleBufferAllocator* buffer_allocator_;
      MoqtFramer framer_;

      // Obtain a pointer to an arbitrary offset in a serialized buffer.
      const uint8_t* BufferAtOffset(quiche::QuicheBuffer& buffer, size_t offset) {
        const char* data = buffer.data();
        return reinterpret_cast<const uint8_t*>(data + offset);
      }
    };

    TEST_F(MoqtFramerSimpleTest, GroupMiddler) {
      auto header = std::make_unique<StreamHeaderSubgroupMessage>();
      auto buffer1 =
          serialize_object(framer_, std::get<MoqtObject>(header->structured_data()),
                          "foo", MoqtDataStreamType::kStreamHeaderSubgroup, true);
      EXPECT_EQ(buffer1.size(), header->total_message_size());
      EXPECT_EQ(buffer1.AsStringView(), header->PacketSample());

      auto middler = std::make_unique<StreamMiddlerSubgroupMessage>();
      auto buffer2 =
          serialize_object(framer_, std::get<MoqtObject>(middler->structured_data()),
                          "bar", MoqtDataStreamType::kStreamHeaderSubgroup, false);
      EXPECT_EQ(buffer2.size(), middler->total_message_size());
      EXPECT_EQ(buffer2.AsStringView(), middler->PacketSample());
    }

    TEST_F(MoqtFramerSimpleTest, FetchMiddler) {
      auto header = std::make_unique<StreamHeaderFetchMessage>();
      auto buffer1 =
          serialize_object(framer_, std::get<MoqtObject>(header->structured_data()),
                          "foo", MoqtDataStreamType::kStreamHeaderFetch, true);
      EXPECT_EQ(buffer1.size(), header->total_message_size());
      EXPECT_EQ(buffer1.AsStringView(), header->PacketSample());

      auto middler = std::make_unique<StreamMiddlerFetchMessage>();
      auto buffer2 =
          serialize_object(framer_, std::get<MoqtObject>(middler->structured_data()),
                          "bar", MoqtDataStreamType::kStreamHeaderFetch, false);
      EXPECT_EQ(buffer2.size(), middler->total_message_size());
      EXPECT_EQ(buffer2.AsStringView(), middler->PacketSample());
    }

    TEST_F(MoqtFramerSimpleTest, BadObjectInput) {
      MoqtObject object = {
          // This is a valid object.
          /*track_alias=*/4,
          /*group_id=*/5,
          /*object_id=*/6,
          /*publisher_priority=*/7,
          /*object_status=*/MoqtObjectStatus::kNormal,
          /*subgroup_id=*/8,
          /*payload_length=*/3,
      };
      quiche::QuicheBuffer buffer;

      // kSubgroup must have a subgroup_id.
      object.subgroup_id = std::nullopt;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectHeader(
                          object, MoqtDataStreamType::kStreamHeaderSubgroup, false),
                      "Object metadata is invalid");
      EXPECT_TRUE(buffer.empty());
      object.subgroup_id = 8;

      // kFetch must have a subgroup_id.
      object.subgroup_id = std::nullopt;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectHeader(
                          object, MoqtDataStreamType::kStreamHeaderFetch, false),
                      "Object metadata is invalid");
      EXPECT_TRUE(buffer.empty());
      object.subgroup_id = 8;

      // Non-normal status must have no payload.
      object.object_status = MoqtObjectStatus::kEndOfGroup;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectHeader(
                          object, MoqtDataStreamType::kStreamHeaderSubgroup, false),
                      "Object metadata is invalid");
      EXPECT_TRUE(buffer.empty());
      // object.object_status = MoqtObjectStatus::kNormal;
    }

    TEST_F(MoqtFramerSimpleTest, BadDatagramInput) {
      MoqtObject object = {
          // This is a valid datagram.
          /*track_alias=*/4,
          /*group_id=*/5,
          /*object_id=*/6,
          /*publisher_priority=*/7,
          /*object_status=*/MoqtObjectStatus::kNormal,
          /*subgroup_id=*/std::nullopt,
          /*payload_length=*/3,
      };
      quiche::QuicheBuffer buffer;

      // No datagrams to SerializeObjectHeader().
      EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectHeader(
                          object, MoqtDataStreamType::kObjectDatagram, false),
                      "Datagrams use SerializeObjectDatagram()");
      EXPECT_TRUE(buffer.empty());

      object.object_status = MoqtObjectStatus::kEndOfGroup;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectDatagram(object, "foo"),
                      "Object metadata is invalid");
      EXPECT_TRUE(buffer.empty());
      object.object_status = MoqtObjectStatus::kNormal;

      object.subgroup_id = 8;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectDatagram(object, "foo"),
                      "Object metadata is invalid");
      EXPECT_TRUE(buffer.empty());
      object.subgroup_id = std::nullopt;

      EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectDatagram(object, "foobar"),
                      "Payload length does not match payload");
      EXPECT_TRUE(buffer.empty());
    }

    TEST_F(MoqtFramerSimpleTest, Datagram) {
      auto datagram = std::make_unique<ObjectDatagramMessage>();
      MoqtObject object = {
          /*track_alias=*/4,
          /*group_id=*/5,
          /*object_id=*/6,
          /*publisher_priority=*/7,
          /*object_status=*/MoqtObjectStatus::kNormal,
          /*subgroup_id=*/std::nullopt,
          /*payload_length=*/3,
      };
      std::string payload = "foo";
      quiche::QuicheBuffer buffer;
      buffer = framer_.SerializeObjectDatagram(object, payload);
      EXPECT_EQ(buffer.size(), datagram->total_message_size());
      EXPECT_EQ(buffer.AsStringView(), datagram->PacketSample());
    }

    TEST_F(MoqtFramerSimpleTest, AllSubscribeInputs) {
      for (std::optional<uint64_t> start_group :
           {std::optional<uint64_t>(), std::optional<uint64_t>(4)}) {
        for (std::optional<uint64_t> start_object :
             {std::optional<uint64_t>(), std::optional<uint64_t>(0)}) {
          for (std::optional<uint64_t> end_group :
               {std::optional<uint64_t>(), std::optional<uint64_t>(7)}) {
            for (std::optional<uint64_t> end_object :
                 {std::optional<uint64_t>(), std::optional<uint64_t>(3)}) {
              MoqtSubscribe subscribe = {
                  /*subscribe_id=*/3,
                  /*track_alias=*/4,
                  /*full_track_name=*/FullTrackName({"foo", "abcd"}),
                  /*subscriber_priority=*/0x20,
                  /*group_order=*/std::nullopt,
                  start_group,
                  start_object,
                  end_group,
                  end_object,
                  MoqtSubscribeParameters{"bar", std::nullopt, std::nullopt,
                                          std::nullopt},
              };
              quiche::QuicheBuffer buffer;
              MoqtFilterType expected_filter_type = MoqtFilterType::kNone;
              if (!start_group.has_value() && !start_object.has_value() &&
                  !end_group.has_value() && !end_object.has_value()) {
                expected_filter_type = MoqtFilterType::kLatestObject;
              } else if (!start_group.has_value() && start_object.has_value() &&
                         *start_object == 0 && !end_group.has_value() &&
                         !end_object.has_value()) {
                expected_filter_type = MoqtFilterType::kLatestGroup;
              } else if (start_group.has_value() && start_object.has_value() &&
                         !end_group.has_value() && !end_object.has_value()) {
                expected_filter_type = MoqtFilterType::kAbsoluteStart;
              } else if (start_group.has_value() && start_object.has_value() &&
                         end_group.has_value()) {
                expected_filter_type = MoqtFilterType::kAbsoluteRange;
              }
              if (expected_filter_type == MoqtFilterType::kNone) {
                EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribe(subscribe),
                                "Invalid object range");
                EXPECT_EQ(buffer.size(), 0);
                continue;
              }
              buffer = framer_.SerializeSubscribe(subscribe);
              // Go to the filter type.
              const uint8_t* read = BufferAtOffset(buffer, 16);
              EXPECT_EQ(static_cast<MoqtFilterType>(*read), expected_filter_type);
              EXPECT_GT(buffer.size(), 0);
              if (expected_filter_type == MoqtFilterType::kAbsoluteRange &&
                  end_object.has_value()) {
                const uint8_t* object_id = read + 4;
                EXPECT_EQ(*object_id, *end_object + 1);
              }
            }
          }
        }
      }
    }

    TEST_F(MoqtFramerSimpleTest, SubscribeEndBeforeStart) {
      MoqtSubscribe subscribe = {
          /*subscribe_id=*/3,
          /*track_alias=*/4,
          /*full_track_name=*/FullTrackName({"foo", "abcd"}),
          /*subscriber_priority=*/0x20,
          /*group_order=*/std::nullopt,
          /*start_group=*/std::optional<uint64_t>(4),
          /*start_object=*/std::optional<uint64_t>(3),
          /*end_group=*/std::optional<uint64_t>(3),
          /*end_object=*/std::nullopt,
          MoqtSubscribeParameters{"bar", std::nullopt, std::nullopt, std::nullopt},
      };
      quiche::QuicheBuffer buffer;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribe(subscribe),
                      "Invalid object range");
      EXPECT_EQ(buffer.size(), 0);
      subscribe.end_group = 4;
      subscribe.end_object = 1;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribe(subscribe),
                      "Invalid object range");
      EXPECT_EQ(buffer.size(), 0);
    }

    TEST_F(MoqtFramerSimpleTest, FetchEndBeforeStart) {
      MoqtFetch fetch = {
          /*subscribe_id =*/1,
          /*full_track_name=*/FullTrackName{"foo", "bar"},
          /*subscriber_priority=*/2,
          /*group_order=*/MoqtDeliveryOrder::kAscending,
          /*start_object=*/FullSequence{1, 2},
          /*end_group=*/1,
          /*end_object=*/1,
          /*parameters=*/
          MoqtSubscribeParameters{"baz", std::nullopt, std::nullopt, std::nullopt},
      };
      quiche::QuicheBuffer buffer;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeFetch(fetch),
                      "Invalid FETCH object range");
      EXPECT_EQ(buffer.size(), 0);
      fetch.end_group = 0;
      fetch.end_object = std::nullopt;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeFetch(fetch),
                      "Invalid FETCH object range");
      EXPECT_EQ(buffer.size(), 0);
    }

    TEST_F(MoqtFramerSimpleTest, SubscribeLatestGroupNonzeroObject) {
      MoqtSubscribe subscribe = {
          /*subscribe_id=*/3,
          /*track_alias=*/4,
          /*full_track_name=*/FullTrackName({"foo", "abcd"}),
          /*subscriber_priority=*/0x20,
          /*group_order=*/std::nullopt,
          /*start_group=*/std::nullopt,
          /*start_object=*/std::optional<uint64_t>(3),
          /*end_group=*/std::nullopt,
          /*end_object=*/std::nullopt,
          MoqtSubscribeParameters{"bar", std::nullopt, std::nullopt, std::nullopt},
      };
      quiche::QuicheBuffer buffer;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribe(subscribe),
                      "Invalid object range");
      EXPECT_EQ(buffer.size(), 0);
    }

    TEST_F(MoqtFramerSimpleTest, SubscribeUpdateEndGroupOnly) {
      MoqtSubscribeUpdate subscribe_update = {
          /*subscribe_id=*/3,
          /*start_group=*/4,
          /*start_object=*/3,
          /*end_group=*/4,
          /*end_object=*/std::nullopt,
          /*subscriber_priority=*/0xaa,
          MoqtSubscribeParameters{std::nullopt, std::nullopt, std::nullopt,
                                  std::nullopt},
      };
      quiche::QuicheBuffer buffer;
      buffer = framer_.SerializeSubscribeUpdate(subscribe_update);
      EXPECT_GT(buffer.size(), 0);
      const uint8_t* end_group = BufferAtOffset(buffer, 5);
      EXPECT_EQ(*end_group, 5);
      const uint8_t* end_object = end_group + 1;
      EXPECT_EQ(*end_object, 0);
    }

    TEST_F(MoqtFramerSimpleTest, SubscribeUpdateIncrementsEnd) {
      MoqtSubscribeUpdate subscribe_update = {
          /*subscribe_id=*/3,
          /*start_group=*/4,
          /*start_object=*/3,
          /*end_group=*/4,
          /*end_object=*/6,
          /*subscriber_priority=*/0xaa,
          MoqtSubscribeParameters{std::nullopt, std::nullopt, std::nullopt,
                                  std::nullopt},
      };
      quiche::QuicheBuffer buffer;
      buffer = framer_.SerializeSubscribeUpdate(subscribe_update);
      EXPECT_GT(buffer.size(), 0);
      const uint8_t* end_group = BufferAtOffset(buffer, 5);
      EXPECT_EQ(*end_group, 5);
      const uint8_t* end_object = end_group + 1;
      EXPECT_EQ(*end_object, 7);
    }

    TEST_F(MoqtFramerSimpleTest, SubscribeUpdateInvalidRange) {
      MoqtSubscribeUpdate subscribe_update = {
          /*subscribe_id=*/3,
          /*start_group=*/4,
          /*start_object=*/3,
          /*end_group=*/std::nullopt,
          /*end_object=*/6,
          /*subscriber_priority=*/0xaa,
          MoqtSubscribeParameters{std::nullopt, std::nullopt, std::nullopt,
                                  std::nullopt},
      };
      quiche::QuicheBuffer buffer;
      EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribeUpdate(subscribe_update),
                      "Invalid object range");
      EXPECT_EQ(buffer.size(), 0);
    }*/
}
