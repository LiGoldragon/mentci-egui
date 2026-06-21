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
    Cmd, ComponentSocketKind, CriomeAccess, EngineEvent, ObservationModel, RenderNota, RenderOrigin,
    RenderedObject, UserEvent,
};
use signal_mentci::{
    ApprovalDecision, ApprovalQuestion, ApprovalSource, InterfaceInterest, MentciReply,
    QuestionIdentifier, SubscriberName,
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
    ordinary_request_in_flight: bool,
}

impl MentciEguiApp {
    pub fn new(tokio_runtime: tokio::runtime::Runtime) -> Self {
        let (daemon_reply_sender, daemon_replies) = mpsc::channel();
        Self {
            tokio_runtime,
            bootstrap_done: false,
            daemon_client: DaemonClient::from_environment(),
            model: ObservationModel::new(SubscriberName::new("mentci-egui")),
            entries: Vec::new(),
            daemon_replies,
            daemon_reply_sender,
            ordinary_request_in_flight: false,
        }
    }

    fn bootstrap_if_needed(&mut self) {
        if self.bootstrap_done {
            return;
        }
        self.bootstrap_done = true;
        self.request_interface_state();
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
            let commands = self.model.on_user_event(UserEvent::AnswerQuestion { verdict });
            self.dispatch(commands);
        }
    }

    /// Move the approval cursor to a pending question (local; no request).
    fn select(&mut self, question: QuestionIdentifier) {
        let _ = self.model.on_user_event(UserEvent::SelectQuestion { question });
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
            if ui
                .add_enabled(
                    !self.ordinary_request_in_flight,
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
        let can_answer = matches!(criome_access, Some(CriomeAccess::ReadWrite));
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
        });
        ui.separator();

        if pending.is_empty() {
            ui.label("no pending questions");
        } else {
            for question in &pending {
                let is_current = current
                    .as_ref()
                    .is_some_and(|selected| selected.identifier.as_str() == question.identifier.as_str());
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
}

impl eframe::App for MentciEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.bootstrap_if_needed();
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
            self.render_transcript(ui);
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
