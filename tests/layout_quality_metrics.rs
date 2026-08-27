//! 布局质量度量（baseline 锚点 + 后续 P1.4 对比）。
//!
//! 与设计文档 `redesign-task-plan.md` 的 P0.2 对应：建立"边交叉数 / 边重叠数 /
//! 线穿节点数"三类指标的 baseline 数字表，作为后续新管线（边感知布局）验收的对照锚点。
//!
//! 度量层级说明（与计划偏差，见 redesign-task-plan.md §5）：
//! 计划原拟"基于 PlacedGraph 几何统计"，但现有基础设施是 **SVG 黑盒解析**
//! （`parse_svg` 提取 rect + line/polyline 段）。采用 SVG 层统计更现实、能立刻跨
//! 新旧管线对比，且口径一致即公平。旧管线（`liemermaid::render`）产出即 baseline。

use serde::Deserialize;

// ============================================================
// SVG 解析（复制自 layout_quality_test.rs，保持口径一致、零耦合）
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
    fn intersects(&self, other: &Rect) -> bool {
        !(self.x + self.w <= other.x
            || other.x + other.w <= self.x
            || self.y + self.h <= other.y
            || other.y + other.h <= self.y)
    }
    fn segment_crosses(&self, x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
        let inside1 = self.contains(x1, y1);
        let inside2 = self.contains(x2, y2);
        if inside1 && inside2 {
            return false;
        }
        if inside1 || inside2 {
            return true;
        }
        cohen_sutherland_clip(
            x1, y1, x2, y2, self.x, self.x + self.w, self.y, self.y + self.h,
        )
        .is_some()
    }
}

#[allow(clippy::too_many_arguments)]
fn cohen_sutherland_clip(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
) -> Option<(f64, f64, f64, f64)> {
    const INSIDE: u8 = 0;
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const BOTTOM: u8 = 4;
    const TOP: u8 = 8;
    let code = |x: f64, y: f64| -> u8 {
        let mut c = INSIDE;
        if x < xmin {
            c |= LEFT;
        } else if x > xmax {
            c |= RIGHT;
        }
        if y < ymin {
            c |= BOTTOM;
        } else if y > ymax {
            c |= TOP;
        }
        c
    };
    let (mut x1, mut y1, mut x2, mut y2) = (x1, y1, x2, y2);
    loop {
        let c1 = code(x1, y1);
        let c2 = code(x2, y2);
        if c1 | c2 == 0 {
            return Some((x1, y1, x2, y2));
        }
        if c1 & c2 != 0 {
            return None;
        }
        let outcode = if c1 != 0 { c1 } else { c2 };
        let (x, y) = if outcode & TOP != 0 {
            (x1 + (x2 - x1) * (ymax - y1) / (y2 - y1), ymax)
        } else if outcode & BOTTOM != 0 {
            (x1 + (x2 - x1) * (ymin - y1) / (y2 - y1), ymin)
        } else if outcode & RIGHT != 0 {
            (xmax, y1 + (y2 - y1) * (xmax - x1) / (x2 - x1))
        } else {
            (xmin, y1 + (y2 - y1) * (xmin - x1) / (x2 - x1))
        };
        if outcode == c1 {
            x1 = x;
            y1 = y;
        } else {
            x2 = x;
            y2 = y;
        }
    }
}

fn parse_canvas_size(svg: &str) -> Option<(f64, f64)> {
    let root = svg.lines().find(|l| l.contains("<svg"))?;
    let w = parse_attr(root, " width=\"")?;
    let h = parse_attr(root, " height=\"")?;
    Some((w, h))
}

fn parse_svg(svg: &str) -> (Vec<Rect>, Vec<(f64, f64, f64, f64)>) {
    let mut raw_rects = Vec::new();
    let mut segs = Vec::new();
    let canvas = parse_canvas_size(svg);
    for line in svg.lines() {
        if line.contains("<rect ")
            && !line.contains("edge-label")
            && let (Some(x), Some(y), Some(w), Some(h)) = (
                parse_attr(line, " x=\""),
                parse_attr(line, " y=\""),
                parse_attr(line, " width=\""),
                parse_attr(line, " height=\""),
            )
        {
            if let Some((cw, ch)) = canvas
                && (w - cw).abs() < 0.5
                && (h - ch).abs() < 0.5
            {
                continue;
            }
            raw_rects.push(Rect { x, y, w, h });
        }
        if line.contains("<line ")
            && let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                parse_attr(line, " x1=\""),
                parse_attr(line, " y1=\""),
                parse_attr(line, " x2=\""),
                parse_attr(line, " y2=\""),
            )
        {
            segs.push((x1, y1, x2, y2));
        }
        if line.contains("<polyline ")
            && let Some(pts_str) = extract_polyline_points(line)
        {
            let pts: Vec<(f64, f64)> = pts_str
                .split(' ')
                .filter_map(|p| {
                    let mut parts = p.splitn(2, ',');
                    let x = parts.next()?.parse::<f64>().ok()?;
                    let y = parts.next()?.parse::<f64>().ok()?;
                    Some((x, y))
                })
                .collect();
            for i in 1..pts.len() {
                segs.push((pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1));
            }
        }
    }
    let rects = dedup_rects(&raw_rects);
    (rects, segs)
}

fn dedup_rects(rects: &[Rect]) -> Vec<Rect> {
    let mut out = Vec::new();
    for r in rects {
        if !out.iter().any(|o: &Rect| {
            (o.x - r.x).abs() < 0.5
                && (o.y - r.y).abs() < 0.5
                && (o.w - r.w).abs() < 0.5
                && (o.h - r.h).abs() < 0.5
        }) {
            out.push(*r);
        }
    }
    out
}

fn parse_attr(s: &str, attr: &str) -> Option<f64> {
    let idx = s.find(attr)?;
    let start = idx + attr.len();
    let end = s[start..].find('"')?;
    s[start..start + end].parse::<f64>().ok()
}

fn extract_polyline_points(s: &str) -> Option<&str> {
    let idx = s.find("points=\"")?;
    let start = idx + 8;
    let end = s[start..].find('"')?;
    Some(&s[start..start + end])
}

// ============================================================
// 三类度量函数（返回计数，不断言）
// ============================================================

/// 指标 1：边穿越非端点节点数。
/// 端点落在节点边框上（on_edge）视为合法，跳过。
fn count_line_through_node(rects: &[Rect], segs: &[(f64, f64, f64, f64)]) -> usize {
    let mut n = 0;
    for (x1, y1, x2, y2) in segs {
        for r in rects {
            let on_edge = |x: f64, y: f64, r: &Rect| -> bool {
                let eps = 0.5;
                let on_v = (x - r.x).abs() < eps || (x - (r.x + r.w)).abs() < eps;
                let on_h = (y - r.y).abs() < eps || (y - (r.y + r.h)).abs() < eps;
                (on_v && y >= r.y - eps && y <= r.y + r.h + eps)
                    || (on_h && x >= r.x - eps && x <= r.x + r.w + eps)
            };
            if on_edge(*x1, *y1, r) || on_edge(*x2, *y2, r) {
                continue;
            }
            if r.segment_crosses(*x1, *y1, *x2, *y2) {
                n += 1;
                break; // 一条边只计一次（无论穿几个节点）
            }
        }
    }
    n
}

/// 指标 2：边-边交叉数（两线段内部相交，排除共享端点）。
fn count_edge_crossings(segs: &[(f64, f64, f64, f64)]) -> usize {
    let mut n = 0;
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            let (a1, a2) = ((segs[i].0, segs[i].1), (segs[i].2, segs[i].3));
            let (b1, b2) = ((segs[j].0, segs[j].1), (segs[j].2, segs[j].3));
            // 共享端点不算交叉
            if a1 == b1 || a1 == b2 || a2 == b1 || a2 == b2 {
                continue;
            }
            if seg_intersect(a1, a2, b1, b2) {
                n += 1;
            }
        }
    }
    n
}

/// 指标 3：边重叠数（两线段近似共线且投影区间重叠）。
fn count_edge_overlaps(segs: &[(f64, f64, f64, f64)]) -> usize {
    let mut n = 0;
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            if seg_collinear_overlap(segs[i], segs[j]) {
                n += 1;
            }
        }
    }
    n
}

fn seg_intersect(
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    p4: (f64, f64),
) -> bool {
    let d = (p2.0 - p1.0) * (p4.1 - p3.1) - (p2.1 - p1.1) * (p4.0 - p3.0);
    if d.abs() < 1e-9 {
        return false;
    }
    let t = ((p3.0 - p1.0) * (p4.1 - p3.1) - (p3.1 - p1.1) * (p4.0 - p3.0)) / d;
    let u = ((p3.0 - p1.0) * (p2.1 - p1.1) - (p3.1 - p1.1) * (p2.0 - p1.0)) / d;
    t > 1e-9 && t < 1.0 - 1e-9 && u > 1e-9 && u < 1.0 - 1e-9
}

fn seg_collinear_overlap(
    s1: (f64, f64, f64, f64),
    s2: (f64, f64, f64, f64),
) -> bool {
    let (ax, ay, bx, by) = s1;
    let (cx, cy, dx, dy) = s2;
    // 方向向量
    let v1 = (bx - ax, by - ay);
    let v2 = (dx - cx, dy - cy);
    let cross = v1.0 * v2.1 - v1.1 * v2.0;
    if cross.abs() > 1e-3 {
        return false; // 不共线
    }
    // 投影到较长轴，判断区间重叠
    let overlap_axis = |a1: f64, a2: f64, b1: f64, b2: f64| -> bool {
        let (lo1, hi1) = if a1 < a2 { (a1, a2) } else { (a2, a1) };
        let (lo2, hi2) = if b1 < b2 { (b1, b2) } else { (b2, b1) };
        lo1 <= hi2 + 0.5 && lo2 <= hi1 + 0.5
    };
    if v1.0.abs() >= v1.1.abs() {
        overlap_axis(ax, bx, cx, dx)
    } else {
        overlap_axis(ay, by, cy, dy)
    }
}

// ============================================================
// catalog 遍历 + baseline 打印
// ============================================================

#[derive(Debug, Deserialize)]
struct Case {
    #[serde(rename = "type")]
    ty: String,
    name: String,
    width: u32,
    height: u32,
    source: String,
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
fn baseline_layout_metrics() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/cases/catalog.json"
    );
    let text = std::fs::read_to_string(path).expect("catalog.json missing");
    let catalog: Catalog = serde_json::from_str(&text).expect("invalid catalog.json");

    println!("\n# Layout Quality Baseline (old pipeline)\n");
    println!("| case | type | cross | overlap | through_node |");
    println!("|---|---|---|---|---|");

    let mut totals = (0usize, 0usize, 0usize);
    for c in &catalog.cases {
        if !c.liemermaid {
            continue;
        }
        let src_path =
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/cases/").to_string() + &c.source;
        let mermaid = match std::fs::read_to_string(&src_path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let svg = match liemermaid::render(&mermaid, c.width, c.height) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let (rects, segs) = parse_svg(&svg);
        let cross = count_edge_crossings(&segs);
        let overlap = count_edge_overlaps(&segs);
        let through = count_line_through_node(&rects, &segs);
        totals.0 += cross;
        totals.1 += overlap;
        totals.2 += through;
        println!(
            "| {}__{} | {} | {} | {} | {} |",
            c.ty, c.name, c.ty, cross, overlap, through
        );
    }
    println!("\n| **TOTAL** | | {} | {} | {} |\n", totals.0, totals.1, totals.2);
    println!("> baseline 锚点：P1.4 新管线 flowchart 重跑后，cross/overlap/through 应较此下降 ≥50%。");
}
