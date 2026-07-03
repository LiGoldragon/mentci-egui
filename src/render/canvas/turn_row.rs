//! One observed Claude session turn, projected for display.
//!
//! Mentci is the VIEW. `ObservedTurn` holds the `signal-harness` contract
//! observation the harness PUSHED and paints it; it performs no provider work,
//! opens no session, and mints no facts of its own. Every rendered value comes
//! straight from the pushed [`ClaudeSessionObservation`].

use signal_harness::{ClaudeSessionLifecycle, ClaudeSessionObservation, TurnLaunch};

/// One `label: value` row the pane paints for a turn. The pane and the render
/// witness both read this one projection, so what a test asserts is exactly
/// what the pane shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnField {
    pub label: &'static str,
    pub value: String,
}

impl TurnField {
    fn new(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
        }
    }
}

/// One Claude session turn the harness observed and PUSHED, projected for
/// display. Holds the whole contract observation; the render methods read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTurn {
    index: usize,
    observation: ClaudeSessionObservation,
}

impl ObservedTurn {
    pub fn new(index: usize, observation: ClaudeSessionObservation) -> Self {
        Self { index, observation }
    }

    pub fn observation(&self) -> &ClaudeSessionObservation {
        &self.observation
    }

    /// How this turn opened its session — a closed contract variant, not an
    /// `is_resume` bool. `SelfHealed` is the psyche-locked path where a resume
    /// target was gone and a fresh session was minted in its place.
    pub fn launch_label(&self) -> &'static str {
        match self.observation.launch {
            TurnLaunch::Fresh => "fresh",
            TurnLaunch::Resumed => "resumed",
            TurnLaunch::SelfHealed => "self-healed",
        }
    }

    pub fn lifecycle_label(&self) -> String {
        match &self.observation.lifecycle {
            ClaudeSessionLifecycle::Ready => "ready".to_string(),
            ClaudeSessionLifecycle::Active => "active".to_string(),
            ClaudeSessionLifecycle::Completed => "completed".to_string(),
            ClaudeSessionLifecycle::Exited(status) => format!("exited ({status:?})"),
        }
    }

    /// The accumulated-context figure is `None` by design until the A/B ruling
    /// (bead og38.1). The view shows it as pending and NEVER fabricates a number
    /// to fill the gap.
    pub fn accumulated_context_label(&self) -> String {
        match self.observation.accumulated_context {
            Some(tokens) => format!("{} tokens", tokens.into_u64()),
            None => "unknown / pending".to_string(),
        }
    }

    pub fn heading(&self) -> String {
        format!(
            "turn {} — {} — {}",
            self.index,
            self.launch_label(),
            self.lifecycle_label()
        )
    }

    pub fn response_text(&self) -> &str {
        self.observation
            .response
            .as_ref()
            .map(|response| response.as_str())
            .unwrap_or("<no response captured for this turn yet>")
    }

    /// The label/value rows painted for this turn, in display order.
    pub fn fields(&self) -> Vec<TurnField> {
        let observation = &self.observation;
        vec![
            TurnField::new(
                "model",
                observation
                    .model
                    .as_ref()
                    .map(|model| model.as_str().to_string())
                    .unwrap_or_else(|| "<unknown>".to_string()),
            ),
            TurnField::new("launch", self.launch_label()),
            TurnField::new("lifecycle", self.lifecycle_label()),
            TurnField::new(
                "end of turn",
                if observation.reached_end_of_turn {
                    "reached"
                } else {
                    "not reached"
                },
            ),
            TurnField::new(
                "streamed events",
                observation.streamed_event_count.into_u64().to_string(),
            ),
            TurnField::new(
                "tool calls",
                observation.tool_call_count.into_u64().to_string(),
            ),
            TurnField::new(
                "status transitions",
                observation.status_transition_count.into_u64().to_string(),
            ),
            TurnField::new("accumulated context", self.accumulated_context_label()),
            TurnField::new(
                "session id",
                observation
                    .session_identifier
                    .as_ref()
                    .map(|identifier| identifier.as_str().to_string())
                    .unwrap_or_else(|| "<pending>".to_string()),
            ),
            TurnField::new(
                "transcript",
                observation
                    .transcript_path
                    .as_ref()
                    .map(|path| path.as_str().to_string())
                    .unwrap_or_else(|| "<pending>".to_string()),
            ),
            TurnField::new(
                "last activity",
                observation.last_activity.payload().to_string(),
            ),
        ]
    }

    /// Paint the turn as a grouped card of labelled fields plus the assistant
    /// response. Each field is a `Label`, so the rendered text is the same text
    /// the render witness reads back from the accessibility tree.
    pub fn paint(&self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.strong(self.heading());
            for field in self.fields() {
                ui.label(format!("{}: {}", field.label, field.value));
            }
            ui.label("response:");
            ui.label(self.response_text());
        });
    }
}
