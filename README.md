# mentci-egui

The first incarnation of the **mentci** interaction surface —
the introspection workbench for criome.

A thin egui shell atop [`mentci-lib`](https://github.com/LiGoldragon/mentci-lib).
The library carries every piece of application logic; this
crate renders what mentci-lib produces and forwards user
events back. Linux + Mac first-class.

See [`ARCHITECTURE.md`](ARCHITECTURE.md) and the design
report [mentci/reports/111](https://github.com/LiGoldragon/mentci/blob/main/reports/111-first-mentci-ui-introspection-2026-04-29.md).

Project intent: [lore/INTENTION.md](https://github.com/LiGoldragon/lore/blob/main/INTENTION.md).
Project-wide architecture: [criome/ARCHITECTURE.md](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md).

## Status

**Skeleton-as-design.** Type signatures pinned; bodies are
`todo!()`. The first running window lands as mentci-lib's
core fills in.

## License

[License of Non-Authority](LICENSE.md).
