# liemermaid

A Mermaid diagram parser and renderer in pure Rust. Output is built on
[lievisual](https://crates.io/crates/lievisual)'s declarative scene IR (`Scene`)
with two backends: SVG and PNG (via `vello_cpu`). No JavaScript runtime required.

**Supported diagrams:** flowchart · sequence · class · state · ER · pie · gitgraph · timeline

## Quick start

```toml
[dependencies]
liemermaid = "0.1"
```

```rust
use liemermaid::{render, render_png};

// Render an SVG string
let svg = render("flowchart TD\n    A[Start] --> B[End]", 800, 600)?;

// Render PNG bytes
let png: Vec<u8> = render_png("pie\n    \"A\": 30\n    \"B\": 50", 600, 400)?;
```

**Canvas semantics** (same as official Mermaid): `width` / `height` are upper
bounds — oversized content is scaled down proportionally and never cropped;
content that fits is never scaled up. Use `render_with_config` /
`render_png_with_config` for custom background colors.

## CLI

```sh
cargo install liemermaid
liemermaid-cli -i diagram.mmd -o out.svg        # format inferred from extension
liemermaid-cli -i diagram.mmd -o out.png -W 1200 -H 800
```

## Online demo

A WebAssembly playground that renders Mermaid to SVG/PNG entirely in the browser:
[zzzdong.github.io/liemermaid](https://zzzdong.github.io/liemermaid/) (source in [`site/`](site)).

## Testing

```sh
cargo test
```

Covers parsing, syntax-compatibility regressions, layout quality (no
edge-through-node crossings, layer alignment, no overlap), SVG structure, PNG
encoding, pathological inputs, and structural comparison against official
mermaid-cli golden output (see `tests/golden/`).

## Known limitations

- Output is structurally close to official Mermaid but not yet pixel-perfect
  (official uses dagre plus a full theme system).
- `sequenceDiagram` `alt`/`par` blocks render only the first branch.
- flowchart `style` / `classDef` / `linkStyle` / `click` statements and
  classDiagram `note for X "..."` are safely skipped (no errors).
- CJK text renders correctly, but long CJK strings do not auto-wrap yet.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
