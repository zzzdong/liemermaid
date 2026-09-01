fn main() {
    std::fs::create_dir_all("examples/out").unwrap();

    // 1. 简单实体关系图
    let basic = r#"erDiagram
    CUSTOMER ||--o{ ORDER : "places"
    ORDER ||--|{ LINE_ITEM : "contains"
    PRODUCT ||--o{ LINE_ITEM : "included in"

    CUSTOMER {
        int id PK
        string name
        string email
        string phone
    }
    
    ORDER {
        int id PK
        int customer_id FK
        datetime created_at
        decimal total_amount
        string status
    }
    
    LINE_ITEM {
        int id PK
        int order_id FK
        string product_sku FK
        int quantity
        decimal unit_price
    }
    
    PRODUCT {
        string sku PK
        string title
        decimal price
        string description
    }
"#;
    let svg = liemermaid::render(basic, 900, 400).expect("render er basic");
    std::fs::write("examples/out/er_basic.svg", svg).expect("write svg");
    println!("已写入 examples/out/er_basic.svg");

    // 2. 所有基数类型
    let all_cards = r#"erDiagram
    A ||--|| B : ExactlyOne
    C |o--|o D : ZeroOrOne
    E }|--|{ F : OneOrMany
    G }o--|o H : ZeroOrMany
"#;
    let svg = liemermaid::render(all_cards, 1000, 500).expect("render er cards");
    std::fs::write("examples/out/er_cards.svg", svg).expect("write svg");
    println!("已写入 examples/out/er_cards.svg");
}
