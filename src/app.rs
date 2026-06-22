//! `eframe::App` implementation for the daemon-connected Mentci client.
//!
//! The shell is now thin in the intended sense: it owns no approval logic and
//! no per-socket state of its own. It holds a `mentci_lib::ObservationModel`
//! (the shared MVU core, keyed by component socket) and feeds it typed
//! `signal-mentci` replies as `EngineEvent`s; it paints the model's
//! `ObservationView` and renders each reply through `mentci_lib`'s NOTA-fallback
//! renderer. mentci-lib is the application; this file is the rendering.

use std::sync::mpsc;

use mentci_lib::{
    Cmd, ComponentSocketKind, CriomeAccess, EngineEvent, ObservationModel, RenderNota,
    RenderOrigin, RenderedObject, UserEvent,
};
use signal_mentci::{
    ApprovalDecision, ApprovalQuestion, ApprovalSource, InterfaceInterest, MentciReply,
    QuestionIdentifier, SubscriberName,
};

use crate::control::{
    GuiControlEndpoint, GuiControlInput, GuiControlOutput, GuiControlRejection,
    GuiControlRejectionReason, GuiControlRequest, GuiControlServer, GuiControlState,
    RemoteControlMode,
};
use crate::daemon_client::{DaemonClient, SocketKind};

/// One rendered observation the shell shows: which operation produced it and
/// the NOTA body the shared renderer produced.
struct ObservationEntry {
    operation: String,
    rendered: RenderedObject,
}

/// A gesture the approval card captured this frame. The immediate-mode render
/// closures only record the gesture; it is applied through the shared model
/// after rendering, so no closure holds a mutable borrow of the app.
enum CardAction {
    Select(QuestionIdentifier),
    Answer(ApprovalDecision),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SystemColorScheme {
    Dark,
    Light,
}

/// Follows the operating-system light/dark preference and mirrors it into
/// egui's visuals. The OS colour-scheme (read through the desktop portal) is the
/// source of truth; this shell never owns a theme of its own — it follows the
/// system, throttling the portal probe to a coarse interval rather than calling
/// it every frame, and applies the visuals on the first probe and on any change.
enum SystemThemeFollower {
    Unprobed,
    Following {
        scheme: SystemColorScheme,
        last_probe: f64,
    },
}

impl SystemColorScheme {
    fn detect() -> Self {
        Self::from_freedesktop_portal()
            .or_else(|| Self::from_dark_light(dark_light::detect()))
            .unwrap_or(Self::Light)
    }

    fn visuals(self) -> egui::Visuals {
        match self {
            Self::Dark => egui::Visuals::dark(),
            Self::Light => egui::Visuals::light(),
        }
    }

    fn from_dark_light(mode: dark_light::Mode) -> Option<Self> {
        match mode {
            dark_light::Mode::Dark => Some(Self::Dark),
            dark_light::Mode::Light => Some(Self::Light),
            dark_light::Mode::Default => None,
        }
    }

    fn from_freedesktop_portal() -> Option<Self> {
        let connection = zbus::blocking::Connection::session().ok()?;
        let reply = connection
            .call_method(
                Some("org.freedesktop.portal.Desktop"),
                "/org/freedesktop/portal/desktop",
                Some("org.freedesktop.portal.Settings"),
                "Read",
                &("org.freedesktop.appearance", "color-scheme"),
            )
            .ok()?;
        match reply.body::<zbus::zvariant::Value<'_>>().ok()? {
            zbus::zvariant::Value::U32(value) => Self::from_portal_value(value),
            _ => None,
        }
    }

    fn from_portal_value(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Dark),
            2 => Some(Self::Light),
            _ => None,
        }
    }
}

impl SystemThemeFollower {
    const PROBE_INTERVAL_SECONDS: f64 = 2.0;

    fn new() -> Self {
        Self::Unprobed
    }

    fn follow(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|input| input.time);
        if let Self::Following { last_probe, .. } = self
            && now - *last_probe < Self::PROBE_INTERVAL_SECONDS
        {
            return;
        }
        let detected = SystemColorScheme::detect();
        let changed = !matches!(self, Self::Following { scheme, .. } if *scheme == detected);
        *self = Self::Following {
            scheme: detected,
            last_probe: now,
        };
        if changed {
            ctx.set_visuals(detected.visuals());
        }
    }
}

pub struct MentciEguiApp {
    tokio_runtime: tokio::runtime::Runtime,
    bootstrap_done: bool,
    daemon_client: DaemonClient,
    /// The shared model. The shell routes every typed reply through it.
    model: ObservationModel,
    /// Rendered transcript — each entry's body comes from mentci-lib's
    /// `RenderNota`, not a hand-rolled `to_nota()` in this shell.
    entries: Vec<ObservationEntry>,
    daemon_replies: mpsc::Receiver<crate::error::Result<MentciReply>>,
    daemon_reply_sender: mpsc::Sender<crate::error::Result<MentciReply>>,
    control_requests: mpsc::Receiver<GuiControlRequest>,
    control_request_sender: mpsc::Sender<GuiControlRequest>,
    ordinary_request_in_flight: bool,
    remote_control_mode: RemoteControlMode,
    control_server_started: bool,
    control_endpoint: GuiControlEndpoint,
    /// Mirrors the operating-system light/dark preference into egui's visuals.
    theme: SystemThemeFollower,
}

impl MentciEguiApp {
    pub fn new(tokio_runtime: tokio::runtime::Runtime) -> Self {
        let (daemon_reply_sender, daemon_replies) = mpsc::channel();
        let (control_request_sender, control_requests) = mpsc::channel();
        Self {
            tokio_runtime,
            bootstrap_done: false,
            daemon_client: DaemonClient::from_environment(),
            model: ObservationModel::new(SubscriberName::new("mentci-egui")),
            entries: Vec::new(),
            daemon_replies,
            daemon_reply_sender,
            control_requests,
            control_request_sender,
            ordinary_request_in_flight: false,
            remote_control_mode: RemoteControlMode::default(),
            control_server_started: false,
            control_endpoint: GuiControlEndpoint::from_environment(),
            theme: SystemThemeFollower::new(),
        }
    }

    fn bootstrap_if_needed(&mut self) {
        if self.bootstrap_done {
            return;
        }
        self.bootstrap_done = true;
        self.start_control_server_if_needed();
        self.request_interface_state();
    }

    fn start_control_server_if_needed(&mut self) {
        if self.control_server_started {
            return;
        }
        self.control_server_started = true;
        let server = GuiControlServer::new(
            self.control_endpoint.clone(),
            self.control_request_sender.clone(),
        );
        let _ = server.spawn();
    }

    fn request_interface_state(&mut self) {
        if self.ordinary_request_in_flight {
            return;
        }
        let commands = self.model.on_user_event(UserEvent::Observe {
            socket: ComponentSocketKind::Mentci,
            interest: InterfaceInterest::FullInterfaceState,
        });
        self.dispatch(commands);
    }

    /// Dispatch the side-effects the shared model produced. The shell sends the
    /// model's own `Cmd::SendRequest` — it does not re-derive the request — so
    /// the model stays the single source of what leaves the client (MVU). Under
    /// daemon-routing the client model only ever emits `SendRequest`: a criome
    /// verdict reaches criome through the daemon, never the shell.
    fn dispatch(&mut self, commands: Vec<Cmd>) {
        for command in commands {
            match command {
                Cmd::SendRequest { request, .. } => {
                    self.ordinary_request_in_flight = true;
                    let sender = self.daemon_reply_sender.clone();
                    let client = self.daemon_client.clone();
                    self.tokio_runtime.spawn_blocking(move || {
                        let _ = sender.send(client.send_request_typed(request));
                    });
                }
            }
        }
    }

    /// Answer the selected question with a closed decision. The shared model
    /// builds the verdict for the cursor and emits the request `Cmd`; the shell
    /// only dispatches it.
    fn answer(&mut self, decision: ApprovalDecision) {
        if let Some(verdict) = self
            .model
            .approval()
            .verdict_for_selected(decision, SubscriberName::new("mentci-egui"))
        {
            let commands = self
                .model
                .on_user_event(UserEvent::AnswerQuestion { verdict });
            self.dispatch(commands);
        }
    }

    /// Move the approval cursor to a pending question (local; no request).
    fn select(&mut self, question: QuestionIdentifier) {
        let _ = self
            .model
            .on_user_event(UserEvent::SelectQuestion { question });
    }

    fn control_state(&self) -> GuiControlState {
        let view = self.model.view();
        GuiControlState {
            mode: self.remote_control_mode,
            pending_questions: view.approval.pending_count as u64,
            answered_questions: view.approval.answered_count as u64,
            selected_question: self
                .model
                .approval()
                .current()
                .map(|question| question.identifier.clone()),
            ordinary_request_in_flight: self.ordinary_request_in_flight,
            transcript_entries: self.entries.len() as u64,
        }
    }

    fn apply_control_input(&mut self, input: GuiControlInput) -> GuiControlOutput {
        if input.requires_remote_drive() && !self.remote_control_mode.remote_can_drive() {
            return GuiControlOutput::Rejected(GuiControlRejection::new(
                GuiControlRejectionReason::RemoteControlDisabled,
            ));
        }
        match input {
            GuiControlInput::ObserveState => GuiControlOutput::State(self.control_state()),
            GuiControlInput::SetRemoteControl(mode) => {
                self.remote_control_mode = mode;
                GuiControlOutput::Accepted(self.control_state())
            }
            GuiControlInput::TriggerObserve => {
                self.request_interface_state();
                GuiControlOutput::Accepted(self.control_state())
            }
            GuiControlInput::SelectQuestion(question) => {
                self.select(question);
                GuiControlOutput::Accepted(self.control_state())
            }
            GuiControlInput::AnswerSelected(decision) => {
                if self.model.approval().current().is_none() {
                    return GuiControlOutput::Rejected(GuiControlRejection::new(
                        GuiControlRejectionReason::NoSelectedQuestion,
                    ));
                }
                self.answer(decision);
                GuiControlOutput::Accepted(self.control_state())
            }
        }
    }

    fn drain_control_requests(&mut self) {
        while let Ok(request) = self.control_requests.try_recv() {
            let output = self.apply_control_input(request.input().clone());
            let _ = request.respond(output);
        }
    }

    fn drain_daemon_replies(&mut self) {
        while let Ok(result) = self.daemon_replies.try_recv() {
            self.ordinary_request_in_flight = false;
            match result {
                Ok(reply) => self.absorb_reply(reply),
                Err(error) => self.entries.push(ObservationEntry {
                    operation: "ObserveInterfaceState (error)".to_string(),
                    rendered: format!("{error}").render_nota(RenderOrigin::Reply),
                }),
            }
        }
    }

    /// Fold a typed reply into the shared model and render it. The shell does
    /// not interpret the reply itself — mentci-lib's model owns the state and
    /// the renderer owns the projection.
    fn absorb_reply(&mut self, reply: MentciReply) {
        let rendered = reply.render_nota(RenderOrigin::Reply);
        if let MentciReply::InterfaceObservationOpened(opened) = &reply {
            self.model.on_engine_event(EngineEvent::ObservationOpened {
                socket: ComponentSocketKind::Mentci,
                opened: opened.clone(),
            });
        }
        self.entries.push(ObservationEntry {
            operation: self.reply_operation_name(&reply).to_string(),
            rendered,
        });
    }

    /// Name the reply variant for the transcript header — a closed match on
    /// the contract enum, no string sniffing.
    fn reply_operation_name(&self, reply: &MentciReply) -> &'static str {
        match reply {
            MentciReply::QuestionPresented(_) => "QuestionPresented",
            MentciReply::UpdateAccepted(_) => "UpdateAccepted",
            MentciReply::InterfaceObservationOpened(_) => "InterfaceObservationOpened",
            MentciReply::VerdictAccepted(_) => "VerdictAccepted",
            MentciReply::AnswerProposalAdmitted(_) => "AnswerProposalAdmitted",
            MentciReply::InterfaceObservationRetracted(_) => "InterfaceObservationRetracted",
            MentciReply::Rejection(_) => "Rejection",
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("mentci");
            ui.separator();
            ui.label(SocketKind::Mentci.label());
            ui.monospace(self.daemon_client.ordinary_socket().display().to_string());
            ui.separator();
            ui.label(format!("remote: {}", self.remote_control_mode.label()));
            ui.monospace(self.control_endpoint.socket_path().display().to_string());
            ui.separator();
            if ui
                .add_enabled(
                    !self.ordinary_request_in_flight && self.remote_control_mode.local_can_drive(),
                    egui::Button::new("observe"),
                )
                .clicked()
            {
                self.request_interface_state();
            }
            if self.ordinary_request_in_flight {
                ui.spinner();
            }
        });

        // The shared model's view: one row per observed component socket plus
        // the approval summary. The shell paints what the model says.
        let view = self.model.view();
        ui.horizontal_wrapped(|ui| {
            for socket in &view.sockets {
                ui.group(|ui| {
                    ui.label(socket.socket.as_str());
                    ui.label(format!("{:?}", socket.liveness));
                    if let Some(revision) = &socket.revision {
                        ui.monospace(format!("rev {}", revision.value()));
                    }
                });
            }
            ui.separator();
            ui.label(format!(
                "pending {} | answered {}",
                view.approval.pending_count, view.approval.answered_count
            ));
        });
    }

    /// The approval card: the pending-question queue, the selected question's
    /// full content, and the closed Approve / Reject / Defer controls. This is
    /// the psyche-escalation surface made real — the shell paints the shared
    /// model's approval cursor and feeds its decisions back through the model.
    fn render_approval_card(&mut self, ui: &mut egui::Ui) {
        let pending: Vec<ApprovalQuestion> = self.model.approval().pending().to_vec();
        let current = self.model.approval().current().cloned();
        let criome_access = self.model.view().criome_access;
        let local_can_drive = self.remote_control_mode.local_can_drive();
        let can_answer = local_can_drive && matches!(criome_access, Some(CriomeAccess::ReadWrite));
        let mut action: Option<CardAction> = None;

        ui.heading("approvals");
        ui.horizontal(|ui| {
            ui.label(format!("{} pending", pending.len()));
            ui.separator();
            ui.label(match criome_access {
                Some(CriomeAccess::ReadWrite) => "criome: read-write",
                Some(CriomeAccess::ReadOnly) => "criome: read-only",
                None => "criome: observation-only",
            });
            if !local_can_drive {
                ui.separator();
                ui.label("local input locked");
            }
        });
        ui.separator();

        if pending.is_empty() {
            ui.label("no pending questions");
        } else {
            for question in &pending {
                let is_current = current.as_ref().is_some_and(|selected| {
                    selected.identifier.as_str() == question.identifier.as_str()
                });
                if ui
                    .selectable_label(is_current, question.identifier.as_str())
                    .clicked()
                {
                    action = Some(CardAction::Select(question.identifier.clone()));
                }
            }
            ui.separator();

            if let Some(question) = &current {
                let source = match &question.proposal.source {
                    ApprovalSource::CriomeEscalation(_) => "criome escalation",
                    ApprovalSource::AgentQuestion => "agent question",
                    ApprovalSource::LocalSystemPrompt => "local system prompt",
                };
                ui.strong(question.identifier.as_str());
                ui.label(format!("source: {source}"));
                ui.label(question.proposal.prompt.as_str());
                ui.label(format!("why: {}", question.proposal.explanation.as_str()));
                if let Some(answer) = question.proposal.suggested_answer() {
                    ui.label(format!("suggested answer: {}", answer.as_str()));
                }
                for entry in question.proposal.context() {
                    ui.collapsing(entry.label.as_str(), |ui| {
                        ui.monospace(entry.body.as_str());
                    });
                }
                ui.add_space(6.0);
                if can_answer {
                    ui.horizontal(|ui| {
                        if ui.button("approve").clicked() {
                            action =
                                Some(CardAction::Answer(ApprovalDecision::ApproveSuggestedAnswer));
                        }
                        if ui.button("reject").clicked() {
                            action = Some(CardAction::Answer(ApprovalDecision::Reject));
                        }
                        if ui.button("defer").clicked() {
                            action = Some(CardAction::Answer(ApprovalDecision::Defer));
                        }
                    });
                } else {
                    ui.label("observation-only — this daemon has no criome write access");
                }
            }
        }

        match action {
            Some(CardAction::Select(identifier)) => self.select(identifier),
            Some(CardAction::Answer(decision)) => self.answer(decision),
            None => {}
        }
    }

    fn render_transcript(&self, ui: &mut egui::Ui) {
        if self.entries.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("waiting for mentci-daemon");
            });
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for entry in self.entries.iter().rev() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(entry.rendered.origin().label());
                            ui.label(&entry.operation);
                        });
                        ui.add(
                            egui::TextEdit::multiline(&mut entry.rendered.body())
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    });
                    ui.add_space(8.0);
                }
            });
    }

    fn render_panes(&self, ui: &mut egui::Ui) {
        let panes = self.model.view().panes;
        if panes.is_empty() {
            return;
        }
        ui.heading("panes");
        for pane in panes {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.strong(pane.pane.as_str());
                let mut body = pane.body.as_str().to_owned();
                ui.add(
                    egui::TextEdit::multiline(&mut body)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
            });
            ui.add_space(8.0);
        }
    }
}

impl eframe::App for MentciEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.follow(ctx);
        self.bootstrap_if_needed();
        self.drain_control_requests();
        self.drain_daemon_replies();

        egui::TopBottomPanel::top("daemon_header").show(ctx, |ui| {
            self.render_header(ui);
        });
        egui::SidePanel::left("approvals")
            .default_width(340.0)
            .show(ctx, |ui| {
                self.render_approval_card(ui);
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_panes(ui);
            if !self.model.view().panes.is_empty() && !self.entries.is_empty() {
                ui.separator();
            }
            self.render_transcript(ui);
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_color_scheme_values_map_to_egui_schemes() {
        assert_eq!(
            SystemColorScheme::from_portal_value(1),
            Some(SystemColorScheme::Dark)
        );
        assert_eq!(
            SystemColorScheme::from_portal_value(2),
            Some(SystemColorScheme::Light)
        );
        assert_eq!(SystemColorScheme::from_portal_value(0), None);
    }

    #[test]
    fn dark_light_default_does_not_force_light_before_other_sources() {
        assert_eq!(
            SystemColorScheme::from_dark_light(dark_light::Mode::Dark),
            Some(SystemColorScheme::Dark)
        );
        assert_eq!(
            SystemColorScheme::from_dark_light(dark_light::Mode::Light),
            Some(SystemColorScheme::Light)
        );
        assert_eq!(
            SystemColorScheme::from_dark_light(dark_light::Mode::Default),
            None
        );
    }

    #[test]
    fn remote_control_rejects_drive_commands_until_enabled() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let mut app = MentciEguiApp::new(runtime);

        let output = app.apply_control_input(GuiControlInput::TriggerObserve);

        assert!(matches!(
            output,
            GuiControlOutput::Rejected(GuiControlRejection {
                reason: GuiControlRejectionReason::RemoteControlDisabled
            })
        ));
    }

    #[test]
    fn remote_control_mode_change_enables_drive_commands() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let mut app = MentciEguiApp::new(runtime);

        let enabled = app.apply_control_input(GuiControlInput::SetRemoteControl(
            RemoteControlMode::DualWrite,
        ));
        assert!(matches!(enabled, GuiControlOutput::Accepted(_)));

        let observed = app.apply_control_input(GuiControlInput::ObserveState);
        assert!(matches!(
            observed,
            GuiControlOutput::State(GuiControlState {
                mode: RemoteControlMode::DualWrite,
                ..
            })
        ));
    }
}
