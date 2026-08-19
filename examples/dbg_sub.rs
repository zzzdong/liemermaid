use liemermaid::parser::MermaidParser;
fn main() {
    for inp in [
        "flowchart TD\nsubgraph One\nA\nB\nend\nA --> B",
        "flowchart TD\nsubgraph One\nA --> B\nend",
    ] {
        println!("=== {:?}", inp);
        match MermaidParser::parse_mermaid(inp) {
            Ok(ast) => println!("PARSE OK: {:#?}", ast),
            Err(e) => println!("ERR: {}", e),
        }
    }
}
