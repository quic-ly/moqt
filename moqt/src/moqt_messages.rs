use crate::moqt_priority::{MoqtDeliveryOrder, MoqtPriority};
use crate::quic_types;
use log::error;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Display;
use std::time::Duration;
/*TODO: inline constexpr quic::ParsedQuicVersionVector GetMoqtSupportedQuicVersions() {
   return quic::ParsedQuicVersionVector{quic::ParsedQuicVersion::RFCv1()}
}
*/

#[allow(non_camel_case_types)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum MoqtVersion {
    #[default]
    kDraft07 = 0xff000007,
    kUnrecognizedVersionForTests = 0xfe0000ff,
}

#[allow(non_upper_case_globals)]
pub const kDefaultMoqtVersion: MoqtVersion = MoqtVersion::kDraft07;
#[allow(non_upper_case_globals)]
pub const kDefaultInitialMaxSubscribeId: u64 = 100;

pub struct MoqtSessionParameters {
    // TODO: support multiple versions.
    // TODO: support roles other than PubSub.
    version: MoqtVersion,
    perspective: quic_types::Perspective,
    using_webtrans: bool,
    path: Option<String>,
    max_subscribe_id: u64,
    deliver_partial_objects: bool,
    support_object_acks: bool,
}

impl MoqtSessionParameters {
    pub fn new(perspective: quic_types::Perspective, path: Option<String>) -> Self {
        Self {
            version: kDefaultMoqtVersion,
            perspective,
            using_webtrans: path.is_none(),
            path,
            max_subscribe_id: kDefaultInitialMaxSubscribeId,
            deliver_partial_objects: false,
            support_object_acks: false,
        }
    }
}

/// The maximum length of a message, excluding any OBJECT payload. This prevents
/// DoS attack via forcing the parser to buffer a large message (OBJECT payloads
/// are not buffered by the parser).
#[allow(non_upper_case_globals)]
pub const kMaxMessageHeaderSize: usize = 2048;

#[allow(non_camel_case_types)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum MoqtDataStreamType {
    #[default]
    kObjectDatagram = 0x01,
    kStreamHeaderSubgroup = 0x04,
    kStreamHeaderFetch = 0x05,

    /// Currently QUICHE-specific.  All data on a kPadding stream is ignored.
    kPadding = 0x26d3,
}

impl Display for MoqtDataStreamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            MoqtDataStreamType::kObjectDatagram => "OBJECT_PREFER_DATAGRAM",
            MoqtDataStreamType::kStreamHeaderSubgroup => "STREAM_HEADER_SUBGROUP",
            MoqtDataStreamType::kStreamHeaderFetch => "STREAM_HEADER_FETCH",
            MoqtDataStreamType::kPadding => "PADDING",
        };

        write!(f, "{}", s)
    }
}

impl MoqtDataStreamType {
    pub fn get_forwarding_preference(&self) -> MoqtForwardingPreference {
        match *self {
            MoqtDataStreamType::kObjectDatagram => return MoqtForwardingPreference::kDatagram,
            MoqtDataStreamType::kStreamHeaderSubgroup => {
                return MoqtForwardingPreference::kSubgroup
            }
            MoqtDataStreamType::kStreamHeaderFetch => {
                error!("Forwarding preference for fetch is not supported");
            }
            _ => {}
        }
        error!("Message type does not indicate forwarding preference");
        MoqtForwardingPreference::kSubgroup
    }
}

#[allow(non_camel_case_types)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum MoqtMessageType {
    #[default]
    kSubscribeUpdate = 0x02,
    kSubscribe = 0x03,
    kSubscribeOk = 0x04,
    kSubscribeError = 0x05,
    kAnnounce = 0x06,
    kAnnounceOk = 0x7,
    kAnnounceError = 0x08,
    kUnannounce = 0x09,
    kUnsubscribe = 0x0a,
    kSubscribeDone = 0x0b,
    kAnnounceCancel = 0x0c,
    kTrackStatusRequest = 0x0d,
    kTrackStatus = 0x0e,
    kGoAway = 0x10,
    kSubscribeAnnounces = 0x11,
    kSubscribeAnnouncesOk = 0x12,
    kSubscribeAnnouncesError = 0x13,
    kUnsubscribeAnnounces = 0x14,
    kMaxSubscribeId = 0x15,
    kFetch = 0x16,
    kFetchCancel = 0x17,
    kFetchOk = 0x18,
    kFetchError = 0x19,
    kClientSetup = 0x40,
    kServerSetup = 0x41,

    /// QUICHE-specific extensions.
    /// kObjectAck (OACK for short) is a frame used by the receiver indicating that
    /// it has received and processed the specified object.
    kObjectAck = 0x3184,
}

impl Display for MoqtMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            MoqtMessageType::kClientSetup => "CLIENT_SETUP",
            MoqtMessageType::kServerSetup => "SERVER_SETUP",
            MoqtMessageType::kSubscribe => "SUBSCRIBE_REQUEST",
            MoqtMessageType::kSubscribeOk => "SUBSCRIBE_OK",
            MoqtMessageType::kSubscribeError => "SUBSCRIBE_ERROR",
            MoqtMessageType::kUnsubscribe => "UNSUBSCRIBE",
            MoqtMessageType::kSubscribeDone => "SUBSCRIBE_DONE",
            MoqtMessageType::kSubscribeUpdate => "SUBSCRIBE_UPDATE",
            MoqtMessageType::kAnnounceCancel => "ANNOUNCE_CANCEL",
            MoqtMessageType::kTrackStatusRequest => "TRACK_STATUS_REQUEST",
            MoqtMessageType::kTrackStatus => "TRACK_STATUS",
            MoqtMessageType::kAnnounce => "ANNOUNCE",
            MoqtMessageType::kAnnounceOk => "ANNOUNCE_OK",
            MoqtMessageType::kAnnounceError => "ANNOUNCE_ERROR",
            MoqtMessageType::kUnannounce => "UNANNOUNCE",
            MoqtMessageType::kGoAway => "GOAWAY",
            MoqtMessageType::kSubscribeAnnounces => "SUBSCRIBE_NAMESPACE",
            MoqtMessageType::kSubscribeAnnouncesOk => "SUBSCRIBE_NAMESPACE_OK",
            MoqtMessageType::kSubscribeAnnouncesError => "SUBSCRIBE_NAMESPACE_ERROR",
            MoqtMessageType::kUnsubscribeAnnounces => "UNSUBSCRIBE_NAMESPACE",
            MoqtMessageType::kMaxSubscribeId => "MAX_SUBSCRIBE_ID",
            MoqtMessageType::kFetch => "FETCH",
            MoqtMessageType::kFetchCancel => "FETCH_CANCEL",
            MoqtMessageType::kFetchOk => "FETCH_OK",
            MoqtMessageType::kFetchError => "FETCH_ERROR",
            MoqtMessageType::kObjectAck => "OBJECT_ACK",
        };

        write!(f, "{}", s)
    }
}

#[allow(non_camel_case_types)]
#[allow(clippy::enum_variant_names)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum MoqtError {
    #[default]
    kNoError = 0x0,
    kInternalError = 0x1,
    kUnauthorized = 0x2,
    kProtocolViolation = 0x3,
    kDuplicateTrackAlias = 0x4,
    kParameterLengthMismatch = 0x5,
    kTooManySubscribes = 0x6,
    kGoawayTimeout = 0x10,
}

// TODO: update with spec-defined error codes once those are available, see
// <https://github.com/moq-wg/moq-transport/issues/481>.
/// Error codes used by MoQT to reset streams.
#[allow(non_upper_case_globals)]
pub const kResetCodeUnknown: u64 = 0x00;
#[allow(non_upper_case_globals)]
pub const kResetCodeSubscriptionGone: u64 = 0x01;
#[allow(non_upper_case_globals)]
pub const kResetCodeTimedOut: u64 = 0x02;

#[allow(non_camel_case_types)]
#[allow(clippy::enum_variant_names)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum MoqtRole {
    #[default]
    kPublisher = 0x1,
    kSubscriber = 0x2,
    kPubSub = 0x3,
    //kRoleMax = 0x3,
}

#[allow(non_camel_case_types)]
#[allow(clippy::enum_variant_names)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum MoqtSetupParameter {
    #[default]
    kRole = 0x0,
    kPath = 0x1,
    kMaxSubscribeId = 0x2,

    /// QUICHE-specific extensions.
    /// Indicates support for OACK messages.
    kSupportObjectAcks = 0xbbf1439,
}

#[allow(non_camel_case_types)]
#[allow(clippy::enum_variant_names)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum MoqtTrackRequestParameter {
    #[default]
    kAuthorizationInfo = 0x2,
    kDeliveryTimeout = 0x3,
    kMaxCacheDuration = 0x4,

    /// QUICHE-specific extensions.
    kOackWindowSize = 0xbbf1439,
}

// TODO: those are non-standard; add standard error codes once those exist, see
// <https://github.com/moq-wg/moq-transport/issues/393>.
#[allow(non_camel_case_types)]
#[allow(clippy::enum_variant_names)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum MoqtAnnounceErrorCode {
    #[default]
    kInternalError = 0,
    kAnnounceNotSupported = 1,
}

#[allow(non_camel_case_types)]
#[allow(clippy::enum_variant_names)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum SubscribeErrorCode {
    #[default]
    kInternalError = 0x0,
    kInvalidRange = 0x1,
    kRetryTrackAlias = 0x2,
    kTrackDoesNotExist = 0x3,
    kUnauthorized = 0x4,
    kTimeout = 0x5,
}

struct MoqtSubscribeErrorReason {
    error_code: SubscribeErrorCode,
    reason_phrase: String,
}

struct MoqtAnnounceErrorReason {
    error_code: MoqtAnnounceErrorCode,
    reason_phrase: String,
}

/// Full track name represents a tuple of name elements. All higher order
/// elements MUST be present, but lower-order ones (like the name) can be
/// omitted.
#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct FullTrackName {
    tuple: Vec<String>,
}

impl fmt::Display for FullTrackName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bits = vec![];
        for raw_bit in &self.tuple {
            //TODO: absl::CHexEscape(raw_bit)
            bits.push("\"".to_owned() + raw_bit + "\"");
        }

        write!(f, "{{{}}}", bits.join(", "))
    }
}

impl FullTrackName {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_namespace_and_name(ns: &str, name: &str) -> Self {
        Self {
            tuple: vec![ns.to_string(), name.to_string()],
        }
    }

    pub fn new_with_elements(elements: Vec<String>) -> Self {
        Self { tuple: elements }
    }

    /// add an element into the last of tuple
    pub fn add_element(&mut self, element: &str) {
        self.tuple.push(element.to_string());
    }

    /// Remove the last element to convert a name to a namespace.
    pub fn name_to_namespace(&mut self) {
        self.tuple.pop();
    }

    /// returns true is |this| is a subdomain of |other|.
    pub fn in_namespace(&self, other: &Self) -> bool {
        if self.tuple.len() < other.tuple.len() {
            return false;
        }
        for i in 0..other.tuple.len() {
            if self.tuple[i] != other.tuple[i] {
                return false;
            }
        }
        true
    }

    pub fn tuple(&self) -> &[String] {
        &self.tuple
    }

    pub fn empty(&self) -> bool {
        self.tuple.is_empty()
    }
}

/// These are absolute sequence numbers.
#[derive(Default, Copy, Clone, Debug)]
pub struct FullSequence {
    group: u64,
    subgroup: u64,
    object: u64,
}

/// These are temporal ordering comparisons, so subgroup ID doesn't matter.
impl PartialOrd for FullSequence {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.group
                .cmp(&other.group)
                .then(self.object.cmp(&other.object)),
        )
    }
}

impl PartialEq for FullSequence {
    fn eq(&self, other: &Self) -> bool {
        self.group == other.group && self.object == other.object
    }
}

impl FullSequence {
    pub fn new(group: u64, subgroup: u64, object: u64) -> Self {
        Self {
            group,
            subgroup,
            object,
        }
    }

    pub fn next(&self) -> Self {
        Self {
            group: self.group,
            subgroup: self.subgroup,
            object: self.object + 1,
        }
    }
}

#[derive(Clone, PartialEq, Debug, PartialOrd)]
pub struct SubgroupPriority {
    publisher_priority: u8,
    subgroup_id: u64,
}

impl Default for SubgroupPriority {
    fn default() -> Self {
        Self {
            publisher_priority: 0xf0,
            subgroup_id: 0,
        }
    }
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtClientSetup {
    supported_versions: Vec<MoqtVersion>,
    role: Option<MoqtRole>,
    path: Option<String>,
    max_subscribe_id: Option<u64>,
    supports_object_ack: bool,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtServerSetup {
    selected_version: MoqtVersion,
    role: Option<MoqtRole>,
    max_subscribe_id: Option<u64>,
    supports_object_ack: bool,
}

/// These codes do not appear on the wire.
#[allow(non_camel_case_types)]
#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub enum MoqtForwardingPreference {
    #[default]
    kSubgroup,
    kDatagram,
}

impl Display for MoqtForwardingPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            MoqtForwardingPreference::kDatagram => "DATAGRAM",
            MoqtForwardingPreference::kSubgroup => "SUBGROUP",
        };

        write!(f, "{}", s)
    }
}

impl MoqtForwardingPreference {
    pub fn get_message_type_for_forwarding_preference(&self) -> MoqtDataStreamType {
        match *self {
            MoqtForwardingPreference::kDatagram => MoqtDataStreamType::kObjectDatagram,
            MoqtForwardingPreference::kSubgroup => MoqtDataStreamType::kStreamHeaderSubgroup,
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
#[repr(u64)]
pub enum MoqtObjectStatus {
    #[default]
    kNormal = 0x0,
    kObjectDoesNotExist = 0x1,
    kGroupDoesNotExist = 0x2,
    kEndOfGroup = 0x3,
    kEndOfTrack = 0x4,
    kEndOfSubgroup = 0x5,
    kInvalidObjectStatus = 0x6,
}

impl From<u64> for MoqtObjectStatus {
    fn from(v: u64) -> Self {
        match v {
            0 => MoqtObjectStatus::kNormal,
            1 => MoqtObjectStatus::kObjectDoesNotExist,
            2 => MoqtObjectStatus::kGroupDoesNotExist,
            3 => MoqtObjectStatus::kEndOfGroup,
            4 => MoqtObjectStatus::kEndOfTrack,
            5 => MoqtObjectStatus::kEndOfSubgroup,
            _ => MoqtObjectStatus::kInvalidObjectStatus,
        }
    }
}

/// The data contained in every Object message, although the message type
/// implies some of the values.
#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtObject {
    track_alias: u64,
    /// For FETCH, this is the subscribe ID.
    group_id: u64,
    object_id: u64,
    publisher_priority: MoqtPriority,
    pub(crate) object_status: MoqtObjectStatus,
    pub(crate) subgroup_id: Option<u64>,
    pub(crate) payload_length: u64,
}

#[allow(non_camel_case_types)]
#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
#[repr(u64)]
pub enum MoqtFilterType {
    #[default]
    kNone = 0x0,
    kLatestGroup = 0x1,
    kLatestObject = 0x2,
    kAbsoluteStart = 0x3,
    kAbsoluteRange = 0x4,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribeParameters {
    pub(crate) authorization_info: Option<String>,
    pub(crate) delivery_timeout: Option<Duration>,
    pub(crate) max_cache_duration: Option<Duration>,

    /// If present, indicates that OBJECT_ACK messages will be sent in response to
    /// the objects on the stream. The actual value is informational, and it
    /// communicates how many frames the subscriber is willing to buffer, in
    /// microseconds.
    pub(crate) object_ack_window: Option<Duration>,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribe {
    subscribe_id: u64,
    track_alias: u64,
    full_track_name: FullTrackName,
    subscriber_priority: MoqtPriority,
    group_order: Option<MoqtDeliveryOrder>,

    // The combinations of these that have values indicate the filter type.
    // SG: Start Group; SO: Start Object; EG: End Group; EO: End Object;
    // (none): KLatestObject
    // SO: kLatestGroup (must be zero)
    // SG, SO: kAbsoluteStart
    // SG, SO, EG, EO: kAbsoluteRange
    // SG, SO, EG: kAbsoluteRange (request whole last group)
    // All other combinations are invalid.
    start_group: Option<u64>,
    start_object: Option<u64>,
    end_group: Option<u64>,
    end_object: Option<u64>,
    // If the mode is kNone, the these are std::nullopt.
    parameters: MoqtSubscribeParameters,
}

/// Deduce the filter type from the combination of group and object IDs. Returns
/// kNone if the state of the subscribe is invalid.
pub fn get_filter_type(message: &MoqtSubscribe) -> MoqtFilterType {
    if message.end_group.is_none() && message.end_object.is_some() {
        return MoqtFilterType::kNone;
    }
    let has_start = message.start_group.is_some() && message.start_object.is_some();
    if let (Some(start_group), Some(end_group)) = (message.start_group, message.end_group) {
        if has_start {
            match end_group.cmp(&start_group) {
                Ordering::Less => return MoqtFilterType::kNone,
                Ordering::Equal => {
                    if let (Some(start_object), Some(end_object)) =
                        (message.start_object, message.end_object)
                    {
                        if end_object <= start_object {
                            match end_object.cmp(&start_object) {
                                Ordering::Less => return MoqtFilterType::kNone,
                                Ordering::Equal => return MoqtFilterType::kAbsoluteStart,
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }

            return MoqtFilterType::kAbsoluteRange;
        }
    } else if has_start {
        return MoqtFilterType::kAbsoluteStart;
    } else if message.start_group.is_none() {
        if let Some(start_object) = message.start_object {
            if start_object == 0 {
                return MoqtFilterType::kLatestGroup;
            }
        } else {
            return MoqtFilterType::kLatestObject;
        }
    }

    MoqtFilterType::kNone
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribeOk {
    subscribe_id: u64,
    /// The message uses ms, but expires is in us.
    expires: Duration,
    group_order: MoqtDeliveryOrder,
    /// If ContextExists on the wire is zero, largest_id has no value.
    largest_id: Option<FullSequence>,
    parameters: MoqtSubscribeParameters,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribeError {
    subscribe_id: u64,
    error_code: SubscribeErrorCode,
    reason_phrase: String,
    track_alias: u64,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtUnsubscribe {
    subscribe_id: u64,
}

#[allow(non_camel_case_types)]
#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
#[repr(u64)]
pub enum SubscribeDoneCode {
    #[default]
    kUnsubscribed = 0x0,
    kInternalError = 0x1,
    kUnauthorized = 0x2,
    kTrackEnded = 0x3,
    kSubscriptionEnded = 0x4,
    kGoingAway = 0x5,
    kExpired = 0x6,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribeDone {
    subscribe_id: u64,
    status_code: SubscribeDoneCode,
    reason_phrase: String,
    final_id: Option<FullSequence>,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribeUpdate {
    subscribe_id: u64,
    start_group: u64,
    start_object: u64,
    end_group: Option<u64>,
    end_object: Option<u64>,
    subscriber_priority: MoqtPriority,
    parameters: MoqtSubscribeParameters,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtAnnounce {
    track_namespace: FullTrackName,
    parameters: MoqtSubscribeParameters,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtAnnounceOk {
    track_namespace: FullTrackName,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtAnnounceError {
    track_namespace: FullTrackName,
    error_code: MoqtAnnounceErrorCode,
    reason_phrase: String,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtUnannounce {
    track_namespace: FullTrackName,
}

#[allow(non_camel_case_types)]
#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
#[repr(u64)]
pub enum MoqtTrackStatusCode {
    #[default]
    kInProgress = 0x0,
    kDoesNotExist = 0x1,
    kNotYetBegun = 0x2,
    kFinished = 0x3,
    kStatusNotAvailable = 0x4,
}

pub fn does_track_status_imply_having_data(code: MoqtTrackStatusCode) -> bool {
    match code {
        MoqtTrackStatusCode::kInProgress | MoqtTrackStatusCode::kFinished => true,
        MoqtTrackStatusCode::kDoesNotExist
        | MoqtTrackStatusCode::kNotYetBegun
        | MoqtTrackStatusCode::kStatusNotAvailable => false,
    }
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtTrackStatus {
    full_track_name: FullTrackName,
    status_code: MoqtTrackStatusCode,
    last_group: u64,
    last_object: u64,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtAnnounceCancel {
    track_namespace: FullTrackName,
    error_code: MoqtAnnounceErrorCode,
    reason_phrase: String,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtTrackStatusRequest {
    full_track_name: FullTrackName,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtGoAway {
    new_session_uri: String,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribeAnnounces {
    track_namespace: FullTrackName,
    parameters: MoqtSubscribeParameters,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribeAnnouncesOk {
    track_namespace: FullTrackName,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtSubscribeAnnouncesError {
    track_namespace: FullTrackName,
    error_code: SubscribeErrorCode,
    reason_phrase: String,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtUnsubscribeAnnounces {
    track_namespace: FullTrackName,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtMaxSubscribeId {
    max_subscribe_id: u64,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtFetch {
    subscribe_id: u64,
    full_track_name: FullTrackName,
    subscriber_priority: MoqtPriority,
    group_order: Option<MoqtDeliveryOrder>,
    start_object: FullSequence,
    /// subgroup is ignored
    end_group: u64,
    end_object: Option<u64>,
    parameters: MoqtSubscribeParameters,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtFetchCancel {
    subscribe_id: u64,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtFetchOk {
    subscribe_id: u64,
    group_order: MoqtDeliveryOrder,
    largest_id: FullSequence, // subgroup is ignored
    parameters: MoqtSubscribeParameters,
}

#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtFetchError {
    subscribe_id: u64,
    error_code: SubscribeErrorCode,
    reason_phrase: String,
}

/// All of the four values in this message are encoded as varints.
/// `delta_from_deadline` is encoded as an absolute value, with the lowest bit
/// indicating the sign (0 if positive).
#[derive(Default, Clone, PartialEq, Debug, PartialOrd)]
pub struct MoqtObjectAck {
    subscribe_id: u64,
    group_id: u64,
    object_id: u64,
    /// Positive if the object has been received before the deadline.
    delta_from_deadline: Duration,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_full_track_name_constructors() {
        let name1 = FullTrackName::new_with_namespace_and_name("foo", "bar");
        let list = vec!["foo".to_string(), "bar".to_string()];
        let name2 = FullTrackName::new_with_elements(list);
        assert_eq!(name1, name2);
        //assert_eq!(HashOf(name1), HashOf(name2));
    }

    #[test]
    fn test_full_track_name_order() {
        let name1 = FullTrackName::new_with_elements(vec!["a".to_string(), "b".to_string()]);
        let name2 = FullTrackName::new_with_elements(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
        ]);
        let name3 = FullTrackName::new_with_elements(vec!["b".to_string(), "a".to_string()]);
        assert!(name1 < name2);
        assert!(name2 < name3);
        assert!(name1 < name3);
    }

    #[test]
    fn test_full_track_name_in_namespace() {
        let name1 = FullTrackName::new_with_elements(vec!["a".to_string(), "b".to_string()]);
        let name2 = FullTrackName::new_with_elements(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
        ]);
        let name3 = FullTrackName::new_with_elements(vec!["b".to_string(), "a".to_string()]);

        assert!(name2.in_namespace(&name1));
        assert!(!name1.in_namespace(&name2));
        assert!(name1.in_namespace(&name1));
        assert!(!name2.in_namespace(&name3));
    }

    #[test]
    fn test_full_track_name_to_string() {
        let name1 = FullTrackName::new_with_elements(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(name1.to_string(), r#"{"a", "b"}"#);

        //TODO: let name2 = FullTrackName::new_with_elements(vec!["\xff".to_string(), "\x61".to_string()]);
        // assert_eq!(name2.to_string(), r#"{"\xff", "a"}"#);
    }
}
