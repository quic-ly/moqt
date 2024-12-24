use crate::moqt_messages::*;
use crate::moqt_priority::MoqtDeliveryOrder;
use crate::serde::{data_writer::*, wire_serialization::*};
use crate::{compute_length_on_wire, serialize_into_writer};


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
        match serialize_into_buffer!($($data),*) {
            Ok(buffer) => buffer,
            Err(err) => {
                error!("Failed to serialize data: {:?}", err);
                BytesMut::new()
            }
        }
    }};
}

#[macro_export]
macro_rules! serialize_control_message {
    ($enum_type:expr, $($data:expr),*) => {{
        let message_type = WireVarInt62::new($enum_type as u64);
        let payload_size = compute_length_on_wire!($($data),*);
        let buffer_size = payload_size
            + compute_length_on_wire!(message_type, WireVarInt62::new(payload_size as u64));

        if buffer_size == 0 {
            return Ok(BytesMut::new());
        }

        let mut buffer = BytesMut::with_capacity(buffer_size);

        serialize_into_writer!(
            &mut buffer,
            message_type,
            WireVarInt62::new(payload_size as u64),
            $($data),*
        ).map_err(|e| anyhow!("Failed to serialize data: {:?}", e))?;

        if buffer.len() != buffer_size {
            Err(anyhow!(
                "Excess {} bytes allocated while serializing",
                buffer_size - buffer.len()
            ))?;
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

    /*
      // Serialize functions. Takes structured data and serializes it into a
      // QuicheBuffer for delivery to the stream.

      // Serializes the header for an object, including the appropriate stream
      // header if `is_first_in_stream` is set to true.
      quiche::QuicheBuffer SerializeObjectHeader(const MoqtObject& message,
                                                 MoqtDataStreamType message_type,
                                                 bool is_first_in_stream) {

    }

      quiche::QuicheBuffer SerializeObjectDatagram(const MoqtObject& message,
                                                   absl::string_view payload);
      quiche::QuicheBuffer SerializeClientSetup(const MoqtClientSetup& message);
      quiche::QuicheBuffer SerializeServerSetup(const MoqtServerSetup& message);
      // Returns an empty buffer if there is an illegal combination of locations.
      quiche::QuicheBuffer SerializeSubscribe(const MoqtSubscribe& message);
      quiche::QuicheBuffer SerializeSubscribeOk(const MoqtSubscribeOk& message);
      quiche::QuicheBuffer SerializeSubscribeError(
          const MoqtSubscribeError& message);
      quiche::QuicheBuffer SerializeUnsubscribe(const MoqtUnsubscribe& message);
      quiche::QuicheBuffer SerializeSubscribeDone(const MoqtSubscribeDone& message);
      quiche::QuicheBuffer SerializeSubscribeUpdate(
          const MoqtSubscribeUpdate& message);
      quiche::QuicheBuffer SerializeAnnounce(const MoqtAnnounce& message);
      quiche::QuicheBuffer SerializeAnnounceOk(const MoqtAnnounceOk& message);
      quiche::QuicheBuffer SerializeAnnounceError(const MoqtAnnounceError& message);
      quiche::QuicheBuffer SerializeAnnounceCancel(
          const MoqtAnnounceCancel& message);
      quiche::QuicheBuffer SerializeTrackStatusRequest(
          const MoqtTrackStatusRequest& message);
      quiche::QuicheBuffer SerializeUnannounce(const MoqtUnannounce& message);
      quiche::QuicheBuffer SerializeTrackStatus(const MoqtTrackStatus& message);
      quiche::QuicheBuffer SerializeGoAway(const MoqtGoAway& message);
      quiche::QuicheBuffer SerializeSubscribeAnnounces(
          const MoqtSubscribeAnnounces& message);
      quiche::QuicheBuffer SerializeSubscribeAnnouncesOk(
          const MoqtSubscribeAnnouncesOk& message);
      quiche::QuicheBuffer SerializeSubscribeAnnouncesError(
          const MoqtSubscribeAnnouncesError& message);
      quiche::QuicheBuffer SerializeUnsubscribeAnnounces(
          const MoqtUnsubscribeAnnounces& message);
      quiche::QuicheBuffer SerializeMaxSubscribeId(
          const MoqtMaxSubscribeId& message);
      quiche::QuicheBuffer SerializeFetch(const MoqtFetch& message);
      quiche::QuicheBuffer SerializeFetchCancel(const MoqtFetchCancel& message);
      quiche::QuicheBuffer SerializeFetchOk(const MoqtFetchOk& message);
      quiche::QuicheBuffer SerializeFetchError(const MoqtFetchError& message);
      quiche::QuicheBuffer SerializeObjectAck(const MoqtObjectAck& message);
    */
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