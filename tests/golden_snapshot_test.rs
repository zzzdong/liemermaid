//! # 视觉快照测试（结构化对比）
//!
//! 本测试读取 `tests/golden/cases/catalog.json` 中的用例，对每个用例：
//!
//! 1. 读取官方 mermaid-cli 生成的"黄金样本" SVG（`tests/golden/golden/{type}__{name}.svg`）
//! 2. 用 liemermaid 解析同一段 Mermaid 源码并渲染为 SVG
//! 3. 解析两侧 SVG 的 DOM 结构，提取节点中心坐标 / 尺寸
//! 4. 做三层结构化对比：
//!    - 节点数量一致（硬断言）
//!    - 归一化坐标下节点中心距离 < 容差（软断言，附差异报告）
//!    - 同层 Y 对齐（硬断言，针对 flowchart）
//!
//! 对比采用"结构化"而非逐字节 diff，因为两侧的样式、文本测宽、边路径
//! 语义均不同（官方输出端口交点，liemermaid 输出中心路由点）。
//! 核心目标是验证**布局算法对齐**：节点放对层、对的位置、对的大小。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

// ============================================================
// 用例目录结构
// ============================================================

#[derive(Debug, Deserialize)]
struct Case {
    #[serde(rename = "type")]
    ty: String,
    name: String,
    #[serde(default)]
    rankdir: Option<String>,
    width: u32,
    height: u32,
    source: String,
    /// liemermaid 是否支持该语法（false 时仅保留官方参考 SVG）。
    #[serde(default = "default_true")]
    liemermaid: bool,
    /// 本快照对比是否已实现（false 时跳过结构化对拍，但仍保留官方参考 SVG）。
    #[serde(default = "default_true")]
    compare: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct Catalog {
    cases: Vec<Case>,
}

fn load_catalog() -> Catalog {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/cases/catalog.json"
    );
    let text = std::fs::read_to_string(path).expect("catalog.json missing");
    serde_json::from_str(&text).expect("invalid catalog.json")
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

// ============================================================
// SVG 解析（通用 DOM 提取）
// ============================================================

#[derive(Debug, Clone, Copy)]
struct NodeBox {
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
}

impl NodeBox {
    fn from_rect(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self {
            cx: x + w / 2.0,
            cy: y + h / 2.0,
            w,
            h,
        }
    }
}

/// 从一段属性文本中提取 `name="value"` 形式的数值属性。
fn attr_f64(s: &str, name: &str) -> Option<f64> {
    let needle = format!(" {}=\"", name);
    let idx = s.find(&needle)?;
    let start = idx + needle.len();
    let end = s[start..].find('"')?;
    s[start..start + end].parse::<f64>().ok()
}

/// 提取 `<g ... transform="translate(X, Y)">` 的 (X, Y)。
fn translate_of(s: &str) -> Option<(f64, f64)> {
    let idx = s.find("translate(")?;
    let start = idx + "translate(".len();
    let end = s[start..].find(')')?;
    let inner = &s[start..start + end];
    let mut parts = inner.split(',');
    let x = parts.next()?.trim().parse::<f64>().ok()?;
    let y = parts.next()?.trim().parse::<f64>().ok()?;
    Some((x, y))
}

// ---- 官方 mermaid SVG 解析 ----
//
// 官方 flowchart SVG 结构（节点）：
//   <g class="node default" id="A" transform="translate(140, 80)">
//     <rect .../> 或 <polygon .../> 或 <path .../>
//     <g class="label">...</g>
//   </g>
// 节点中心 = `<g>` 的 translate 值；节点尺寸 = 内部形状的外包矩形。

/// 解析官方 mermaid SVG，返回节点 id -> (中心, 外包尺寸)。
///
/// 官方 mermaid-cli 输出是**单行压缩** SVG，不能用逐行解析。这里按字符串扫描：
/// 找到每个节点包装 `<g class="node" id="..." transform="translate(cx,cy)">`，
/// 从打开标签提取 id 与中心坐标；再从该包装内部的形状元素（`<rect>`/`<polygon>`
/// `/`<path>`，在 `<g class="label"` 之前）提取节点尺寸。
fn parse_official_nodes(svg: &str) -> HashMap<String, NodeBox> {
    let mut out = HashMap::new();
    let bytes = svg.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        // 查找下一个节点包装 <g class="node"
        let Some(marker) = find_at(bytes, i, b"<g class=\"node") else {
            break;
        };
        // 打开标签：从 marker 到下一个 '>'
        let open_end = find_char(bytes, marker, b'>').unwrap_or(n);
        let open_tag = &svg[marker..open_end];

        let Some(id) = extract_attr(open_tag, "id") else {
            i = open_end + 1;
            continue;
        };
        let Some((cx, cy)) = translate_of(open_tag) else {
            i = open_end + 1;
            continue;
        };

        // 节点尺寸：扫描包装内部（open_end 之后到 <g class="label" 或 </g>）的形状
        let body_end = find_str(&svg[open_end + 1..], b"<g class=\"label")
            .map(|off| open_end + 1 + off)
            .unwrap_or(open_end + 1 + 2048);
        let body = &svg[open_end + 1..body_end.min(n)];
        let (w, h) = shape_size(body).unwrap_or((0.0, 0.0));

        out.insert(id, NodeBox { cx, cy, w, h });
        // 跳到下一个包装：从当前 body_end 之后继续
        i = body_end + 1;
    }
    out
}

/// 在 `s` 的字节序列中查找 `needle`，返回其字节偏移。
fn find_at(s: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= s.len() {
        return None;
    }
    s[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

fn find_char(s: &[u8], from: usize, c: u8) -> Option<usize> {
    s[from..].iter().position(|&b| b == c).map(|p| from + p)
}

/// 在 `s`（UTF-8）中查找子串 `needle`，返回相对偏移。
fn find_str(s: &str, needle: &[u8]) -> Option<usize> {
    s.as_bytes()
        .windows(needle.len())
        .position(|w| w == needle)
}

/// 提取 `name="value"` 属性的值。
fn extract_attr(s: &str, name: &str) -> Option<String> {
    let needle = format!(" {}=\"", name);
    let idx = s.find(&needle)?;
    let start = idx + needle.len();
    let end = s[start..].find('"')?;
    Some(s[start..start + end].to_string())
}

/// 从一段文本中计算第一个形状元素（rect/polygon/path）的外包尺寸。
fn shape_size(body: &str) -> Option<(f64, f64)> {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut found = false;

    // 扫描第一个形状：rect/polygon/path
    for marker in ["<rect ", "<polygon ", "<path "] {
        let Some(pos) = find_str(body, marker.as_bytes()) else {
            continue;
        };
        let end = find_char(body.as_bytes(), pos, b'>').unwrap_or(body.len());
        let tag = &body[pos..end];
        if marker == "<rect " {
            if let (Some(x), Some(y), Some(w), Some(h)) = (
                attr_f64(tag, "x"),
                attr_f64(tag, "y"),
                attr_f64(tag, "width"),
                attr_f64(tag, "height"),
            ) {
                // 跳过空 rect（如 <rect/> label 占位）
                if w > 0.0 && h > 0.0 {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x + w);
                    max_y = max_y.max(y + h);
                    found = true;
                }
            }
        } else if let Some(pts) = extract_points(tag) {
            for (px, py) in pts {
                min_x = min_x.min(px);
                min_y = min_y.min(py);
                max_x = max_x.max(px);
                max_y = max_y.max(py);
            }
            found = true;
        }
        if found {
            break;
        }
    }
    if found && min_x != f64::MAX {
        Some((max_x - min_x, max_y - min_y))
    } else {
        None
    }
}

/// 提取 `points="x,y x,y ..."` 或 `<path ... d="M... L...">` 的坐标点集。
/// 对 polygon 返回折线顶点；对 path 返回所有 M/L 关键点。
fn extract_points(tag: &str) -> Option<Vec<(f64, f64)>> {
    if let Some(points_idx) = tag.find("points=\"") {
        let start = points_idx + "points=\"".len();
        let end = tag[start..].find('"')?;
        let inner = &tag[start..start + end];
        let mut pts = Vec::new();
        for tok in inner.split(' ') {
            if let Some((x, y)) = parse_xy(tok) {
                pts.push((x, y));
            }
        }
        if !pts.is_empty() {
            return Some(pts);
        }
    }
    if tag.contains("<path ") {
        return extract_path_points(tag);
    }
    None
}

fn parse_xy(tok: &str) -> Option<(f64, f64)> {
    let mut it = tok.splitn(2, ',');
    let x = it.next()?.parse::<f64>().ok()?;
    let y = it.next()?.parse::<f64>().ok()?;
    Some((x, y))
}

/// 解析 path 的 d 属性，返回所有关键点。
///
/// 兼容两种写法：
/// - `M 10 20 L 30 40`（命令与坐标分开）
/// - `M10,20L30,40`（命令与坐标粘连，mermaid 常用）
/// 以及曲线命令 `C`/`Q`（取其控制点与端点参与外包计算）。
fn extract_path_points(tag: &str) -> Option<Vec<(f64, f64)>> {
    let idx = tag.find(" d=\"")?;
    let start = idx + " d=\"".len();
    let end = tag[start..].find('"')?;
    let d = &tag[start..start + end];
    let mut pts: Vec<(f64, f64)> = Vec::new();

    // 按字符扫描：命令字母（M/L/C/Q/Z/H/V）之后跟随坐标。
    let bytes = d.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let is_cmd = matches!(c, b'M' | b'L' | b'C' | b'Q' | b'H' | b'V' | b'S' | b'T' | b'Z' | b'z');
        if is_cmd {
            // 跳过 Z/z（闭合，无坐标）
            if c == b'Z' || c == b'z' {
                i += 1;
                continue;
            }
            // 从命令后收集坐标对（到下一个命令字母为止）
            let mut j = i + 1;
            while j < bytes.len() && !is_cmd_char(bytes[j]) {
                j += 1;
            }
            let coords = &d[i + 1..j];
            // 用非数字字符分隔出数值序列
            let mut nums: Vec<f64> = Vec::new();
            let mut cur = String::new();
            for ch in coords.chars() {
                if ch.is_ascii_digit() || ch == '-' || ch == '+' || ch == '.' || ch == 'e' || ch == 'E' {
                    cur.push(ch);
                } else if !cur.is_empty() {
                    if let Ok(v) = cur.parse::<f64>() {
                        nums.push(v);
                    }
                    cur.clear();
                }
            }
            if !cur.is_empty() {
                if let Ok(v) = cur.parse::<f64>() {
                    nums.push(v);
                }
            }
            // 每 2 个数组成一个点
            let mut k = 0;
            while k + 1 < nums.len() {
                pts.push((nums[k], nums[k + 1]));
                k += 2;
            }
            i = j;
            continue;
        }
        i += 1;
    }

    if pts.is_empty() {
        None
    } else {
        Some(pts)
    }
}

fn is_cmd_char(c: u8) -> bool {
    matches!(
        c,
        b'M' | b'L' | b'C' | b'Q' | b'H' | b'V' | b'S' | b'T' | b'Z' | b'z'
    )
}

// ---- liemermaid SVG 解析 ----

/// 从 `<svg ... width="W" height="H">` 根元素解析画布尺寸。
fn parse_canvas_size(svg: &str) -> Option<(f64, f64)> {
    let root = svg.lines().find(|l| l.contains("<svg"))?;
    let w = attr_f64(root, "width")?;
    let h = attr_f64(root, "height")?;
    Some((w, h))
}

/// 解析 liemermaid 的 SVG：返回节点矩形列表（含画布变换修正）。
/// liemermaid 输出 `<rect>`（矩形/圆角/圆）与 `<path>`（菱形）等形状，
/// 外包在 `<g transform="matrix(...)">` 中用于画布适配。这里提取所有形状的
/// 外包矩形作为节点 box（已去重），并排除铺满画布的背景矩形。
fn parse_ours_boxes(svg: &str) -> Vec<NodeBox> {
    let canvas = parse_canvas_size(svg);
    let mut rects: Vec<(f64, f64, f64, f64)> = Vec::new(); // x,y,w,h
    for line in svg.lines() {
        let mut candidate: Option<(f64, f64, f64, f64)> = None; // x,y,w,h
        if line.contains("<rect ") {
            if let (Some(x), Some(y), Some(w), Some(h)) = (
                attr_f64(line, "x"),
                attr_f64(line, "y"),
                attr_f64(line, "width"),
                attr_f64(line, "height"),
            ) {
                // 跳过铺满画布的背景矩形
                if let Some((cw, ch)) = canvas
                    && (w - cw).abs() < 0.5
                    && (h - ch).abs() < 0.5
                {
                    continue;
                }
                candidate = Some((x, y, w, h));
            }
        } else if line.contains("<path ") && line.contains("d=\"M") {
            if let Some(pts) = extract_path_points(line) {
                let xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
                let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
                let minx = xs.iter().cloned().fold(f64::MAX, f64::min);
                let miny = ys.iter().cloned().fold(f64::MAX, f64::min);
                let maxx = xs.iter().cloned().fold(f64::MIN, f64::max);
                let maxy = ys.iter().cloned().fold(f64::MIN, f64::max);
                candidate = Some((minx, miny, maxx - minx, maxy - miny));
            }
        }
        // 过滤箭头/标记等微小形状（尺寸明显小于节点）。节点至少 ~36px 高，
        // 箭头三角约 10px。此阈值不影响任何节点形状。
        if let Some((x, y, w, h)) = candidate
            && w > 12.0
            && h > 12.0
        {
            rects.push((x, y, w, h));
        }
    }
    dedup_rects(&rects)
        .into_iter()
        .map(|(x, y, w, h)| NodeBox::from_rect(x, y, w, h))
        .collect()
}

fn dedup_rects(rects: &[(f64, f64, f64, f64)]) -> Vec<(f64, f64, f64, f64)> {
    let mut out: Vec<(f64, f64, f64, f64)> = Vec::new();
    for r in rects {
        if !out.iter().any(|o: &(f64, f64, f64, f64)| {
            (o.0 - r.0).abs() < 0.5
                && (o.1 - r.1).abs() < 0.5
                && (o.2 - r.2).abs() < 0.5
                && (o.3 - r.3).abs() < 0.5
        }) {
            out.push(*r);
        }
    }
    out
}

// ============================================================
// 结构化对比辅助
// ============================================================

/// 将一组坐标点归一化到 [0,1] 包围盒。
fn normalize(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.is_empty() {
        return Vec::new();
    }
    let mut minx = f64::MAX;
    let mut miny = f64::MAX;
    let mut maxx = f64::MIN;
    let mut maxy = f64::MIN;
    for &(x, y) in points {
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    }
    let w = (maxx - minx).max(1.0);
    let h = (maxy - miny).max(1.0);
    points
        .iter()
        .map(|&(x, y)| ((x - minx) / w, (y - miny) / h))
        .collect()
}

/// 计算两点距离。
fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

// ============================================================
// 主测试
// ============================================================

/// 单个用例的执行结果。
enum CaseOutcome {
    /// 黄金样本缺失（跳过）。
    MissingGolden,
    /// 类型/语法当前不支持（跳过）。本套件当前只做 flowchart 布局对拍。
    Skipped,
    /// 渲染/解析/断言失败（含库内部 panic）。
    Error(String),
    /// 通过（hard 为真表示硬断言通过，coord/size 为软断言结果）。
    Done {
        hard: bool,
        coord: bool,
        size: bool,
        node_ours: usize,
        node_official: usize,
        layer_ours: usize,
        layer_official: usize,
    },
}

const COORD_TOL: f64 = 0.15; // 归一化坐标容差（[0,1] 空间）。同层节点可左右互换，故放宽。
const Y_ALIGN_TOL: f64 = 1.0; // 同层节点中心 Y 偏差（绝对 px）

#[test]
fn golden_snapshot_structural_match() {
    let catalog = load_catalog();
    assert!(
        !catalog.cases.is_empty(),
        "no cases in golden catalog; see tests/golden/cases/catalog.json"
    );

    let mut total = 0;
    let mut hard_pass = 0;
    let mut soft_pass = 0;
    let mut missing_golden = Vec::new();
    let mut failures = Vec::new();

    for c in &catalog.cases {
        total += 1;
        match run_single_case(c) {
            CaseOutcome::MissingGolden => missing_golden.push(format!("{}__{}", c.ty, c.name)),
            CaseOutcome::Skipped => {
                // 非 flowchart / 语法暂不支持，跳过（不计入总数）
                total -= 1;
            }
            CaseOutcome::Error(msg) => {
                failures.push(format!("{}__{}: {}", c.ty, c.name, msg));
            }
            CaseOutcome::Done {
                hard,
                coord,
                size,
                node_ours,
                node_official,
                layer_ours,
                layer_official,
            } => {
                if hard {
                    hard_pass += 1;
                }
                if coord {
                    soft_pass += 1;
                }
                eprintln!(
                    "[{}__{}] nodes: ours={} official={} layers: ours={} official={}  hard={} soft={} size={}",
                    c.ty,
                    c.name,
                    node_ours,
                    node_official,
                    layer_ours,
                    layer_official,
                    if hard { "OK" } else { "DIFF" },
                    if coord { "OK" } else { "DIFF" },
                    if size { "OK" } else { "DIFF" }
                );
                if !hard {
                    failures.push(format!("{}__{}: HARD assertion failed", c.ty, c.name));
                }
            }
        }
    }

    if !missing_golden.is_empty() {
        eprintln!(
            "\nSKIPPED {} case(s) without golden SVG (run `node tests/golden/generate_golden.js`): {:?}",
            missing_golden.len(),
            missing_golden
        );
    }
    if !failures.is_empty() {
        eprintln!("\nCase failures:");
        for f in &failures {
            eprintln!("  ✗ {}", f);
        }
    }

    eprintln!(
        "\n=== golden_snapshot: {}/{} cases hard-pass, {}/{} soft-pass (coord tol={}) ===",
        hard_pass, total, soft_pass, total, COORD_TOL
    );
    assert!(
        hard_pass >= total - missing_golden.len(),
        "{} case(s) failed hard assertions: {:?}",
        total - missing_golden.len() - hard_pass,
        failures
    );
}

/// 执行单个用例的结构化对比。用 catch_unwind 包裹，使库内部 panic
/// 被捕获并转成 `CaseOutcome::Error`，避免一个用例崩溃中断整个套件。
fn run_single_case(c: &Case) -> CaseOutcome {
    // 当前结构化对拍仅针对 flowchart（dagre 布局）实现节点位置/尺寸解析。
    // 其他类型（sequence/class/state/er/pie/timeline/gitgraph）走不同布局引擎，
    // SVG 结构不同，待后续实现各自的解析器后再纳入。
    // `compare: false` 的用例（如 subgraph/shapes）因解析器尚未覆盖容器框/曲线
    // 形状，也先跳过对拍（官方参考 SVG 仍保留）。
    if c.ty != "flowchart" || !c.liemermaid || !c.compare {
        return CaseOutcome::Skipped;
    }

    let case_name = format!("{}__{}", c.ty, c.name);
    let golden_path = golden_dir()
        .join("golden")
        .join(format!("{}.svg", case_name));
    let src_path = golden_dir().join("cases").join(&c.source);

    if !golden_path.exists() {
        return CaseOutcome::MissingGolden;
    }

    let result = std::panic::catch_unwind(|| {
        let golden_svg = std::fs::read_to_string(&golden_path)
            .unwrap_or_else(|_| panic!("cannot read golden {}", golden_path.display()));
        let src = std::fs::read_to_string(&src_path)
            .unwrap_or_else(|_| panic!("cannot read source {}", src_path.display()));

        // 官方节点（含 id + 中心 + 尺寸）
        let official = parse_official_nodes(&golden_svg);
        // liemermaid 节点（仅中心 + 尺寸）
        let ours = parse_ours_boxes(&liemermaid::render(&src, c.width, c.height).expect("render"));

        (official, ours)
    });

    let (official, ours) = match result {
        Ok(v) => v,
        Err(e) => {
            let msg = match e.downcast_ref::<&str>() {
                Some(s) => (*s).to_string(),
                None => match e.downcast_ref::<String>() {
                    Some(s) => s.clone(),
                    None => "panic (no message)".to_string(),
                },
            };
            return CaseOutcome::Error(msg);
        }
    };

    // --- 硬断言 1: 节点数量一致 ---
    assert_eq!(
        ours.len(),
        official.len(),
        "[{}] node count mismatch: ours={} official={}",
        case_name,
        ours.len(),
        official.len()
    );

    // --- 归一化坐标集合对比 ---
    let official_pts: Vec<(f64, f64)> = official.values().map(|n| (n.cx, n.cy)).collect();
    let ours_pts: Vec<(f64, f64)> = ours.iter().map(|n| (n.cx, n.cy)).collect();
    let official_norm = normalize(&official_pts);
    let ours_norm = normalize(&ours_pts);

    // 同层集合：按主轴向（TB/BT 用 y，LR/RL 用 x）对官方节点分组，
    // 要求我们的节点在同层数量一致（拓扑正确性）。
    let official_boxes: Vec<NodeBox> = official.values().copied().collect();
    let official_layers = layers_by_main_boxes(c, &official_boxes);
    let ours_layers = layers_by_main_boxes(c, &ours);
    assert_eq!(
        official_layers.len(),
        ours_layers.len(),
        "[{}] layer count mismatch: official={} ours={}",
        case_name,
        official_layers.len(),
        ours_layers.len()
    );

    // 每个官方层应能在我们的层中找到同大小集合（容忍左右顺序）。
    let mut coord_ok = true;
    let mut layer_diff = false;
    for (oidx, olayer) in official_layers.iter().enumerate() {
        let matched = ours_layers.iter().any(|ol| same_count(ol, olayer.len()));
        if !matched {
            layer_diff = true;
            eprintln!(
                "[{}] layer {} ({} nodes) has no same-size layer in ours",
                case_name,
                oidx,
                olayer.len()
            );
        }
    }
    // 坐标距离：对每个官方节点，找最近我们的节点，度量误差。
    for on in &official_norm {
        let mut best = f64::MAX;
        for om in &ours_norm {
            let d = dist(*on, *om);
            if d < best {
                best = d;
            }
        }
        if best > COORD_TOL {
            coord_ok = false;
            eprintln!(
                "[{}] nearest-node distance {} > tol {}",
                case_name, best, COORD_TOL
            );
        }
    }
    // 尺寸对比（软）：对每个官方节点，找最近我们的节点，比较宽高（相对容差）。
    // 官方 mermaid 用真实文本测宽，liemermaid 用近似宽度，故用相对误差。
    let mut size_ok = true;
    for on in &official_boxes {
        let mut best_idx = 0usize;
        let mut best = f64::MAX;
        for (k, om) in ours.iter().enumerate() {
            let d = dist((on.cx, on.cy), (om.cx, om.cy));
            if d < best {
                best = d;
                best_idx = k;
            }
        }
        let matched = &ours[best_idx];
        // 相对尺寸容差（默认 60%，允许文本测宽差异；尺寸不参与硬断言）
        let size_tol = 0.6;
        if (matched.w - on.w).abs() > size_tol * on.w.max(1.0)
            || (matched.h - on.h).abs() > size_tol * on.h.max(1.0)
        {
            size_ok = false;
            eprintln!(
                "[{}] size mismatch: official=(w={:.1},h={:.1}) ours=(w={:.1},h={:.1})",
                case_name, on.w, on.h, matched.w, matched.h
            );
        }
    }

    // --- 硬断言 2: 同层 Y 对齐（ours 内部） ---
    let yalign_ok = check_y_alignment(&ours, c, Y_ALIGN_TOL, &case_name);

    let hard_ok = yalign_ok && !layer_diff;

    CaseOutcome::Done {
        hard: hard_ok,
        coord: coord_ok,
        size: size_ok,
        node_ours: ours.len(),
        node_official: official.len(),
        layer_ours: ours_layers.len(),
        layer_official: official_layers.len(),
    }
}

/// `nodes` 按主轴向分成若干"层"。
fn layers_by_main_boxes(c: &Case, nodes: &[NodeBox]) -> Vec<Vec<f64>> {
    // 缺省按 TB（自上而下）处理；只有显式 LR/RL 才用 X 作主轴向。
    let horizontal = matches!(c.rankdir.as_deref(), Some("LR" | "RL"));
    let primary = |n: &NodeBox| -> f64 {
        if horizontal {
            n.cx
        } else {
            n.cy
        }
    };
    let mut vals: Vec<f64> = nodes.iter().map(primary).collect();
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut layers: Vec<Vec<f64>> = Vec::new();
    let mut prev: Option<f64> = None;
    let mut cur: Vec<f64> = Vec::new();
    for v in vals {
        if let Some(p) = prev
            && (v - p).abs() > 5.0
        {
            layers.push(std::mem::take(&mut cur));
        }
        cur.push(v);
        prev = Some(v);
    }
    if !cur.is_empty() {
        layers.push(cur);
    }
    layers
}

fn same_count(v: &[f64], n: usize) -> bool {
    v.len() == n
}

/// 检查 liemermaid 内部同层节点中心 Y 对齐（硬断言）。
fn check_y_alignment(ours: &[NodeBox], c: &Case, tol: f64, name: &str) -> bool {
    let layers = layers_by_main_boxes(c, ours);
    let mut ok = true;
    for (i, layer) in layers.iter().enumerate() {
        if layer.len() < 2 {
            continue;
        }
        // layer 存的是主轴坐标值；对 TB 是同层 Y，对 LR 是同层 X。
        let mn = layer.iter().cloned().fold(f64::MAX, f64::min);
        let mx = layer.iter().cloned().fold(f64::MIN, f64::max);
        if (mx - mn) > tol {
            ok = false;
            eprintln!(
                "[{}] ours layer {} span {:.2} > tol {}",
                name,
                i,
                mx - mn,
                tol
            );
        }
    }
    ok
}
