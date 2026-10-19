//! API：只解析为 AST（不渲染），并按图表类型打印结构摘要。
//!
//! 运行：`cargo run --example api_parse_ast`

use liemermaid::{Diagram, MermaidParser};

const FLOWCHART: &str = r#"flowchart TD
    A[Start] --> B{Check}
    B -->|yes| C[End]
    B -->|no| A
"#;

const SEQUENCE: &str = r#"sequenceDiagram
    Alice->>Bob: hello
    Bob-->>Alice: hi
"#;

fn summarize(source: &str) {
    match MermaidParser::parse_mermaid(source) {
        Ok(Diagram::Flowchart(fc)) => {
            println!(
                "flowchart: direction={:?}, nodes={}, edges={}, subgraphs={}",
                fc.direction,
                fc.nodes.len(),
                fc.edges.len(),
                fc.subgraphs.len()
            );
            for e in &fc.edges {
                println!("  edge {} -> {} label={:?}", e.source, e.target, e.label);
            }
        }
        Ok(Diagram::Sequence(sd)) => {
            println!(
                "sequence: participants={}, statements={}",
                sd.participants.len(),
                sd.statements.len()
            );
        }
        Ok(other) => println!("parsed diagram: {other:?}"),
        Err(e) => println!("parse error: {e}"),
    }
}

fn main() {
    summarize(FLOWCHART);
    summarize(SEQUENCE);

    // 错误处理：未知图表类型返回 Err，而不是 panic。
    summarize("not a mermaid diagram");
}
