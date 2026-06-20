# ARCHITECTURE — mentci-egui

The first egui client for the **Mentci** interaction surface. A thin
shell atop mentci-lib and, once the daemon exists, a subscriber of
daemon-owned Mentci state.

## Role

mentci-lib is heavy; this shell is thin. The shell does three
things:

1. Wraps a [`mentci_lib::WorkbenchState`] and runs the
   `eframe::App` event loop.
2. Each frame: derives a `WorkbenchView` snapshot, paints it
   with egui widgets and Painter primitives.
3. Captures user gestures and forwards them as `UserEvent`s
   back to mentci-lib.

The shell never owns application state, wire vocabulary, or durable
state. Shared state-machine logic lives in mentci-lib; canonical
runtime state will live in the future `mentci` daemon, whose component
triad is `mentci` + `signal-mentci` + `meta-signal-mentci`. This shell
is rendering + input.

```
                    user
                     │
                     │ gestures
                     ▼
           ┌─────────────────────┐
           │   mentci-egui (this)│
           │                     │
           │  eframe::App +      │
           │  egui rendering     │
           │  per pane           │
           └────────┬────────────┘
                    │ UserEvent
                    ▼
           ┌─────────────────────┐
           │     mentci-lib      │
           │                     │
           │  shared state +     │
           │  view + update +    │
           │  cmds               │
           └────────┬────────────┘
                    │ daemon/client protocol
                    ▼
           ┌─────────────────────┐
           │ mentci component    │
           │                     │
           │ mentci daemon       │
           │ signal-mentci       │
           │ meta-signal-mentci  │
           └────────┬────────────┘
                    │
                    ▼
                 criome
```

## Boundaries

Owns:

- `eframe::App` impl wrapping mentci-lib state.
- Per-pane render functions that paint a `WorkbenchView` with
  egui widgets and the Painter API.
- Custom canvas painting (flow-graph rendering today; future
  kinds add render functions in `src/render/canvas/`).
- Gesture capture → `UserEvent` translation.
- Cmd dispatch — when mentci-lib returns `Cmd::SendCriome`
  etc., this shell sends the actual signal frame on the
  socket.
- A daemon transcript panel that sends real `signal-mentci` frames to
  `mentci-daemon` and renders request/reply payloads as NOTA while
  purpose-built panes are still growing.

Does not own:

- Application logic (lives in mentci-lib).
- Schema knowledge (lives in mentci-lib).
- Connection state machines (lives in mentci-lib).
- Mentci wire contracts (future `signal-mentci` and
  `meta-signal-mentci`).
- Daemon lifecycle, sockets, persistence, and key-unlock flow (future
  `mentci` daemon).
- Theme record interpretation (lives in mentci-lib;
  produces semantic-intent values; this shell maps them to
  egui `Visuals`).

## Code map

```
src/
├── main.rs           — entry; constructs WorkbenchState, runs
│                       eframe loop
├── app.rs            — eframe::App impl: each frame derives
│                       view, dispatches paints, executes cmds
├── daemon_client.rs  — synchronous `signal-mentci` Unix-socket client
│                       used by the GUI transcript panel
├── error.rs          — Error enum
└── render/
    ├── mod.rs        — render dispatcher
    ├── workbench.rs  — top-level multi-pane layout
    ├── header.rs     — connection states + toggles
    ├── canvas/
    │   ├── mod.rs    — canvas dispatcher; pick renderer per
    │   │              CanvasView variant
    │   └── flow_graph.rs — flow-graph paint via Painter
    ├── inspector.rs  — inspector pane paint
    ├── diagnostics.rs — diagnostics pane paint
    ├── wire.rs       — wire pane paint
    └── constructor.rs — modal/in-place constructor flows
```

All bodies are `todo!()` skeleton-as-design.

## Cross-cutting context

- Project intent:
  lore/INTENTION.md
- Project-wide architecture:
  criome/ARCHITECTURE.md
- The first design report:
  workspace/reports/111
- The library:
  mentci-lib

## Status

**First daemon-connected GUI slice.** The crate compiles against the
current mentci-lib state model and now has a real `signal-mentci`
ordinary-mode client panel. It sends `ObserveInterfaceState` to a live
`mentci-daemon` socket and renders the reply as NOTA. Meta mode is
visible in the UI as the root-like lane, but the daemon does not yet
expose a live meta socket; startup configuration remains the binary
`meta-signal-mentci` file.
