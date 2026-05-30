use liemermaid;

fn main() {
    // 测试各种节点形状（只测试当前支持的形状）
    let shapes = r#"flowchart TD
    A[Rectangle]
    B(Rounded)
    D[[Subroutine]]
    E((Circle))
    F{Diamond}
    G{{Hexagon}}
    H[/Parallelogram/]
    A --> B
    B --> D
    D --> E
    E --> F
    F --> G
    G --> H
"#;

    let svg = liemermaid::render(shapes, 1000, 1200).expect("render shapes");
    std::fs::write("flow_shapes.svg", svg).expect("write svg file");
    println!("flow_shapes.svg generated");

    // 测试不同方向的流程图
    let directions = r#"flowchart LR
    A[Start]
    B[Process]
    C[End]
    A --> B
    B --> C
"#;

    let svg = liemermaid::render(directions, 800, 300).expect("render directions");
    std::fs::write("flow_directions.svg", svg).expect("write svg file");
    println!("flow_directions.svg generated");

    // 测试分支结构
    let branch = r#"flowchart TD
    A[Start]
    B{Decision}
    C[Branch1]
    D[Branch2]
    E[End]
    A --> B
    B --> C
    B --> D
    C --> E
    D --> E
"#;

    let svg = liemermaid::render(branch, 800, 600).expect("render branch");
    std::fs::write("flow_branch.svg", svg).expect("write svg file");
    println!("flow_branch.svg generated");

    // 测试循环结构
    let loop_flow = r#"flowchart TD
    A[Start]
    B{Continue}
    C[Process]
    D[End]
    A --> B
    B -->|Yes| C
    C --> B
    B -->|No| D
"#;

    let svg = liemermaid::render(loop_flow, 600, 600).expect("render loop");
    std::fs::write("flow_loop.svg", svg).expect("write svg file");
    println!("flow_loop.svg generated");
}
