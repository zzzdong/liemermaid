//! 手绘（sketch）风格演示：本地 patch 的 lievisual 手绘 pass 接入 liemermaid 管线。
//!
//! ```text
//! cargo run --example sketch_demo
//! ```
//!
//! 产出（写在工作目录）：
//! - `sketch_flow_normal.svg` —— 普通渲染（开关前的行为，作对照）
//! - `sketch_flow.svg/.png`   —— 全量手绘 flowchart
//! - `sketch_flow_nodes.svg`  —— 选择性手绘：节点抖、连线与箭头主干保持笔直
//! - `sketch_seq.svg`         —— sequence（含激活条 / 备注 / 虚线返回消息）
//! - `sketch_pie.svg`         —— pie（排线扇区）

use liemermaid::{
    OutputConfig, SketchFillStyle, SketchKind, SketchKinds, SketchOptions, render,
    render_png_sketch, render_sketch, render_sketch_with_config,
};

const FLOWCHART: &str = r#"flowchart TD
    Start([Start]) --> Input[/Input/]
    Input --> Decision{OK?}
    Decision -->|yes| Process[Process]
    Decision -->|no| Retry[Retry]
    Retry --> Input
    subgraph Done[收尾]
        Process --> Report[(Report)]
    end
    Report --> End([End])
"#;

const SEQUENCE: &str = r#"sequenceDiagram
    participant U as User
    participant S as Server
    U->>+S: request
    S-->>-U: response
    note over S: handled
"#;

const PIE: &str = r#"pie title Pets
    "Dogs" : 386
    "Cats" : 250
    "Birds" : 90
"#;

fn write(name: &str, svg: &str) {
    std::fs::write(name, svg).expect("写文件失败");
    println!("  {name:<26} {:>7} 字节", svg.len());
}

fn main() {
    // 1) 基线：不开手绘。
    let normal = render(FLOWCHART, 800, 600).expect("render failed");
    write("sketch_flow_normal.svg", &normal);

    // 2) 全量手绘（SVG + PNG）。
    let sketch = render_sketch(FLOWCHART, 800, 600).expect("render_sketch failed");
    write("sketch_flow.svg", &sketch);
    let png = render_png_sketch(FLOWCHART, 1000, 750).expect("render_png_sketch failed");
    std::fs::write("sketch_flow.png", &png).expect("写 PNG 失败");
    println!("  {:<26} {:>7} 字节", "sketch_flow.png", png.len());

    // 3) 选择性：排除 Path（连线曲线）与 Line（箭头细线）——节点抖、边保持笔直。
    let nodes_only = render_sketch_with_config(
        FLOWCHART,
        &OutputConfig {
            width: Some(800.0),
            height: Some(600.0),
            ..OutputConfig::default()
        },
        &SketchOptions::new()
            .with_seed(42)
            .with_fill(SketchFillStyle::Hachure)
            .with_kinds(
                SketchKinds::ALL
                    .without(SketchKind::Path)
                    .without(SketchKind::Line),
            ),
    )
    .expect("selective sketch failed");
    write("sketch_flow_nodes.svg", &nodes_only);

    // 4) 其它图型：sequence / pie。
    write(
        "sketch_seq.svg",
        &render_sketch(SEQUENCE, 800, 500).expect("seq sketch failed"),
    );
    write(
        "sketch_pie.svg",
        &render_sketch(PIE, 500, 400).expect("pie sketch failed"),
    );
    let seq_png = render_png_sketch(SEQUENCE, 800, 500).expect("seq png failed");
    std::fs::write("sketch_seq.png", &seq_png).expect("写 PNG 失败");
    let pie_png = render_png_sketch(PIE, 500, 400).expect("pie png failed");
    std::fs::write("sketch_pie.png", &pie_png).expect("写 PNG 失败");
    println!("  sketch_seq.png / sketch_pie.png 已生成");

    println!("\n说明：render()/render_png() 行为不变；手绘只在 render_sketch* 路径生效。");
}
