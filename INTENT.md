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

## Thin client of mentci-lib's ObservationModel

`mentci-egui` is the interactive egui client for the `mentci` daemon, and it is
a thin client of `mentci-lib`'s `ObservationModel`: it holds the model, feeds
typed daemon replies in as `EngineEvent`s, paints the model's `ObservationView`,
and renders unknown inner objects through `mentci-lib`'s NOTA-fallback renderer
before purpose-built panes exist (Spirit record `xlrk`). The shell owns no
approval logic, queue, or per-socket state of its own — that is the model's.

It renders the approval card for each pending question and feeds the human's
decision back through the model as `UserEvent::AnswerQuestion`. Because of the
daemon-routing decision (2026-06-21) the shell has NO criome connection: the
model emits the answer to the mentci daemon over the mentci socket, and the
daemon routes the verdict to criome by the parked `AuthorizationRequestSlot`.
The shell never opens a criome socket and never sees a criome verdict. If the
daemon's criome access is read-only, the mirrored access level reaches the shell
through the model, and the shell presents observation only — no answer controls.

It connects over the ordinary `signal-mentci` contract and exposes a root-like
meta mode for privileged `meta-signal-mentci` operations as that daemon surface
lands. The full client is long-lived and subscription-oriented; it should
receive daemon events as they arrive rather than feeling like the single
synchronous CLI path.

## Constraints

- Keep the shell thin. Application state, daemon connection state, and
  approval-flow logic live in `mentci`.
- The shell owns egui/eframe integration, rendering, and daemon request
  dispatch only.
- NOTA text is a human/client projection and acceptable GUI fallback for
  typed values without dedicated panes. Component communication stays
  typed binary through the daemon/client boundary.
- Socket paths are displayed with component-channel labels such as `Criome`
  and `MetaCriome`, or `Mentci` and `MetaMentci`, rather than generic
  ordinary/meta labels.

*Source statements live in Spirit, especially the Mentci approval
surface decision and the workspace thin-shell discipline captured in
the Mentci component intent.*
