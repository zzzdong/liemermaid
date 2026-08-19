fn main() {
    let input = r#"sequenceDiagram
    participant A
    actor B
    database DB
    entity E
    A <<->> B
    B --x DB
    DB -) E
    A->>DB: query
"#;
    match liemermaid::render(input, 800, 600) {
        Ok(svg) => {
            println!("RENDER OK, svg len = {}", svg.len());
            std::fs::write("dbg_seq.svg", &svg).expect("write svg");
        }
        Err(e) => println!("RENDER ERROR: {e}"),
    }
}
