//! Constructor flow rendering — modal/in-place editors per
//! verb-flow.

use mentci_lib::constructor::ConstructorView;
use mentci_lib::event::ConstructorField;
use mentci_lib::UserEvent;

pub fn constructor(
    ctx: &egui::Context,
    view: &ConstructorView,
    out_events: &mut Vec<UserEvent>,
) {
    egui::Window::new(title_for(view))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| match view {
            ConstructorView::NewNode(v) => render_new_node(ui, v, out_events),
            ConstructorView::NewEdge(_)
            | ConstructorView::Rename(_)
            | ConstructorView::Retract(_)
            | ConstructorView::Batch(_) => {
                ui.label("(this constructor flow lands in a later iteration)");
                if ui.button("close").clicked() {
                    out_events.push(UserEvent::ConstructorCancel);
                }
            }
        });
}

fn title_for(view: &ConstructorView) -> &'static str {
    match view {
        ConstructorView::NewNode(_) => "new node",
        ConstructorView::NewEdge(_) => "new edge",
        ConstructorView::Rename(_) => "rename",
        ConstructorView::Retract(_) => "retract",
        ConstructorView::Batch(_) => "batch",
    }
}

fn render_new_node(
    ui: &mut egui::Ui,
    v: &mentci_lib::constructor::NewNodeView,
    out_events: &mut Vec<UserEvent>,
) {
    ui.label("kind:");
    ui.horizontal(|ui| {
        for k in &v.kind_choices {
            let selected = v.kind_choice.as_deref() == Some(k);
            if ui.selectable_label(selected, k).clicked() {
                out_events.push(UserEvent::ConstructorFieldChanged {
                    field: ConstructorField::EnumChoice {
                        field_name: "kind".to_string(),
                        variant: k.clone(),
                    },
                });
            }
        }
    });

    ui.add_space(8.0);
    ui.label("name:");
    let mut name = v.display_name_input.clone();
    if ui.text_edit_singleline(&mut name).changed() {
        out_events.push(UserEvent::ConstructorFieldChanged {
            field: ConstructorField::Text {
                field_name: "name".to_string(),
                value: name,
            },
        });
    }

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("cancel").clicked() {
            out_events.push(UserEvent::ConstructorCancel);
        }
        ui.add_space(8.0);
        let commit = ui.add_enabled(v.commit_enabled, egui::Button::new("commit"));
        if commit.clicked() {
            out_events.push(UserEvent::ConstructorCommit);
        }
    });
}
