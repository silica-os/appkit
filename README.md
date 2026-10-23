# AppKit

Silica's native system toolkit: a shared library (`libappkit.so`, C API `appkit.h`) that draws every
Silica 1.0 Amalfi component, talks directly to the compositor's Wayland protocols and Silica's
system services, and is used by the shell and every app through a safe Rust crate (`appkit-rs`) and
a declarative layer (`appkit-ui`). It is migrated from Vetro (Skia) onto its own renderer, written
in Rust only, so that it builds from source with the base OS.

## Layout

| Path | What |
| --- | --- |
| `crates/appkit` | Core, renderer, text, Amalfi controls, Wayland platform, services, a11y, C ABI |
| `crates/appkit-sys` | The C ABI list, `appkit.h` and the symbol map |
| `crates/appkit-rs` | Safe Rust crate over the C ABI |
| `crates/appkit-ui` | Declarative layer (views, `State`/`Binding`, reconciler) |
| `crates/appkit-test` | Pixel parity with the Amalfi kit renders (`silica-design`) |
| `crates/silica-protocols` | Silica's Wayland protocols (glass, window, menu, shell) |
| `examples/bench` | Frame-time budgets |
| `xtask` | `sync-sources`, `check-sources`, ABI checks |

## Renderer

`render::canvas::Canvas` draws on [vello_cpu](https://github.com/linebender/vello) with a few
additions:

- curves and strokes are flattened at 0.1 device pixels, finer than vello_cpu's quarter pixel;
- a frame rasterizes only its damage (`begin_region`): a flush costs the rows it covers;
- the raster cache's images are copied, not resampled, when they sit on whole pixels;
- glyph outlines are read once and cached (`render::glyphs`);
- backdrops (blur, saturation, glass refraction) are filtered on the CPU, in bands on threads.

Text is laid out by parley (fontique for fonts: Inter, JetBrains Mono and Silica Symbols are
bundled, the system's fonts join as fallback once loaded on a thread).

## Sources of truth

Tokens, motion curves and symbols come from `silica-os/ui-kit` and `silica-os/symbols`;
`cargo xtask sync-sources` copies them into `crates/appkit/data/` and `cargo xtask check-sources`
checks the copies.

## Gate

```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run --release -p appkit-bench   # exits non-zero over budget; --snapshot DIR writes PNGs
```

Bench on the development machine (release, single window):

| Scenario | AppKit | Vetro | Budget |
| --- | --- | --- | --- |
| First frame, 500 controls | 89 ms | 125 ms | 400 ms |
| Hover change | 0.16 ms | 0.14 ms | 8 ms |
| Typing, 10 MB document | 0.62 ms/key | 0.6 ms/key | 8 ms |
| Spinner beside 500 controls at 2×, damage only | 0.13 ms | — | 8.3 ms |
| Same, whole window repainted (raster cache) | 4.4 ms | 3.5 ms | 8.3 ms |
| Dock, 20 magnifying tiles on glass, 1920×1200 | 3.8 ms | — | 8.3 ms |
| Symbols grid, 975 glyphs at 2×, repaint | 9.6 ms | — | 16.7 ms |

Licensed under either of Apache-2.0 or MIT, at your option.
