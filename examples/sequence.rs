fn main() {
    // 1. 基础时序图
    let basic = r#"sequenceDiagram
    participant Alice
    participant Bob
    Alice->>Bob: Hello Bob, how are you?
    Bob-->>Alice: I am good thanks!
    Alice->>Bob: What about you?
"#;
    let svg = liemermaid::render(basic, 600, 400).expect("render basic sequence");
    std::fs::write("seq_basic.svg", svg).expect("write svg");
    println!("seq_basic.svg generated");

    // 2. 带备注的时序图
    let notes = r#"sequenceDiagram
    participant A
    participant B
    A->>B: Request
    note over A,B: Processing
    B-->>A: Response
    note left of A: Note left
    note right of B: Note right
"#;
    let svg = liemermaid::render(notes, 700, 450).expect("render sequence with notes");
    std::fs::write("seq_notes.svg", svg).expect("write svg");
    println!("seq_notes.svg generated");

    // 3. 多种箭头类型
    let arrows = r#"sequenceDiagram
    participant A
    participant B
    A->B: Solid
    A->>B: SolidTip
    A-->B: Dashed
    A-->>B: DashedTip
    A-xB: Cross
    A-)B: Open
"#;
    let svg = liemermaid::render(arrows, 700, 500).expect("render sequence arrows");
    std::fs::write("seq_arrows.svg", svg).expect("write svg");
    println!("seq_arrows.svg generated");

    // 4. 自消息
    let self_msg = r#"sequenceDiagram
    participant A
    participant B
    A->>A: Self message
    A->>B: To B
"#;
    let svg = liemermaid::render(self_msg, 600, 350).expect("render self message");
    std::fs::write("seq_self.svg", svg).expect("write svg");
    println!("seq_self.svg generated");
}
