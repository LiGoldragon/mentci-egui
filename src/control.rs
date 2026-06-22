//! Remote-control transports for the egui shell.
//!
//! Working drive commands use `signal-mentci-client`; policy commands use
//! `meta-signal-mentci-client`.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use signal_frame::{ExchangeIdentifier, ExchangeLane, LaneSequence, RequestPayload, SessionEpoch};

use crate::error::{Error, Result};

pub use meta_signal_mentci_client::{
    ConfigurationGeneration as MetaControlConfigurationGeneration,
    MetaMentciClientFrame as MetaControlFrame, MetaMentciClientFrameBody as MetaControlFrameBody,
    MetaMentciClientReply as MetaControlOutput, MetaMentciClientRequest as MetaControlInput,
    RemoteControlMode,
};
pub use signal_mentci_client::{
    ClientFrame as GuiControlFrame, ClientFrameBody as GuiControlFrameBody,
    ClientReply as GuiControlOutput, ClientRequest as GuiControlInput,
    RejectionReason as GuiControlRejectionReason, StateSnapshot as GuiControlState,
};

#[derive(Debug)]
pub struct GuiControlRequest {
    input: GuiControlInput,
    reply: mpsc::Sender<GuiControlOutput>,
}

#[derive(Debug)]
pub struct MetaControlRequest {
    input: MetaControlInput,
    reply: mpsc::Sender<MetaControlOutput>,
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

#[derive(Debug)]
pub struct MetaControlServer {
    endpoint: GuiControlEndpoint,
    request_sender: mpsc::Sender<MetaControlRequest>,
}

#[derive(Debug, Clone)]
pub struct GuiControlClient {
    endpoint: GuiControlEndpoint,
}

#[derive(Debug, Clone)]
pub struct MetaControlClient {
    endpoint: GuiControlEndpoint,
}

pub trait GuiControlInputPolicy {
    fn requires_remote_drive(&self) -> bool;
}

impl GuiControlInputPolicy for GuiControlInput {
    fn requires_remote_drive(&self) -> bool {
        !matches!(self, Self::ObserveState(_))
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

impl MetaControlRequest {
    pub fn new(input: MetaControlInput, reply: mpsc::Sender<MetaControlOutput>) -> Self {
        Self { input, reply }
    }

    pub fn input(&self) -> &MetaControlInput {
        &self.input
    }

    pub fn respond(self, output: MetaControlOutput) -> Result<()> {
        self.reply
            .send(output)
            .map_err(|error| Error::ControlReply(error.to_string()))
    }
}

impl GuiControlEndpoint {
    pub fn from_environment() -> Self {
        Self::from_environment_variable(
            "MENTCI_EGUI_CONTROL_SOCKET",
            "mentci-egui.socket",
            "/tmp/mentci-egui.socket",
        )
    }

    pub fn meta_from_environment() -> Self {
        Self::from_environment_variable(
            "MENTCI_EGUI_META_CONTROL_SOCKET",
            "mentci-egui-meta.socket",
            "/tmp/mentci-egui-meta.socket",
        )
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: path.into(),
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    fn from_environment_variable(
        variable: &str,
        runtime_file: &str,
        fallback: &'static str,
    ) -> Self {
        match std::env::var_os(variable) {
            Some(path) => Self::new(path),
            None => match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(directory) => Self::new(PathBuf::from(directory).join(runtime_file)),
                None => Self::new(fallback),
            },
        }
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
        let listener = ControlListener::bind(&self.endpoint)?;
        let (mut stream, _address) = listener.accept()?;
        self.handle_stream(&mut stream)
    }

    fn serve_forever(self) -> Result<()> {
        let listener = ControlListener::bind(&self.endpoint)?;
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => self.handle_stream(&mut stream)?,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
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
        GuiControlExchange::write_reply(stream, exchange, output)
    }
}

impl MetaControlServer {
    pub fn new(
        endpoint: GuiControlEndpoint,
        request_sender: mpsc::Sender<MetaControlRequest>,
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
        let listener = ControlListener::bind(&self.endpoint)?;
        let (mut stream, _address) = listener.accept()?;
        self.handle_stream(&mut stream)
    }

    fn serve_forever(self) -> Result<()> {
        let listener = ControlListener::bind(&self.endpoint)?;
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => self.handle_stream(&mut stream)?,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn handle_stream(&self, stream: &mut UnixStream) -> Result<()> {
        let (exchange, input) = MetaControlExchange::read_request(stream)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.request_sender
            .send(MetaControlRequest::new(input, reply_sender))
            .map_err(|error| Error::ControlRequest(error.to_string()))?;
        let output = reply_receiver
            .recv()
            .map_err(|error| Error::ControlReply(error.to_string()))?;
        MetaControlExchange::write_reply(stream, exchange, output)
    }
}

impl GuiControlClient {
    pub fn new(endpoint: GuiControlEndpoint) -> Self {
        Self { endpoint }
    }

    pub fn submit(&self, input: GuiControlInput) -> Result<GuiControlOutput> {
        let exchange = ControlExchangeIdentifier::next();
        let mut stream = UnixStream::connect(self.endpoint.socket_path())?;
        GuiControlExchange::write_request(&mut stream, exchange, input)?;
        stream.shutdown(std::net::Shutdown::Write)?;
        GuiControlExchange::read_reply(&mut stream)
    }
}

impl MetaControlClient {
    pub fn new(endpoint: GuiControlEndpoint) -> Self {
        Self { endpoint }
    }

    pub fn submit(&self, input: MetaControlInput) -> Result<MetaControlOutput> {
        let exchange = ControlExchangeIdentifier::next();
        let mut stream = UnixStream::connect(self.endpoint.socket_path())?;
        MetaControlExchange::write_request(&mut stream, exchange, input)?;
        stream.shutdown(std::net::Shutdown::Write)?;
        MetaControlExchange::read_reply(&mut stream)
    }
}

struct ControlListener {
    listener: UnixListener,
}

struct ControlExchangeIdentifier {
    exchange: ExchangeIdentifier,
}

struct GuiControlExchange {
    frame: GuiControlFrame,
}

struct MetaControlExchange {
    frame: MetaControlFrame,
}

impl ControlListener {
    fn bind(endpoint: &GuiControlEndpoint) -> Result<UnixListener> {
        let socket_path = endpoint.socket_path();
        if socket_path.exists() {
            std::fs::remove_file(socket_path)?;
        }
        let listener = UnixListener::bind(socket_path)?;
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self { listener }.listener)
    }
}

impl ControlExchangeIdentifier {
    fn next() -> ExchangeIdentifier {
        Self {
            exchange: ExchangeIdentifier::new(
                SessionEpoch::new(1),
                ExchangeLane::Connector,
                LaneSequence::first(),
            ),
        }
        .exchange
    }
}

impl GuiControlExchange {
    fn write_request(
        stream: &mut UnixStream,
        exchange: ExchangeIdentifier,
        input: GuiControlInput,
    ) -> Result<()> {
        Self {
            frame: GuiControlFrame::new(GuiControlFrameBody::Request {
                exchange,
                request: input.into_request(),
            }),
        }
        .write(stream)
    }

    fn write_reply(
        stream: &mut UnixStream,
        exchange: ExchangeIdentifier,
        output: GuiControlOutput,
    ) -> Result<()> {
        Self {
            frame: output.into_reply_frame(exchange),
        }
        .write(stream)
    }

    fn read_request(stream: &mut UnixStream) -> Result<(ExchangeIdentifier, GuiControlInput)> {
        match Self::read(stream)?.frame.into_body() {
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
        match Self::read(stream)?.frame.into_body() {
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

    fn write(self, stream: &mut UnixStream) -> Result<()> {
        stream.write_all(&self.frame.encode_length_prefixed()?)?;
        stream.flush()?;
        Ok(())
    }

    fn read(stream: &mut UnixStream) -> Result<Self> {
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        Ok(Self {
            frame: GuiControlFrame::decode_length_prefixed(&bytes)?,
        })
    }
}

impl MetaControlExchange {
    fn write_request(
        stream: &mut UnixStream,
        exchange: ExchangeIdentifier,
        input: MetaControlInput,
    ) -> Result<()> {
        Self {
            frame: MetaControlFrame::new(MetaControlFrameBody::Request {
                exchange,
                request: input.into_request(),
            }),
        }
        .write(stream)
    }

    fn write_reply(
        stream: &mut UnixStream,
        exchange: ExchangeIdentifier,
        output: MetaControlOutput,
    ) -> Result<()> {
        Self {
            frame: output.into_reply_frame(exchange),
        }
        .write(stream)
    }

    fn read_request(stream: &mut UnixStream) -> Result<(ExchangeIdentifier, MetaControlInput)> {
        match Self::read(stream)?.frame.into_body() {
            MetaControlFrameBody::Request { exchange, request } => {
                if request.payloads().len() != 1 {
                    return Err(Error::UnexpectedControlFrame(format!(
                        "expected one meta control operation, found {}",
                        request.payloads().len()
                    )));
                }
                Ok((exchange, request.payloads().head().clone()))
            }
            other => Err(Error::UnexpectedControlFrame(format!(
                "expected meta request frame, got {other:?}"
            ))),
        }
    }

    fn read_reply(stream: &mut UnixStream) -> Result<MetaControlOutput> {
        match Self::read(stream)?.frame.into_body() {
            MetaControlFrameBody::Reply { reply, .. } => match reply {
                signal_frame::Reply::Accepted { per_operation, .. } => {
                    match per_operation.into_head() {
                        signal_frame::SubReply::Ok(output) => Ok(output),
                        other => Err(Error::UnexpectedControlFrame(format!(
                            "expected accepted meta control reply, got {other:?}"
                        ))),
                    }
                }
                signal_frame::Reply::Rejected { reason } => Err(Error::UnexpectedControlFrame(
                    format!("meta control request rejected at frame layer: {reason:?}"),
                )),
            },
            other => Err(Error::UnexpectedControlFrame(format!(
                "expected meta reply frame, got {other:?}"
            ))),
        }
    }

    fn write(self, stream: &mut UnixStream) -> Result<()> {
        stream.write_all(&self.frame.encode_length_prefixed()?)?;
        stream.flush()?;
        Ok(())
    }

    fn read(stream: &mut UnixStream) -> Result<Self> {
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        Ok(Self {
            frame: MetaControlFrame::decode_length_prefixed(&bytes)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meta_signal_mentci_client::RemoteControlSet;
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
    fn ordinary_control_input_round_trips_as_nota() {
        let input =
            GuiControlInput::ObserveState(StateObservation::new(ControllerName::new("agent")));
        let recovered: GuiControlInput = input.to_string().parse().expect("parse");

        assert_eq!(recovered, input);
    }

    #[test]
    fn meta_control_input_round_trips_as_nota() {
        let input = MetaControlInput::SetRemoteControl(RemoteControlMode::DualWrite);
        let recovered: MetaControlInput = input.to_string().parse().expect("parse");

        assert_eq!(recovered, input);
    }

    #[test]
    fn ordinary_control_socket_carries_one_typed_request_and_reply() {
        let directory = tempfile::tempdir().expect("tempdir");
        let endpoint = GuiControlEndpoint::new(directory.path().join("control.socket"));
        let (request_sender, request_receiver) = mpsc::channel();
        let server = GuiControlServer::new(endpoint.clone(), request_sender);
        let server_thread = thread::spawn(move || server.serve_once());

        ControlSocketWait::new(&endpoint).wait();

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
        assert!(matches!(reply, GuiControlOutput::State(_)));
        server_thread
            .join()
            .expect("server thread joins")
            .expect("server result");
    }

    #[test]
    fn meta_control_socket_carries_one_typed_request_and_reply() {
        let directory = tempfile::tempdir().expect("tempdir");
        let endpoint = GuiControlEndpoint::new(directory.path().join("meta-control.socket"));
        let (request_sender, request_receiver) = mpsc::channel();
        let server = MetaControlServer::new(endpoint.clone(), request_sender);
        let server_thread = thread::spawn(move || server.serve_once());

        ControlSocketWait::new(&endpoint).wait();

        let client = MetaControlClient::new(endpoint);
        let input = MetaControlInput::SetRemoteControl(RemoteControlMode::DualWrite);
        let expected = input.clone();
        let client_thread = thread::spawn(move || client.submit(input));
        let request = request_receiver.recv().expect("request received");
        assert_eq!(request.input(), &expected);
        request
            .respond(MetaControlOutput::RemoteControlSet(RemoteControlSet::new(
                RemoteControlMode::DualWrite,
            )))
            .expect("respond");

        let reply = client_thread
            .join()
            .expect("client thread joins")
            .expect("client reply");
        assert!(matches!(reply, MetaControlOutput::RemoteControlSet(_)));
        server_thread
            .join()
            .expect("server thread joins")
            .expect("server result");
    }

    struct ControlSocketWait<'endpoint> {
        endpoint: &'endpoint GuiControlEndpoint,
    }

    impl<'endpoint> ControlSocketWait<'endpoint> {
        fn new(endpoint: &'endpoint GuiControlEndpoint) -> Self {
            Self { endpoint }
        }

        fn wait(&self) {
            let started = Instant::now();
            while !self.endpoint.socket_path().exists()
                && started.elapsed() < Duration::from_secs(5)
            {
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                self.endpoint.socket_path().exists(),
                "control socket exists"
            );
        }
    }
}
