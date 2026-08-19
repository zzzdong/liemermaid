// Pie 渲染示例
//
// 运行：cargo run --example pie
// 产物：examples/out/pie.svg

use liemermaid::{render, MermaidParser};
use std::fs;

fn main() {
    // 本次新增语法：
    //   pie showData           同行写法显示具体数值
    //   pie\n showData         换行写法同样支持
    let input = r#"pie showData
    title 语言占比
    "Rust" : 40
    "Python" : 35
    "Go" : 25
"#;

    let _diagram = MermaidParser::parse_mermaid(input).expect("parse failed");
    println!("解析成功");

    let svg = render(input, 600, 500).expect("render failed");
    fs::create_dir_all("examples/out").unwrap();
    fs::write("examples/out/pie.svg", &svg).unwrap();
    println!("已写入 examples/out/pie.svg ({} 字节)", svg.len());
}
