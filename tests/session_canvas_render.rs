//! Render witness for the live Mentci session window.
//!
//! This proves two things with real, machine-checkable evidence:
//!
//! 1. NO-POLL CONSUMER: the window's `SessionStreamSubscription` opens once
//!    against the REAL harness producer plane (`TranscriptSubscriptionManager` +
//!    `TranscriptDeltaPublisher`), takes the connect snapshot, and then the
//!    canvas gains a turn ONLY after the producer PUSHES a
//!    `ClaudeSessionObservation` — received via `recv().await`, never polled.
//!
//! 2. REAL RENDER: the pane is actually painted headlessly through egui, and the
//!    pushed observation's content is read BACK out of the paint output — the
//!    laid-out text galleys `egui` produced for the frame. The text is in the
//!    output only because the paint pass emitted labels carrying it, so this is
//!    the text the window actually draws.
//!
//! The observation is the same realistically-shaped turn the harness witness
//! (`harness/tests/claude_session_stream.rs`) uses — not a hardcoded string
//! poked into the canvas.

use harness::{
    PublishStreamEvent, TranscriptDeltaPublisher, TranscriptSubscriptionManager,
};
use kameo::actor::Spawn;
use mentci_egui::render::{SessionCanvas, SessionDelivery, SessionStreamSubscription};
use signal_harness::{
    AssistantResponseText, ClaudeModel, ClaudeSessionIdentifier, ClaudeSessionLifecycle,
    ClaudeSessionObservation, HarnessName, StatusTransitionCount, StreamedEventCount, ToolCallCount,
    TranscriptPath, TurnLaunch,
};
use signal_persona::TimestampNanos;

/// The realistically-shaped observation of one completed headless turn. Mirrors
/// the harness producer's own witness fixture; `accumulated_context` is `None`
/// by design (deferred, og38.1) — the view must render it as pending, never a
/// fabricated figure.
fn realistic_observation(harness: &str) -> ClaudeSessionObservation {
    ClaudeSessionObservation {
        harness: HarnessName::new(harness),
        session_identifier: Some(ClaudeSessionIdentifier::new("session-alpha")),
        model: Some(ClaudeModel::new("claude-3-5-haiku-latest")),
        launch: TurnLaunch::Fresh,
        reached_end_of_turn: true,
        streamed_event_count: StreamedEventCount::new(9),
        tool_call_count: ToolCallCount::new(2),
        status_transition_count: StatusTransitionCount::new(4),
        transcript_path: Some(TranscriptPath::new("/tmp/session-alpha.jsonl")),
        response: Some(AssistantResponseText::new("FINAL_MARKER done")),
        accumulated_context: None,
        last_activity: TimestampNanos::new(1),
        lifecycle: ClaudeSessionLifecycle::Completed,
    }
}

/// Paint the canvas headlessly through the real egui pipeline and read every
/// laid-out text galley back out of the paint output. The returned text is
/// exactly what the window draws: a string is present only because the paint
/// pass emitted a label carrying it.
fn rendered_text(canvas: &SessionCanvas) -> String {
    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1600.0, 4000.0),
        )),
        ..Default::default()
    };
    // Two frames: fonts and layout settle on the first, so the second frame's
    // galleys carry the final, non-elided text.
    let mut collected = Vec::new();
    for _ in 0..2 {
        let output = ctx.run(raw_input.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.paint(ui));
        });
        collected.clear();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut collected);
        }
    }
    collected.join("\n")
}

/// Walk a paint shape (recursing through grouped `Shape::Vec`) and collect the
/// text of every laid-out galley.
fn collect_shape_text(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
    match shape {
        egui::epaint::Shape::Text(text_shape) => out.push(text_shape.galley.text().to_string()),
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_shape_text(shape, out);
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn pushed_observation_appears_in_the_rendered_pane() {
    // The real producer plane — the same actors the harness daemon runs.
    let manager = TranscriptSubscriptionManager::spawn(TranscriptSubscriptionManager::new());
    manager.wait_for_startup().await;
    let publisher = TranscriptDeltaPublisher::spawn(TranscriptDeltaPublisher::new(manager.clone()));
    publisher.wait_for_startup().await;

    // The window's consumer subscribes ONCE and takes the connect snapshot.
    let harness = HarnessName::new("mentci-live-view-1");
    let mut subscription = SessionStreamSubscription::open(manager.clone(), harness.clone())
        .await
        .expect("subscription opens");
    assert_eq!(subscription.snapshot().token.harness.as_str(), "mentci-live-view-1");

    let mut canvas = SessionCanvas::new();

    // The connect snapshot is PUSHED first; it seeds the pane's connection state.
    let first = subscription
        .next_delivery()
        .await
        .expect("connect snapshot pushed");
    assert!(
        matches!(first, SessionDelivery::Connected(_)),
        "expected connect snapshot, got {first:?}"
    );
    canvas.apply(&first);
    // NO-POLL WITNESS: with nothing published, no turn exists. The pane gains a
    // turn only from a producer push, never from a poll.
    assert!(
        canvas.turns().is_empty(),
        "no turn before the producer pushes one"
    );

    // The producer PUSHES one Claude session observation.
    let observation = realistic_observation("mentci-live-view-1");
    let receipt = publisher
        .ask(PublishStreamEvent {
            event: observation.clone().into(),
        })
        .await
        .expect("publish observation");
    assert!(receipt.published);
    assert_eq!(receipt.fanned_out, 1, "the one open window received the push");

    // The consumer's park on recv().await is woken by the push; the pane folds
    // the turn in.
    let delivery = subscription
        .next_delivery()
        .await
        .expect("observation pushed as a delta");
    canvas.apply(&delivery);
    assert_eq!(canvas.turns().len(), 1, "the pushed turn is now rendered");

    // The pane actually paints the turn: read the content back out of the
    // rendered text galleys egui laid out for the frame.
    let painted = rendered_text(&canvas);

    for expected in [
        "FINAL_MARKER done",        // assistant response text
        "claude-3-5-haiku-latest",  // model
        "fresh",                    // launch provenance
        "completed",                // lifecycle
        "unknown / pending",        // accumulated context — deferred, not fabricated
        "/tmp/session-alpha.jsonl", // transcript path
        "session-alpha",            // recovered session id
        "reached",                  // end-of-turn signal
        "streamed events: 9",       // one of the three activity counts
        "tool calls: 2",
        "status transitions: 4",
    ] {
        assert!(
            painted.contains(expected),
            "rendered pane is missing {expected:?}; painted text was:\n{painted}"
        );
    }

    // The deferred figure is never fabricated into a token count.
    assert!(
        !painted.contains("accumulated context: 0 tokens"),
        "accumulated_context must not be synthesized to a number while deferred"
    );

    let _ = publisher.stop_gracefully().await;
    publisher.wait_for_shutdown().await;
    let _ = manager.stop_gracefully().await;
    manager.wait_for_shutdown().await;
}
