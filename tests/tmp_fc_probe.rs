use liemermaid::MermaidParser;

fn probe(name: &str, input: &str) {
    match MermaidParser::parse_mermaid(input) {
        Ok(liemermaid::Diagram::Flowchart(fc)) => {
            println!("=== {} OK ===", name);
            println!("  dir={:?}", fc.direction);
            for n in &fc.nodes {
                println!("  node id={} shape={:?} text={:?}", n.id, n.shape, n.text);
            }
            for e in &fc.edges {
                println!("  edge {} ->{:?} {} [{}]", e.source, e.arrow_type, e.target, e.label.as_deref().unwrap_or("-"));
            }
            for sg in &fc.subgraphs {
                println!("  subgraph title={:?}", sg.title);
                for n in &sg.nodes {
                    println!("    node id={} shape={:?} text={:?}", n.id, n.shape, n.text);
                }
                for e in &sg.edges {
                    println!("    edge {} ->{:?} {} [{}]", e.source, e.arrow_type, e.target, e.label.as_deref().unwrap_or("-"));
                }
            }
        }
        Ok(other) => println!("=== {} (not flowchart: {:?}) ===", name, std::mem::discriminant(&other)),
        Err(e) => println!("=== {} ERR: {:?} ===", name, e),
    }
}

#[test]
fn probe_fc() {
    // 1. 所有节点形状
    probe(
        "shapes",
        r#"flowchart TD
A[rect]
B(rounded)
C([stadium])
D[[subroutine]]
E{diamond}
F{{hexagon}}
G((circle))
H(((double circle)))
I[(cylinder)]
J>asymmetric]
K[/parallelogram/]
L[\parallelogram alt\]
M[/trapezoid\]
N[\trapezoid alt/]
"#,
    );

    // 2. 所有箭头类型 + 标签
    probe(
        "arrows",
        r#"flowchart LR
A --> B
C -.-> D
E ==> F
G --- H
I --o J
K --x L
M <--> N
O ~~~ P
Q o--o R
S x--x T
"#,
    );

    // 3. 链式边 + 混合标签
    probe(
        "chains",
        r#"flowchart TD
A --> B --> C --> D
A -- yes --> B
C == strong ==> D
E -. dotted .-> F
G--go-->H
"#,
    );

    // 4. 引号 id 与特殊文本
    probe(
        "quoted",
        r#"flowchart TD
"my node"[Start Here]
"my node" --> B{check}
B -->|label with | pipe| C
"#,
    );

    // 5. 子图 + 嵌套
    probe(
        "subgraph",
        r#"flowchart TD
subgraph one[Group One]
A --> B
end
subgraph two
C --> D
end
A --> C
"#,
    );

    // 6. graph 关键字 + 无方向
    probe(
        "graph_nodir",
        "graph\nA --> B\n",
    );

    // 7. 注释
    probe(
        "comments",
        "%% top comment\nflowchart TD\n%% mid\nA --> B\n%% tail\n",
    );

    // 8. 边界: 节点文本含括号/引号
    probe(
        "tricky_text",
        r#"flowchart TD
A[has "quotes" inside]
B(a (nested) paren)
C{a {brace} brace}
"#,
    );

    // 9. 同 id 重复声明 (后覆盖)
    probe(
        "dup_id",
        "flowchart TD\nA --> B\nA[final shape]\n",
    );

    // 10. 空图
    probe("empty", "flowchart TD\n");
}
