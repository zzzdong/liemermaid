fn main() {
    // 1. 基础 Git 分支图
    let basic = r#"gitGraph
    commit
    commit
    branch develop
    checkout develop
    commit
    commit
    checkout main
    merge develop
    commit
"#;
    let svg = liemermaid::render(basic, 700, 400).expect("render gitgraph basic");
    std::fs::write("git_basic.svg", svg).expect("write svg");
    println!("git_basic.svg generated");

    // 2. 带标签的 Git 图
    let tagged = r#"gitGraph
    commit tag: "v1.0"
    branch feature
    checkout feature
    commit tag: "wip"
    commit
    checkout main
    merge feature
    commit tag: "v2.0"
"#;
    let svg = liemermaid::render(tagged, 700, 400).expect("render gitgraph tagged");
    std::fs::write("git_tagged.svg", svg).expect("write svg");
    println!("git_tagged.svg generated");
}
