//! The push-based consumer of the harness `HarnessTranscriptStream`.
//!
//! This is the ONLY place the harness runtime touches the view. The
//! subscription opens ONE per-open `OpenTranscriptSubscription`, takes the
//! connect snapshot, then parks on `recv().await`: the harness producer plane
//! PUSHES each delivery and wakes the park. There is no poll loop, no interval
//! tick, and no separate "what is it now?" re-query — the connect snapshot seeds
//! the view and deltas follow on the same stream.
//!
//! The harness runtime's [`TranscriptDeliveryEvent`] is mapped to the view's own
//! [`SessionDelivery`] here, at the boundary, so the canvas depends only on the
//! `signal-harness` contract vocabulary.

use std::sync::{Arc, Mutex};

use harness::{
    OpenTranscriptSubscription, TranscriptDeliveryEvent, TranscriptSubscriptionManager,
    TranscriptSubscriptionSink,
};
use kameo::actor::ActorRef;
use signal_harness::{HarnessName, HarnessTranscriptSnapshot, HarnessTranscriptToken};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::error::{Error, Result};
use crate::render::canvas::{SessionCanvas, SessionDelivery};

impl From<TranscriptDeliveryEvent> for SessionDelivery {
    fn from(event: TranscriptDeliveryEvent) -> Self {
        match event {
            TranscriptDeliveryEvent::Snapshot(snapshot) => Self::Connected(snapshot),
            TranscriptDeliveryEvent::Delta(stream_event) => Self::StreamEvent(stream_event),
            TranscriptDeliveryEvent::FinalAcknowledgement(ack) => Self::Retracted(ack),
        }
    }
}

/// A single open subscription to one harness's transcript stream: the token and
/// connect snapshot the producer minted, plus the channel the producer pushes
/// deliveries onto.
pub struct SessionStreamSubscription {
    token: HarnessTranscriptToken,
    snapshot: HarnessTranscriptSnapshot,
    receiver: UnboundedReceiver<TranscriptDeliveryEvent>,
}

impl SessionStreamSubscription {
    /// Open ONE subscription against the harness subscription manager. The reply
    /// carries the per-open token and the connect snapshot; the sink is a channel
    /// the producer plane pushes each delivery onto. The consumer never polls the
    /// producer for current state — the snapshot IS the current state on connect.
    pub async fn open(
        manager: ActorRef<TranscriptSubscriptionManager>,
        harness: HarnessName,
    ) -> Result<Self> {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let sink = TranscriptSubscriptionSink::channel(sender);
        let opened = manager
            .ask(OpenTranscriptSubscription { harness, sink })
            .await
            .map_err(|error| Error::SessionSubscription(error.to_string()))?;
        Ok(Self {
            token: opened.token,
            snapshot: opened.snapshot,
            receiver,
        })
    }

    pub fn token(&self) -> &HarnessTranscriptToken {
        &self.token
    }

    pub fn snapshot(&self) -> &HarnessTranscriptSnapshot {
        &self.snapshot
    }

    /// Park until the producer PUSHES the next delivery, then hand it back in the
    /// view's own vocabulary. This is the only wait in the consumer path:
    /// `UnboundedReceiver::recv().await` sleeps the task until a push wakes it,
    /// and returns `None` only when the producer side is fully dropped.
    pub async fn next_delivery(&mut self) -> Option<SessionDelivery> {
        self.receiver.recv().await.map(SessionDelivery::from)
    }

    /// Run the pump to completion: park on each pushed delivery, fold it into the
    /// shared canvas, and wake the UI with `request_repaint` — a push wake, never
    /// a clock tick. Returns when the stream is retracted or the producer drops.
    pub async fn run(mut self, canvas: Arc<Mutex<SessionCanvas>>, repaint: egui::Context) {
        while let Some(delivery) = self.next_delivery().await {
            {
                let mut guard = canvas.lock().expect("session canvas lock");
                guard.apply(&delivery);
            }
            repaint.request_repaint();
            if delivery.is_retraction() {
                break;
            }
        }
    }
}
