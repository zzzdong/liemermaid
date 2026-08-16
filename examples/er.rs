fn main() {
    // 1. 简单实体关系图
    let basic = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE_ITEM : contains
    CUSTOMER {
        int id, string name
    }
    ORDER {
        int id, string date
    }
    LINE_ITEM {
        int id, int quantity
    }
"#;
    let svg = liemermaid::render(basic, 900, 400).expect("render er basic");
    std::fs::write("er_basic.svg", svg).expect("write svg");
    println!("er_basic.svg generated");

    // 2. 所有基数类型
    let all_cards = r#"erDiagram
    A ||--|| B : ExactlyOne
    C |o--|o D : ZeroOrOne
    E }|--|{ F : OneOrMany
    G }o--|o H : ZeroOrMany
"#;
    let svg = liemermaid::render(all_cards, 1000, 500).expect("render er cards");
    std::fs::write("er_cards.svg", svg).expect("write svg");
    println!("er_cards.svg generated");
}
