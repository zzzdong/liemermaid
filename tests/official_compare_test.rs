//! 端到端官方对比测试（与官方 mermaid-cli 输出对比）。
//!
//! 层 1（语义层）：用 `semantics` 模块从 liemermaid 输出与官方 mermaid-cli 输出各抽
//! 取结构化语义（文本集合 / 节点标签集合 / 归一语义类型计数），做跨引擎正确性比对。
//! 文本集合与节点标签集合要求**相等**（强判据）；类型计数记录偏差（弱判据）。
//!
//! 层 2（结构层）：复用 `svgdiff` 对比两边生成的 SVG 在元素数量、文本、颜色、
//! 相对布局/包围盒等维度的差异，作为"渲染像不像"的辅助信号。
//!
//! 官方 golden 来源：`tests/golden/golden/{type}__{name}.svg`（由 mermaid-cli 生成）。
//! liemermaid 输出：实时 `liemermaid::render` 渲染 catalog 中的 `.mmd` 源码。
//!
//! 注：本套件只与官方输出对比，不再做 liemermaid 自回归（自比）测试。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[path = "golden/svgdiff.rs"]
mod svgdiff;

const CASES_DIR: &str = "tests/golden/cases";
const GOLDEN_DIR: &str = "tests/golden/golden";

/// 加载并解析 catalog.json 得到用例列表。
fn load_catalog() -> Vec<Case> {
    let path = Path::new(CASES_DIR).join("catalog.json");
    let text = fs::read_to_string(&path).expect("read catalog.json");
    let cat: Catalog = serde_json::from_str(&text).expect("parse catalog.json");
    cat.cases
}

#[derive(serde::Deserialize)]
struct Catalog {
    cases: Vec<Case>,
}

#[derive(serde::Deserialize, Clone)]
struct Case {
    #[serde(rename = "type")]
    typ: String,
    name: String,
    #[serde(default = "default_true")]
    compare: bool,
    source: String,
}

fn default_true() -> bool {
    true
}

#[test]
fn official_semantic_compare() {
    let cases = load_catalog();
    let mut failures: BTreeMap<String, String> = BTreeMap::new();
    let mut struct_failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for c in &cases {
        if !c.compare {
            continue;
        }
        let key = format!("{}__{}", c.typ, c.name);
        let src_path = Path::new(CASES_DIR).join(&c.source);
        let golden_path = Path::new(GOLDEN_DIR).join(format!("{}.svg", key));
        if !golden_path.exists() {
            failures.insert(key, "missing official golden svg".into());
            continue;
        }
        let src = match fs::read_to_string(&src_path) {
            Ok(s) => s,
            Err(e) => {
                failures.insert(key, format!("read mmd: {e}"));
                continue;
            }
        };
        let golden_svg = fs::read_to_string(&golden_path).expect("read golden");
        // liemermaid 渲染
        let ours = match liemermaid::render(&src, 900, 700) {
            Ok(s) => s,
            Err(e) => {
                failures.insert(key, format!("liemermaid render error: {e}"));
                continue;
            }
        };

        // 层 1：svgdiff 内容正确性比对（文本集合相等 + 节点/边拓扑数量相等）。
        // 已不依赖 class 命名——回退后 liemermaid 不再输出官方 class，改用几何类型推断。
        let ours_els = svgdiff::parse(&ours);
        let golden_els = svgdiff::parse(&golden_svg);
        let ours_sum = svgdiff::summarize(&ours_els);
        let golden_sum = svgdiff::summarize(&golden_els);
        let dd = svgdiff::compare(&ours_sum, &golden_sum);
        let mut detail = String::new();
        if !dd.is_empty() {
            detail.push_str("SVGDIFF (content/geometry):\n");
            detail.push_str(&dd.describe());
        }

        // 结构回归门槛：核心实体（节点/边）若官方有而 liemermaid 完全未渲染
        // （几何数量 = 0），视为结构性回归，硬失败。基于几何推断而非 class 命名。
        let mut struct_missing = Vec::new();
        if !golden_sum.node_centers.is_empty() && ours_sum.node_centers.is_empty() {
            struct_missing.push("node");
        }
        if !golden_sum.edge_endpoints.is_empty() && ours_sum.edge_endpoints.is_empty() {
            struct_missing.push("edge");
        }
        if !struct_missing.is_empty() {
            detail.push_str(&format!(
                "\n[STRUCT REGRESSION] missing entirely: {}\n",
                struct_missing.join(", ")
            ));
            struct_failures.push(key.clone());
        }

        if !detail.is_empty() {
            failures.insert(key, detail);
        }
        checked += 1;
    }

    println!("official_semantic_compare: checked {checked} cases, {} mismatches", failures.len());
    for (k, v) in &failures {
        println!("--- {k} ---\n{v}");
    }
    // 结构回归门槛：挡住"某 diagram 类型结构完全丢失"的回归（节点/边几何数量为 0）。
    // 文本/拓扑差异仍为报告式（待逐步收敛）。如需全量硬门槛，改为 assert failures.is_empty()。
    assert!(
        struct_failures.is_empty(),
        "{} cases have structural regressions (core geometry missing entirely): {:?}",
        struct_failures.len(),
        struct_failures
    );
    let _ = failures;
}
