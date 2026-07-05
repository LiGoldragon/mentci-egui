# mentci-egui

The first egui client for the **mentci** daemon.

The app speaks `signal-mentci` to `mentci-daemon`, renders typed
request/reply values as NOTA, keeps shared approval state in the daemon, and
exposes its client-local controls through `signal-mentci-client` plus
`meta-signal-mentci-client`.
Linux + Mac first-class.

See `ARCHITECTURE.md` for this client's direction and shape.

Project-wide architecture: criome/ARCHITECTURE.md.

## Status

**Daemon-connected slice.** The window runs, sends
`ObserveInterfaceState` to `mentci-daemon` over `signal-mentci`, and
renders the request/reply transcript as NOTA. Purpose-built panes grow from
that typed fallback.

The companion `mentci-egui-control` CLI accepts one `signal-mentci-client`
NOTA input, sends the generated binary signal frame to the GUI control socket,
and prints the typed reply as NOTA.

The companion `mentci-egui-meta-control` CLI does the same for
`meta-signal-mentci-client` policy inputs.

## License

[License of Non-Authority](LICENSE.md).
