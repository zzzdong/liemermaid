//! 验证器：检查画布是否裁切了内容，以及文本节点是否携带预排版 layout。
//!
//! 1. 画布裁切：用 lievisual 的 `fit::scene_bounds`（正确处理描边、变换、文本）算出
//!    真实内容包围盒，与 liemermaid 当前画布尺寸对比。若内容超出画布即判定为裁切。
//! 2. 文本 layout：lievisual 0.2 起 `Element::text()/rich_text()` 构造时即排版，
//!    `layout: None` 只会出现在"环境无可用字体"的降级场景 —— 据此区分真降级与漏排版。

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

/// 统计场景中 Text 节点的排版状态。
///
/// 返回 (总数, 带布局数)。若环境有可用字体，带布局数应等于总数 ——
/// `layout: None` 只允许出现在空文本或无字体的降级场景。
fn text_layout_stats(scene: &lievisual::Scene) -> (usize, usize) {
    fn walk(node: &lievisual::SceneNode, total: &mut usize, laid: &mut usize) {
        if let lievisual::scene::Element::Text { spans, layout, .. } = &node.element {
            *total += 1;
            // 空文本不排版（lievisual 的约定），不计为漏排版。
            let non_empty = spans.iter().any(|s| !s.text.is_empty());
            if layout.is_some() || !non_empty {
                *laid += 1;
            }
        }
        if let lievisual::scene::Element::Group { children } = &node.element {
            for c in children {
                walk(c, total, laid);
            }
        }
    }
    let (mut total, mut laid) = (0, 0);
    for n in &scene.nodes {
        walk(n, &mut total, &mut laid);
    }
    for layer in &scene.layers {
        for n in &layer.nodes {
            walk(n, &mut total, &mut laid);
        }
    }
    (total, laid)
}

fn main() {
    println!(
        "{:<12} {:>18} {:>22} {:>10}",
        "图表", "画布 (w×h)", "内容包围盒 (w×h)", "结论"
    );
    println!("{}", "-".repeat(70));
    let mut clipped = 0;
    let mut texts_total = 0;
    let mut texts_laid = 0;
    for (name, src) in CASES {
        let scene = build_diagram(&parse(src)).expect("构建失败");
        let (cw, ch) = (scene.width, scene.height);

        // 文本 layout 检查（lievisual 0.2：构造时即排版）。
        let (t, l) = text_layout_stats(&scene);
        texts_total += t;
        texts_laid += l;

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
    let texts_missing = texts_total - texts_laid;
    println!("文本节点: {texts_total} 个，{texts_laid} 个带预排版 layout（{texts_missing} 个缺失）");
    // 缺失数 > 0 说明有文本未排版 —— 环境无字体（降级正常）或构造路径漏了排版。
    std::process::exit(if clipped > 0 || texts_missing > 0 { 1 } else { 0 });
}
