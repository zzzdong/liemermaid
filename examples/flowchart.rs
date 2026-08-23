// Flowchart 渲染示例
//
// 运行：cargo run --example flowchart
// 产物：examples/out/flowchart.svg  (用浏览器打开即可查看渲染效果)
//
// 新增箭头类型演示：
//   ~~~   不可见连线
//   o--o  双向圆头（multi circle）
//   x--x  双向叉头（multi cross）
//   --o   单圆头
//   --x   单叉头
//   <->   双向箭头

use liemermaid::{render, MermaidParser};
use std::fs;

fn main() {
    let input = r#"flowchart TB
    A[Rectangle]
    B(Rounded)
    C([Stadium])
    D[[Subroutine]]
    E[(Database)]
    F((Circle))
    G{Diamond}
    H>Flag]
    I[/Sloped/]
    J((DoubleCircle))
"#;

    let _diagram = MermaidParser::parse_mermaid(input).expect("parse failed");
    println!("解析成功");

    let svg = render(input, 800, 600).expect("render failed");
    fs::create_dir_all("examples/out").unwrap();
    fs::write("examples/out/flowchart.svg", &svg).unwrap();
    println!("已写入 examples/out/flowchart.svg ({} 字节)", svg.len());
}
