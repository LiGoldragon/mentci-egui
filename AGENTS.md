# Agent instructions — mentci-egui

You **MUST** read AGENTS.md at `github:ligoldragon/lore` — the workspace contract.

## Repo role

The egui client for the mentci daemon.

The daemon carries approval state, criome bridge logic, subscriptions, and
wire vocabulary. This crate opens a window, sends typed `signal-mentci`
requests, and renders daemon replies.

---

## Carve-outs worth knowing

- **Keep this shell thin.** When tempted to put approval or state logic here,
  put it in the mentci daemon instead.
