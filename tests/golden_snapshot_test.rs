//! 端到端渲染回归测试套件（liemermaid 自有 golden）。
//!
//! 策略：用 `liemermaid::render` 把 `tests/golden/cases/*.mmd` 渲染为 SVG，
//! 与锁定的自有 golden（`tests/golden/liemermaid_golden/{type}__{name}.svg`）
//! 做**结构化**比对（见 `svgdiff` 模块），覆盖：
//!   1. 元素数量与类型（rect/circle/path/text/... 或语义 class）
//!   2. 文本标签内容集合
//!   3. 颜色 / 样式（fill / stroke）
//!
//! 与 mermaid-cli 的官方 SVG 不做逐字节比对——两套渲染引擎几何不同，
//! 因此 golden 来自 liemermaid 自身（首次渲染即锁定，后续防止回归）。
//!
//! 更新 golden：`UPDATE_GOLDEN=1 cargo test --test golden_snapshot_test`
//! 首次建立：`rm -rf tests/golden/liemermaid_golden && UPDATE_GOLDEN=1 cargo test ...`

#[path = "golden/svgdiff.rs"]
mod svgdiff;

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use serde::Deserialize;

use svgdiff::{compare, parse, summarize};

/// 全局串行，避免并行写/读 golden 文件冲突。
static LOCK: Mutex<()> = Mutex::new(());

const CASES_DIR: &str = "tests/golden/cases";
const GOLDEN_DIR: &str = "tests/golden/liemermaid_golden";

#[derive(Debug, Clone, Deserialize)]
struct Case {
    #[serde(rename = "type")]
    ty: String,
    name: String,
    source: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    liemermaid: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct Catalog {
    cases: Vec<Case>,
}

fn load_catalog() -> Catalog {
    let raw = fs::read_to_string("tests/golden/cases/catalog.json").expect("read catalog");
    serde_json::from_str(&raw).expect("parse catalog")
}

fn case_src(case: &Case) -> String {
    // source 形如 "flowchart__long_edge.mmd"，相对 cases 目录
    let p = Path::new(CASES_DIR).join(&case.source);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn golden_path(case: &Case) -> String {
    format!("{}/{}-{}.svg", GOLDEN_DIR, case.ty, case.name)
}

/// 对单个 case 做 render + svgdiff 比对（或生成 golden）。
fn run_case(case: &Case) -> Result<(), String> {
    let _g = LOCK.lock().unwrap();
    let src = case_src(case);
    let width = if case.width > 0 { case.width } else { 800 };
    let height = if case.height > 0 { case.height } else { 600 };

    let ours = liemermaid::render(&src, width, height)
        .map_err(|e| format!("render failed: {e:?}"))?;

    let gpath = golden_path(case);
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    if update || !Path::new(&gpath).exists() {
        fs::create_dir_all(GOLDEN_DIR).ok();
        fs::write(&gpath, &ours).map_err(|e| format!("write golden {gpath}: {e}"))?;
        return Ok(()); // 生成模式直接通过
    }

    let golden_svg = fs::read_to_string(&gpath).map_err(|e| format!("read golden {gpath}: {e}"))?;
    let diff = compare(&summarize(&parse(&ours)), &summarize(&parse(&golden_svg)));

    if diff.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "svgdiff mismatch for {}/{}:\n{}",
            case.ty,
            case.name,
            diff.describe()
        ))
    }
}

#[test]
fn liemermaid_self_golden() {
    let catalog = load_catalog();
    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0usize;

    for case in &catalog.cases {
        // 仅对 liemermaid 支持的图表做端到端验证
        if !case.liemermaid {
            continue;
        }
        ran += 1;
        match run_case(case) {
            Ok(()) => {}
            Err(e) => failures.push(format!("[{}/{}] {e}", case.ty, case.name)),
        }
    }

    if !failures.is_empty() {
        panic!(
            "liemermaid self-golden: {} cases ran, {} failures:\n{}",
            ran,
            failures.len(),
            failures.join("\n")
        );
    }
    println!("liemermaid self-golden: {ran} cases checked");
}
