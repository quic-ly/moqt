use crate::handler::Handler;
use crate::message::announce::Announce;
use crate::message::announce_cancel::AnnounceCancel;
use crate::message::announce_error::AnnounceError;
use crate::message::announce_ok::AnnounceOk;
use crate::message::client_setup::ClientSetup;
use crate::message::go_away::GoAway;
use crate::message::message_framer::MessageFramer;
use crate::message::message_parser::{ErrorCode, MessageParser, MessageParserEvent};
use crate::message::object::ObjectHeader;
use crate::message::server_setup::ServerSetup;
use crate::message::subscribe::Subscribe;
use crate::message::subscribe_done::SubscribeDone;
use crate::message::subscribe_error::SubscribeError;
use crate::message::subscribe_ok::SubscribeOk;
use crate::message::subscribe_update::SubscribeUpdate;
use crate::message::track_status::TrackStatus;
use crate::message::track_status_request::TrackStatusRequest;
use crate::message::unannounce::UnAnnounce;
use crate::message::unsubscribe::UnSubscribe;
use crate::message::{ControlMessage, Role};
use crate::session::config::{Config, Perspective};
use crate::session::remote_track::RemoteTrackOnObjectFragment;
use crate::session::Session;
use crate::{Error, Result, StreamId};
use bytes::{BufMut, Bytes, BytesMut};
use log::{info, trace};
use retty::transport::{Transmit, TransportContext};
use std::collections::VecDeque;
use std::time::Instant;

pub enum StreamEventIn {
    ResetStreamReceived(u64),
    StopSendingReceived(u64),
    WriteSideInDataRecvState,
    MessageParserEvent(MessageParserEvent),
}

pub enum StreamEventOut {
    RemoteTrackOnObjectFragment(RemoteTrackOnObjectFragment),

    SessionEstablished(Option<Role>, Option<String>),
    SessionTerminated,
    SessionDeleted,
    IncomingAnnounce,
}

pub struct StreamMessage {
    pub message: BytesMut,
    pub fin: bool,
}

pub(super) struct StreamState {
    config: Config,
    stream_id: StreamId,
    is_control_stream: Option<bool>,
    transport: TransportContext,
    partial_object: Option<BytesMut>,
    parser: MessageParser,

    eouts: VecDeque<StreamEventOut>,
    routs: VecDeque<Transmit<StreamMessage>>,
    wouts: VecDeque<Transmit<StreamMessage>>,
}

impl StreamState {
    pub fn new(
        config: Config,
        stream_id: StreamId,
        is_control_stream: Option<bool>,
        transport: TransportContext,
    ) -> Self {
        Self {
            parser: MessageParser::new(config.use_web_transport),
            config,
            stream_id,
            is_control_stream,
            transport,
            partial_object: None,

            eouts: VecDeque::new(),
            routs: VecDeque::new(),
            wouts: VecDeque::new(),
        }
    }

    fn perspective(&self) -> Perspective {
        self.config.perspective
    }

    fn check_if_is_control_stream(&self, message_name: &str) -> Result<()> {
        if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
            if !is_control_stream {
                return Err(Error::ErrStreamError(
                    ErrorCode::ProtocolViolation,
                    format!("Received {} on non-control stream", message_name),
                ));
            }
        } else {
            return Err(Error::ErrStreamError(
                ErrorCode::ProtocolViolation,
                format!("Received {} on first message", message_name),
            ));
        }

        Ok(())
    }

    fn send_control_message(&mut self, control_message: ControlMessage) -> Result<()> {
        let mut message = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(control_message, &mut message)?;
        self.wouts.push_back(Transmit {
            now: Instant::now(),
            transport: self.transport,
            message: StreamMessage { message, fin: true },
        });
        Ok(())
    }

    fn on_object_message(
        &mut self,
        object_header: ObjectHeader,
        mut payload: Bytes,
        fin: bool,
    ) -> Result<()> {
        if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
            if is_control_stream {
                return Err(Error::ErrStreamError(
                    ErrorCode::ProtocolViolation,
                    "Received OBJECT message on control stream".to_string(),
                ));
            }
        }
        trace!(
            "{}",
            format!(
                "{:?} Received OBJECT message on stream {} for subscribe_id {} for
           track alias {} with sequence {}:{} send_order {} forwarding_preference {:?} length {}
           explicit length {} {}",
                self.config.perspective,
                self.stream_id,
                object_header.subscribe_id,
                object_header.track_alias,
                object_header.group_id,
                object_header.object_id,
                object_header.object_send_order,
                object_header.object_forwarding_preference,
                payload.len(),
                if let Some(&payload_length) = object_header.object_payload_length.as_ref() {
                    payload_length as i64
                } else {
                    -1
                },
                if fin { "F" } else { "" },
            )
        );

        if !self.config.deliver_partial_objects {
            if !fin {
                // Buffer partial object.
                if self.partial_object.is_none() {
                    self.partial_object = Some(BytesMut::new());
                }
                if let Some(partial_object) = self.partial_object.as_mut() {
                    partial_object.put(payload);
                }
                return Ok(());
            }
            if let Some(mut partial_object) = self.partial_object.take() {
                // Completes the object
                partial_object.put(payload);
                payload = partial_object.freeze();
            }
        }
        self.eouts
            .push_back(StreamEventOut::RemoteTrackOnObjectFragment(
                RemoteTrackOnObjectFragment {
                    object_header,
                    payload,
                    fin,
                },
            ));

        Ok(())
    }

    fn on_client_setup_message(&mut self, client_setup: ClientSetup) -> Result<()> {
        if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
            if !is_control_stream {
                return Err(Error::ErrStreamError(
                    ErrorCode::ProtocolViolation,
                    "Received CLIENT_SETUP on non-control stream".to_string(),
                ));
            }
        } else {
            self.is_control_stream = Some(true);
        }
        if self.perspective() == Perspective::Client {
            return Err(Error::ErrStreamError(
                ErrorCode::ProtocolViolation,
                "Received CLIENT_SETUP from server".to_string(),
            ));
        }
        if !client_setup
            .supported_versions
            .contains(&self.config.version)
        {
            return Err(Error::ErrStreamError(
                ErrorCode::ProtocolViolation,
                format!("Version mismatch: expected {:?}", self.config.version),
            ));
        }
        info!("{:?} Received the CLIENT_SETUP message", self.perspective());
        if self.config.perspective == Perspective::Server {
            let response = ServerSetup {
                supported_version: self.config.version,
                role: Some(Role::PubSub),
            };
            let mut message = BytesMut::new();
            MessageFramer::serialize_control_message(
                ControlMessage::ServerSetup(response),
                &mut message,
            )?;
            self.wouts.push_back(Transmit {
                now: Instant::now(),
                transport: self.transport,
                message: StreamMessage { message, fin: true },
            });
            info!("{:?} Sent the SERVER_SETUP message", self.perspective());
        }
        self.eouts.push_back(StreamEventOut::SessionEstablished(
            client_setup.role,
            client_setup.path,
        ));
        Ok(())
    }

    fn on_server_setup_message(&mut self, server_setup: ServerSetup) -> Result<()> {
        if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
            if !is_control_stream {
                return Err(Error::ErrStreamError(
                    ErrorCode::ProtocolViolation,
                    "Received SERVER_SETUP on non-control stream".to_string(),
                ));
            }
        } else {
            self.is_control_stream = Some(true);
        }

        if self.config.perspective == Perspective::Server {
            return Err(Error::ErrStreamError(
                ErrorCode::ProtocolViolation,
                "Received SERVER_SETUP from client".to_string(),
            ));
        }
        if server_setup.supported_version != self.config.version {
            return Err(Error::ErrStreamError(
                ErrorCode::ProtocolViolation,
                format!("Version mismatch: expected {:?}", self.config.version),
            ));
        }
        info!("{:?} Received the SERVER_SETUP message", self.perspective());
        self.eouts
            .push_back(StreamEventOut::SessionEstablished(server_setup.role, None));

        Ok(())
    }

    fn on_subscribe_message(&mut self, _subscribe: Subscribe) -> Result<()> {
        self.check_if_is_control_stream("SUBSCRIBE")?;
        /*
                if (session_->peer_role_ == MoqtRole::kPublisher) {
                    QUIC_DLOG(INFO) << ENDPOINT << "Publisher peer sent SUBSCRIBE";
                    session_->Error(MoqtError::kProtocolViolation,
                                    "Received SUBSCRIBE from publisher");
                    return;
                }
                QUIC_DLOG(INFO) << ENDPOINT << "Received a SUBSCRIBE for "
                    << message.track_namespace << ":" << message.track_name;
                auto it = session_->local_tracks_.find(FullTrackName(
                    std::string(message.track_namespace), std::string(message.track_name)));
                if (it == session_->local_tracks_.end()) {
                    QUIC_DLOG(INFO) << ENDPOINT << "Rejected because "
                        << message.track_namespace << ":" << message.track_name
                        << " does not exist";
                    SendSubscribeError(message, SubscribeErrorCode::kInternalError,
                                       "Track does not exist", message.track_alias);
                    return;
                }
                LocalTrack& track = it->second;
                if (it->second.canceled()) {
                    // Note that if the track has already been deleted, there will not be a
                    // protocol violation, which the spec says there SHOULD be. It's not worth
                    // keeping state on deleted tracks.
                    session_->Error(MoqtError::kProtocolViolation,
                                    "Received SUBSCRIBE for canceled track");
                    return;
                }
                if ((track.track_alias().has_value() &&
                    message.track_alias != *track.track_alias()) ||
                    session_->used_track_aliases_.contains(message.track_alias)) {
                    // Propose a different track_alias.
                    SendSubscribeError(message, SubscribeErrorCode::kRetryTrackAlias,
                                       "Track alias already exists",
                                       session_->next_local_track_alias_++);
                    return;
                } else {  // Use client-provided alias.
                    track.set_track_alias(message.track_alias);
                    if (message.track_alias >= session_->next_local_track_alias_) {
                        session_->next_local_track_alias_ = message.track_alias + 1;
                    }
                    session_->used_track_aliases_.insert(message.track_alias);
                }
                FullSequence start;
                if (message.start_group.has_value()) {
                    // The filter is AbsoluteStart or AbsoluteRange.
                    QUIC_BUG_IF(quic_bug_invalid_subscribe, !message.start_object.has_value())
                        << "Start group without start object";
                    start = FullSequence(*message.start_group, *message.start_object);
                } else {
                    // The filter is LatestObject or LatestGroup.
                    start = track.next_sequence();
                    if (message.start_object.has_value()) {
                        // The filter is LatestGroup.
                        QUIC_BUG_IF(quic_bug_invalid_subscribe, *message.start_object != 0)
                            << "LatestGroup does not start with zero";
                        start.object = 0;
                    } else {
                        --start.object;
                    }
                }
                LocalTrack::Visitor::PublishPastObjectsCallback publish_past_objects;
                std::optional<SubscribeWindow> past_window;
                if (start < track.next_sequence() && track.visitor() != nullptr) {
                    // Pull a copy of objects that have already been published.
                    FullSequence end_of_past_subscription{
                        message.end_group.has_value() ? *message.end_group : UINT64_MAX,
                        message.end_object.has_value() ? *message.end_object : UINT64_MAX};
                    end_of_past_subscription =
                        std::min(end_of_past_subscription, track.next_sequence());
                    past_window.emplace(message.subscribe_id, track.forwarding_preference(),
                                        track.next_sequence(), start, end_of_past_subscription);
                    absl::StatusOr<LocalTrack::Visitor::PublishPastObjectsCallback>
                        past_objects_available =
                        track.visitor()->OnSubscribeForPast(*past_window);
                    if (!past_objects_available.ok()) {
                        SendSubscribeError(message, SubscribeErrorCode::kInternalError,
                                           past_objects_available.status().message(),
                                           message.track_alias);
                        return;
                    }
                    publish_past_objects = *std::move(past_objects_available);
                }
                MoqtSubscribeOk subscribe_ok;
                subscribe_ok.subscribe_id = message.subscribe_id;
                SendOrBufferMessage(session_->framer_.serialize_subscribe_ok(subscribe_ok));
                QUIC_DLOG(INFO) << ENDPOINT << "Created subscription for "
                    << message.track_namespace << ":" << message.track_name;
                if (!message.end_group.has_value()) {
                    track.AddWindow(message.subscribe_id, start.group, start.object);
                } else if (message.end_object.has_value()) {
                    track.AddWindow(message.subscribe_id, start.group, start.object,
                                    *message.end_group, *message.end_object);
                } else {
                    track.AddWindow(message.subscribe_id, start.group, start.object,
                                    *message.end_group);
                }
                session_->local_track_by_subscribe_id_.emplace(message.subscribe_id,
                                                               track.full_track_name());
                if (publish_past_objects) {
                    QUICHE_DCHECK(past_window.has_value());
                    std::move(publish_past_objects)();
                }
        */
        Ok(())
    }

    fn on_subscribe_ok_message(&mut self, _subscribe_ok: SubscribeOk) -> Result<()> {
        self.check_if_is_control_stream("SUBSCRIBE_OK")?;

        Ok(())
    }

    fn on_subscribe_done_message(&mut self, _subscribe_done: SubscribeDone) -> Result<()> {
        self.check_if_is_control_stream("SUBSCRIBE_DONE")?;

        Ok(())
    }

    fn on_subscribe_error_message(&mut self, _subscribe_error: SubscribeError) -> Result<()> {
        self.check_if_is_control_stream("SUBSCRIBE_ERROR")?;

        Ok(())
    }

    fn on_subscribe_update_message(&mut self, _subscribe_update: SubscribeUpdate) -> Result<()> {
        self.check_if_is_control_stream("SUBSCRIBE_UPDATE")?;

        Ok(())
    }

    fn on_unsubscribe_message(&mut self, _unsubscribe: UnSubscribe) -> Result<()> {
        self.check_if_is_control_stream("UNSUBSCRIBE")?;

        Ok(())
    }

    fn on_announce_message(&mut self, _announce: Announce) -> Result<()> {
        self.check_if_is_control_stream("ANNOUNCE")?;

        Ok(())
    }

    fn on_announce_ok_message(&mut self, _announcee_ok: AnnounceOk) -> Result<()> {
        self.check_if_is_control_stream("ANNOUNCE_OK")?;

        Ok(())
    }

    fn on_announce_error_message(&mut self, _announce_error: AnnounceError) -> Result<()> {
        self.check_if_is_control_stream("ANNOUNCE_ERROR")?;

        Ok(())
    }

    fn on_announce_cancel_message(&mut self, _announce_cancel: AnnounceCancel) -> Result<()> {
        self.check_if_is_control_stream("ANNOUNCE_CANCEL")?;

        Ok(())
    }

    fn on_unannounce_message(&mut self, _unannounce: UnAnnounce) -> Result<()> {
        self.check_if_is_control_stream("UNANNOUNCE")?;

        Ok(())
    }

    fn on_track_status_request_message(
        &mut self,
        _track_status_request: TrackStatusRequest,
    ) -> Result<()> {
        self.check_if_is_control_stream("TRACK_STATUS_REQUEST")?;

        Ok(())
    }

    fn on_track_status_message(&mut self, _track_status: TrackStatus) -> Result<()> {
        self.check_if_is_control_stream("TRACK_STATUS")?;

        Ok(())
    }

    fn on_go_away_message(&mut self, _go_away: GoAway) -> Result<()> {
        self.check_if_is_control_stream("GO_AWAY")?;

        Ok(())
    }
}

pub struct Stream<'a> {
    pub(crate) stream_id: StreamId,
    pub(crate) session: &'a mut Session,
}

impl Stream<'_> {
    fn stream_state(&mut self) -> Result<&mut StreamState> {
        self.session
            .streams
            .get_mut(&self.stream_id)
            .ok_or(Error::ErrStreamClosed)
    }

    pub(crate) fn send_control_message(&mut self, control_message: ControlMessage) -> Result<()> {
        let stream_state = self.stream_state()?;
        stream_state.send_control_message(control_message)
    }
}

impl Handler for Stream<'_> {
    type Ein = StreamEventIn;
    type Eout = StreamEventOut;
    type Rin = StreamMessage;
    type Rout = StreamMessage;
    type Win = StreamMessage;
    type Wout = StreamMessage;

    fn transport_active(&mut self) -> Result<()> {
        Ok(())
    }

    fn transport_inactive(&mut self) -> Result<()> {
        Ok(())
    }

    fn handle_read(&mut self, msg: Transmit<Self::Rin>) -> Result<()> {
        let stream_state = self.stream_state()?;
        stream_state
            .parser
            .process_data(&mut &msg.message.message[..], msg.message.fin);
        Ok(())
    }

    fn poll_read(&mut self) -> Option<Transmit<Self::Rout>> {
        let stream_state = self.stream_state().ok()?;
        stream_state.routs.pop_front()
    }

    fn handle_write(&mut self, msg: Transmit<Self::Win>) -> Result<()> {
        let stream_state = self.stream_state()?;
        stream_state.wouts.push_back(msg);
        Ok(())
    }

    fn poll_write(&mut self) -> Option<Transmit<Self::Wout>> {
        let stream_state = self.stream_state().ok()?;
        stream_state.wouts.pop_front()
    }

    /// Handles event
    fn handle_event(&mut self, evt: Self::Ein) -> Result<()> {
        let stream_state = self.stream_state()?;
        match evt {
            StreamEventIn::ResetStreamReceived(error_code) => {
                if let Some(&is_control_stream) = stream_state.is_control_stream.as_ref() {
                    if is_control_stream {
                        return Err(Error::ErrStreamError(
                            ErrorCode::ProtocolViolation,
                            format!("Control stream reset with error code {}", error_code),
                        ));
                    }
                }
                Ok(())
            }
            StreamEventIn::StopSendingReceived(error_code) => {
                if let Some(&is_control_stream) = stream_state.is_control_stream.as_ref() {
                    if is_control_stream {
                        return Err(Error::ErrStreamError(
                            ErrorCode::ProtocolViolation,
                            format!("Control stream reset with error code {}", error_code),
                        ));
                    }
                }
                Ok(())
            }
            StreamEventIn::WriteSideInDataRecvState => Ok(()),
            StreamEventIn::MessageParserEvent(message_parser_event) => match message_parser_event {
                MessageParserEvent::ParsingError(error_code, reason) => Err(Error::ErrStreamError(
                    error_code,
                    format!("Parse error: {}", reason),
                )),
                MessageParserEvent::ObjectMessage(object_header, payload, fin) => {
                    stream_state.on_object_message(object_header, payload, fin)
                }
                MessageParserEvent::ControlMessage(control_message) => match control_message {
                    ControlMessage::SubscribeUpdate(subscribe_update) => {
                        stream_state.on_subscribe_update_message(subscribe_update)
                    }
                    ControlMessage::Subscribe(subscribe) => {
                        stream_state.on_subscribe_message(subscribe)
                    }
                    ControlMessage::SubscribeOk(subscribe_ok) => {
                        stream_state.on_subscribe_ok_message(subscribe_ok)
                    }
                    ControlMessage::SubscribeError(subscribe_error) => {
                        stream_state.on_subscribe_error_message(subscribe_error)
                    }
                    ControlMessage::Announce(announce) => {
                        stream_state.on_announce_message(announce)
                    }
                    ControlMessage::AnnounceOk(announce_ok) => {
                        stream_state.on_announce_ok_message(announce_ok)
                    }
                    ControlMessage::AnnounceError(announce_error) => {
                        stream_state.on_announce_error_message(announce_error)
                    }
                    ControlMessage::UnAnnounce(unannounce) => {
                        stream_state.on_unannounce_message(unannounce)
                    }
                    ControlMessage::UnSubscribe(unsubscribe) => {
                        stream_state.on_unsubscribe_message(unsubscribe)
                    }
                    ControlMessage::SubscribeDone(subscribe_done) => {
                        stream_state.on_subscribe_done_message(subscribe_done)
                    }
                    ControlMessage::AnnounceCancel(announce_cancel) => {
                        stream_state.on_announce_cancel_message(announce_cancel)
                    }
                    ControlMessage::TrackStatusRequest(track_status_request) => {
                        stream_state.on_track_status_request_message(track_status_request)
                    }
                    ControlMessage::TrackStatus(track_status) => {
                        stream_state.on_track_status_message(track_status)
                    }
                    ControlMessage::GoAway(go_away) => stream_state.on_go_away_message(go_away),
                    ControlMessage::ClientSetup(client_setup) => {
                        stream_state.on_client_setup_message(client_setup)
                    }
                    ControlMessage::ServerSetup(server_setup) => {
                        stream_state.on_server_setup_message(server_setup)
                    }
                },
            },
        }
    }

    /// Polls event
    fn poll_event(&mut self) -> Option<Self::Eout> {
        let stream_state = self.stream_state().ok()?;
        stream_state.eouts.pop_front()
    }

    /// Handles timeout
    fn handle_timeout(&mut self, _now: Instant) -> Result<()> {
        Ok(())
    }

    /// Polls timeout
    fn poll_timeout(&mut self) -> Option<Instant> {
        None
    }
}
