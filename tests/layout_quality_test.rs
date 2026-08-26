/// 辅助：解析 Mermaid 文本并通过 lievisual 后端渲染为 SVG 字符串
fn render(mermaid: &str, width: u32, height: u32) -> String {
    liemermaid::render(mermaid, width, height).expect("render")
}

// ============================================================
// 辅助结构：SVG 元素解析
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn center_x(&self) -> f64 {
        self.x + self.w / 2.0
    }
    fn center_y(&self) -> f64 {
        self.y + self.h / 2.0
    }
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
            x1,
            y1,
            x2,
            y2,
            self.x,
            self.x + self.w,
            self.y,
            self.y + self.h,
        )
        .is_some()
    }
}

#[allow(clippy::too_many_arguments)] // 纯函数式裁剪算法，参数即输入坐标与裁剪窗口
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

/// 从 `<svg ... width="W" height="H">` 根元素解析画布尺寸。
fn parse_canvas_size(svg: &str) -> Option<(f64, f64)> {
    let root = svg.lines().find(|l| l.contains("<svg"))?;
    let w = parse_attr(root, " width=\"")?;
    let h = parse_attr(root, " height=\"")?;
    Some((w, h))
}

/// 从 SVG 文本中提取节点矩形和边线段
#[allow(clippy::type_complexity)] // 测试辅助：矩形 + 线段集合
fn parse_svg(svg: &str) -> (Vec<Rect>, Vec<(f64, f64, f64, f64)>) {
    let mut raw_rects = Vec::new();
    let mut segs = Vec::new();

    // lievisual 会输出一个铺满画布的背景 <rect>，需排除，避免被当作节点矩形。
    let canvas = parse_canvas_size(svg);

    for line in svg.lines() {
        // 排除边标签白底框（class="edge-label"）：它是边的一部分，
        // 边从其上穿过是正常行为，不应判为"边穿过节点"。
        if line.contains("<rect ")
            && !line.contains("edge-label")
            && let (Some(x), Some(y), Some(w), Some(h)) = (
                parse_attr(line, " x=\""),
                parse_attr(line, " y=\""),
                parse_attr(line, " width=\""),
                parse_attr(line, " height=\""),
            )
        {
            // 跳过铺满画布的背景矩形
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

    // 去重：类图中同个节点有 2 个重叠 rect（背景+边框），只保留一个
    let rects = dedup_rects(&raw_rects);
    (rects, segs)
}

/// 去重完全相同的 rect（位置和大小一样 → 同个节点的不同视觉层）
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
// 布局质量检测函数
// ============================================================

/// 检测 1: 边不得穿越任何节点
fn check_no_edge_crosses_node(rects: &[Rect], segs: &[(f64, f64, f64, f64)], svg_name: &str) {
    let mut violations = Vec::new();
    for (x1, y1, x2, y2) in segs {
        for (i, r) in rects.iter().enumerate() {
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
                violations.push((i, *x1, *y1, *x2, *y2));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "{}: {} edge segment(s) cross through node rects! Violations: {:?}",
        svg_name,
        violations.len(),
        violations
    );
}

/// 检测 2: 同层节点中心 Y 偏差 < 1px
fn check_same_layer_y_alignment(rects: &[Rect], svg_name: &str) {
    let mut layers: Vec<Vec<Rect>> = Vec::new();
    for &r in rects {
        let mut found = false;
        for layer in &mut layers {
            if (layer[0].y - r.y).abs() < 5.0 {
                layer.push(r);
                found = true;
                break;
            }
        }
        if !found {
            layers.push(vec![r]);
        }
    }
    for layer in &layers {
        if layer.len() <= 1 {
            continue;
        }
        let centers: Vec<f64> = layer.iter().map(|r| r.center_y()).collect();
        let max_cy = centers
            .iter()
            .cloned()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let min_cy = centers
            .iter()
            .cloned()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        assert!(
            (max_cy - min_cy).abs() < 1.0,
            "{}: same-layer Y deviation {:.2}px (min={:.2}, max={:.2})",
            svg_name,
            max_cy - min_cy,
            min_cy,
            max_cy
        );
    }
}

/// 检测 3: 无节点重叠（去重后的 rect 应互不重叠）
fn check_no_node_overlap(rects: &[Rect], svg_name: &str) {
    for i in 0..rects.len() {
        for j in i + 1..rects.len() {
            let r1 = &rects[i];
            let r2 = &rects[j];
            let is_touching = (r1.x + r1.w - r2.x).abs() < 0.5
                || (r2.x + r2.w - r1.x).abs() < 0.5
                || (r1.y + r1.h - r2.y).abs() < 0.5
                || (r2.y + r2.h - r1.y).abs() < 0.5;
            if is_touching {
                continue;
            }
            assert!(
                !r1.intersects(r2),
                "{}: nodes overlap! {:?} vs {:?}",
                svg_name,
                r1,
                r2
            );
        }
    }
}

// ============================================================
// Flowchart 布局质量测试
// ============================================================

#[test]
fn flowchart_loop_no_edge_crosses() {
    let svg = render(
        "\
flowchart TD
    A[\"Start\"]
    B[\"Continue\"]
    C[\"Process\"]
    D[\"End\"]
    A --> B
    B -->|Yes| C
    C --> B
    B -->|No| D\
    ",
        800,
        600,
    );
    let (rects, segs) = parse_svg(&svg);
    check_no_edge_crosses_node(&rects, &segs, "flow_loop");
    check_same_layer_y_alignment(&rects, "flow_loop");
    check_no_node_overlap(&rects, "flow_loop");
}

#[test]
fn flowchart_cycle_no_edge_crosses() {
    let svg = render(
        "\
flowchart TD
    A[\"Start\"]
    B[\"Process\"]
    C[\"Check\"]
    D[\"Complete\"]
    E[\"End\"]
    A --> B
    B --> C
    C -->|Yes| D
    C -->|No| B
    D --> E\
    ",
        800,
        600,
    );
    let (rects, segs) = parse_svg(&svg);
    check_no_edge_crosses_node(&rects, &segs, "flow_cycle");
    check_same_layer_y_alignment(&rects, "flow_cycle");
    check_no_node_overlap(&rects, "flow_cycle");
}

#[test]
fn flowchart_branch_no_edge_crosses() {
    let svg = render(
        "\
flowchart TD
    S[\"Start\"]
    D{\"Decision\"}
    A[\"Approved\"]
    R[\"Rejected\"]
    E[\"End\"]
    S --> D
    D -->|Yes| A
    D -->|No| R
    A --> E
    R --> E\
    ",
        800,
        600,
    );
    let (rects, segs) = parse_svg(&svg);
    check_no_edge_crosses_node(&rects, &segs, "flow_branch");
    check_same_layer_y_alignment(&rects, "flow_branch");
    check_no_node_overlap(&rects, "flow_branch");
}

#[test]
fn flowchart_chain_alignment() {
    let svg = render("flowchart TD\nA --> B\nB --> C", 600, 400);
    let (rects, _segs) = parse_svg(&svg);
    let cx: Vec<f64> = rects.iter().map(|r| r.center_x()).collect();
    let max_cx = cx
        .iter()
        .cloned()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    let min_cx = cx
        .iter()
        .cloned()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    assert!(
        (max_cx - min_cx).abs() < 1.0,
        "Chain nodes not aligned! deviation={:.2}px",
        max_cx - min_cx
    );
}

// ============================================================
// 回归测试：确保之前修复的问题不重现
// ============================================================

/// 同层节点 Y 对齐回归测试：真实同层（无环）节点 Y 中心一致
#[test]
fn regression_same_layer_y_alignment() {
    // diamond: B 和 C 同层（无环，真实同层），验证 Y 对齐
    let svg = render(
        "\
flowchart TD
    A[\"Start\"]
    B[\"Left\"]
    C[\"Right\"]
    D[\"End\"]
    A --> B
    A --> C
    B --> D
    C --> D\
    ",
        800,
        600,
    );
    let (rects, _) = parse_svg(&svg);
    // B(Continue) 和 C(Process) 应在同层且 Y 中心一致
    let by_y: Vec<&Rect> = {
        let mut v: Vec<&Rect> = rects.iter().collect();
        v.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());
        v
    };
    // 找到 B 和 C 对应的 rect（它们是第二层的两个节点）
    assert!(by_y.len() >= 3, "Expected at least 3 unique rects");
    let layer1_rects: Vec<&Rect> = by_y
        .iter()
        .filter(|r| (r.y - by_y[1].y).abs() < 5.0)
        .copied()
        .collect();
    assert!(
        layer1_rects.len() >= 2,
        "Layer 1 should have at least 2 nodes"
    );
    let cy1 = layer1_rects[0].center_y();
    for r in &layer1_rects {
        assert!(
            (r.center_y() - cy1).abs() < 1.0,
            "Same-layer nodes Y misaligned: {:.2} vs {:.2}",
            r.center_y(),
            cy1
        );
    }
}

/// 边不穿越节点回归测试
#[test]
fn regression_edges_avoid_nodes() {
    let svg = render(
        "\
classDiagram
    class A
    class B
    class C
    class D
    class E
    A <|-- B
    A *-- C
    A o-- D
    A --> E\
    ",
        900,
        400,
    );
    let (rects, segs) = parse_svg(&svg);
    check_no_edge_crosses_node(&rects, &segs, "regression_class");
}

/// 子图（subgraph）渲染测试：验证 subgraph 容器框存在且包围其成员节点
#[test]
fn flowchart_subgraph_container() {
    let svg = render(
        "\
flowchart TD
    A[Start]
    subgraph One
        B[Process]
        C[Decision]
    end
    A --> B
    B --> C
    C --> A\
        ",
        800,
        600,
    );
    let (rects, _segs) = parse_svg(&svg);

    // 至少应有 A、B、C 三个节点矩形 + 1 个子图容器矩形
    assert!(
        rects.len() >= 4,
        "expected nodes + subgraph container, got {}",
        rects.len()
    );

    // 找出面积最大的矩形（通常是子图容器框）
    let container = rects
        .iter()
        .max_by(|a, b| (a.w * a.h).partial_cmp(&(b.w * b.h)).unwrap())
        .expect("should have a container rect");

    // 容器应包围其余所有较小的节点矩形
    let container_idx = rects
        .iter()
        .position(|r| (r.w * r.h) >= (container.w * container.h) - 1e-6)
        .unwrap();
    let nodes_inside: Vec<&Rect> = rects
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != container_idx)
        .filter(|(_, r)| container.contains(r.center_x(), r.center_y()))
        .map(|(_, r)| r)
        .collect();
    // subgraph One 的成员为 B、C（A 是顶层节点，应在容器外）
    assert!(
        nodes_inside.len() == 2,
        "subgraph container should contain exactly 2 member nodes (B,C), got {}",
        nodes_inside.len()
    );
}
