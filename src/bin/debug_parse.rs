use pest::Parser;
use liemermaid::parser::MermaidParser;
use liemermaid::parser::Rule;

fn main() {
    println!("=== Detailed classDiagram Debug ===");
    let class_input = "classDiagram\nclass Foo\nFoo : int";
    match MermaidParser::parse(Rule::class_diagram, class_input) {
        Ok(pairs) => {
            println!("OK len={}", pairs.len());
            for p in pairs {
                let inners: Vec<_> = p.clone().into_inner().collect();
                println!(" rule={:?} text={:?}", p.as_rule(), p.as_str());
                println!("  inner count={}", inners.len());
                for inner in inners {
                    println!("  inner rule={:?} text={:?}", inner.as_rule(), inner.as_str());
                }
            }
        }
        Err(e) => println!("ERR {}", e),
    }

    println!("\n=== Detailed flowchart Debug ===");
    let flowchart_input = "flowchart TD\nA --> B";
    match MermaidParser::parse(Rule::flowchart_diagram, flowchart_input) {
        Ok(pairs) => {
            println!("OK len={}", pairs.len());
            for p in pairs {
                let inners: Vec<_> = p.clone().into_inner().collect();
                println!(" rule={:?} text={:?}", p.as_rule(), p.as_str());
                println!("  inner count={}", inners.len());
                for inner in inners {
                    println!("  inner rule={:?} text={:?}", inner.as_rule(), inner.as_str());
                }
            }
        }
        Err(e) => println!("ERR {}", e),
    }

    println!("\n=== Edge comparison ===");
    println!("edge in flowchart context: {:?}", MermaidParser::parse(Rule::edge, "A --> B").map(|p| p.as_str()));
    println!("edge in state context: {:?}", MermaidParser::parse(Rule::transition, "[*] --> Idle").map(|p| p.as_str()));

    println!("\n=== Keyword match test ===");
    println!("'flowchart' literal match: {:?}", "flowchart".starts_with("flowchart"));
    println!("'flowchart TD' first word: {:?}", "flowchart TD".split_whitespace().next());
}
