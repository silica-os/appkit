# AppKit: architecture

AppKit is one Rust library with three faces: the C ABI (`libappkit.so`, `appkit.h`), a safe Rust
crate over it (`appkit-rs`) and a declarative layer (`appkit-ui`). The shell and every Silica app
use it; it builds from source with the base OS (`silica-os/system`, `appkit.bst`).

```
appkit-ui (components, State/Binding, reconciler)
   │
appkit-rs (safe objects) ── C ABI (ffi, appkit-sys: for_each_function!, appkit.h, appkit.map)
   │
appkit
 ├─ core       view arena, layout, events, responder chain, animations, damage, a11y, menus,
 │             restoration, raster cache
 ├─ amalfi     the controls, one module per kit family, drawn as named kit layers
 ├─ text       parley layout, fonts (bundled + fontconfig on a thread), editor, TextView
 ├─ render     Canvas on vello_cpu, glyph outlines (skrifa), backdrops, images, paths
 ├─ platform   Wayland (SCTK), Silica protocols, AccessKit, inspector socket
 └─ services   zbus proxies of Silica's daemons and the portals
```

## A frame

1. Input or a timer changes view state; changed views mark damage (`core`).
2. Layout runs where sizes changed (proposal layout: stacks, grids, overlays).
3. Each damaged view records its display list: named kit layers (`Surface`, `Label`,
   `Focus Halo` …) with kit styles. The same list is read back by the parity tests and the
   inspector.
4. The platform opens a canvas region over the damage (`Canvas::begin_region`), paints the
   lists (static subtrees come from the raster cache), and copies the region into the wl_shm
   buffer, then sends `damage_buffer` and commits on the frame callback.

## Renderer

`render::canvas::Canvas` keeps vello_cpu's API small: paths, rects, rounded rects, gradients,
images, glyph runs, clips, layers and backdrops.

- **Precision.** Curves and strokes are flattened at 0.1 device pixels before vello sees them.
- **Damage.** In a region, device space is shifted by the region's origin and only the rows
  that were drawn are flushed.
- **Text.** Glyphs come from an outline cache (`render::glyphs`, keyed by font, glyph and
  variation coordinates), filled as paths. Colour fonts go through vello's glyph runs.
- **Backdrops.** Blur, saturation and the glass refraction profile are filtered on the CPU,
  in row bands on threads, without libm calls in the pixel loops.
- **Blits.** Large copies are split across threads.

A GPU backend on wgpu (`render::gpu`: Vulkan, GLES as the fallback) implements the same
`render::Painter` trait with the same formulas: stencil-and-cover paths on a 4x multisampled
frame, shaders for gradients, images, analytic shadows and the backdrop (blur, saturation,
refraction), clip masks rendered for the region of each draw, and a retained frame texture with
damage tracking. Glyphs, path coverages, clips and shadows are cached in atlases, and static
subtrees and layers are kept as textures. On Wayland a window presents through a wgpu surface
(`render::gpu::present`); `APPKIT_RENDERER=cpu` keeps the CPU canvas, `gl` or `vulkan` picks
the backend, and Venus comes after the other adapters. `GpuContext::new()` is `None` without an
adapter; the CPU canvas then stays, and it always makes the snapshots and previews. The
agreement tests (`render/gpu/tests.rs`) render the same scenes on both canvases.

## Sources of truth

| What | Where | Copy |
| --- | --- | --- |
| Tokens, motion curves | `silica-os/ui-kit` (`tokens/`, `rust/silica_motion.rs`) | `crates/appkit/data/` |
| Symbols | `silica-os/symbols` (fonts, `codepoints.json`) | `crates/appkit/data/`, `crates/appkit/fonts/` |
| Component behaviour | ui-kit `src/lib/S*.vue` (states, transitions, reduced motion) | ported by hand |
| Kit renders for parity | `silica-os/silica-design` (`kit/`) | read by `appkit-test` |

`cargo xtask sync-sources` refreshes the copies and `cargo xtask check-sources` fails when they
drift.

## Tests

- Unit tests next to the code (layout, events, editor, canvas).
- Parity: `crates/appkit-test/tests/<family>.rs` renders every kit master of a family and
  compares geometry exactly and pixels within a named tolerance.
- FFI round trips (`appkit-rs`), the reconciler (`appkit-ui`).
- `examples/bench`: frame budgets; exits non-zero over budget.
- VM: `tools/test-vm --desktop --appkit` in `silica-os/system`.
