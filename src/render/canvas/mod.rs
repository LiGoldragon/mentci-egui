//! The live-session canvas — the pane that renders a Claude session's turns as
//! the harness PUSHES them.
//!
//! The canvas is the VIEW and depends only on the `signal-harness` contract
//! vocabulary — never on the harness runtime. It receives deltas expressed as
//! [`SessionDelivery`] (the mapping from the harness runtime's delivery type
//! lives at the subscription boundary in `session_stream`), folds each into
//! view state, and paints it. Provider and mapping logic do not re-enter it.

pub mod turn_row;

pub use turn_row::{ObservedTurn, TurnField};

use signal_harness::{
    ClaudeSessionObservation, HarnessStreamEvent, HarnessSubscriptionRetracted,
    HarnessTranscriptSnapshot, TranscriptObservation,
};

/// One delivery from the harness `HarnessTranscriptStream`, expressed purely in
/// the `signal-harness` contract vocabulary the canvas renders.
///
/// The connect snapshot opens the stream ([`SessionDelivery::Connected`]);
/// stream events — a Claude session observation or a transcript line — follow
/// as deltas ([`SessionDelivery::StreamEvent`]); the final retraction ack closes
/// it ([`SessionDelivery::Retracted`]).
#[derive(Debug, Clone)]
pub enum SessionDelivery {
    Connected(HarnessTranscriptSnapshot),
    StreamEvent(HarnessStreamEvent),
    Retracted(HarnessSubscriptionRetracted),
}

impl SessionDelivery {
    pub fn is_retraction(&self) -> bool {
        matches!(self, Self::Retracted(_))
    }
}

/// The subscription's connection state, mirrored into the pane so the psyche
/// can see whether the stream is live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionConnection {
    Connecting,
    Subscribed { current_sequence: u64 },
    Retracted,
}

impl SessionConnection {
    fn label(&self) -> String {
        match self {
            Self::Connecting => "connecting…".to_string(),
            Self::Subscribed { current_sequence } => {
                format!("subscribed (seq {current_sequence})")
            }
            Self::Retracted => "retracted".to_string(),
        }
    }
}

/// One transcript line the harness pushed on the same stream as the session
/// observations. Secondary to the turn cards; shown as a running log.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TranscriptLine {
    sequence: u64,
    text: String,
}

/// The live-session pane. Owns the observed turns, the connection status, and
/// the transcript-line log, and paints them. One canvas backs one Mentci
/// window.
#[derive(Debug, Clone)]
pub struct SessionCanvas {
    harness: Option<String>,
    connection: SessionConnection,
    turns: Vec<ObservedTurn>,
    transcript_lines: Vec<TranscriptLine>,
}

impl SessionCanvas {
    pub fn new() -> Self {
        Self {
            harness: None,
            connection: SessionConnection::Connecting,
            turns: Vec::new(),
            transcript_lines: Vec::new(),
        }
    }

    pub fn connection(&self) -> &SessionConnection {
        &self.connection
    }

    pub fn turns(&self) -> &[ObservedTurn] {
        &self.turns
    }

    /// Fold one pushed delivery into view state. This is the single write path;
    /// the subscription pump calls it for each `recv().await`.
    pub fn apply(&mut self, delivery: &SessionDelivery) {
        match delivery {
            SessionDelivery::Connected(snapshot) => self.absorb_snapshot(snapshot),
            SessionDelivery::StreamEvent(event) => self.absorb_stream_event(event),
            SessionDelivery::Retracted(_) => self.connection = SessionConnection::Retracted,
        }
    }

    fn absorb_snapshot(&mut self, snapshot: &HarnessTranscriptSnapshot) {
        self.harness = Some(snapshot.token.harness.as_str().to_string());
        self.connection = SessionConnection::Subscribed {
            current_sequence: snapshot.current_sequence.into_u64(),
        };
    }

    fn absorb_stream_event(&mut self, event: &HarnessStreamEvent) {
        match event {
            HarnessStreamEvent::ClaudeSessionObservation(observation) => {
                self.absorb_observation(observation)
            }
            HarnessStreamEvent::TranscriptObservation(line) => self.absorb_transcript_line(line),
        }
    }

    fn absorb_observation(&mut self, observation: &ClaudeSessionObservation) {
        if self.harness.is_none() {
            self.harness = Some(observation.harness.as_str().to_string());
        }
        let index = self.turns.len() + 1;
        self.turns
            .push(ObservedTurn::new(index, observation.clone()));
    }

    fn absorb_transcript_line(&mut self, line: &TranscriptObservation) {
        self.transcript_lines.push(TranscriptLine {
            sequence: line.sequence.into_u64(),
            text: line.line.clone(),
        });
    }

    fn harness_label(&self) -> &str {
        self.harness.as_deref().unwrap_or("<pending>")
    }

    /// Paint the whole pane: a status header, the turn cards (newest first), and
    /// the transcript-line log if any lines arrived.
    pub fn paint(&self, ui: &mut egui::Ui) {
        ui.heading("live session");
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("harness: {}", self.harness_label()));
            ui.separator();
            ui.label(self.connection.label());
            ui.separator();
            ui.label(format!("turns: {}", self.turns.len()));
        });
        ui.separator();

        if self.turns.is_empty() {
            ui.label("waiting for the first turn — the harness will PUSH it (no polling)");
        } else {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for turn in self.turns.iter().rev() {
                        turn.paint(ui);
                        ui.add_space(8.0);
                    }
                    self.paint_transcript_log(ui);
                });
        }
    }

    fn paint_transcript_log(&self, ui: &mut egui::Ui) {
        if self.transcript_lines.is_empty() {
            return;
        }
        ui.separator();
        ui.strong(format!("transcript lines: {}", self.transcript_lines.len()));
        for line in &self.transcript_lines {
            ui.monospace(format!("[{}] {}", line.sequence, line.text));
        }
    }
}

impl Default for SessionCanvas {
    fn default() -> Self {
        Self::new()
    }
}
