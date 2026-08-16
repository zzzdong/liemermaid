fn main() {
    // 1. 基础状态图
    let basic = r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Processing
    Processing --> Done
    Done --> [*]
"#;
    let svg = liemermaid::render(basic, 600, 400).expect("render basic state");
    std::fs::write("state_basic.svg", svg).expect("write svg");
    println!("state_basic.svg generated");

    // 2. 带标签的状态图
    let labeled = r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Processing: start
    Processing --> Done: complete
    Done --> [*]: finished
"#;
    let svg = liemermaid::render(labeled, 600, 450).expect("render labeled state");
    std::fs::write("state_labeled.svg", svg).expect("write svg");
    println!("state_labeled.svg generated");

    // 3. 带描述的状态
    let desc = r#"stateDiagram-v2
    state Idle: System is idle
    state Processing: Working on task
    state Done: Task completed
    [*] --> Idle
    Idle --> Processing
    Processing --> Done
    Done --> [*]
"#;
    let svg = liemermaid::render(desc, 700, 500).expect("render state with descriptions");
    std::fs::write("state_desc.svg", svg).expect("write svg");
    println!("state_desc.svg generated");

    // 4. 分支状态
    let branch = r#"stateDiagram-v2
    [*] --> Pending
    Pending --> Approved
    Pending --> Rejected
    Approved --> [*]
    Rejected --> [*]
"#;
    let svg = liemermaid::render(branch, 700, 450).expect("render branch state");
    std::fs::write("state_branch.svg", svg).expect("write svg");
    println!("state_branch.svg generated");
}
