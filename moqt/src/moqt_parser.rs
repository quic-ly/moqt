use crate::moqt_messages::*;
use crate::moqt_priority::MoqtDeliveryOrder;
use crate::serde::data_reader::DataReader;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::VecDeque;
use std::io::{Error, ErrorKind};
use std::time::Duration;

/// All of these are called only when the entire message has arrived. The
/// parser retains ownership of the memory.
pub enum MoqtControlParserEvent {
    OnClientSetupMessage(MoqtClientSetup),
    OnServerSetupMessage(MoqtServerSetup),
    OnSubscribeMessage(MoqtSubscribe),
    OnSubscribeOkMessage(MoqtSubscribeOk),
    OnSubscribeErrorMessage(MoqtSubscribeError),
    OnUnsubscribeMessage(MoqtUnsubscribe),
    OnSubscribeDoneMessage(MoqtSubscribeDone),
    OnSubscribeUpdateMessage(MoqtSubscribeUpdate),
    OnAnnounceMessage(MoqtAnnounce),
    OnAnnounceOkMessage(MoqtAnnounceOk),
    OnAnnounceErrorMessage(MoqtAnnounceError),
    OnAnnounceCancelMessage(MoqtAnnounceCancel),
    OnTrackStatusRequestMessage(MoqtTrackStatusRequest),
    OnUnannounceMessage(MoqtUnannounce),
    OnTrackStatusMessage(MoqtTrackStatus),
    OnGoAwayMessage(MoqtGoAway),
    OnSubscribeAnnouncesMessage(MoqtSubscribeAnnounces),
    OnSubscribeAnnouncesOkMessage(MoqtSubscribeAnnouncesOk),
    OnSubscribeAnnouncesErrorMessage(MoqtSubscribeAnnouncesError),
    OnUnsubscribeAnnouncesMessage(MoqtUnsubscribeAnnounces),
    OnMaxSubscribeIdMessage(MoqtMaxSubscribeId),
    OnFetchMessage(MoqtFetch),
    OnFetchCancelMessage(MoqtFetchCancel),
    OnFetchOkMessage(MoqtFetchOk),
    OnFetchErrorMessage(MoqtFetchError),
    OnObjectAckMessage(MoqtObjectAck),
    OnParsingError(MoqtError, String /*reason*/),
}

/// If |end_of_message| is true, |payload| contains the last bytes of the
/// OBJECT payload. If not, there will be subsequent calls with further payload
/// data. The parser retains ownership of |message| and |payload|, so the
/// visitor needs to copy anything it wants to retain.
pub enum MoqtDataParserEvent {
    OnObjectMessage(
        MoqtObject,
        Bytes, /*payload*/
        bool,  /*end_of_message*/
    ),
    OnParsingError(MoqtError, String /*reason*/),
}

fn parse_delivery_order(raw_value: u8) -> Result<Option<MoqtDeliveryOrder>, Error> {
    match raw_value {
        0x00 => Ok(None),
        0x01 => Ok(Some(MoqtDeliveryOrder::kAscending)),
        0x02 => Ok(Some(MoqtDeliveryOrder::kDescending)),
        _ => Err(Error::from(ErrorKind::InvalidInput)),
    }
}

fn signed_varint_unserialized_form(value: u64) -> u64 {
    if (value & 0x01) != 0 {
        // Handle negative result using two's complement representation
        (!((value >> 1) as i64) + 1) as u64
    } else {
        value >> 1
    }
}

fn is_allowed_stream_type(value: u64) -> bool {
    let allowed_stream_types = [
        MoqtDataStreamType::kStreamHeaderSubgroup,
        MoqtDataStreamType::kStreamHeaderFetch,
        MoqtDataStreamType::kPadding,
    ];
    for allowed_stream_type in allowed_stream_types {
        if allowed_stream_type as u64 == value {
            return true;
        }
    }
    false
}

pub struct MoqtControlParser {
    events: VecDeque<MoqtControlParserEvent>,
    uses_web_transport: bool,
    no_more_data: bool,
    parsing_error: bool,

    buffered_message: Option<BytesMut>,

    processing: bool,
}

impl MoqtControlParser {
    pub fn new(uses_web_transport: bool) -> Self {
        Self {
            events: VecDeque::new(),
            uses_web_transport,
            no_more_data: false, // Fatal error or fin. No more parsing.
            parsing_error: false,

            buffered_message: None,

            processing: false, // True if currently in process_data(), to prevent re-entrancy.
        }
    }

    /// Take a buffer from the transport in |data|. Parse each complete message and
    /// call the appropriate visitor function. If |fin| is true, there
    /// is no more data arriving on the stream, so the parser will deliver any
    /// message encoded as to run to the end of the stream.
    /// All bytes can be freed. Calls OnParsingError() when there is a parsing
    /// error.
    /// Any calls after sending |fin| = true will be ignored.
    /// TODO: Figure out what has to happen if the message arrives via datagram rather than a stream.
    ///
    /// The buffering philosophy is complicated, to minimize copying. Here is an
    /// overview:
    /// If the entire message body is present (except for OBJECT payload), it is
    /// parsed and delivered. If not, the partial body is buffered. (requiring a
    /// copy).
    /// Any OBJECT payload is always delivered to the application without copying.
    /// If something has been buffered, when more data arrives copy just enough of it
    /// to finish parsing that thing, then resume normal processing.
    pub fn process_data<R: Buf>(&mut self, data: &mut R, fin: bool) {
        if self.no_more_data {
            self.parse_error(MoqtError::kProtocolViolation, "Data after end of stream");
        }
        if self.processing {
            return;
        }

        // Check for early fin
        if fin {
            self.no_more_data = true;
            if self.buffered_message.is_some() && !data.has_remaining() {
                self.parse_error(
                    MoqtError::kProtocolViolation,
                    "End of stream before complete message",
                );
                return;
            }
        }

        if let Some(buffered_message) = self.buffered_message.as_mut() {
            buffered_message.put(data);
        } else if data.has_remaining() {
            let mut buffered_message = BytesMut::new();
            buffered_message.put(data);
            self.buffered_message = Some(buffered_message);
        } else {
            return;
        }

        self.processing = true;
        let mut buffered_message = self.buffered_message.take().unwrap();
        while buffered_message.has_remaining() {
            let message_len = self
                .process_message(&mut buffered_message.as_ref())
                .unwrap_or(0);
            if message_len == 0 {
                if buffered_message.remaining() > kMaxMessageHeaderSize {
                    self.parse_error(
                        MoqtError::kInternalError,
                        "Cannot parse non-OBJECT messages > 2KB",
                    );
                } else if fin {
                    self.parse_error(
                        MoqtError::kProtocolViolation,
                        "FIN after incomplete message",
                    );
                }
                break;
            }
            // A message was successfully processed.
            buffered_message.advance(message_len);
        }

        if buffered_message.has_remaining() {
            self.buffered_message = Some(buffered_message);
        }
        self.processing = false;
    }

    // The central switch statement to dispatch a message to the correct
    // Process* function. Returns 0 if it could not parse the full messsage
    // (except for object payload). Otherwise, returns the number of bytes
    // processed.
    fn process_message<R: Buf>(&mut self, data: &mut R) -> Result<usize, Error> {
        let mut reader = DataReader::new(data);
        let value = reader.read_var_int62()?;
        let length = reader.read_var_int62()? as usize;

        if length > reader.remaining() {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "MoqtControl parser buffer is full",
            ));
        }
        let t = MoqtMessageType::try_from(value)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        let message_header_length = reader.bytes_read();
        let bytes_read = match t {
            MoqtMessageType::kClientSetup => self.process_client_setup(&mut reader)?,
            MoqtMessageType::kServerSetup => self.process_server_setup(&mut reader)?,
            MoqtMessageType::kSubscribe => self.process_subscribe(&mut reader)?,
            MoqtMessageType::kSubscribeOk => self.process_subscribe_ok(&mut reader)?,
            MoqtMessageType::kSubscribeError => self.process_subscribe_error(&mut reader)?,
            MoqtMessageType::kUnsubscribe => self.process_unsubscribe(&mut reader)?,
            MoqtMessageType::kSubscribeDone => self.process_subscribe_done(&mut reader)?,
            MoqtMessageType::kSubscribeUpdate => self.process_subscribe_update(&mut reader)?,
            MoqtMessageType::kAnnounce => self.process_announce(&mut reader)?,
            MoqtMessageType::kAnnounceOk => self.process_announce_ok(&mut reader)?,
            MoqtMessageType::kAnnounceError => self.process_announce_error(&mut reader)?,
            MoqtMessageType::kAnnounceCancel => self.process_announce_cancel(&mut reader)?,
            MoqtMessageType::kTrackStatusRequest => {
                self.process_track_status_request(&mut reader)?
            }
            MoqtMessageType::kUnannounce => self.process_unannounce(&mut reader)?,
            MoqtMessageType::kTrackStatus => self.process_track_status(&mut reader)?,
            MoqtMessageType::kGoAway => self.process_go_away(&mut reader)?,
            MoqtMessageType::kSubscribeAnnounces => {
                self.process_subscribe_announces(&mut reader)?
            }
            MoqtMessageType::kSubscribeAnnouncesOk => {
                self.process_subscribe_announces_ok(&mut reader)?
            }
            MoqtMessageType::kSubscribeAnnouncesError => {
                self.process_subscribe_announces_error(&mut reader)?
            }
            MoqtMessageType::kUnsubscribeAnnounces => {
                self.process_unsubscribe_announces(&mut reader)?
            }
            MoqtMessageType::kMaxSubscribeId => self.process_max_subscribe_id(&mut reader)?,
            MoqtMessageType::kFetch => self.process_fetch(&mut reader)?,
            MoqtMessageType::kFetchCancel => self.process_fetch_cancel(&mut reader)?,
            MoqtMessageType::kFetchOk => self.process_fetch_ok(&mut reader)?,
            MoqtMessageType::kFetchError => self.process_fetch_error(&mut reader)?,
            MoqtMessageType::kObjectAck => self.process_object_ack(&mut reader)?,
        };
        if (bytes_read - message_header_length) != length {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "Message length does not match payload length",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }
        Ok(bytes_read)
    }

    // The Process* functions parse the serialized data into the appropriate
    // structs, and call the relevant visitor function for further action. Returns
    // the number of bytes consumed if the message is complete; returns 0
    // otherwise.
    fn process_client_setup(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let mut setup = MoqtClientSetup::default();
        let number_of_supported_versions = reader.read_var_int62()?;
        for _ in 0..number_of_supported_versions {
            let version = reader.read_var_int62()?;
            setup.supported_versions.push(version);
        }
        let num_params = reader.read_var_int62()?;
        // Parse parameters
        for _ in 0..num_params {
            let (t, value) = Self::read_parameter(reader)?;
            if let Ok(key) = MoqtSetupParameter::try_from(t) {
                match key {
                    MoqtSetupParameter::kRole => {
                        if setup.role.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "ROLE parameter appears twice in SETUP",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        let index = self.string_view_to_var_int(value.as_str())?;
                        setup.role = match MoqtRole::try_from(index) {
                            Ok(role) => Some(role),
                            Err(_) => {
                                self.parse_error(
                                    MoqtError::kProtocolViolation,
                                    "Invalid ROLE parameter",
                                );
                                return Err(Error::new(
                                    ErrorKind::Other,
                                    MoqtError::kProtocolViolation,
                                ));
                            }
                        };
                    }
                    MoqtSetupParameter::kPath => {
                        if self.uses_web_transport {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "WebTransport connection is using PATH parameter in SETUP",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        if setup.path.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "PATH parameter appears twice in CLIENT_SETUP",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        setup.path = Some(value);
                    }
                    MoqtSetupParameter::kMaxSubscribeId => {
                        if setup.max_subscribe_id.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "MAX_SUBSCRIBE_ID parameter appears twice in SETUP",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        let max_id = match self.string_view_to_var_int(value.as_str()) {
                            Ok(max_id) => max_id,
                            Err(_) => {
                                self.parse_error(
                                    MoqtError::kProtocolViolation,
                                    "MAX_SUBSCRIBE_ID parameter is not a valid varint",
                                );
                                return Err(Error::new(
                                    ErrorKind::Other,
                                    MoqtError::kProtocolViolation,
                                ));
                            }
                        };
                        setup.max_subscribe_id = Some(max_id);
                    }
                    MoqtSetupParameter::kSupportObjectAcks => {
                        let flag = self.string_view_to_var_int(value.as_str())?;
                        if flag > 1 {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "Invalid kSupportObjectAcks value",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        setup.supports_object_ack = flag == 1;
                    }
                }
            }
        }
        if setup.role.is_none() {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "ROLE parameter missing from CLIENT_SETUP message",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }
        if !self.uses_web_transport && setup.path.is_none() {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "PATH SETUP parameter missing from Client message over QUIC",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }
        self.events
            .push_back(MoqtControlParserEvent::OnClientSetupMessage(setup));
        Ok(reader.bytes_read())
    }
    fn process_server_setup(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let mut setup = MoqtServerSetup {
            selected_version: reader.read_var_int62()?,
            ..Default::default()
        };

        let num_params = reader.read_var_int62()?;
        // Parse parameters
        for _ in 0..num_params {
            let (t, value) = Self::read_parameter(reader)?;
            if let Ok(key) = MoqtSetupParameter::try_from(t) {
                match key {
                    MoqtSetupParameter::kRole => {
                        if setup.role.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "ROLE parameter appears twice in SETUP",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        let index = self.string_view_to_var_int(value.as_str())?;
                        setup.role = match MoqtRole::try_from(index) {
                            Ok(role) => Some(role),
                            Err(_) => {
                                self.parse_error(
                                    MoqtError::kProtocolViolation,
                                    "Invalid ROLE parameter",
                                );
                                return Err(Error::new(
                                    ErrorKind::Other,
                                    MoqtError::kProtocolViolation,
                                ));
                            }
                        };
                    }
                    MoqtSetupParameter::kPath => {
                        self.parse_error(
                            MoqtError::kProtocolViolation,
                            "PATH parameter in SERVER_SETUP",
                        );
                        return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
                    }
                    MoqtSetupParameter::kMaxSubscribeId => {
                        if setup.max_subscribe_id.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "MAX_SUBSCRIBE_ID parameter appears twice in SETUP",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        let max_id = match self.string_view_to_var_int(value.as_str()) {
                            Ok(max_id) => max_id,
                            Err(_) => {
                                self.parse_error(
                                    MoqtError::kProtocolViolation,
                                    "MAX_SUBSCRIBE_ID parameter is not a valid varint",
                                );
                                return Err(Error::new(
                                    ErrorKind::Other,
                                    MoqtError::kProtocolViolation,
                                ));
                            }
                        };
                        setup.max_subscribe_id = Some(max_id);
                    }
                    MoqtSetupParameter::kSupportObjectAcks => {
                        let flag = self.string_view_to_var_int(value.as_str())?;
                        if flag > 1 {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "Invalid kSupportObjectAcks value",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        setup.supports_object_ack = flag == 1;
                    }
                }
            }
        }
        if setup.role.is_none() {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "ROLE parameter missing from SERVER_SETUP message",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }
        self.events
            .push_back(MoqtControlParserEvent::OnServerSetupMessage(setup));
        Ok(reader.bytes_read())
    }
    fn process_subscribe(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let track_alias = reader.read_var_int62()?;
        let mut full_track_name = Self::read_track_namespace(reader)?;
        let track_name = reader.read_string_piece_var_int62()?;
        let subscriber_priority = reader.read_uint8()?;
        let group_order = reader.read_uint8()?;
        let filter = reader.read_var_int62()?;
        full_track_name.add_element(track_name);
        let group_order = match parse_delivery_order(group_order) {
            Ok(group_order) => group_order,
            Err(_) => {
                self.parse_error(
                    MoqtError::kProtocolViolation,
                    "Invalid group order value in SUBSCRIBE message",
                );
                return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
            }
        };
        let filter_type = MoqtFilterType::try_from(filter)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        let mut subscribe_request = MoqtSubscribe {
            subscribe_id,
            track_alias,
            full_track_name,
            subscriber_priority,
            group_order,
            ..Default::default()
        };
        match filter_type {
            MoqtFilterType::kLatestGroup => {
                subscribe_request.start_object = Some(0);
            }
            MoqtFilterType::kLatestObject => {}
            MoqtFilterType::kAbsoluteStart | MoqtFilterType::kAbsoluteRange => {
                let start_group = reader.read_var_int62()?;
                let start_object = reader.read_var_int62()?;
                subscribe_request.start_group = Some(start_group);
                subscribe_request.start_object = Some(start_object);
                if filter_type != MoqtFilterType::kAbsoluteStart {
                    let end_group = reader.read_var_int62()?;
                    let end_object = reader.read_var_int62()?;
                    subscribe_request.end_group = Some(end_group);
                    if end_group < start_group {
                        self.parse_error(
                            MoqtError::kProtocolViolation,
                            "End group is less than start group",
                        );
                        return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
                    }
                    if end_object == 0 {
                        subscribe_request.end_object = None;
                    } else {
                        subscribe_request.end_object = Some(end_object - 1);
                        if start_group == end_group && end_object < start_object {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "End object comes before start object",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                    }
                }
            }
            _ => {
                self.parse_error(MoqtError::kProtocolViolation, "Invalid filter type");
                return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
            }
        }
        subscribe_request.parameters = self.read_subscribe_parameters(reader)?;
        self.events
            .push_back(MoqtControlParserEvent::OnSubscribeMessage(
                subscribe_request,
            ));
        Ok(reader.bytes_read())
    }
    fn process_subscribe_ok(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let milliseconds = reader.read_var_int62()?;
        let group_order = reader.read_uint8()?;
        let content_exists = reader.read_uint8()?;
        if content_exists > 1 {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "SUBSCRIBE_OK ContentExists has invalid value",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }

        let expires = Duration::from_micros(milliseconds);
        let group_order = match MoqtDeliveryOrder::try_from(group_order) {
            Ok(group_order) => group_order,
            Err(_) => {
                self.parse_error(
                    MoqtError::kProtocolViolation,
                    "Invalid group order value in SUBSCRIBE_OK",
                );
                return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
            }
        };
        let largest_id = if content_exists != 0 {
            let largest_id_group = reader.read_var_int62()?;
            let largest_id_object = reader.read_var_int62()?;
            Some(FullSequence::new(largest_id_group, 0, largest_id_object))
        } else {
            None
        };
        let parameters = self.read_subscribe_parameters(reader)?;
        if parameters.authorization_info.is_some() {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "SUBSCRIBE_OK has authorization info",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }
        self.events
            .push_back(MoqtControlParserEvent::OnSubscribeOkMessage(
                MoqtSubscribeOk {
                    subscribe_id,
                    expires,
                    group_order,
                    largest_id,
                    parameters,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_subscribe_error(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let error_code = reader.read_var_int62()?;
        let reason_phrase = reader.read_string_var_int62()?;
        let track_alias = reader.read_var_int62()?;
        let error_code = SubscribeErrorCode::try_from(error_code)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        self.events
            .push_back(MoqtControlParserEvent::OnSubscribeErrorMessage(
                MoqtSubscribeError {
                    subscribe_id,
                    error_code,
                    reason_phrase,
                    track_alias,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_unsubscribe(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        self.events
            .push_back(MoqtControlParserEvent::OnUnsubscribeMessage(
                MoqtUnsubscribe { subscribe_id },
            ));
        Ok(reader.bytes_read())
    }
    fn process_subscribe_done(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let value = reader.read_var_int62()?;
        let reason_phrase = reader.read_string_var_int62()?;
        let content_exists = reader.read_uint8()?;
        let status_code = SubscribeDoneCode::try_from(value)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        if content_exists > 1 {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "SUBSCRIBE_DONE ContentExists has invalid value",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }
        let final_id = if content_exists == 1 {
            let final_id_group = reader.read_var_int62()?;
            let final_id_object = reader.read_var_int62()?;
            Some(FullSequence::new(final_id_group, 0, final_id_object))
        } else {
            None
        };
        self.events
            .push_back(MoqtControlParserEvent::OnSubscribeDoneMessage(
                MoqtSubscribeDone {
                    subscribe_id,
                    status_code,
                    reason_phrase,
                    final_id,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_subscribe_update(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let start_group = reader.read_var_int62()?;
        let start_object = reader.read_var_int62()?;
        let mut end_group = reader.read_var_int62()?;
        let mut end_object = reader.read_var_int62()?;
        let subscriber_priority = reader.read_uint8()?;
        let parameters = self.read_subscribe_parameters(reader)?;
        let end_group_opt = if end_group == 0 {
            // end_group remains nullopt.
            if end_object > 0 {
                self.parse_error(
                    MoqtError::kProtocolViolation,
                    "SUBSCRIBE_UPDATE has end_object but no end_group",
                );
                return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
            }
            None
        } else {
            end_group -= 1;
            if end_group < start_group {
                self.parse_error(
                    MoqtError::kProtocolViolation,
                    "End group is less than start group",
                );
                return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
            }
            Some(end_group)
        };

        let end_object = if end_object > 0 {
            end_object -= 1;
            if start_group == end_group && end_object < start_object {
                self.parse_error(
                    MoqtError::kProtocolViolation,
                    "End object comes before start object",
                );
                return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
            }
            Some(end_object)
        } else {
            None
        };
        if parameters.authorization_info.is_some() {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "SUBSCRIBE_UPDATE has authorization info",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }
        self.events
            .push_back(MoqtControlParserEvent::OnSubscribeUpdateMessage(
                MoqtSubscribeUpdate {
                    subscribe_id,
                    start_group,
                    start_object,
                    end_group: end_group_opt,
                    end_object,
                    subscriber_priority,
                    parameters,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_announce(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        let parameters = self.read_subscribe_parameters(reader)?;
        if parameters.delivery_timeout.is_some() {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "ANNOUNCE has delivery timeout",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }
        self.events
            .push_back(MoqtControlParserEvent::OnAnnounceMessage(MoqtAnnounce {
                track_namespace,
                parameters,
            }));
        Ok(reader.bytes_read())
    }
    fn process_announce_ok(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        self.events
            .push_back(MoqtControlParserEvent::OnAnnounceOkMessage(
                MoqtAnnounceOk { track_namespace },
            ));
        Ok(reader.bytes_read())
    }
    fn process_announce_error(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        let error_code = reader.read_var_int62()?;
        let reason_phrase = reader.read_string_var_int62()?;
        let error_code = MoqtAnnounceErrorCode::try_from(error_code)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        self.events
            .push_back(MoqtControlParserEvent::OnAnnounceErrorMessage(
                MoqtAnnounceError {
                    track_namespace,
                    error_code,
                    reason_phrase,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_announce_cancel(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        let error_code = reader.read_var_int62()?;
        let reason_phrase = reader.read_string_var_int62()?;
        let error_code = MoqtAnnounceErrorCode::try_from(error_code)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        self.events
            .push_back(MoqtControlParserEvent::OnAnnounceCancelMessage(
                MoqtAnnounceCancel {
                    track_namespace,
                    error_code,
                    reason_phrase,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_track_status_request(
        &mut self,
        reader: &mut DataReader<'_>,
    ) -> Result<usize, Error> {
        let mut full_track_name = Self::read_track_namespace(reader)?;
        let name = reader.read_string_piece_var_int62()?;
        full_track_name.add_element(name);
        self.events
            .push_back(MoqtControlParserEvent::OnTrackStatusRequestMessage(
                MoqtTrackStatusRequest { full_track_name },
            ));
        Ok(reader.bytes_read())
    }
    fn process_unannounce(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        self.events
            .push_back(MoqtControlParserEvent::OnUnannounceMessage(
                MoqtUnannounce { track_namespace },
            ));
        Ok(reader.bytes_read())
    }
    fn process_track_status(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let mut full_track_name = Self::read_track_namespace(reader)?;
        let name = reader.read_string_piece_var_int62()?;
        full_track_name.add_element(name);
        let value = reader.read_var_int62()?;
        let last_group = reader.read_var_int62()?;
        let last_object = reader.read_var_int62()?;
        let status_code = MoqtTrackStatusCode::try_from(value)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        self.events
            .push_back(MoqtControlParserEvent::OnTrackStatusMessage(
                MoqtTrackStatus {
                    full_track_name,
                    status_code,
                    last_group,
                    last_object,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_go_away(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let new_session_uri = reader.read_string_var_int62()?;
        self.events
            .push_back(MoqtControlParserEvent::OnGoAwayMessage(MoqtGoAway {
                new_session_uri,
            }));
        Ok(reader.bytes_read())
    }
    fn process_subscribe_announces(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        let parameters = self.read_subscribe_parameters(reader)?;
        self.events
            .push_back(MoqtControlParserEvent::OnSubscribeAnnouncesMessage(
                MoqtSubscribeAnnounces {
                    track_namespace,
                    parameters,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_subscribe_announces_ok(
        &mut self,
        reader: &mut DataReader<'_>,
    ) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        self.events
            .push_back(MoqtControlParserEvent::OnSubscribeAnnouncesOkMessage(
                MoqtSubscribeAnnouncesOk { track_namespace },
            ));
        Ok(reader.bytes_read())
    }
    fn process_subscribe_announces_error(
        &mut self,
        reader: &mut DataReader<'_>,
    ) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        let error_code = reader.read_var_int62()?;
        let reason_phrase = reader.read_string_var_int62()?;
        let error_code = SubscribeErrorCode::try_from(error_code)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        self.events
            .push_back(MoqtControlParserEvent::OnSubscribeAnnouncesErrorMessage(
                MoqtSubscribeAnnouncesError {
                    track_namespace,
                    error_code,
                    reason_phrase,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_unsubscribe_announces(
        &mut self,
        reader: &mut DataReader<'_>,
    ) -> Result<usize, Error> {
        let track_namespace = Self::read_track_namespace(reader)?;
        self.events
            .push_back(MoqtControlParserEvent::OnUnsubscribeAnnouncesMessage(
                MoqtUnsubscribeAnnounces { track_namespace },
            ));
        Ok(reader.bytes_read())
    }
    fn process_max_subscribe_id(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let max_subscribe_id = reader.read_var_int62()?;
        self.events
            .push_back(MoqtControlParserEvent::OnMaxSubscribeIdMessage(
                MoqtMaxSubscribeId { max_subscribe_id },
            ));
        Ok(reader.bytes_read())
    }
    fn process_fetch(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let mut full_track_name = Self::read_track_namespace(reader)?;
        let track_name = reader.read_string_piece_var_int62()?;
        let subscriber_priority = reader.read_uint8()?;
        let group_order = reader.read_uint8()?;
        let start_object_group = reader.read_var_int62()?;
        let start_object_object = reader.read_var_int62()?;
        let end_group = reader.read_var_int62()?;
        let end_object = reader.read_var_int62()?;
        let parameters = self.read_subscribe_parameters(reader)?;

        // Elements that have to be translated from the literal value.
        full_track_name.add_element(track_name);
        let group_order = parse_delivery_order(group_order)?;
        let end_object = if end_object == 0 {
            None
        } else {
            Some(end_object - 1)
        };
        if end_group < start_object_group
            || (end_group == start_object_group
                && end_object.is_some()
                && *end_object.as_ref().unwrap() < start_object_object)
        {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "End object comes before start object in FETCH",
            );
            return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
        }

        self.events
            .push_back(MoqtControlParserEvent::OnFetchMessage(MoqtFetch {
                subscribe_id,
                full_track_name,
                subscriber_priority,
                group_order,
                start_object: FullSequence::new(start_object_group, 0, start_object_object),
                end_group,
                end_object,
                parameters,
            }));
        Ok(reader.bytes_read())
    }
    fn process_fetch_cancel(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        self.events
            .push_back(MoqtControlParserEvent::OnFetchCancelMessage(
                MoqtFetchCancel { subscribe_id },
            ));
        Ok(reader.bytes_read())
    }
    fn process_fetch_ok(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let group_order = reader.read_uint8()?;
        let largest_id_group = !reader.read_var_int62()?;
        let largest_id_object = !reader.read_var_int62()?;
        let parameters = self.read_subscribe_parameters(reader)?;
        let group_order = match MoqtDeliveryOrder::try_from(group_order) {
            Ok(group_order) => group_order,
            Err(_) => {
                self.parse_error(
                    MoqtError::kProtocolViolation,
                    "Invalid group order value in FETCH_OK",
                );
                return Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation));
            }
        };
        self.events
            .push_back(MoqtControlParserEvent::OnFetchOkMessage(MoqtFetchOk {
                subscribe_id,
                group_order,
                largest_id: FullSequence::new(largest_id_group, 0, largest_id_object),
                parameters,
            }));
        Ok(reader.bytes_read())
    }
    fn process_fetch_error(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let error_code = reader.read_var_int62()?;
        let reason_phrase = reader.read_string_var_int62()?;
        let error_code = SubscribeErrorCode::try_from(error_code)
            .map_err(|_| Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))?;
        self.events
            .push_back(MoqtControlParserEvent::OnFetchErrorMessage(
                MoqtFetchError {
                    subscribe_id,
                    error_code,
                    reason_phrase,
                },
            ));
        Ok(reader.bytes_read())
    }
    fn process_object_ack(&mut self, reader: &mut DataReader<'_>) -> Result<usize, Error> {
        let subscribe_id = reader.read_var_int62()?;
        let group_id = reader.read_var_int62()?;
        let object_id = reader.read_var_int62()?;
        let raw_delta = reader.read_var_int62()?;
        let delta_from_deadline = Duration::from_micros(signed_varint_unserialized_form(raw_delta));
        self.events
            .push_back(MoqtControlParserEvent::OnObjectAckMessage(MoqtObjectAck {
                subscribe_id,
                group_id,
                object_id,
                delta_from_deadline,
            }));
        Ok(reader.bytes_read())
    }

    // If |error| is not provided, assumes kProtocolViolation.
    fn parse_error(&mut self, error: MoqtError, reason: &str) {
        // Don't send multiple parse errors.
        if !self.parsing_error {
            self.no_more_data = true;
            self.parsing_error = true;
            self.events
                .push_back(MoqtControlParserEvent::OnParsingError(
                    error,
                    reason.to_string(),
                ));
        }
    }

    // Reads an integer whose length is specified by a preceding VarInt62 and
    // returns it in |result|. Returns false if parsing fails.
    fn read_var_int_piece_var_int62(&mut self, reader: &mut DataReader<'_>) -> Result<u64, Error> {
        let length = reader.read_var_int62()?;
        let actual_length = reader.peek_var_int62_length() as u64;
        if length != actual_length {
            self.parse_error(
                MoqtError::kProtocolViolation,
                "Parameter VarInt has length field mismatch",
            );
            Err(Error::new(ErrorKind::Other, MoqtError::kProtocolViolation))
        } else {
            reader.read_var_int62()
        }
    }
    // Read a parameter and return the value as a string_view. Returns false if
    // |reader| does not have enough data.
    fn read_parameter(reader: &mut DataReader<'_>) -> Result<(u64, String), Error> {
        let t = reader.read_var_int62()?;
        let v = reader.read_string_piece_var_int62()?;
        Ok((t, v))
    }
    // Reads MoqtSubscribeParameter from one of the message types that supports
    // it. The cursor in |reader| should point to the "number of parameters"
    // field in the message. The cursor will move to the end of the parameters.
    // Returns false if it could not parse the full message, in which case the
    // cursor in |reader| should not be used.
    fn read_subscribe_parameters(
        &mut self,
        reader: &mut DataReader<'_>,
    ) -> Result<MoqtSubscribeParameters, Error> {
        let mut params = MoqtSubscribeParameters::default();

        let num_params = reader.read_var_int62()?;
        for _ in 0..num_params {
            let (t, value) = MoqtControlParser::read_parameter(reader)?;
            if let Ok(key) = MoqtTrackRequestParameter::try_from(t) {
                match key {
                    MoqtTrackRequestParameter::kAuthorizationInfo => {
                        if params.authorization_info.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "AUTHORIZATION_INFO parameter appears twice",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        params.authorization_info = Some(value);
                    }
                    MoqtTrackRequestParameter::kDeliveryTimeout => {
                        if params.delivery_timeout.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "DELIVERY_TIMEOUT parameter appears twice",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        let raw_value = self.string_view_to_var_int(value.as_str())?;
                        params.delivery_timeout = Some(Duration::from_millis(raw_value));
                    }
                    MoqtTrackRequestParameter::kMaxCacheDuration => {
                        if params.max_cache_duration.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "MAX_CACHE_DURATION parameter appears twice",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        let raw_value = self.string_view_to_var_int(value.as_str())?;
                        params.max_cache_duration = Some(Duration::from_millis(raw_value));
                    }
                    MoqtTrackRequestParameter::kOackWindowSize => {
                        if params.object_ack_window.is_some() {
                            self.parse_error(
                                MoqtError::kProtocolViolation,
                                "OACK_WINDOW_SIZE parameter appears twice in SUBSCRIBE",
                            );
                            return Err(Error::new(
                                ErrorKind::Other,
                                MoqtError::kProtocolViolation,
                            ));
                        }
                        let raw_value = self.string_view_to_var_int(value.as_str())?;
                        params.object_ack_window = Some(Duration::from_micros(raw_value));
                    }
                }
            }
        }
        Ok(params)
    }

    // Convert a string view to a varint. Throws an error and returns false if the
    // string_view is not exactly the right length.
    fn string_view_to_var_int(&mut self, sv: &str) -> Result<u64, Error> {
        let sv_len = sv.len();
        let mut buffer = sv.as_bytes();
        let mut reader = DataReader::new(&mut buffer);
        if reader.peek_var_int62_length() as usize != sv_len {
            self.parse_error(
                MoqtError::kParameterLengthMismatch,
                "Parameter length does not match varint encoding",
            );
            Err(Error::new(
                ErrorKind::Other,
                MoqtError::kParameterLengthMismatch,
            ))
        } else {
            reader.read_var_int62()
        }
    }

    // Parses a message that a track namespace but not name. The last element of
    // |full_track_name| will be set to the empty string. Returns false if it
    // could not parse the full namespace field.
    fn read_track_namespace(reader: &mut DataReader<'_>) -> Result<FullTrackName, Error> {
        let mut full_track_name = FullTrackName::new();
        let num_elements = reader.read_var_int62()?;
        for _ in 0..num_elements {
            let element = reader.read_string_var_int62()?;
            full_track_name.add_element(element);
        }
        Ok(full_track_name)
    }
}

/*
// Parses an MoQT datagram. Returns the payload bytes, or std::nullopt on error.
// The caller provides the whole datagram in `data`.  The function puts the
// object metadata in `object_metadata`.
std::optional<absl::string_view> ParseDatagram(absl::string_view data,
                                               MoqtObject& object_metadata);

// Parser for MoQT unidirectional data stream.
class QUICHE_EXPORT MoqtDataParser {
 public:
  // `stream` must outlive the parser.  The parser does not configure itself as
  // a listener for the read events of the stream; it is responsibility of the
  // caller to do so via one of the read methods below.
  explicit MoqtDataParser(quiche::ReadStream* stream,
                          MoqtDataParserVisitor* visitor)
      : stream_(*stream), visitor_(*visitor) {}

  // Reads all of the available objects on the stream.
  void ReadAllData();

  void ReadStreamType();
  void ReadTrackAlias();
  void ReadAtMostOneObject();

  // Returns the type of the unidirectional stream, if already known.
  std::optional<MoqtDataStreamType> stream_type() const { return type_; }

 private:
  friend class test::MoqtDataParserPeer;

  // Current state of the parser.
  enum NextInput {
    kStreamType,
    kTrackAlias,
    kGroupId,
    kSubgroupId,
    kPublisherPriority,
    kObjectId,
    kObjectPayloadLength,
    kStatus,
    kData,
    kPadding,
    kFailed,
  };

  // If a StopCondition callback returns true, parsing will terminate.
  using StopCondition = quiche::UnretainedCallback<bool()>;

  struct State {
    NextInput next_input;
    uint64_t payload_remaining;

    bool operator==(const State&) const = default;
  };
  State state() const { return State{next_input_, payload_length_remaining_}; }

  void ReadDataUntil(StopCondition stop_condition);

  // Reads a single varint from the underlying stream.
  std::optional<uint64_t> read_var_int62(bool& fin_read);
  // Reads a single varint from the underlying stream. Triggers a parse error if
  // a FIN has been encountered.
  std::optional<uint64_t> ReadVarInt62NoFin();
  // Reads a single uint8 from the underlying stream. Triggers a parse error if
  // a FIN has been encountered.
  std::optional<uint8_t> ReadUint8NoFin();

  // Advances the state machine of the parser to the next expected state.
  void AdvanceParserState();
  // Reads the next available item from the stream.
  void ParseNextItemFromStream();
  // Checks if we have encountered a FIN without data.  If so, processes it and
  // returns true.
  bool CheckForFinWithoutData();

  void parse_error(absl::string_view reason);

  quiche::ReadStream& stream_;
  MoqtDataParserVisitor& visitor_;

  bool no_more_data_ = false;  // Fatal error or fin. No more parsing.
  bool parsing_error_ = false;

  std::string buffered_message_;

  std::optional<MoqtDataStreamType> type_ = std::nullopt;
  NextInput next_input_ = kStreamType;
  MoqtObject metadata_;
  size_t payload_length_remaining_ = 0;
  size_t num_objects_read_ = 0;

  bool processing_ = false;  // True if currently in ProcessData(), to prevent
                             // re-entrancy.
};*/
