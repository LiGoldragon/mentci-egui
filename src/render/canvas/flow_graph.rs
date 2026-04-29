//! Paint a flow-graph view via egui's Painter API.
//!
//! Boxes-and-edges rendering. Glyph encodes node-kind; stroke
//! style encodes RelationKind; colour encodes state intent.
//! Drag gestures (drag-new-box, drag-wire) emit UserEvents
//! back through `out_events`.

use mentci_lib::canvas::flow_graph::{
    EdgeStateIntent, FlowGraphView, KindGlyph, NodeStateIntent, RenderedEdge, RenderedNode,
};
use mentci_lib::UserEvent;

const NODE_W: f32 = 120.0;
const NODE_H: f32 = 60.0;

pub fn paint(
    ui: &mut egui::Ui,
    view: &FlowGraphView,
    out_events: &mut Vec<UserEvent>,
) {
    // Title + add-node affordance at top.
    ui.horizontal(|ui| {
        ui.label(format!("graph: {}", view.graph.title));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ node").clicked() {
                out_events.push(UserEvent::OpenNewNodeFlow);
            }
        });
    });
    ui.separator();

    // Allocate the full remaining rect for the canvas.
    let avail = ui.available_size();
    let (rect, response) =
        ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Background.
    painter.rect_filled(rect, 4.0, egui::Color32::from_gray(28));

    // Edges first (so they sit under the boxes).
    for edge in &view.edges {
        paint_edge(&painter, rect, edge, &view.nodes);
    }

    // Then nodes.
    for node in &view.nodes {
        paint_node(&painter, rect, node);
    }

    // Hint text when empty.
    if view.nodes.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "(this graph has no member records yet)",
            egui::FontId::proportional(14.0),
            egui::Color32::from_gray(140),
        );
    }

    // Click on empty canvas selects nothing (future: drag to
    // new-box flow).
    let _ = response;
    let _ = out_events;
}

fn paint_node(painter: &egui::Painter, canvas: egui::Rect, node: &RenderedNode) {
    let pos = canvas.left_top() + egui::vec2(node.at.0, node.at.1);
    let r = egui::Rect::from_min_size(pos, egui::vec2(NODE_W, NODE_H));

    let (fill, stroke) = node_colours(node.state_intent);
    painter.rect(r, 6.0, fill, egui::Stroke::new(1.5, stroke));

    // Glyph + name on two lines.
    let glyph = glyph_char(node.kind_glyph);
    painter.text(
        r.left_top() + egui::vec2(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        glyph,
        egui::FontId::proportional(16.0),
        egui::Color32::from_gray(220),
    );
    painter.text(
        r.left_top() + egui::vec2(28.0, 8.0),
        egui::Align2::LEFT_TOP,
        &node.display_name,
        egui::FontId::proportional(13.0),
        egui::Color32::from_gray(220),
    );
}

fn paint_edge(
    painter: &egui::Painter,
    canvas: egui::Rect,
    edge: &RenderedEdge,
    nodes: &[RenderedNode],
) {
    // Endpoint resolution: real sema slots since the wire grew
    // records-with-slots. Edges whose endpoints aren't in the
    // cached node set silently skip painting — the line would
    // have nowhere to go. (A future tweaks-pane toggle could
    // surface them as "outside-graph" stubs.)
    let from = match nodes.iter().find(|n| n.slot == edge.from) {
        Some(n) => n,
        None => return,
    };
    let to = match nodes.iter().find(|n| n.slot == edge.to) {
        Some(n) => n,
        None => return,
    };
    let p_from =
        canvas.left_top() + egui::vec2(from.at.0 + NODE_W / 2.0, from.at.1 + NODE_H / 2.0);
    let p_to =
        canvas.left_top() + egui::vec2(to.at.0 + NODE_W / 2.0, to.at.1 + NODE_H / 2.0);

    let stroke_colour = edge_colour(edge.state_intent);
    painter.line_segment([p_from, p_to], egui::Stroke::new(1.5, stroke_colour));

    // Tiny relation-kind label at the midpoint.
    let mid = p_from + (p_to - p_from) * 0.5;
    painter.text(
        mid,
        egui::Align2::CENTER_CENTER,
        format!("{:?}", edge.relation_intent),
        egui::FontId::proportional(10.0),
        egui::Color32::from_gray(170),
    );
}

fn node_colours(intent: NodeStateIntent) -> (egui::Color32, egui::Color32) {
    match intent {
        NodeStateIntent::Stable => (
            egui::Color32::from_gray(48),
            egui::Color32::from_gray(120),
        ),
        NodeStateIntent::Pending => (
            egui::Color32::from_rgb(60, 50, 30),
            egui::Color32::from_rgb(220, 180, 90),
        ),
        NodeStateIntent::Stale => (
            egui::Color32::from_rgb(40, 40, 50),
            egui::Color32::from_rgb(140, 140, 200),
        ),
        NodeStateIntent::Rejected => (
            egui::Color32::from_rgb(60, 30, 30),
            egui::Color32::from_rgb(220, 90, 90),
        ),
    }
}

fn edge_colour(intent: EdgeStateIntent) -> egui::Color32 {
    match intent {
        EdgeStateIntent::Stable => egui::Color32::from_gray(140),
        EdgeStateIntent::Pending => egui::Color32::from_rgb(220, 180, 90),
        EdgeStateIntent::Stale => egui::Color32::from_rgb(140, 140, 200),
        EdgeStateIntent::Rejected => egui::Color32::from_rgb(220, 90, 90),
    }
}

fn glyph_char(g: KindGlyph) -> &'static str {
    match g {
        KindGlyph::Source => "⊙",
        KindGlyph::Transformer => "⊡",
        KindGlyph::Sink => "⊠",
        KindGlyph::Junction => "⊕",
        KindGlyph::Supervisor => "▶",
        KindGlyph::Unknown => "○",
    }
}
