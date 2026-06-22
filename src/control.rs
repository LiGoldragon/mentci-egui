//! Remote-control surface for the egui shell itself.
//!
//! This is the first local implementation of Spirit record `6kw3`: the GUI has
//! its own socket-addressable control state, while shared component data still
//! flows through `mentci-daemon`.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use nota_next::{NotaDecode, NotaEncode, NotaSource};
use signal_mentci::{ApprovalDecision, QuestionIdentifier};

use crate::error::{Error, Result};

#[derive(NotaEncode, NotaDecode, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RemoteControlMode {
    #[default]
    LocalOnly,
    RemoteEnabled,
    RemotePresentation,
    DualWrite,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub enum GuiControlInput {
    ObserveState,
    SetRemoteControl(RemoteControlMode),
    TriggerObserve,
    SelectQuestion(QuestionIdentifier),
    AnswerSelected(ApprovalDecision),
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub enum GuiControlOutput {
    State(GuiControlState),
    Accepted(GuiControlState),
    Rejected(GuiControlRejection),
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct GuiControlState {
    pub mode: RemoteControlMode,
    pub pending_questions: u64,
    pub answered_questions: u64,
    pub selected_question: Option<QuestionIdentifier>,
    pub ordinary_request_in_flight: bool,
    pub transcript_entries: u64,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct GuiControlRejection {
    pub reason: GuiControlRejectionReason,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiControlRejectionReason {
    RemoteControlDisabled,
    NoSelectedQuestion,
}

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

impl RemoteControlMode {
    pub fn remote_can_drive(self) -> bool {
        matches!(
            self,
            Self::RemoteEnabled | Self::RemotePresentation | Self::DualWrite
        )
    }

    pub fn local_can_drive(self) -> bool {
        !matches!(self, Self::RemotePresentation)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LocalOnly => "local only",
            Self::RemoteEnabled => "remote enabled",
            Self::RemotePresentation => "remote presentation",
            Self::DualWrite => "dual write",
        }
    }
}

impl GuiControlInput {
    pub fn from_nota(text: &str) -> Result<Self> {
        NotaSource::new(text)
            .parse()
            .map_err(|error| Error::ControlParse(error.to_string()))
    }

    pub fn requires_remote_drive(&self) -> bool {
        !matches!(self, Self::ObserveState | Self::SetRemoteControl(_))
    }
}

impl GuiControlOutput {
    pub fn to_nota_text(&self) -> String {
        self.to_nota()
    }
}

impl GuiControlRejection {
    pub fn new(reason: GuiControlRejectionReason) -> Self {
        Self { reason }
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
        let mut request_text = String::new();
        stream.read_to_string(&mut request_text)?;
        let input = GuiControlInput::from_nota(request_text.trim())?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.request_sender
            .send(GuiControlRequest::new(input, reply_sender))
            .map_err(|error| Error::ControlRequest(error.to_string()))?;
        let output = reply_receiver
            .recv()
            .map_err(|error| Error::ControlReply(error.to_string()))?;
        stream.write_all(output.to_nota_text().as_bytes())?;
        stream.flush()?;
        Ok(())
    }
}

impl GuiControlClient {
    pub fn new(endpoint: GuiControlEndpoint) -> Self {
        Self { endpoint }
    }

    pub fn submit(&self, input: GuiControlInput) -> Result<String> {
        let mut stream = UnixStream::connect(self.endpoint.socket_path())?;
        stream.write_all(input.to_nota().as_bytes())?;
        stream.shutdown(std::net::Shutdown::Write)?;
        let mut reply = String::new();
        stream.read_to_string(&mut reply)?;
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn remote_control_mode_exposes_local_and_remote_drive_policy() {
        assert!(!RemoteControlMode::LocalOnly.remote_can_drive());
        assert!(RemoteControlMode::RemoteEnabled.remote_can_drive());
        assert!(RemoteControlMode::RemotePresentation.remote_can_drive());
        assert!(RemoteControlMode::DualWrite.remote_can_drive());
        assert!(!RemoteControlMode::RemotePresentation.local_can_drive());
        assert!(RemoteControlMode::DualWrite.local_can_drive());
    }

    #[test]
    fn control_input_round_trips_as_nota() {
        let input = GuiControlInput::SetRemoteControl(RemoteControlMode::DualWrite);
        let recovered = GuiControlInput::from_nota(input.to_nota().as_str()).expect("parse");

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
        let client_thread = thread::spawn(move || client.submit(GuiControlInput::ObserveState));
        let request = request_receiver.recv().expect("request received");
        assert_eq!(request.input(), &GuiControlInput::ObserveState);
        request
            .respond(GuiControlOutput::State(GuiControlState {
                mode: RemoteControlMode::RemoteEnabled,
                pending_questions: 1,
                answered_questions: 2,
                selected_question: None,
                ordinary_request_in_flight: false,
                transcript_entries: 3,
            }))
            .expect("respond");

        let reply = client_thread
            .join()
            .expect("client thread joins")
            .expect("client reply");
        assert!(reply.contains("RemoteEnabled"));
        server_thread
            .join()
            .expect("server thread joins")
            .expect("server result");
    }
}
