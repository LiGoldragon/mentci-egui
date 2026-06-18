# INTENT - mentci-egui

*What the psyche has explicitly intended for this project.
Synthesised from Spirit records and applicable workspace constraints;
not embellished. `ARCHITECTURE.md` says what mentci-egui IS; this file
says what the psyche wants it to BE.*

## Purpose

`mentci-egui` is a thin egui shell over `mentci-lib`. It renders
`WorkbenchView` snapshots, captures gestures, executes library `Cmd`
values, and keeps application logic out of the GUI crate.

## Mentci Approval Surface

Mentci is becoming the psyche-facing criome approval component: a
daemon plus TUI/CLI approval surface for escalated criome questions.
`mentci-egui` is one possible client shell for that approval surface, not
the owner of the approval logic. The question queue, suggested answer,
context, and response state live in `mentci-lib`; this crate paints and
dispatches only.

## Constraints

- Keep the shell thin. Application state, constructor flows, daemon
  connection state, and approval-flow logic live in `mentci-lib`.
- The shell owns egui/eframe integration, rendering, gesture capture,
  and command execution only.
- NOTA text is a human/CLI projection. Component communication stays
  typed binary through the daemon/client boundary.

*Source statements live in Spirit, especially the Mentci approval
surface decision and the workspace thin-shell discipline captured in
`mentci-lib` intent.*
