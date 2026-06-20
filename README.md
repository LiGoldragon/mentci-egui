# mentci-egui

The first incarnation of the **mentci** interaction surface —
the introspection workbench for criome.

A thin egui shell atop `mentci-lib`.
The library carries every piece of application logic; this
crate renders what mentci-lib produces and forwards user
events back. Linux + Mac first-class.

See `ARCHITECTURE.md`.

Project intent: lore/INTENTION.md.
Project-wide architecture: criome/ARCHITECTURE.md.

## Status

**First daemon-connected slice.** The window runs, sends
`ObserveInterfaceState` to `mentci-daemon` over `signal-mentci`, and
renders the request/reply transcript as NOTA. Purpose-built panes grow
from that typed fallback.

## License

[License of Non-Authority](LICENSE.md).
