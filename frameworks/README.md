# frameworks

Incremental, clean-room reimplementations of the public API surface of
iOS frameworks — Foundation first, then a UIKit subset — built from
Apple's public developer documentation and driven by what real test apps
actually call. No Apple framework binaries are ever vendored here; see
[../docs/LEGAL.md](../docs/LEGAL.md).

Planned layout as coverage grows:

- `frameworks/foundation/` — `NSObject`, collections, strings, run loop,
  KVO/KVC.
- `frameworks/uikit/` — view hierarchy, layout, controls, event dispatch.

Status: not started. See [Phase 5-6](../docs/ROADMAP.md) and
[Architecture §5](../docs/ARCHITECTURE.md#5-framework-shims-frameworks).
