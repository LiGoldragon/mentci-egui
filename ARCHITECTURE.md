# ARCHITECTURE — mentci-egui

The egui client for the **Mentci** daemon. It is an interactive
`signal-mentci` client, not an independent workbench runtime.

## Role

`mentci-egui` opens a window, connects to `mentci-daemon`, sends typed
`signal-mentci` requests, and renders daemon replies. The daemon owns shared
approval state, subscriptions, persistence, and the criome bridge. The client
owns its own view state, presentation state, selected question, transcript
focus, and remote-control policy.

The first UI surface is intentionally generic: it shows request and reply
payloads as NOTA so new daemon objects are visible before dedicated panes
exist.

## Boundaries

Owns:

- `eframe::App` lifecycle and egui rendering.
- A Unix-socket `signal-mentci` client for ordinary daemon requests.
- A Unix-socket `signal-mentci-client` control endpoint for driving this client
  instance remotely.
- A Unix-socket `meta-signal-mentci-client` control endpoint for configuring
  remote-control policy.
- A visible meta lane for privileged operations as `meta-signal-mentci`
  grows a live socket surface.
- NOTA rendering for typed replies without dedicated widgets.
- Following the operating-system light/dark preference: a
  `SystemThemeFollower` reads the desktop colour-scheme (via the desktop
  portal) and mirrors it into egui's visuals, re-probing on a coarse timer
  so a live system theme flip is picked up. The OS preference is the source
  of truth; the shell holds no theme of its own. Light is the fallback when
  the desktop reports no preference.

Does not own:

- Approval logic.
- Criome authorization decisions.
- Daemon lifecycle, persistence, subscriptions, or notification fan-out.
- Direct criome or nexus driver connections.
- The `signal-mentci` or `meta-signal-mentci` wire vocabulary.
- The `signal-mentci-client` or `meta-signal-mentci-client` control
  vocabularies.

## Code Map

```
src/
├── main.rs           — entry; constructs tokio runtime and eframe window
├── app.rs            — daemon-first egui app and transcript view
├── control.rs        — `signal-mentci-client` and `meta-signal-mentci-client`
│                       Unix-socket control endpoints
├── daemon_client.rs  — synchronous `signal-mentci` Unix-socket client
├── error.rs          — Error enum
└── lib.rs            — testable public client modules
```

## Runtime Flow

On first frame the app sends `ObserveInterfaceState` to the configured
ordinary Mentci socket. Replies are rendered in the transcript as NOTA.
The `observe` button repeats the request. Remote controllers send
`signal-mentci-client::Input` frames to the ordinary control socket; the app
maps them to the same `mentci-lib::UserEvent` path used by egui controls.
Controllers use the meta control socket for `Configure`, `SetRemoteControl`,
and `ResetRemoteControl`.

Socket paths come from:

- `MENTCI_SOCKET`, defaulting to `$XDG_RUNTIME_DIR/mentci.socket` or
  `/tmp/mentci.socket`.
- `MENTCI_META_SOCKET`, defaulting to `$XDG_RUNTIME_DIR/mentci-meta.socket`
  or `/tmp/mentci-meta.socket`.
- `MENTCI_EGUI_CONTROL_SOCKET`, defaulting to
  `$XDG_RUNTIME_DIR/mentci-egui.socket` or `/tmp/mentci-egui.socket`.
- `MENTCI_EGUI_META_CONTROL_SOCKET`, defaulting to
  `$XDG_RUNTIME_DIR/mentci-egui-meta.socket` or
  `/tmp/mentci-egui-meta.socket`.

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
decided 2026-06-21. The old hand-rolled remote-control NOTA socket has been
removed; ordinary drive is now binary `signal-frame` over the generated
`signal-mentci-client` contract, policy is binary `signal-frame` over
`meta-signal-mentci-client`, and NOTA appears only at the CLI text edge.
