use crate::connection::Connection;
use crate::handler::Handler;
use crate::message::announce_error::AnnounceErrorReason;
use crate::message::client_setup::ClientSetup;
use crate::message::object::ObjectForwardingPreference;
use crate::message::subscribe::Subscribe;
use crate::message::{ControlMessage, FullTrackName, Role};
use crate::session::config::{Config, Perspective};
use crate::session::local_track::LocalTrack;
use crate::session::remote_track::RemoteTrack;
use crate::session::stream::{Stream, StreamState};
use crate::StreamId;
use crate::{Error, Result};
use log::info;
use retty::transport::Transmit;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

mod config;
mod local_track;
mod remote_track;
mod stream;
mod subscribe_window;

/// match between client and server perspective, since there may be a proxy
/// between them.
pub type StreamId = u32;

// If |error_message| is none, the ANNOUNCE was successful.
pub type OutgoingAnnounceCallback = fn(track_namespace: String, error: Option<AnnounceErrorReason>);

/// Indexed by subscribe_id.
pub struct ActiveSubscribe {
    message: Subscribe,
    // The forwarding preference of the first received object, which all
    // subsequent objects must match.
    forwarding_preference: Option<ObjectForwardingPreference>,
    // If true, an object has arrived for the subscription before SUBSCRIBE_OK
    // arrived.
    received_object: bool,
}

pub struct Session {
    config: Config,
    conn: Connection,
    control_stream_id: Option<StreamId>,
    streams: HashMap<StreamId, StreamState>,

    // All the tracks the session is subscribed to, indexed by track_alias.
    // Multiple subscribes to the same track are recorded in a single
    // subscription.
    remote_tracks: HashMap<u64, RemoteTrack>,
    // Look up aliases for remote tracks by name
    remote_track_aliases: HashMap<FullTrackName, u64>,
    next_remote_track_alias: u64,

    // All the tracks the peer can subscribe to.
    local_tracks: HashMap<FullTrackName, LocalTrack>,
    local_track_by_subscribe_id: HashMap<u64, FullTrackName>,
    // This is only used to check for track_alias collisions.
    used_track_aliases: HashSet<u64>,
    next_local_track_alias: u64,

    // Outgoing SUBSCRIBEs that have not received SUBSCRIBE_OK or SUBSCRIBE_ERROR.
    active_subscribes: HashMap<u64, ActiveSubscribe>,
    next_subscribe_id: u64,

    // Indexed by track namespace.
    pending_outgoing_announces: HashMap<String, OutgoingAnnounceCallback>,

    // The role the peer advertised in its SETUP message. Initialize it to avoid
    // an uninitialized value if no SETUP arrives or it arrives with no Role
    // parameter, and other checks have changed/been disabled.
    peer_role: Role,
}

impl Session {
    pub fn new(config: Config, conn: Connection) -> Self {
        Self {
            config,
            conn,
            control_stream_id: None,
            streams: HashMap::new(),
            remote_tracks: Default::default(),
            remote_track_aliases: Default::default(),
            next_remote_track_alias: 0,
            local_tracks: Default::default(),
            local_track_by_subscribe_id: Default::default(),
            used_track_aliases: Default::default(),
            next_local_track_alias: 0,
            active_subscribes: Default::default(),
            next_subscribe_id: 0,
            pending_outgoing_announces: Default::default(),
            peer_role: Default::default(),
        }
    }

    fn stream(&mut self, stream_id: StreamId) -> Result<Stream<'_>> {
        if !self.streams.contains_key(&stream_id) {
            Err(Error::ErrStreamNotExisted)
        } else {
            Ok(Stream {
                stream_id,
                session: self,
            })
        }
    }

    fn get_control_stream(&mut self) -> Result<Stream<'_>> {
        if let Some(control_stream_id) = self.control_stream_id.as_ref() {
            self.stream(*control_stream_id)
        } else {
            Err(Error::ErrStreamNotExisted)
        }
    }

    fn send_control_message(&mut self, control_message: ControlMessage) -> Result<()> {
        let mut control_stream = self.get_control_stream()?;
        control_stream.send_control_message(control_message)
    }
}

impl Handler for Session {
    type Ein = ();
    type Eout = ();
    type Rin = ();
    type Rout = ();
    type Win = ();
    type Wout = ();

    fn transport_active(&mut self) -> Result<()> {
        info!("{:?} Underlying session ready", self.config.perspective);
        if self.config.perspective == Perspective::Server {
            return Ok(());
        }

        let control_stream_id = self.conn.open_bi_stream()?;
        let control_stream = StreamState::new(
            self.config.clone(),
            control_stream_id,
            Some(true),
            self.conn.transport(),
        );
        self.streams.insert(control_stream_id, control_stream);
        self.control_stream_id = Some(control_stream_id);
        let mut client_setup = ClientSetup {
            supported_versions: vec![self.config.version],
            role: Some(Role::PubSub),
            path: None,
            uses_web_transport: self.config.use_web_transport,
        };
        if !self.config.use_web_transport {
            client_setup.path = Some(self.config.path.clone());
        }

        info!("{:?} Send the SETUP message", self.config.perspective);
        self.send_control_message(ControlMessage::ClientSetup(client_setup))
    }

    fn transport_inactive(&mut self) -> Result<()> {
        todo!()
    }

    fn handle_read(&mut self, _msg: Transmit<Self::Rin>) -> Result<()> {
        todo!()
    }

    fn poll_read(&mut self) -> Option<Transmit<Self::Rout>> {
        todo!()
    }

    fn handle_write(&mut self, _msg: Transmit<Self::Win>) -> Result<()> {
        todo!()
    }

    fn poll_write(&mut self) -> Option<Transmit<Self::Wout>> {
        todo!()
    }

    fn handle_event(&mut self, _evt: Self::Ein) -> Result<()> {
        todo!()
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        todo!()
    }

    fn handle_timeout(&mut self, _now: Instant) -> Result<()> {
        todo!()
    }

    fn poll_timeout(&mut self) -> Option<Instant> {
        todo!()
    }
}
