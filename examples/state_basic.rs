//! State 基础：起止节点、转移标签与复合状态。
//!
//! 运行：`cargo run --example state_basic`
//! 产物：`examples/out/state_basic.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"stateDiagram-v2
    [*] --> Idle

    state Active {
        [*] --> Working
        Working --> Paused : pause
        Paused --> Working : resume
        Working --> [*]
    }

    Idle --> Active : start
    Active --> Done : complete
    Done --> [*]
"#;

fn main() {
    support::svg("state_basic", DIAGRAM, 900, 650);
}
