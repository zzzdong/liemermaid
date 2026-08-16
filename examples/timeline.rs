fn main() {
    // 1. 基础时间线
    let basic = r#"timeline
    title History of Social Media
    2002 : LinkedIn
    2004 : Facebook
    2005 : Youtube
    2006 : Twitter
"#;
    let svg = liemermaid::render(basic, 800, 300).expect("render timeline basic");
    std::fs::write("timeline_basic.svg", svg).expect("write svg");
    println!("timeline_basic.svg generated");

    // 2. 带分段的时间线
    let sections = r#"timeline
    title Project Timeline
    section Phase 1
        Requirement analysis : 2 weeks
        Design : 3 weeks
    section Phase 2
        Development : 8 weeks
        Testing : 4 weeks
    section Phase 3
        Deployment : 1 week
        Maintenance : ongoing
"#;
    let svg = liemermaid::render(sections, 900, 400).expect("render timeline sections");
    std::fs::write("timeline_sections.svg", svg).expect("write svg");
    println!("timeline_sections.svg generated");
}
