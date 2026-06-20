# mentci-egui

The first egui client for the **mentci** daemon.

The app speaks `signal-mentci` to `mentci-daemon`, renders typed
request/reply values as NOTA, and keeps approval state in the daemon.
Linux + Mac first-class.

See `ARCHITECTURE.md`.

Project intent: lore/INTENTION.md.
Project-wide architecture: criome/ARCHITECTURE.md.

## Status

**Daemon-connected slice.** The window runs, sends
`ObserveInterfaceState` to `mentci-daemon` over `signal-mentci`, and
renders the request/reply transcript as NOTA. Purpose-built panes grow from
that typed fallback.

## License

[License of Non-Authority](LICENSE.md).
