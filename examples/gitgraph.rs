// GitGraph 渲染示例
//
// 运行：cargo run --example gitgraph
// 产物：examples/out/gitgraph.svg

use liemermaid::{render, MermaidParser};
use std::fs;

fn main() {
    // 本次新增语法：
    //   gitGraph LR:                方向 (LR / TB / BT)
    //   commit id: "c1" type: ... tag: "v1.0"   commit 属性
    //   checkout / switch <branch>  切换分支
    //   merge <branch> id: ... tag: ...         合并属性
    //   cherry-pick id: "c1" <parent>           拣选提交
    let input = r#"gitGraph LR:
    commit id: "c1" type: HIGHLIGHT tag: "v1.0"
    commit
    branch develop
    checkout develop
    commit id: "c2"
    checkout main
    switch develop
    commit
    merge develop id: "m1" tag: "merge-tag"
    cherry-pick id: "c1"
"#;

    let _diagram = MermaidParser::parse_mermaid(input).expect("parse failed");
    println!("解析成功");

    let svg = render(input, 900, 500).expect("render failed");
    fs::create_dir_all("examples/out").unwrap();
    fs::write("examples/out/gitgraph.svg", &svg).unwrap();
    println!("已写入 examples/out/gitgraph.svg ({} 字节)", svg.len());
}
