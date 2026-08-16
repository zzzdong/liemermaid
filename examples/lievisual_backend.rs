//! 演示用 lievisual 后端（SVG / vello_cpu PNG）渲染 Mermaid 图。
//!
//! liemermaid 的渲染统一走 lievisual：`render`（SVG）与 `render_png`（PNG）。
use liemermaid::{render, render_png};

fn main() {
    let flow = r#"flowchart TD
    A[Start]
    B{Decision}
    C[Yes]
    D[No]
    A --> B
    B -->|Yes| C
    B -->|No| D
"#;

    let svg = render(flow, 800, 600).expect("svg render failed");
    std::fs::write("lievisual_flow.svg", &svg).expect("write svg");
    println!("lievisual_flow.svg 生成 ({} 字节)", svg.len());

    let png = render_png(flow, 800, 600).expect("png render failed");
    std::fs::write("lievisual_flow.png", &png).expect("write png");
    println!("lievisual_flow.png 生成 ({} 字节)", png.len());
}
