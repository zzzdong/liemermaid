use liemermaid;

fn main() {
    // 1. 所有节点形状
    let shapes = r#"flowchart TD
    A[Rectangle]
    B(Rounded)
    C[[Subroutine]]
    D([Stadium])
    E[(Database)]
    F((Circle))
    G(((DoubleCircle)))
    H>Asymmetric]
    I{Diamond}
    J{{Hexagon}}
    K[/Parallelogram/]
    L[\ParallelogramAlt\]
    M[/Trapezoid\]
    N[\TrapezoidAlt/]
    A --> B
    B --> C
    C --> D
    D --> E
    E --> F
    F --> G
    G --> H
    H --> I
    I --> J
    J --> K
    K --> L
    L --> M
    M --> N
"#;
    let svg = liemermaid::render(shapes, 1000, 1500).expect("render shapes");
    std::fs::write("flow_shapes.svg", svg).expect("write svg file");
    println!("flow_shapes.svg generated");

    // 2. 全部方向
    for (name, dir, w, h) in [
        ("flow_lr", "LR", 900, 300),
        ("flow_td", "TD", 600, 500),
        ("flow_bt", "BT", 600, 500),
        ("flow_rl", "RL", 900, 300),
    ] {
        let dsl = format!(r#"flowchart {}
    A[Start]
    B[Process]
    C[Decision]
    D[Action]
    E[End]
    A --> B
    B --> C
    C --> D
    D --> E
"#, dir);
        let svg = liemermaid::render(&dsl, w, h).expect(&format!("render {}", dir));
        let path = format!("{}.svg", name);
        std::fs::write(&path, svg).expect("write svg file");
        println!("{}.svg generated", name);
    }

    // 3. 边类型
    let edges = r#"flowchart TD
    A[Arrow]
    B[Thick]
    C[Dotted]
    D[Labeled]
    E[Merge]
    A --> B
    B ==> C
    C -.-> D
    D -->|"YES"| E
"#;
    let svg = liemermaid::render(edges, 700, 700).expect("render edges");
    std::fs::write("flow_edges.svg", svg).expect("write svg file");
    println!("flow_edges.svg generated");

    // 4. 分支结构
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

    // 5. 循环结构
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
    let svg = liemermaid::render(loop_flow, 800, 600).expect("render loop");
    std::fs::write("flow_loop.svg", svg).expect("write svg file");
    println!("flow_loop.svg generated");

    // 6. 长链
    let chain = r#"flowchart LR
    A[Step1]
    B[Step2]
    C[Step3]
    D[Step4]
    E[Step5]
    F[Step6]
    G[Done]
    A --> B
    B --> C
    C --> D
    D --> E
    E --> F
    F --> G
"#;
    let svg = liemermaid::render(chain, 1400, 250).expect("render chain");
    std::fs::write("flow_chain.svg", svg).expect("write svg file");
    println!("flow_chain.svg generated");

    // 7. 多重分支
    let multi = r#"flowchart TD
    A[Start]
    B{Check}
    C[Fast]
    D[Medium]
    E[Slow]
    F[End]
    A --> B
    B -->|"<10"| C
    B -->|"10-100"| D
    B -->|">100"| E
    C --> F
    D --> F
    E --> F
"#;
    let svg = liemermaid::render(multi, 800, 600).expect("render multi branch");
    std::fs::write("flow_multi_branch.svg", svg).expect("write svg file");
    println!("flow_multi_branch.svg generated");

    // 8. 简单链 (来自 flow_basic)
    let basic = r#"flowchart TD
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
    let svg = liemermaid::render(basic, 800, 600).expect("render basic");
    std::fs::write("flow_basic.svg", svg).expect("write svg file");
    println!("flow_basic.svg generated");

    // 9. 饼图 (来自 flow_basic)
    let pie = r#"pie
    title Quarterly Revenue
    "Q1": 120
    "Q2": 90
    "Q3": 150
    "Q4": 100
"#;
    let svg = liemermaid::render(pie, 600, 450).expect("render pie");
    std::fs::write("pie_basic.svg", svg).expect("write svg file");
    println!("pie_basic.svg generated");
}