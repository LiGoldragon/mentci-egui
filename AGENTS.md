# Agent Bootstrap — mentci-egui

The first incarnation of the mentci interaction surface — a
thin egui shell atop [mentci-lib](https://github.com/LiGoldragon/mentci-lib).

The library carries every piece of application logic
(workbench state, view snapshots, schema-aware action flows,
dual-daemon connection management). This crate **renders**
the snapshots and **forwards** gestures back. That's all.

Skeleton-as-design today; bodies are `todo!()`.

Read [ARCHITECTURE.md](ARCHITECTURE.md) for the shell's
responsibilities (and what stays in mentci-lib).

For project intent: [mentci/INTENTION.md](https://github.com/LiGoldragon/mentci/blob/main/INTENTION.md).
For project-wide rules: [mentci/AGENTS.md](https://github.com/LiGoldragon/mentci/blob/main/AGENTS.md).
For project-wide architecture: [criome/ARCHITECTURE.md](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md).

## Process

- Jujutsu only (`jj`).
- Push immediately after every change.
- Skeleton-as-design over prose-as-design.
- One artifact per repo.
- **Keep this shell thin.** When tempted to put logic here,
  put it in mentci-lib instead.
