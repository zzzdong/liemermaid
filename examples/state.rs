// State 渲染示例
//
// 运行：cargo run --example state
// 产物：examples/out/state.svg

use liemermaid::{render, MermaidParser};
use std::fs;

fn main() {
    // 本次新增的三种声明形式：
    //   state "描述" as s1       带描述的别名状态
    //   s2 : 一段描述            裸状态 + 描述
    //   s3                      裸状态
    let input = r#"stateDiagram-v2
    state "流量处理" as s1
    s2 : 等待输入
    s3
    s1 --> s2
    s2 --> s3 : done
    s3 --> s1 : retry
"#;

    let _diagram = MermaidParser::parse_mermaid(input).expect("parse failed");
    println!("解析成功");

    let svg = render(input, 700, 500).expect("render failed");
    fs::create_dir_all("examples/out").unwrap();
    fs::write("examples/out/state.svg", &svg).unwrap();
    println!("已写入 examples/out/state.svg ({} 字节)", svg.len());
}
