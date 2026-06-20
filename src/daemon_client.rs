//! Real Mentci daemon client used by the egui shell.
//!
//! The GUI talks to `mentci-daemon` through `signal-mentci` frames and
//! renders request/reply payloads as NOTA until purpose-built views exist.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use nota_next::NotaEncode;
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, Reply, RequestPayload, SessionEpoch, SubReply,
};
use signal_mentci::{
    InterfaceInterest, InterfaceStateObservation, MentciFrame, MentciFrameBody, MentciReply,
    MentciRequest, SubscriberName,
};

#[derive(Clone, Debug)]
pub struct DaemonClient {
    ordinary_socket: PathBuf,
    meta_socket: PathBuf,
    maximum_frame_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct DaemonTranscriptEntry {
    pub socket_kind: SocketKind,
    pub operation: String,
    pub socket_path: PathBuf,
    pub request_nota: String,
    pub reply_nota: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketKind {
    Mentci,
    MetaMentci,
}

impl DaemonClient {
    pub fn from_environment() -> Self {
        Self::new(Self::ordinary_socket_from_environment())
            .with_meta_socket(Self::meta_socket_from_environment())
    }

    pub fn new(ordinary_socket: impl Into<PathBuf>) -> Self {
        Self {
            ordinary_socket: ordinary_socket.into(),
            meta_socket: PathBuf::from("/tmp/mentci-meta.socket"),
            maximum_frame_bytes: 16 * 1024 * 1024,
        }
    }

    pub fn with_meta_socket(mut self, meta_socket: impl Into<PathBuf>) -> Self {
        self.meta_socket = meta_socket.into();
        self
    }

    pub fn ordinary_socket(&self) -> &PathBuf {
        &self.ordinary_socket
    }

    pub fn meta_socket(&self) -> &PathBuf {
        &self.meta_socket
    }

    pub fn observe_interface_state(&self) -> crate::error::Result<DaemonTranscriptEntry> {
        let request = MentciRequest::ObserveInterfaceState(InterfaceStateObservation {
            subscriber: SubscriberName::new("mentci-egui"),
            interest: InterfaceInterest::FullInterfaceState,
        });
        let request_nota = request.to_nota();
        let frame = MentciFrame::new(MentciFrameBody::Request {
            exchange: self.exchange(),
            request: request.clone().into_request(),
        });
        let reply = self.send_ordinary_frame(&frame)?;
        Ok(DaemonTranscriptEntry {
            socket_kind: SocketKind::Mentci,
            operation: "ObserveInterfaceState".to_string(),
            socket_path: self.ordinary_socket.clone(),
            request_nota,
            reply_nota: self.reply_nota(reply)?,
        })
    }

    /// Observe the daemon's interface state and return the TYPED reply so the
    /// caller can fold it into mentci-lib's `ObservationModel`. The typed
    /// `MentciReply` is the shared-model input; the NOTA string is only the
    /// fallback rendering.
    pub fn observe_interface_state_typed(
        &self,
        interest: InterfaceInterest,
    ) -> crate::error::Result<MentciReply> {
        let request = MentciRequest::ObserveInterfaceState(InterfaceStateObservation {
            subscriber: SubscriberName::new("mentci-egui"),
            interest,
        });
        let frame = MentciFrame::new(MentciFrameBody::Request {
            exchange: self.exchange(),
            request: request.into_request(),
        });
        let reply = self.send_ordinary_frame(&frame)?;
        self.reply_output(reply)
    }

    pub fn meta_mode_placeholder(&self) -> DaemonTranscriptEntry {
        DaemonTranscriptEntry {
            socket_kind: SocketKind::MetaMentci,
            operation: "MetaMode".to_string(),
            socket_path: self.meta_socket.clone(),
            request_nota: "(meta mode selected)".to_string(),
            reply_nota: "mentci-daemon does not expose a live meta socket yet; startup configuration is still supplied as one binary meta-signal Configure file.".to_string(),
        }
    }

    fn send_ordinary_frame(&self, frame: &MentciFrame) -> crate::error::Result<MentciFrame> {
        let mut stream = UnixStream::connect(&self.ordinary_socket)?;
        self.write_mentci_frame(&mut stream, frame)?;
        self.read_mentci_frame(&mut stream)
    }

    fn read_mentci_frame(&self, stream: &mut UnixStream) -> crate::error::Result<MentciFrame> {
        let mut length_bytes = [0_u8; 4];
        stream.read_exact(&mut length_bytes)?;
        let length = u32::from_be_bytes(length_bytes) as usize;
        if length > self.maximum_frame_bytes {
            return Err(crate::error::Error::FrameTooLarge {
                maximum: self.maximum_frame_bytes,
                found: length,
            });
        }
        let mut bytes = Vec::with_capacity(4 + length);
        bytes.extend_from_slice(&length_bytes);
        let start = bytes.len();
        bytes.resize(start + length, 0);
        stream.read_exact(&mut bytes[start..])?;
        Ok(MentciFrame::decode_length_prefixed(&bytes)?)
    }

    fn write_mentci_frame(
        &self,
        stream: &mut UnixStream,
        frame: &MentciFrame,
    ) -> crate::error::Result<()> {
        stream.write_all(&frame.encode_length_prefixed()?)?;
        stream.flush()?;
        Ok(())
    }

    fn reply_nota(&self, frame: MentciFrame) -> crate::error::Result<String> {
        match frame.into_body() {
            MentciFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(reply) => Ok(reply.to_nota()),
                    other => Ok(format!("{other:?}")),
                },
                Reply::Rejected { reason } => Ok(format!("{reason:?}")),
            },
            MentciFrameBody::SubscriptionEvent { event, .. } => Ok(event.to_nota()),
            other => Err(crate::error::Error::UnexpectedDaemonFrame(format!(
                "{other:?}"
            ))),
        }
    }

    /// Extract the typed `MentciReply` (the contract `Output`) from a reply
    /// frame's accepted head — the shape mentci-lib folds into its model.
    fn reply_output(&self, frame: MentciFrame) -> crate::error::Result<MentciReply> {
        match frame.into_body() {
            MentciFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(output) => Ok(output),
                    other => Err(crate::error::Error::UnexpectedDaemonFrame(format!(
                        "{other:?}"
                    ))),
                },
                Reply::Rejected { reason } => Err(crate::error::Error::UnexpectedDaemonFrame(
                    format!("rejected: {reason:?}"),
                )),
            },
            other => Err(crate::error::Error::UnexpectedDaemonFrame(format!(
                "{other:?}"
            ))),
        }
    }

    fn exchange(&self) -> ExchangeIdentifier {
        ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::first(),
        )
    }

    fn ordinary_socket_from_environment() -> PathBuf {
        match std::env::var_os("MENTCI_SOCKET") {
            Some(path) => PathBuf::from(path),
            None => match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(directory) => PathBuf::from(directory).join("mentci.socket"),
                None => PathBuf::from("/tmp/mentci.socket"),
            },
        }
    }

    fn meta_socket_from_environment() -> PathBuf {
        match std::env::var_os("MENTCI_META_SOCKET") {
            Some(path) => PathBuf::from(path),
            None => match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(directory) => PathBuf::from(directory).join("mentci-meta.socket"),
                None => PathBuf::from("/tmp/mentci-meta.socket"),
            },
        }
    }
}

impl SocketKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mentci => "Mentci",
            Self::MetaMentci => "MetaMentci",
        }
    }
}
