//! Class 基础：成员块、类外成员声明、注解、泛型与关系。
//!
//! 运行：`cargo run --example class_basic`
//! 产物：`examples/out/class_basic.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"classDiagram
    class Animal {
        +String name
        +int age
        +eat() void
    }
    Animal : +isMammal() bool

    class Duck~T~ {
        +String beakColor
        +swim()
    }

    Animal <|-- Duck
    Duck "1" o-- "many" Feather : has
    Duck ..> Pond : depends
"#;

fn main() {
    support::svg("class_basic", DIAGRAM, 1000, 650);
}
