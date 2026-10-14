//! 审查辅助：把 golden 用例渲染成 PNG，便于与官方 golden SVG 逐帧目检对比。
//!
//! 运行：cargo run --example review_png
//! 产物：examples/out/review/{type}__{name}.png

use liemermaid::{MermaidParser, render, render_png};
use std::fs;
use std::path::Path;

const CASES: &[(&str, &str)] = &[
    ("flowchart", "chain"),
    ("flowchart", "shapes"),
    ("flowchart", "diamond_3way"),
    ("flowchart", "edge_types"),
    ("flowchart", "subgraph"),
    ("flowchart", "self_loop"),
    ("sequence", "basic"),
    ("sequence", "activation"),
    ("sequence", "notes"),
    ("sequence", "loops"),
    ("class", "fields_methods"),
    ("class", "relations"),
    ("state", "basic"),
    ("state", "composite"),
    ("er", "basic"),
    ("pie", "basic"),
    ("gitgraph", "basic"),
    ("timeline", "basic"),
];

fn main() {
    let out_dir = Path::new("examples/out/review");
    fs::create_dir_all(out_dir).unwrap();
    for (typ, name) in CASES {
        let key = format!("{typ}__{name}");
        let src_path = format!("tests/golden/cases/{key}.mmd");
        let src = fs::read_to_string(&src_path).expect("read mmd");
        if let Err(e) = MermaidParser::parse_mermaid(&src) {
            println!("{key}: PARSE ERROR: {e}");
            continue;
        }
        match render_png(&src, 900, 700) {
            Ok(png) => {
                let path = out_dir.join(format!("{key}.png"));
                fs::write(&path, &png).unwrap();
                println!("{key}: {} bytes", png.len());
            }
            Err(e) => println!("{key}: RENDER PNG ERROR: {e}"),
        }
        if let Ok(svg) = render(&src, 900, 700) {
            let path = out_dir.join(format!("{key}.svg"));
            fs::write(&path, &svg).unwrap();
        }
    }
}
