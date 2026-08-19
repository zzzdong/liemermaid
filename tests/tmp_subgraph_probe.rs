use liemermaid::render;

#[test]
fn probe_subgraph_parse_and_render() {
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
    let svg = render(input, 800, 600).expect("render");
    for line in svg.lines() {
        let t = line.trim();
        if t.contains("<text")
            || t.contains("<path d=\"M104")
            || (t.starts_with("<rect") && t.contains("rx="))
        {
            println!("LINE: {}", t);
        }
    }
    assert!(svg.contains(">Start<"), "A label missing");
    assert!(svg.contains(">Process B<"), "B label missing");
    assert!(svg.contains(">Decision C<"), "C label missing");
    assert!(svg.contains(">Process D<"), "D label missing");
}
