//! GitGraph 基础：分支、切换、标签、合并与 cherry-pick。
//!
//! 运行：`cargo run --example gitgraph_basic`
//! 产物：`examples/out/gitgraph_basic.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"gitGraph
    commit id: "init"
    branch develop
    checkout develop
    commit id: "feat-1"
    commit id: "feat-2" tag: "v0.1"
    checkout main
    commit id: "hotfix"
    merge develop id: "merge-1" tag: "v0.2"
    checkout develop
    cherry-pick id: "hotfix"
"#;

fn main() {
    support::svg("gitgraph_basic", DIAGRAM, 800, 700);
}
