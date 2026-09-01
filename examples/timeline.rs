// Timeline 渲染示例
//
// 运行：cargo run --example timeline
// 产物：examples/out/timeline.svg

use liemermaid::{MermaidParser, render};
use std::fs;

fn main() {
    // 本次新增语法：
    //   timeline LR           方向 (TD / LR)
    //   1950 : A : B          同一时间点包含多个事件（冒号分隔）
    let input = r#"timeline
    title Project Roadmap
    2024 : Design : Prototype
    2025 : Launch : Iterate : Feedback
    2026 : Scale : Optimize : GA
"#;

    let _diagram = MermaidParser::parse_mermaid(input).expect("parse failed");
    println!("解析成功");

    let svg = render(input, 900, 400).expect("render failed");
    fs::create_dir_all("examples/out").unwrap();
    fs::write("examples/out/timeline.svg", &svg).unwrap();
    println!("已写入 examples/out/timeline.svg ({} 字节)", svg.len());
}
