use liemermaid::{
    MermaidParser,
    diagram_builder::build_diagram,
    render::SvgRenderer,
};
use std::fs;

const OUT_DIR: &str = "examples";

fn main() {
    fs::create_dir_all(OUT_DIR).expect("create examples dir");

    // ========== Pie 图表 ==========
    gen_example(
        "pie_simple",
        "pie\ntitle Quarterly Sales\n\"Q1\": 120\n\"Q2\": 90\n\"Q3\": 150\n\"Q4\": 100",
        600, 400,
    );

    gen_example(
        "pie_showdata",
        "pie\nshowData\ntitle Budget Breakdown\n\"Engineering\": 250\n\"Marketing\": 120\n\"Sales\": 180\n\"Support\": 80",
        600, 450,
    );

    gen_example(
        "pie_single",
        "pie\ntitle Just One\n\"Entire\": 100",
        400, 350,
    );

    // ========== Flowchart ==========
    gen_example(
        "flow_simple",
        "flowchart TD\nA --> B",
        500, 300,
    );

    gen_example(
        "flow_chain",
        "flowchart LR\nA --> B\nB --> C\nC --> D",
        700, 300,
    );

    gen_example(
        "flow_branch",
        "flowchart TD\nA[\"Start\"]\nB[\"Branch\"]\nC[\"Left\"]\nD[\"Right\"]\nA --> B\nB --> C\nB --> D",
        600, 450,
    );

    gen_example(
        "flow_with_text",
        "flowchart TD\nA[\"Init\"]\nB[\"Process Data\"]\nC[\"Done\"]\nA --> B\nB --> C",
        500, 400,
    );

    gen_example(
        "flow_multi_branch",
        "flowchart TD\nA[\"Start\"]\nB[\"Validate\"]\nC[\"Success\"]\nD[\"Failure\"]\nE[\"Retry\"]\nA --> B\nB --> C\nB --> D\nD --> E\nE --> B",
        700, 500,
    );

    gen_example(
        "flow_top_down",
        "flowchart TD\nA[\"Step1\"]\nB[\"Step2\"]\nC[\"Step3\"]\nD[\"Step4\"]\nA --> B\nB --> C\nC --> D",
        400, 500,
    );

    gen_example(
        "flow_left_right",
        "flowchart LR\nA[\"Alpha\"]\nB[\"Beta\"]\nC[\"Gamma\"]\nA --> B\nB --> C",
        600, 250,
    );

    gen_example(
        "flow_workflow",
        "flowchart TD\nA[\"Create PR\"]\nB[\"Code Review\"]\nC[\"CI Checks\"]\nD[\"Merge\"]\nE[\"Deploy\"]\nF[\"Failed\"]\nA --> B\nB --> C\nC --> D\nD --> E\nC --> F",
        600, 500,
    );

    println!("\nAll examples generated in ./{}/", OUT_DIR);
}

fn gen_example(name: &str, mermaid_text: &str, width: u32, height: u32) {
    let path = format!("{}/{}.svg", OUT_DIR, name);

    let diagram = match MermaidParser::parse_mermaid(mermaid_text) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[SKIP] {} — parse failed: {}", name, e);
            return;
        }
    };

    let elements = match build_diagram(&diagram) {
        Ok(el) => el,
        Err(e) => {
            eprintln!("[SKIP] {} — build failed: {}", name, e);
            return;
        }
    };

    let renderer = SvgRenderer::new();
    let svg = match renderer.render(&elements, width, height) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[SKIP] {} — render failed: {}", name, e);
            return;
        }
    };

    fs::write(&path, &svg).expect("write SVG file");
    let line_count = svg.lines().count();
    let element_count = count_elements(&svg);
    println!("  ✓ {} — {} lines, {} elements", name, line_count, element_count);
}

fn count_elements(svg: &str) -> usize {
    let tags = ["<path ", "<rect ", "<circle ", "<line ", "<polyline ", "<text "];
    tags.iter().map(|t| svg.matches(t).count()).sum()
}