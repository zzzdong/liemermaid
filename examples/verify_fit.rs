//! 验证器：检查画布是否裁切了内容。
//!
//! 对每个图表：用 lievisual 的 `fit::scene_bounds`（正确处理描边、变换、文本）算出
//! 真实内容包围盒，与 liemermaid 当前画布尺寸对比。若内容超出画布即判定为裁切。

use liemermaid::builder::build_diagram;

use liemermaid::ast::Diagram;

const CASES: &[(&str, &str)] = &[
    ("flowchart", "flowchart TD\n  A[Start] --> B{Is it?}\n  B -->|Yes| C[OK]\n  B -->|No| D[End]\n  C --> D"),
    ("class", "classDiagram\n  class Animal {\n    +String name\n    +makeSound()\n  }\n  class Dog\n  Animal <|-- Dog"),
    ("state", "stateDiagram-v2\n  [*] --> Idle\n  Idle --> Running : start\n  Running --> Idle : stop\n  Running --> [*]"),
    ("sequence", "sequenceDiagram\n  Alice->>John: Hello John\n  John-->>Alice: Hi Alice\n  Alice->>John: How are you?"),
    ("pie", "pie title Pets\n  \"Dogs\" : 386\n  \"Cats\" : 85\n  \"Rats\" : 15"),
    ("gitgraph", "gitGraph\n  commit\n  branch develop\n  commit\n  checkout main\n  commit\n  merge develop"),
];

fn parse(text: &str) -> Diagram {
    liemermaid::parser::WinnowParser::parse_mermaid(text).expect("解析失败")
}

fn main() {
    println!(
        "{:<12} {:>18} {:>22} {:>10}",
        "图表", "画布 (w×h)", "内容包围盒 (w×h)", "结论"
    );
    println!("{}", "-".repeat(70));
    let mut clipped = 0;
    for (name, src) in CASES {
        let scene = build_diagram(&parse(src)).expect("构建失败");
        let (cw, ch) = (scene.width, scene.height);
        let b = lievisual::fit::scene_bounds(&scene);
        match b {
            Some(b) => {
                // fit_to_canvas 后内容应恰好落在 [margin, size-margin]。
                let fits = b.min_x() >= -0.5
                    && b.min_y() >= -0.5
                    && b.max_x() <= cw + 0.5
                    && b.max_y() <= ch + 0.5;
                if !fits {
                    clipped += 1;
                }
                println!(
                    "{:<12} {:>18} {:>22} {:>10}   bbox=({:.2},{:.2})-({:.2},{:.2})",
                    name,
                    format!("{cw:.2}×{ch:.2}"),
                    format!("{:.2}×{:.2}", b.width(), b.height()),
                    if fits { "OK" } else { "裁切!" },
                    b.min_x(),
                    b.min_y(),
                    b.max_x(),
                    b.max_y()
                );
            }
            None => println!("{:<12} {:>18} {:>22}", name, format!("{cw:.2}×{ch:.2}"), "(空)"),
        }
    }
    println!("{}", "-".repeat(70));
    println!("裁切数: {clipped}");
    std::process::exit(if clipped > 0 { 1 } else { 0 });
}
