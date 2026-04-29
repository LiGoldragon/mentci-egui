//! Wire pane — every signal frame seen on this connection,
//! at typed-variant level. User-toggled.

use mentci_lib::wire::WireView;
use mentci_lib::UserEvent;

pub fn wire(
    _ui: &mut egui::Ui,
    _view: &WireView,
    _out_events: &mut Vec<UserEvent>,
) {
    todo!()
}
