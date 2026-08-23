//! 生成可人工对比的 HTML 报告。
//!
//! 跑完测试后，在浏览器打开 `tests/golden/report.html` 即可看到每个用例的：
//!   - 原始 Mermaid 源码
//!   - liemermaid 渲染产物（SVG）
//!   - 官方 mermaid-cli 渲染产物（SVG，若存在 golden）
//!   - 语义 / 结构差异摘要（来自 semantics + svgdiff 模块）
//!
//! 运行：`cargo test --test html_report_test -- --nocapture`
//! 输出路径会打印到 stdout。

#[path = "golden/semantics.rs"]
mod semantics;
#[path = "golden/svgdiff.rs"]
mod svgdiff;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const CASES_DIR: &str = "tests/golden/cases";
const GOLDEN_DIR: &str = "tests/golden/golden";
const REPORT_PATH: &str = "tests/golden/report.html";

#[derive(serde::Deserialize)]
struct Catalog {
    cases: Vec<Case>,
}

#[derive(serde::Deserialize, Clone)]
struct Case {
    #[serde(rename = "type")]
    typ: String,
    name: String,
    #[serde(default)]
    liemermaid: bool,
    #[serde(default = "default_true")]
    compare: bool,
    source: String,
}

fn default_true() -> bool {
    true
}

fn load_catalog() -> Catalog {
    let raw = fs::read_to_string(Path::new(CASES_DIR).join("catalog.json")).expect("read catalog.json");
    serde_json::from_str(&raw).expect("parse catalog.json")
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// 复用 official_compare 的差异摘要逻辑（语义层 + 结构层）。
fn describe_diff(ours_svg: &str, golden_svg: &str) -> String {
    let ours_sem = semantics::extract(ours_svg, false);
    let golden_sem = semantics::extract(golden_svg, true);
    let sd = semantics::compare(&ours_sem, &golden_sem);
    let mut detail = String::new();
    if !sd.is_empty() {
        detail.push_str("SEMANTICS:\n");
        detail.push_str(&sd.describe());
    }
    let ours_sum = svgdiff::summarize(&svgdiff::parse(ours_svg));
    let golden_sum = svgdiff::summarize(&svgdiff::parse(golden_svg));
    let dd = svgdiff::compare(&ours_sum, &golden_sum);
    if !dd.is_empty() {
        detail.push_str("\nSVGDIFF:\n");
        detail.push_str(&dd.describe());
    }
    detail
}

#[test]
fn generate_html_report() {
    let catalog = load_catalog();
    let cases = &catalog.cases;

    // 按 type 分组，构建导航索引
    let mut by_type: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, c) in cases.iter().enumerate() {
        by_type.entry(c.typ.clone()).or_default().push(i);
    }

    let mut html = String::new();
    html.push_str(
        "<!DOCTYPE html>\n<html lang=\"zh\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>liemermaid vs official mermaid — SVG 对比报告</title>\n\
         <style>\n",
    );
    html.push_str(
        "body{font-family:-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif;\
         margin:0;background:#f5f6f8;color:#222}\n\
         header{background:#1f2937;color:#fff;padding:16px 24px;position:sticky;top:0;z-index:10}\n\
         header h1{margin:0;font-size:18px}\n\
         nav{padding:12px 24px;background:#fff;border-bottom:1px solid #e5e7eb;line-height:1.9}\n\
         nav a{display:inline-block;margin:2px 8px 2px 0;color:#2563eb;text-decoration:none;\
         font-size:13px;padding:2px 8px;border:1px solid #dbeafe;border-radius:10px}\n\
         nav a:hover{background:#eff6ff}\n\
         .case{border:1px solid #e5e7eb;background:#fff;margin:20px 24px;border-radius:10px;overflow:hidden}\n\
         .case h2{margin:0;padding:12px 16px;font-size:15px;background:#f9fafb;border-bottom:1px solid #eee}\n\
         .case h2 .tag{font-weight:400;color:#888;font-size:12px;margin-left:8px}\n\
         .src{margin:0;padding:0;border-bottom:1px solid #eee}\n\
         .src summary{cursor:pointer;padding:8px 16px;font-size:12px;color:#2563eb;background:#f8fafc;user-select:none}\n\
         .src summary:hover{background:#eff6ff}\n\
         .src pre{margin:0;border-radius:0}\n\
         .cols{display:grid;grid-template-columns:1fr 1fr;gap:0}\n\
         @media(max-width:1100px){.cols{grid-template-columns:1fr}}\n\
         .col{border-right:1px solid #eee;padding:12px;min-width:0}\n\
         .col:last-child{border-right:none}\n\
         .col h3{margin:0 0 8px;font-size:12px;text-transform:uppercase;letter-spacing:.5px;color:#666}\n\
         .badge{display:inline-block;font-size:11px;font-weight:600;padding:1px 8px;border-radius:8px;margin-left:6px;vertical-align:middle}\n\
         .badge.ours{background:#dbeafe;color:#1d4ed8}\n\
         .badge.gold{background:#dcfce7;color:#15803d}\n\
         .svg-box{overflow:auto;border:1px solid #eee;border-radius:6px;background:#fff;padding:8px}\n\
         .svg-box svg{max-width:100%;height:auto}\n\
         pre{white-space:pre-wrap;word-break:break-word;font-family:ui-monospace,Menlo,Consolas,monospace;\
         font-size:12px;background:#0f172a;color:#e2e8f0;padding:10px;border-radius:6px;margin:0}\n\
         .diff{margin:0;padding:12px 16px;background:#fffbeb;border-top:1px solid #fde68a;\
         white-space:pre-wrap;font-family:ui-monospace,Menlo,Consolas,monospace;font-size:12px;color:#713f12}\n\
         .missing{color:#b91c1c;font-style:italic}\n\
         </style></head>\n<body>\n",
    );
    html.push_str(
        "<header><h1>liemermaid vs 官方 mermaid — SVG 对比报告</h1></header>\n",
    );

    // 导航
    html.push_str("<nav>\n");
    for (typ, idxs) in &by_type {
        let first = idxs[0];
        html.push_str(&format!(
            "<a href=\"#case-{}\">{}</a> <span style=\"color:#aaa;font-size:12px\">({})</span> &nbsp; ",
            first, typ, idxs.len()
        ));
    }
    html.push_str("</nav>\n");

    let mut rendered = 0usize;
    let mut with_official = 0usize;
    for (i, c) in cases.iter().enumerate() {
        let key = format!("{}__{}", c.typ, c.name);
        let src_path = Path::new(CASES_DIR).join(&c.source);
        let golden_path = Path::new(GOLDEN_DIR).join(format!("{}.svg", key));
        let src = fs::read_to_string(&src_path).unwrap_or_else(|e| format!("(read mmd error: {e})"));
        let ours = match liemermaid::render(&src, 900, 700) {
            Ok(s) => s,
            Err(e) => {
                html.push_str(&format!(
                    "<div class=\"case\"><h2 id=\"case-{i}\">{key} <span class=\"tag\">render error</span></h2>\
                     <pre>{}</pre></div>\n",
                    escape_html(&format!("{e:?}"))
                ));
                continue;
            }
        };
        rendered += 1;

        let (official_html, diff_html) = if golden_path.exists() {
            with_official += 1;
            let golden = fs::read_to_string(&golden_path).expect("read golden");
            let diff = if c.compare { describe_diff(&ours, &golden) } else { String::new() };
            let diff_block = if diff.is_empty() {
                String::new()
            } else {
                format!("<div class=\"diff\">{}</div>\n", escape_html(&diff))
            };
            (
                format!("<div class=\"svg-box\">{golden}</div>"),
                diff_block,
            )
        } else {
            (
                "<div class=\"svg-box\"><span class=\"missing\">（无官方 golden SVG）</span></div>".to_string(),
                String::new(),
            )
        };

        html.push_str(&format!(
            "<div class=\"case\">\n\
             <h2 id=\"case-{i}\">{key} <span class=\"tag\">{}</span></h2>\n\
             <details class=\"src\"><summary>查看 Mermaid 源码</summary><pre>{}</pre></details>\n\
             <div class=\"cols\">\n\
             <div class=\"col\"><h3>liemermaid 输出<span class=\"badge ours\">ours</span></h3><div class=\"svg-box\">{}</div></div>\n\
             <div class=\"col\"><h3>官方 mermaid 输出<span class=\"badge gold\">golden</span></h3>{}</div>\n\
             </div>\n\
             {}\
             </div>\n",
            if c.liemermaid { "liemermaid 支持" } else { "官方对比" },
            escape_html(&src),
            ours,
            official_html,
            diff_html,
        ));
    }

    html.push_str("</body>\n</html>\n");

    fs::create_dir_all(Path::new(REPORT_PATH).parent().unwrap()).ok();
    fs::write(REPORT_PATH, &html).expect("write report.html");
    println!(
        "HTML report written: {} (rendered {}, with official golden {})",
        REPORT_PATH, rendered, with_official
    );
    assert!(Path::new(REPORT_PATH).exists(), "report.html not generated");
}
