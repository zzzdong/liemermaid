//! 手绘风格输出：rough.js 质感的抖动描边 + 排线填充。
//!
//! 运行：`cargo run --example output_sketch`
//! 产物：`examples/out/sketch_flowchart.svg`、`sketch_flowchart.png`

#[path = "support/mod.rs"]
mod support;

use liemermaid::{OutputConfig, SketchOptions, render_png_sketch, render_sketch};

const DIAGRAM: &str = r#"flowchart TD
    A[Idea] --> B{Feasible?}
    B -->|yes| C[Prototype]
    B -->|no| D[Archive]
    C --> E[Ship]
"#;

fn main() {
    // 默认：rough.js 风格（roughness 0.7、hachure -41°）。
    let path = support::out_dir().join("sketch_flowchart.svg");
    let svg = render_sketch(DIAGRAM, 900, 600).expect("render sketch svg");
    std::fs::write(&path, svg).expect("write sketch svg");
    println!("SVG (sketch): {}", path.display());

    let path = support::out_dir().join("sketch_flowchart.png");
    let png = render_png_sketch(DIAGRAM, 900, 600).expect("render sketch png");
    std::fs::write(&path, png).expect("write sketch png");
    println!("PNG (sketch): {}", path.display());

    // 自定义：固定种子（同图同输出）；不调用 `with_fill` 即保持实底、只抖轮廓。
    let config = OutputConfig {
        width: Some(900.0),
        height: Some(600.0),
        ..OutputConfig::default()
    };
    let options = SketchOptions::new().with_seed(7);
    let path = support::out_dir().join("sketch_outline.svg");
    let svg = liemermaid::render_sketch_with_config(DIAGRAM, &config, &options)
        .expect("render sketch outline");
    std::fs::write(&path, svg).expect("write sketch outline");
    println!("SVG (outline only): {}", path.display());
}
