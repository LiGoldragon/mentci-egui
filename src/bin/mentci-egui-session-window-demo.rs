//! mentci-egui-session-window-demo — the psyche's launch command for the live
//! Mentci session window.
//!
//! It runs one or more REAL headless `claude` turns in a throwaway sandbox
//! (refusing the primary workspace), observes each turn through the harness
//! JSONL observer, folds it into a `ClaudeSessionObservation` via the harness's
//! own `ObservedClaudeTurn` projection (Mentci adds no provider logic), and
//! PUSHES it onto the harness `HarnessTranscriptStream`. The egui window
//! subscribes once and renders each turn as it is pushed — the psyche watches
//! turns appear live, with no polling on the consumer path.
//!
//! Usage:
//!   mentci-egui-session-window-demo <harness-name> <prompt> [<prompt> …] [--model <alias>]
//!
//! The first prompt opens a FRESH session; each later prompt RESUMES it, so the
//! window shows a fresh turn followed by resumed turns with the right launch
//! provenance. Building on the mentci-headless-session-demo plumbing, this needs
//! a display, the `claude` CLI, and network — it is a manual witness, not a Nix
//! check.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use harness::{
    ClaudeArtifactObserver, ObservedClaudeTurn, PublishStreamEvent, TranscriptDeltaPublisher,
    TranscriptSubscriptionManager,
};
use kameo::actor::Spawn;
use mentci_egui::render::{SessionStreamSubscription, SessionViewApp, SessionWindow};
use serde_json::Value;
use signal_harness::{ClaudeSessionLifecycle, HarnessName, StreamedEventCount, TurnLaunch};
use thiserror::Error;

/// The one workspace a headless run must never be pointed at.
const PRIMARY_WORKSPACE: &str = "/home/li/primary";

fn main() {
    if let Err(error) = Invocation::from_process_arguments().and_then(Invocation::run) {
        eprintln!("MentciEguiSessionWindowDemoBlocked {error}");
        std::process::exit(2);
    }
}

/// The typed boundary error for this demo binary.
#[derive(Debug, Error)]
enum DemoError {
    #[error(
        "usage: mentci-egui-session-window-demo <harness-name> <prompt> [<prompt> …] [--model <alias>]"
    )]
    Usage,
    #[error("refused: sandbox {0:?} is inside the primary workspace {PRIMARY_WORKSPACE}")]
    PrimaryWorkspaceRefused(PathBuf),
    #[error("could not mint a session identifier: {0}")]
    SessionIdentifier(std::io::Error),
    #[error("filesystem error at {path:?}: {source}")]
    Filesystem {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to build the tokio runtime: {0}")]
    Runtime(std::io::Error),
    #[error("failed to spawn `claude`: {0}")]
    Spawn(std::io::Error),
    #[error("`claude` produced no result event; combined output was:\n{0}")]
    MissingResultEvent(String),
    #[error("observing the claude transcript failed: {0}")]
    Observation(harness::Error),
    #[error("opening the window's subscription failed: {0}")]
    Subscription(String),
    #[error("opening the eframe window failed: {0}")]
    Window(String),
}

/// The parsed command line: a harness name, one or more prompts, and a model.
struct Invocation {
    harness: HarnessName,
    prompts: Vec<String>,
    model: String,
}

impl Invocation {
    fn from_process_arguments() -> Result<Self, DemoError> {
        let mut arguments = std::env::args().skip(1);
        let harness = HarnessName::new(arguments.next().ok_or(DemoError::Usage)?);
        let mut prompts = Vec::new();
        let mut model = "haiku".to_string();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--model" => model = arguments.next().ok_or(DemoError::Usage)?,
                _ => prompts.push(argument),
            }
        }
        if prompts.is_empty() {
            return Err(DemoError::Usage);
        }
        Ok(Self {
            harness,
            prompts,
            model,
        })
    }

    fn run(self) -> Result<(), DemoError> {
        let sandbox = Sandbox::for_harness(self.harness.as_str())?;
        let session = SessionIdentifier::generate()?;

        println!("== mentci-egui-session-window-demo ==");
        println!("harness  : {}", self.harness.as_str());
        println!("sandbox  : {}", sandbox.path().display());
        println!("session  : {}", session.as_str());
        println!("model    : {}", self.model);
        println!("turns    : {}", self.prompts.len());
        println!();
        println!("the window opens now; each turn is PUSHED and appears live.");
        println!();

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(DemoError::Runtime)?;

        // The producer plane — the same actors the harness daemon runs.
        let (manager, publisher) = runtime.block_on(async {
            let manager = TranscriptSubscriptionManager::spawn(TranscriptSubscriptionManager::new());
            manager.wait_for_startup().await;
            let publisher =
                TranscriptDeltaPublisher::spawn(TranscriptDeltaPublisher::new(manager.clone()));
            publisher.wait_for_startup().await;
            (manager, publisher)
        });

        // The window's consumer subscribes ONCE, BEFORE any turn runs. There is
        // no replay of a `ClaudeSessionObservation` on connect — the producer
        // fans a push out only to subscriptions that are already open — so a
        // subscription opened after a turn had already been observed would miss
        // it. Opening here, before the producer task is even spawned, guarantees
        // the window is watching when the first turn is pushed.
        let subscription = runtime
            .block_on(SessionStreamSubscription::open(
                manager.clone(),
                self.harness.clone(),
            ))
            .map_err(|error| DemoError::Subscription(error.to_string()))?;

        // Only now spawn the headless turns off the UI thread; each pushes as it
        // lands, into the already-open subscription.
        let producer = TurnProducer {
            publisher,
            harness: self.harness.clone(),
            prompts: self.prompts.clone(),
            model: self.model.clone(),
            sandbox_directory: sandbox.path().to_path_buf(),
            session: session.as_str().to_string(),
        };
        runtime.spawn(producer.run());

        let window = SessionWindow::new(
            format!("mentci · {}", self.harness.as_str()),
            subscription,
        );
        let application = SessionViewApp::single(runtime, window);

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(format!("mentci · live session · {}", self.harness.as_str()))
                .with_inner_size([980.0, 780.0])
                .with_min_inner_size([560.0, 400.0]),
            ..Default::default()
        };
        eframe::run_native(
            "mentci-live-session",
            native_options,
            Box::new(move |_cc| Box::new(application)),
        )
        .map_err(|error| DemoError::Window(error.to_string()))
    }
}

/// Runs the real headless turns and pushes each observation. Data-bearing: it
/// owns the publisher handle and the run parameters.
struct TurnProducer {
    publisher: kameo::actor::ActorRef<TranscriptDeltaPublisher>,
    harness: HarnessName,
    prompts: Vec<String>,
    model: String,
    sandbox_directory: PathBuf,
    session: String,
}

impl TurnProducer {
    async fn run(self) {
        for (index, prompt) in self.prompts.iter().enumerate() {
            let launch = if index == 0 {
                TurnLaunch::Fresh
            } else {
                TurnLaunch::Resumed
            };
            if let Err(error) = self.run_turn(index, prompt, launch).await {
                eprintln!("turn {} blocked: {error}", index + 1);
                break;
            }
        }
    }

    async fn run_turn(
        &self,
        index: usize,
        prompt: &str,
        launch: TurnLaunch,
    ) -> Result<(), DemoError> {
        let turn = HeadlessTurn::execute(
            &self.sandbox_directory,
            &self.session,
            prompt,
            &self.model,
            launch,
        )
        .await?;

        let observer = ClaudeArtifactObserver::new(self.sandbox_directory.clone())
            .with_session_identifier(self.session.clone());
        let snapshot = observer.snapshot().map_err(DemoError::Observation)?;

        // Mentci is the view: the harness owns the projection from a recovered
        // turn into the pushed observation. `accumulated_context` stays `None`
        // (deferred, og38.1) — no figure is synthesized here.
        let observation = ObservedClaudeTurn::new(
            self.harness.clone(),
            launch,
            ClaudeSessionLifecycle::Completed,
            StreamedEventCount::new(turn.streamed_event_count),
            snapshot,
        )
        .into_session_observation();

        let receipt = self
            .publisher
            .ask(PublishStreamEvent {
                event: observation.into(),
            })
            .await
            .expect("publisher accepts the pushed observation");
        println!(
            "pushed turn {} ({:?}, published={}, fanned_out={})",
            index + 1,
            launch,
            receipt.published,
            receipt.fanned_out
        );
        Ok(())
    }
}

/// One real headless `claude` turn's captured facts.
struct HeadlessTurn {
    streamed_event_count: u64,
}

impl HeadlessTurn {
    async fn execute(
        directory: &Path,
        session: &str,
        prompt: &str,
        model: &str,
        launch: TurnLaunch,
    ) -> Result<Self, DemoError> {
        let directory = directory.to_path_buf();
        let session = session.to_string();
        let prompt = prompt.to_string();
        let model = model.to_string();
        let output = tokio::task::spawn_blocking(move || {
            let mut arguments: Vec<String> = vec!["-p".to_string(), prompt];
            match launch {
                TurnLaunch::Fresh => {
                    arguments.push("--session-id".to_string());
                    arguments.push(session);
                }
                TurnLaunch::Resumed | TurnLaunch::SelfHealed => {
                    arguments.push("--resume".to_string());
                    arguments.push(session);
                }
            }
            arguments.extend(
                [
                    "--output-format",
                    "stream-json",
                    "--verbose",
                    "--model",
                    &model,
                    "--allowedTools",
                    "",
                ]
                .into_iter()
                .map(str::to_string),
            );
            Command::new("claude")
                .args(&arguments)
                .current_dir(&directory)
                .output()
        })
        .await
        .expect("claude command task joins")
        .map_err(DemoError::Spawn)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let events: Vec<Value> = stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect();
        let has_result = events
            .iter()
            .any(|event| event.get("type").and_then(Value::as_str) == Some("result"));
        if !has_result {
            return Err(DemoError::MissingResultEvent(format!("{stdout}\n{stderr}")));
        }
        Ok(Self {
            streamed_event_count: events.len() as u64,
        })
    }
}

/// The deterministic per-harness sandbox. Refuses the primary workspace before
/// creating anything.
struct Sandbox {
    directory: PathBuf,
}

impl Sandbox {
    fn for_harness(harness: &str) -> Result<Self, DemoError> {
        let intended = Self::base().join(format!("harness-{harness}"));
        Self::refuse_primary(Self::lexically_absolute(&intended)?)?;
        fs::create_dir_all(&intended).map_err(|source| DemoError::Filesystem {
            path: intended.clone(),
            source,
        })?;
        let directory = fs::canonicalize(&intended).map_err(|source| DemoError::Filesystem {
            path: intended.clone(),
            source,
        })?;
        Self::refuse_primary(directory.clone())?;
        Ok(Self { directory })
    }

    fn base() -> PathBuf {
        std::env::var_os("MENTCI_EGUI_SESSION_WINDOW_DEMO_BASE")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("mentci-egui-session-window-demo"))
    }

    fn lexically_absolute(path: &Path) -> Result<PathBuf, DemoError> {
        std::path::absolute(path).map_err(|source| DemoError::Filesystem {
            path: path.to_path_buf(),
            source,
        })
    }

    fn refuse_primary(path: PathBuf) -> Result<(), DemoError> {
        if path.starts_with(PRIMARY_WORKSPACE) {
            return Err(DemoError::PrimaryWorkspaceRefused(path));
        }
        Ok(())
    }

    fn path(&self) -> &Path {
        &self.directory
    }
}

/// A `claude` session identifier (a UUID). Domain newtype.
struct SessionIdentifier {
    value: String,
}

impl SessionIdentifier {
    fn generate() -> Result<Self, DemoError> {
        let raw = fs::read_to_string("/proc/sys/kernel/random/uuid")
            .map_err(DemoError::SessionIdentifier)?;
        Ok(Self {
            value: raw.trim().to_string(),
        })
    }

    fn as_str(&self) -> &str {
        &self.value
    }
}
