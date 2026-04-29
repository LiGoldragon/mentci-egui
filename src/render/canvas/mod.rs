//! Canvas paint dispatcher.
//!
//! mentci-lib produces a `CanvasView` whose variant determines
//! which renderer paints. Each renderer lives in its own
//! submodule.

use mentci_lib::canvas::CanvasView;
use mentci_lib::UserEvent;

pub mod flow_graph;

pub fn canvas(
    ui: &mut egui::Ui,
    view: &CanvasView,
    out_events: &mut Vec<UserEvent>,
) {
    match view {
        CanvasView::Empty => {
            ui.centered_and_justified(|ui| {
                ui.label("(select a graph to view)");
            });
        }
        CanvasView::FlowGraph(flow_view) => {
            flow_graph::paint(ui, flow_view, out_events);
        }
    }
}
