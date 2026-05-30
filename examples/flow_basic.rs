use liemermaid;

fn main() {
    // 基础流程图示例
    let flow = r#"flowchart TD
    A[Start]
    B[Process]
    C[Decision]
    D[Action]
    E[End]
    A --> B
    B --> C
    C --> D
    D --> E
"#;

    let svg = liemermaid::render(flow, 800, 600).expect("render flowchart");
    std::fs::write("flow_basic.svg", svg).expect("write svg file");
    println!("flow_basic.svg generated");
}