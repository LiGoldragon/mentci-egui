//! One-shot CLI for the `mentci-egui` remote-control socket.

use mentci_egui::control::{GuiControlClient, GuiControlEndpoint, GuiControlInput};

struct ControlInvocation {
    input: GuiControlInput,
}

impl ControlInvocation {
    fn from_arguments() -> mentci_egui::error::Result<Self> {
        let argument = std::env::args().nth(1).unwrap_or_else(|| {
            panic!("usage: mentci-egui-control '<GuiControlInput NOTA>'");
        });
        Ok(Self {
            input: GuiControlInput::from_nota(&argument)?,
        })
    }

    fn run(self) -> mentci_egui::error::Result<()> {
        let reply =
            GuiControlClient::new(GuiControlEndpoint::from_environment()).submit(self.input)?;
        println!("{reply}");
        Ok(())
    }
}

fn main() -> mentci_egui::error::Result<()> {
    ControlInvocation::from_arguments()?.run()
}
