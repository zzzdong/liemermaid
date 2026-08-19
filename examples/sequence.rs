// Sequence 渲染示例
//
// 运行：cargo run --example sequence
// 产物：examples/out/sequence.svg

use liemermaid::{render, MermaidParser};
use std::fs;

fn main() {
    // 本次新增语法：
    //   Alice ->>+ Bob    消息发送后激活 Bob (+)
    //   Bob -->>- Alice   消息返回时反激活 Bob (-)
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    Alice ->>+ Bob: 请求数据
    Bob -->>- Alice: 返回结果
    Alice ->> Bob: 感谢
"#;

    let _diagram = MermaidParser::parse_mermaid(input).expect("parse failed");
    println!("解析成功");

    let svg = render(input, 700, 500).expect("render failed");
    fs::create_dir_all("examples/out").unwrap();
    fs::write("examples/out/sequence.svg", &svg).unwrap();
    println!("已写入 examples/out/sequence.svg ({} 字节)", svg.len());
}
