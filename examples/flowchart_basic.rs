//! Flowchart 基础：形状、带空格标签、判定分支与方向。
//!
//! 运行：`cargo run --example flowchart_basic`
//! 产物：`examples/out/flowchart_basic.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"flowchart TD
    Start([开始]) --> Input[/接收请求/]
    Input --> Check{参数合法?}
    Check -->|是| Process[处理业务]
    Check -->|否| Reject[返回 400]
    Process --> Save[(写入数据库)]
    Save --> Done([结束])
    Reject --> Done
"#;

fn main() {
    support::svg("flowchart_basic", DIAGRAM, 900, 600);
}
