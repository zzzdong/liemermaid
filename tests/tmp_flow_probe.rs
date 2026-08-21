// 临时探针：flowchart parser 语法覆盖检查（检查完可删）
use liemermaid::MermaidParser;

fn try_parse(name: &str, input: &str) {
    match MermaidParser::parse_mermaid(input) {
        Ok(liemermaid::Diagram::Flowchart(f)) => {
            println!(
                "[{}] OK: nodes={} edges={} subgraphs={}",
                name,
                f.nodes.len(),
                f.edges.len(),
                f.subgraphs.len()
            );
        }
        Ok(_) => println!("[{}] OK: not flowchart", name),
        Err(e) => println!("[{}] FAIL: {}", name, e),
    }
}

#[test]
fn probe_flowchart_syntax_coverage() {
    try_parse("amp_multi_target", "flowchart TD\nA --> B & C\nD --> E");
    try_parse(
        "nested_subgraph",
        "flowchart TD\nsubgraph outer\n  subgraph inner\n    A --> B\n  end\nend\nC --> A",
    );
    try_parse(
        "subgraph_id_title",
        "flowchart TD\nsubgraph sg1 [My Title]\n  A --> B\nend",
    );
    try_parse(
        "subgraph_direction",
        "flowchart TD\nsubgraph s\n  direction LR\n  A --> B\nend",
    );
    try_parse(
        "click_stmt",
        "flowchart TD\nA --> B\nclick A \"https://x.com\" \"tip\"",
    );
    try_parse("linkstyle", "flowchart TD\nA --> B\nlinkStyle 0 stroke:red");
    try_parse("classdef", "flowchart TD\nA --> B\nclassDef c fill:#f9f\nclass A c");
    try_parse("class_shorthand", "flowchart TD\nA:::c --> B");
    try_parse("long_arrow", "flowchart TD\nA ----> B");
    try_parse("dotted_labeled", "flowchart TD\nA -. text .-> B");
    try_parse("thick_labeled", "flowchart TD\nA == text ==> B");
    try_parse("quoted_special", "flowchart TD\n\"a b\"[\"text: #35;\"] --> B");
    try_parse("fa_icon", "flowchart TD\nB[fa:fa-twitter]");
    try_parse("new_shape_syntax", "flowchart TD\nA@{ shape: rect }");
    try_parse("graph_kw", "graph TD\nA --> B");
    try_parse("no_direction", "flowchart\nA --> B");
}
