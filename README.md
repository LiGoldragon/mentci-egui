# mentci-egui

The first incarnation of the **mentci** interaction surface —
the introspection workbench for criome.

A thin egui shell atop `mentci-lib`.
The library carries every piece of application logic; this
crate renders what mentci-lib produces and forwards user
events back. Linux + Mac first-class.

See [`ARCHITECTURE.md`](ARCHITECTURE.md) and the design
report workspace/reports/111.

Project intent: lore/INTENTION.md.
Project-wide architecture: criome/ARCHITECTURE.md.

## Status

**Skeleton-as-design.** Type signatures pinned; bodies are
`todo!()`. The first running window lands as mentci-lib's
core fills in.

## License

[License of Non-Authority](LICENSE.md).
