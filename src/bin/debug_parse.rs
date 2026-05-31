use std::fs;

use liemermaid::render;

fn generate_svg(filename: &str, mermaid: &str) {
    match render(mermaid, 800, 600) {
        Ok(svg) => {
            fs::write(filename, &svg).expect("write failed");
            println!("✅ {}", filename);
        }
        Err(e) => eprintln!("❌ {}: {}", filename, e),
    }
}

fn main() {
    // flowchart
    generate_svg("flow_shapes.svg", r#"flowchart TD
    A[Start]
    B[End]
    A --> B
"#);

    generate_svg("flow_branch.svg", r#"flowchart TD
    A[Start]
    B{Decision}
    C[Approved]
    D[Rejected]
    E[End]
    A --> B
    B -->|Yes| C
    B -->|No| D
    C --> E
    D --> E
"#);

    generate_svg("flow_cycle.svg", r#"flowchart TD
    A[Start]
    B[Process]
    C{Check}
    D[Complete]
    E[End]
    A --> B
    B --> C
    C -->|Pass| D
    C -->|Fail| B
    D --> E
"#);

    generate_svg("flow_loop.svg", r#"flowchart TD
    A[Start]
    B{Continue}
    C[Process]
    D[End]
    A --> B
    B -->|Yes| C
    C --> B
    B -->|No| D
"#);

    // state diagrams
    generate_svg("state_basic.svg", r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Processing
    Processing --> Done
    Done --> [*]
"#);

    generate_svg("state_branch.svg", r#"stateDiagram-v2
    [*] --> Review
    Review --> Approved
    Review --> Rejected
    Approved --> Merged
    Rejected --> Closed
    Merged --> [*]
    Closed --> [*]
"#);

    generate_svg("state_desc.svg", r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Processing : start
    Processing --> Done : finish
    Processing --> Error : fail
    Done --> [*]
    Error --> Idle : retry
"#);

    generate_svg("state_labeled.svg", r#"stateDiagram-v2
    [*] --> Still : enter
    Still --> [*] : exit
    Still --> Moving
    Moving --> Still : push
    Moving --> Crash
    Crash --> [*]
"#);

    // class diagram
    generate_svg("class_basic.svg", r#"classDiagram
    class Animal {
        +name : String
        +makeSound()
    }
    class Dog
    class Cat
    Animal <|-- Dog
    Animal <|-- Cat
"#);

    // er diagram
    generate_svg("er_basic.svg", r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE_ITEM : contains
"#);

    // sequence diagram
    generate_svg("seq_basic.svg", r#"sequenceDiagram
    Alice->>John: Hello John
    John-->>Alice: Hi Alice
    Alice->>John: How are you?
    John-->>Alice: I'm fine
"#);

    // timeline
    generate_svg("timeline_basic.svg", r#"timeline
    title History of Computing
    1940s : First computers
    1950s : Transistors
    1960s : Integrated circuits
    1970s : Microprocessors
    1980s : Personal computers
"#);

    // gitgraph
    generate_svg("git_basic.svg", r#"gitGraph
    commit tag: "init"
    branch develop
    commit tag: "feat-1"
    checkout main
    commit tag: "fix-1"
    merge develop
    commit tag: "release"
"#);

    // pie chart
    generate_svg("pie_basic.svg", r#"pie
    title Languages
    "Rust" : 40
    "Python" : 30
    "TypeScript" : 20
    "Other" : 10
"#);

    println!("\nDone! SVGs generated.");
}