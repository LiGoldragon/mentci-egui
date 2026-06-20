# INTENT - mentci-egui

*What the psyche has explicitly intended for this project.
Synthesised from Spirit records and applicable workspace constraints;
not embellished. `ARCHITECTURE.md` says what mentci-egui IS; this file
says what the psyche wants it to BE.*

## Purpose

`mentci-egui` is a thin egui client for the `mentci` daemon. It sends typed
`signal-mentci` requests, renders daemon replies, and keeps approval state and
application logic in the daemon.

## Mentci Approval Surface

Mentci is becoming the psyche-facing criome approval component: a
daemon-owned programmable UI surface for escalated criome questions.
The full component shape is the future `mentci` daemon repository plus
the `signal-mentci` and `meta-signal-mentci` contract repositories.
`mentci-egui` is one possible client shell for that surface, not the owner of
approval logic, wire vocabulary, daemon lifecycle, or durable state. The
question queue, suggested answer, context, response state, and subscription
model live in the Mentci daemon; this crate paints and dispatches only.

## Daemon-Connected GUI

Spirit record `xlrk` clarifies the revived GUI concept: `mentci-egui`
is the interactive client for the `mentci` daemon. It connects over the
ordinary `signal-mentci` contract, exposes a root-like meta mode for
privileged `meta-signal-mentci` operations as that daemon surface lands,
and renders typed replies or unknown inner objects through NOTA text as
the first fallback before purpose-built panes exist. The full client is
long-lived and subscription-oriented; it should receive daemon events as
they arrive rather than feeling like the single synchronous CLI path.

## Constraints

- Keep the shell thin. Application state, daemon connection state, and
  approval-flow logic live in `mentci`.
- The shell owns egui/eframe integration, rendering, and daemon request
  dispatch only.
- NOTA text is a human/client projection and acceptable GUI fallback for
  typed values without dedicated panes. Component communication stays
  typed binary through the daemon/client boundary.

*Source statements live in Spirit, especially the Mentci approval
surface decision and the workspace thin-shell discipline captured in
the Mentci component intent.*
