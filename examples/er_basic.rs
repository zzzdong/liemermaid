//! ER 基础：实体属性块、关系基数与关系标签。
//!
//! 运行：`cargo run --example er_basic`
//! 产物：`examples/out/er_basic.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE_ITEM : contains
    PRODUCT ||--o{ LINE_ITEM : "included in"

    CUSTOMER {
        int id PK
        string name
        string email
    }

    ORDER {
        int id PK
        int customer_id FK
        datetime created_at
    }

    PRODUCT {
        int id PK
        string title
        decimal price
    }
"#;

fn main() {
    support::svg("er_basic", DIAGRAM, 1000, 700);
}
