use liemermaid::MermaidParser;

#[test]
fn probe_parse() {
    let input = r#"flowchart TD
    A["Start"]
    subgraph One
        B["Process B"]
        C{"Decision C"}
    end
    subgraph Two
        D["Process D"]
    end
    A --> B
    B --> C
    C --> D
    D --> A"#;
    match MermaidParser::parse_mermaid(input) {
        Ok(liemermaid::Diagram::Flowchart(fc)) => {
            println!("NODES:");
            for n in &fc.nodes {
                println!("  top: id={} shape={:?} text={:?}", n.id, n.shape, n.text);
            }
            for sg in &fc.subgraphs {
                println!("  subgraph title={:?}", sg.title);
                for n in &sg.nodes {
                    println!("    id={} shape={:?} text={:?}", n.id, n.shape, n.text);
                }
            }
        }
        Ok(_) => println!("not flowchart"),
        Err(e) => println!("ERR: {:?}", e),
    }
}
