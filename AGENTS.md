# Agent instructions — mentci-egui

You **MUST** read AGENTS.md at `github:ligoldragon/lore` — the workspace contract.

## Repo role

The first incarnation of the mentci interaction surface — a thin egui shell atop mentci-lib.

The library carries every piece of application logic (workbench state, view snapshots, schema-aware action flows, dual-daemon connection management). This crate **renders** the snapshots and **forwards** gestures back. That's all.

Skeleton-as-design today; bodies are `todo!()`.

---

## Carve-outs worth knowing

- **Keep this shell thin.** When tempted to put logic here, put it in mentci-lib instead.
