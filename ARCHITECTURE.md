# ARCHITECTURE — mentci-egui

The first incarnation of the **mentci** interaction surface.
A thin egui shell atop [mentci-lib](https://github.com/LiGoldragon/mentci-lib).

## Role

mentci-lib is heavy; this shell is thin. The shell does three
things:

1. Wraps a [`mentci_lib::WorkbenchState`] and runs the
   `eframe::App` event loop.
2. Each frame: derives a `WorkbenchView` snapshot, paints it
   with egui widgets and Painter primitives.
3. Captures user gestures and forwards them as `UserEvent`s
   back to mentci-lib.

The shell never holds application state. State lives in
mentci-lib. The shell is rendering + input.

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
           │  state + view +     │
           │  update + cmds      │
           └──┬──────────────┬───┘
              │              │
       signal │              │ signal
              ▼              ▼
          criome      nexus-daemon
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

Does not own:

- Application logic (lives in mentci-lib).
- Schema knowledge (lives in mentci-lib).
- Connection state machines (lives in mentci-lib).
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
  [lore/INTENTION.md](https://github.com/LiGoldragon/lore/blob/main/INTENTION.md)
- Project-wide architecture:
  [criome/ARCHITECTURE.md](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md)
- The first design report:
  [mentci/reports/111](https://github.com/LiGoldragon/mentci/blob/main/reports/111-first-mentci-ui-introspection-2026-04-29.md)
- The library:
  [mentci-lib](https://github.com/LiGoldragon/mentci-lib)

## Status

**Skeleton-as-design.** First running window lands once
mentci-lib's `view`/`update` functions fill in.
