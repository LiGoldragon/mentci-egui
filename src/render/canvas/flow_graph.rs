//! Paint a flow-graph view via egui's Painter API.
//!
//! Boxes-and-edges rendering. Glyph encodes node-kind; stroke
//! style encodes RelationKind; colour encodes state intent.
//! Drag gestures (drag-new-box, drag-wire) emit UserEvents
//! back through `out_events`.
//!
//! Skeleton — fills in as real records arrive via subscription.

use mentci_lib::canvas::flow_graph::FlowGraphView;
use mentci_lib::UserEvent;

pub fn paint(
    ui: &mut egui::Ui,
    _view: &FlowGraphView,
    _out_events: &mut Vec<UserEvent>,
) {
    ui.label("(flow-graph rendering lands as records flow in)");
}
