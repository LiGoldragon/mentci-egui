//! Remote-control transport for the egui shell.
//!
//! The wire vocabulary lives in `signal-mentci-client`; this module only binds
//! that generated contract to the local Unix socket and the app's event loop.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use signal_frame::{ExchangeIdentifier, ExchangeLane, LaneSequence, RequestPayload, SessionEpoch};

use crate::error::{Error, Result};

pub use signal_mentci_client::{
    ClientFrame as GuiControlFrame, ClientFrameBody as GuiControlFrameBody,
    ClientReply as GuiControlOutput, ClientRequest as GuiControlInput,
    Rejection as GuiControlRejection, RejectionReason as GuiControlRejectionReason,
    RemoteControlMode, StateSnapshot as GuiControlState,
};

#[derive(Debug)]
pub struct GuiControlRequest {
    input: GuiControlInput,
    reply: mpsc::Sender<GuiControlOutput>,
}

#[derive(Debug, Clone)]
pub struct GuiControlEndpoint {
    socket_path: PathBuf,
}

#[derive(Debug)]
pub struct GuiControlServer {
    endpoint: GuiControlEndpoint,
    request_sender: mpsc::Sender<GuiControlRequest>,
}

#[derive(Debug, Clone)]
pub struct GuiControlClient {
    endpoint: GuiControlEndpoint,
}

pub trait RemoteControlModePolicy {
    fn remote_can_drive(self) -> bool;
    fn local_can_drive(self) -> bool;
    fn label(self) -> &'static str;
}

pub trait GuiControlInputPolicy {
    fn requires_remote_drive(&self) -> bool;
}

impl RemoteControlModePolicy for RemoteControlMode {
    fn remote_can_drive(self) -> bool {
        matches!(
            self,
            Self::RemoteEnabled | Self::Presentation | Self::DualWrite
        )
    }

    fn local_can_drive(self) -> bool {
        !matches!(self, Self::Presentation)
    }

    fn label(self) -> &'static str {
        match self {
            Self::LocalOnly => "local only",
            Self::RemoteEnabled => "remote enabled",
            Self::Presentation => "presentation",
            Self::DualWrite => "dual write",
        }
    }
}

impl GuiControlInputPolicy for GuiControlInput {
    fn requires_remote_drive(&self) -> bool {
        !matches!(self, Self::ObserveState(_) | Self::SetRemoteControl(_))
    }
}

impl GuiControlRequest {
    pub fn new(input: GuiControlInput, reply: mpsc::Sender<GuiControlOutput>) -> Self {
        Self { input, reply }
    }

    pub fn input(&self) -> &GuiControlInput {
        &self.input
    }

    pub fn respond(self, output: GuiControlOutput) -> Result<()> {
        self.reply
            .send(output)
            .map_err(|error| Error::ControlReply(error.to_string()))
    }
}

impl GuiControlEndpoint {
    pub fn from_environment() -> Self {
        match std::env::var_os("MENTCI_EGUI_CONTROL_SOCKET") {
            Some(path) => Self::new(path),
            None => match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(directory) => Self::new(PathBuf::from(directory).join("mentci-egui.socket")),
                None => Self::new("/tmp/mentci-egui.socket"),
            },
        }
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: path.into(),
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl GuiControlServer {
    pub fn new(
        endpoint: GuiControlEndpoint,
        request_sender: mpsc::Sender<GuiControlRequest>,
    ) -> Self {
        Self {
            endpoint,
            request_sender,
        }
    }

    pub fn spawn(self) -> thread::JoinHandle<Result<()>> {
        thread::spawn(move || self.serve_forever())
    }

    pub fn serve_once(&self) -> Result<()> {
        let listener = self.bind_listener()?;
        let (mut stream, _address) = listener.accept()?;
        self.handle_stream(&mut stream)
    }

    fn serve_forever(self) -> Result<()> {
        let listener = self.bind_listener()?;
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => self.handle_stream(&mut stream)?,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn bind_listener(&self) -> Result<UnixListener> {
        let socket_path = self.endpoint.socket_path();
        if socket_path.exists() {
            std::fs::remove_file(socket_path)?;
        }
        let listener = UnixListener::bind(socket_path)?;
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;
        Ok(listener)
    }

    fn handle_stream(&self, stream: &mut UnixStream) -> Result<()> {
        let (exchange, input) = GuiControlExchange::read_request(stream)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.request_sender
            .send(GuiControlRequest::new(input, reply_sender))
            .map_err(|error| Error::ControlRequest(error.to_string()))?;
        let output = reply_receiver
            .recv()
            .map_err(|error| Error::ControlReply(error.to_string()))?;
        GuiControlExchange::write_reply(stream, exchange, output)?;
        Ok(())
    }
}

impl GuiControlClient {
    pub fn new(endpoint: GuiControlEndpoint) -> Self {
        Self { endpoint }
    }

    pub fn submit(&self, input: GuiControlInput) -> Result<GuiControlOutput> {
        let exchange = GuiControlExchange::next();
        let mut stream = UnixStream::connect(self.endpoint.socket_path())?;
        GuiControlExchange::write_request(&mut stream, exchange, input)?;
        stream.shutdown(std::net::Shutdown::Write)?;
        GuiControlExchange::read_reply(&mut stream)
    }
}

struct GuiControlExchange;

impl GuiControlExchange {
    fn next() -> ExchangeIdentifier {
        ExchangeIdentifier::new(
            SessionEpoch::new(1),
            ExchangeLane::Connector,
            LaneSequence::first(),
        )
    }

    fn write_request(
        stream: &mut UnixStream,
        exchange: ExchangeIdentifier,
        input: GuiControlInput,
    ) -> Result<()> {
        let frame = GuiControlFrame::new(GuiControlFrameBody::Request {
            exchange,
            request: input.into_request(),
        });
        Self::write_frame(stream, frame)
    }

    fn write_reply(
        stream: &mut UnixStream,
        exchange: ExchangeIdentifier,
        output: GuiControlOutput,
    ) -> Result<()> {
        Self::write_frame(stream, output.into_reply_frame(exchange))
    }

    fn write_frame(stream: &mut UnixStream, frame: GuiControlFrame) -> Result<()> {
        stream.write_all(&frame.encode_length_prefixed()?)?;
        stream.flush()?;
        Ok(())
    }

    fn read_request(stream: &mut UnixStream) -> Result<(ExchangeIdentifier, GuiControlInput)> {
        match Self::read_frame(stream)?.into_body() {
            GuiControlFrameBody::Request { exchange, request } => {
                if request.payloads().len() != 1 {
                    return Err(Error::UnexpectedControlFrame(format!(
                        "expected one control operation, found {}",
                        request.payloads().len()
                    )));
                }
                Ok((exchange, request.payloads().head().clone()))
            }
            other => Err(Error::UnexpectedControlFrame(format!(
                "expected request frame, got {other:?}"
            ))),
        }
    }

    fn read_reply(stream: &mut UnixStream) -> Result<GuiControlOutput> {
        match Self::read_frame(stream)?.into_body() {
            GuiControlFrameBody::Reply { reply, .. } => match reply {
                signal_frame::Reply::Accepted { per_operation, .. } => {
                    match per_operation.into_head() {
                        signal_frame::SubReply::Ok(output) => Ok(output),
                        other => Err(Error::UnexpectedControlFrame(format!(
                            "expected accepted control reply, got {other:?}"
                        ))),
                    }
                }
                signal_frame::Reply::Rejected { reason } => Err(Error::UnexpectedControlFrame(
                    format!("control request rejected at frame layer: {reason:?}"),
                )),
            },
            other => Err(Error::UnexpectedControlFrame(format!(
                "expected reply frame, got {other:?}"
            ))),
        }
    }

    fn read_frame(stream: &mut UnixStream) -> Result<GuiControlFrame> {
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        Ok(GuiControlFrame::decode_length_prefixed(&bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_mentci_client::{
        ControllerName, QuestionCount, StateObservation, TranscriptEntryCount,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn remote_control_mode_exposes_local_and_remote_drive_policy() {
        assert!(!RemoteControlMode::LocalOnly.remote_can_drive());
        assert!(RemoteControlMode::RemoteEnabled.remote_can_drive());
        assert!(RemoteControlMode::Presentation.remote_can_drive());
        assert!(RemoteControlMode::DualWrite.remote_can_drive());
        assert!(!RemoteControlMode::Presentation.local_can_drive());
        assert!(RemoteControlMode::DualWrite.local_can_drive());
    }

    #[test]
    fn control_input_round_trips_as_nota() {
        let input = GuiControlInput::SetRemoteControl(RemoteControlMode::DualWrite);
        let recovered: GuiControlInput = input.to_string().parse().expect("parse");

        assert_eq!(recovered, input);
    }

    #[test]
    fn control_socket_carries_one_typed_request_and_reply() {
        let directory = tempfile::tempdir().expect("tempdir");
        let endpoint = GuiControlEndpoint::new(directory.path().join("control.socket"));
        let (request_sender, request_receiver) = mpsc::channel();
        let server = GuiControlServer::new(endpoint.clone(), request_sender);
        let server_thread = thread::spawn(move || server.serve_once());

        let started = Instant::now();
        while !endpoint.socket_path().exists() && started.elapsed() < Duration::from_secs(5) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(endpoint.socket_path().exists(), "control socket exists");

        let client = GuiControlClient::new(endpoint);
        let input =
            GuiControlInput::ObserveState(StateObservation::new(ControllerName::new("agent")));
        let expected = input.clone();
        let client_thread = thread::spawn(move || client.submit(input));
        let request = request_receiver.recv().expect("request received");
        assert_eq!(request.input(), &expected);
        request
            .respond(GuiControlOutput::State(GuiControlState::new(
                RemoteControlMode::RemoteEnabled,
                QuestionCount::new(1),
                QuestionCount::new(2),
                None,
                false,
                TranscriptEntryCount::new(3),
            )))
            .expect("respond");

        let reply = client_thread
            .join()
            .expect("client thread joins")
            .expect("client reply");
        assert!(matches!(
            reply,
            GuiControlOutput::State(GuiControlState {
                mode: RemoteControlMode::RemoteEnabled,
                ..
            })
        ));
        server_thread
            .join()
            .expect("server thread joins")
            .expect("server result");
    }
}
