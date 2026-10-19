//! Pie 基础：`showData`、标题、图例与百分比。
//!
//! 运行：`cargo run --example pie_basic`
//! 产物：`examples/out/pie_basic.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"pie showData
    title 语言使用占比
    "Rust" : 45
    "TypeScript" : 30
    "Go" : 15
    "其他" : 10
"#;

fn main() {
    support::svg("pie_basic", DIAGRAM, 800, 500);
}
