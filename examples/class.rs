use liemermaid;

fn main() {
    // 1. 基础类图（继承关系：Animal 在上，Dog/Cat 在下）
    let basic = r#"classDiagram
    class Animal
    class Dog
    class Cat
    Animal <|-- Dog
    Animal <|-- Cat
"#;
    let svg = liemermaid::render(basic, 700, 400).expect("render basic class");
    std::fs::write("class_basic.svg", svg).expect("write svg");
    println!("class_basic.svg generated");

    // 2. 带成员的类图
    let members = r#"classDiagram
    class Animal {
        +name : String
        +age : int
        +makeSound()
    }
    class Dog {
        -breed : String
        +bark()
    }
    Animal <|-- Dog
"#;
    let svg = liemermaid::render(members, 700, 400).expect("render class with members");
    std::fs::write("class_members.svg", svg).expect("write svg");
    println!("class_members.svg generated");

    // 3. 全关系类型
    let relations = r#"classDiagram
    class A
    class B
    class C
    class D
    class E
    A <|-- B : Inheritance
    A *-- C : Composition
    A o-- D : Aggregation
    A --> E : Association
    A ..> B : Dependency
"#;
    let svg = liemermaid::render(relations, 900, 400).expect("render class relations");
    std::fs::write("class_relations.svg", svg).expect("write svg");
    println!("class_relations.svg generated");
}
