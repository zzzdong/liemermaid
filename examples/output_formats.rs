//! 输出格式：同一张图的 SVG、PNG 与透明背景 SVG。
//!
//! 运行：`cargo run --example output_formats`
//! 产物：`examples/out/output_svg.svg`、`output_png.png`、`output_transparent.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"flowchart LR
    A[Source] --> B{Check}
    B -->|ok| C[(Store)]
    B -->|fail| D[Retry]
    D --> A
"#;

fn main() {
    // SVG：width/height 是上限，内容不会被放大。
    support::svg("output_svg", DIAGRAM, 800, 500);

    // PNG：width/height 是目标尺寸（放大内容以提高位图分辨率）。
    support::png("output_png", DIAGRAM, 1200, 800);

    // 透明底：官方 mermaid 的默认视觉，适合深色页面或再合成。
    support::svg_transparent("output_transparent", DIAGRAM, 800, 500);
}
