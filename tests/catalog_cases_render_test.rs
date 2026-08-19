//! 目录冒烟测试：catalog 中每个用例必须能被 liemermaid 解析并渲染。
//!
//! 与 `golden_snapshot_test.rs` 配合：本测试保证用例源码对 liemermaid 有效；
//! 快照测试保证布局与官方对齐。

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Case {
    #[serde(rename = "type")]
    ty: String,
    name: String,
    #[allow(dead_code)]
    rankdir: Option<String>,
    width: u32,
    height: u32,
    source: String,
    /// 若为 false，表示 liemermaid 当前尚不支持该语法（仅保留官方参考 SVG）。
    /// 默认 true。
    #[serde(default = "default_true")]
    liemermaid: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct Catalog {
    cases: Vec<Case>,
}

#[test]
fn all_catalog_cases_parse_and_render() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/cases/catalog.json"
    );
    let text = std::fs::read_to_string(path).expect("catalog.json missing");
    let catalog: Catalog = serde_json::from_str(&text).expect("invalid catalog.json");
    assert!(!catalog.cases.is_empty(), "catalog is empty");

    let mut failed = Vec::new();
    let mut skipped = Vec::new();
    for c in &catalog.cases {
        // liemermaid 尚不支持的语法，跳过（官方参考 SVG 仍保留）
        if !c.liemermaid {
            skipped.push(format!("{}__{}", c.ty, c.name));
            continue;
        }
        let src_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/cases/")
            .to_string()
            + &c.source;
        let mermaid = match std::fs::read_to_string(&src_path) {
            Ok(t) => t,
            Err(e) => {
                failed.push(format!("{}__{}: missing source {} ({})", c.ty, c.name, c.source, e));
                continue;
            }
        };
        match liemermaid::render(&mermaid, c.width, c.height) {
            Ok(_) => {}
            Err(e) => {
                failed.push(format!("{}__{}: render failed: {}", c.ty, c.name, e));
            }
        }
    }

    if !skipped.is_empty() {
        eprintln!(
            "\nSKIPPED {} case(s) (liemermaid 尚不支持): {:?}",
            skipped.len(),
            skipped
        );
    }

    assert!(
        failed.is_empty(),
        "{} case(s) failed to parse/render:\n{}",
        failed.len(),
        failed.join("\n")
    );
}
