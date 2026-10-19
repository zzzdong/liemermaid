//! Timeline 基础：标题、section 与同一时间点的多事件。
//!
//! 运行：`cargo run --example timeline_basic`
//! 产物：`examples/out/timeline_basic.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"timeline
    title 项目里程碑
    section 启动
        2024 : 立项 : 团队组建
    section 开发
        2025 : 内核完成 : 内部预览
    section 发布
        2026 : 1.0 发布
"#;

fn main() {
    support::svg("timeline_basic", DIAGRAM, 1100, 450);
}
