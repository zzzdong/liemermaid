// Class 渲染示例
//
// 运行：cargo run --example class
// 产物：examples/out/class.svg

use liemermaid::{render, MermaidParser};
use std::fs;

fn main() {
    // 本次新增语法：
    //   class Animal~T~<<Interface>>{ ... }   泛型 ~T~ + 注解 <<Interface>>
    //   +String name                          字段（带类型）
    //   +eat()                                方法（无返回类型）
    //   +get() T                              方法（带返回类型）
    //   Animal "1" <|-- "*" Dog : parent      关系基数（"1" / "*"）
    let input = r#"classDiagram
    class Animal~T~<<Interface>>{
        +String name
        +int age
        +eat()
        +get() T
    }
    class Dog
    Animal <|-- Dog
    Animal "1" <|-- "*" Dog : parent
"#;

    let _diagram = MermaidParser::parse_mermaid(input).expect("parse failed");
    println!("解析成功");

    let svg = render(input, 800, 500).expect("render failed");
    fs::create_dir_all("examples/out").unwrap();
    fs::write("examples/out/class.svg", &svg).unwrap();
    println!("已写入 examples/out/class.svg ({} 字节)", svg.len());
}
