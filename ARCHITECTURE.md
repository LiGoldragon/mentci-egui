# ARCHITECTURE — mentci-egui

The egui client for the **Mentci** daemon. It is an interactive
`signal-mentci` client, not an independent workbench runtime.

## Role

`mentci-egui` opens a window, connects to `mentci-daemon`, sends typed
`signal-mentci` requests, and renders daemon replies. The daemon owns
approval state, subscriptions, persistence, and the criome bridge.

The first UI surface is intentionally generic: it shows request and reply
payloads as NOTA so new daemon objects are visible before dedicated panes
exist.

## Boundaries

Owns:

- `eframe::App` lifecycle and egui rendering.
- A Unix-socket `signal-mentci` client for ordinary daemon requests.
- A visible meta lane for privileged operations as `meta-signal-mentci`
  grows a live socket surface.
- NOTA rendering for typed replies without dedicated widgets.

Does not own:

- Approval logic.
- Criome authorization decisions.
- Daemon lifecycle, persistence, subscriptions, or notification fan-out.
- Direct criome or nexus driver connections.
- The `signal-mentci` or `meta-signal-mentci` wire vocabulary.

## Code Map

```
src/
├── main.rs           — entry; constructs tokio runtime and eframe window
├── app.rs            — daemon-first egui app and transcript view
├── daemon_client.rs  — synchronous `signal-mentci` Unix-socket client
├── error.rs          — Error enum
└── lib.rs            — testable public client modules
```

## Runtime Flow

On first frame the app sends `ObserveInterfaceState` to the configured
ordinary Mentci socket. Replies are rendered in the transcript as NOTA.
The `observe` button repeats the request. The `meta` button records the
current meta-mode placeholder until the daemon exposes a live meta channel.

Socket paths come from:

- `MENTCI_SOCKET`, defaulting to `$XDG_RUNTIME_DIR/mentci.socket` or
  `/tmp/mentci.socket`.
- `MENTCI_META_SOCKET`, defaulting to `$XDG_RUNTIME_DIR/mentci-meta.socket`
  or `/tmp/mentci-meta.socket`.

The UI labels socket paths by component and authority channel. The Mentci
ordinary socket is `Mentci`; the Mentci meta socket is `MetaMentci`. Future
component sockets follow the same shape, for example `Criome` and
`MetaCriome`.

## Status

The old `mentci-lib` direct criome/nexus driver workbench has been removed. The
app is now a thin client of `mentci-lib`'s `ObservationModel`: it holds the
model, feeds daemon replies in as `EngineEvent`s, paints the `ObservationView`,
and renders the approval card. The human's decision goes back through
`UserEvent::AnswerQuestion`; the model emits it to the mentci daemon, which
routes the verdict to criome by the parked `AuthorizationRequestSlot`. The shell
holds no criome connection and never sees a criome verdict — daemon-routing,
decided 2026-06-21.
