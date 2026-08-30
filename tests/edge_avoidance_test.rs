//! 边-节点碰撞回归测试：边路由不得穿过非端点节点。
//!
//! 覆盖官方对比中的关键用例：cycle / cycle2 / self_loop / diamond_3way /
//! long_edge / dense / diamond 四方向 / fan_in / fan_out。
//!
//! 检测方式：直接取 `layout::engine::run` 产出的 `Geograph`，
//! 把每条边的 route 按贝塞尔采样成点列，检查任意采样点是否落入
//! 「非端点节点」的形状内部（形状感知：矩形 / 菱形 / 圆）。

use lievisual::geometry::Point;

fn render_gg(src: &str) -> liemermaid::builder::ir::geograph::Geograph {
    use liemermaid::builder::{extract, layout, measure};
    let diagram = liemermaid::MermaidParser::parse_mermaid(src).expect("parse");
    let ug = extract::run(&diagram).expect("extract");
    let ug = measure::measure_all(ug);
    let (gg, _) = layout::engine::run(&ug).expect("layout");
    gg
}

/// 点是否在节点形状内部（含 margin 收缩，边界接触不算碰撞）。
fn inside_shape(
    p: Point,
    center: Point,
    w: f64,
    h: f64,
    shape: &liemermaid::builder::ir::shape::ShapeKind,
) -> bool {
    use liemermaid::builder::ir::shape::ShapeKind;
    let (dx, dy) = (p.x - center.x, p.y - center.y);
    let (hw, hh) = (w / 2.0, h / 2.0);
    const MARGIN: f64 = 2.0;
    match shape {
        ShapeKind::Diamond => (dx.abs() / hw.max(1e-9)) + (dy.abs() / hh.max(1e-9)) < 1.0 - 0.02,
        ShapeKind::Circle | ShapeKind::DoubleCircle | ShapeKind::StartDot | ShapeKind::EndDot => {
            (dx / hw.max(1e-9)).powi(2) + (dy / hh.max(1e-9)).powi(2) < 1.0 - 0.02
        }
        _ => {
            // 矩形类：边界留 MARGIN 余量（端口贴边出发属正常）。
            dx.abs() < hw - MARGIN && dy.abs() < hh - MARGIN
        }
    }
}

/// 采样 RoutePath（贝塞尔按 n 等分）。
fn sample_path(r: &liemermaid::builder::ir::geograph::RoutePath, n: usize) -> Vec<Point> {
    use liemermaid::builder::ir::geograph::RouteSegment;
    let mut out = Vec::new();
    for seg in r.iter() {
        match *seg {
            RouteSegment::Line { from, to } => {
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    out.push(Point::new(
                        from.x + (to.x - from.x) * t,
                        from.y + (to.y - from.y) * t,
                    ));
                }
            }
            RouteSegment::CubicBezier { p0, p1, p2, p3 } => {
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push(Point::new(
                        a * p0.x + b * p1.x + c * p2.x + d * p3.x,
                        a * p0.y + b * p1.y + c * p2.y + d * p3.y,
                    ));
                }
            }
        }
    }
    out
}

fn assert_no_collision(src: &str, case: &str) {
    let gg = render_gg(src);
    let node_by_id: std::collections::HashMap<&str, &liemermaid::builder::ir::geograph::GGNode> =
        gg.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut violations = Vec::new();
    for e in &gg.edges {
        let samples = sample_path(&e.route, 24);
        for (i, n) in gg.nodes.iter().enumerate() {
            if n.id == e.source || n.id == e.target {
                continue;
            }
            for (si, p) in samples.iter().enumerate() {
                if inside_shape(*p, n.center, n.size.width, n.size.height, &n.shape) {
                    violations.push(format!(
                        "edge#{} {}->{} sample#{si} ({:.1},{:.1}) inside node {} (case {case})",
                        i, e.source, e.target, p.x, p.y, n.id
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "{case}: 连线穿过节点!\n{}",
        violations.join("\n")
    );
    // 端点自检：每条边起止点应接近其端点节点的形状边界（不能悬空）。
    for e in &gg.edges {
        let (Some(s), Some(t)) = (
            node_by_id.get(e.source.as_str()),
            node_by_id.get(e.target.as_str()),
        ) else {
            continue;
        };
        if e.source == e.target {
            continue; // 自环单独验证
        }
        for (pt, n) in [(e.route.start(), *s), (e.route.end(), *t)] {
            let on = on_boundary(pt, n);
            assert!(
                on,
                "{case}: 边 {}->{} 的端点 ({:.1},{:.1}) 未落在节点 {} 边界上",
                e.source, e.target, pt.x, pt.y, n.id
            );
        }
    }
}

/// 点是否落在节点形状边界上（容差 3px）。
fn on_boundary(p: Point, n: &liemermaid::builder::ir::geograph::GGNode) -> bool {
    use liemermaid::builder::ir::shape::ShapeKind;
    let (dx, dy) = (p.x - n.center.x, p.y - n.center.y);
    let (hw, hh) = (n.size.width / 2.0, n.size.height / 2.0);
    const TOL: f64 = 3.0;
    match n.shape {
        ShapeKind::Diamond => {
            ((dx.abs() / hw.max(1e-9)) + (dy.abs() / hh.max(1e-9)) - 1.0).abs() * hh.min(hw) < TOL
        }
        ShapeKind::Circle | ShapeKind::DoubleCircle | ShapeKind::StartDot | ShapeKind::EndDot => {
            ((dx / hw.max(1e-9)).powi(2) + (dy / hh.max(1e-9)).powi(2) - 1.0).abs() < 0.25
        }
        _ => {
            let on_v = (dx.abs() - hw).abs() < TOL && dy.abs() <= hh + TOL;
            let on_h = (dy.abs() - hh).abs() < TOL && dx.abs() <= hw + TOL;
            on_v || on_h
        }
    }
}

#[test]
fn cycle_edges() {
    assert_no_collision(
        "flowchart TB\n    A[A]\n    B[B]\n    C[C]\n    D[D]\n    A --> B\n    B --> C\n    C --> B\n    C --> D\n",
        "cycle",
    );
}

#[test]
fn cycle2_edges() {
    assert_no_collision(
        "flowchart TB\n    A[A]\n    B[B]\n    C[C]\n    A --> B\n    B --> C\n    C --> A\n    C --> B\n",
        "cycle2",
    );
}

#[test]
fn self_loop_edges() {
    let gg = render_gg(
        "flowchart TB\n    A[A]\n    B[B]\n    C[C]\n    A --> B\n    B --> B\n    B --> C\n",
    );
    // 自环应可见：route 非空且外凸出节点包围盒。
    let e = gg
        .edges
        .iter()
        .find(|e| e.source == "B" && e.target == "B")
        .expect("self loop edge");
    assert!(!e.route.is_empty(), "自环路由为空");
    let max_x = sample_path(&e.route, 32)
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let b = gg.nodes.iter().find(|n| n.id == "B").unwrap();
    assert!(
        max_x > b.center.x + b.size.width / 2.0 + 5.0,
        "自环应外凸出右边缘: max_x={max_x}"
    );
}

#[test]
fn diamond_3way_edges() {
    assert_no_collision(
        "flowchart TB\n    S[S]\n    D{D}\n    A[A]\n    B[B]\n    C[C]\n    E[E]\n    S --> D\n    D -->|one| A\n    D -->|two| B\n    D -->|three| C\n    A --> E\n    B --> E\n    C --> E\n",
        "diamond_3way",
    );
}

#[test]
fn long_edge_avoids_nodes() {
    assert_no_collision(
        "flowchart TB\n    A[A]\n    B[B]\n    C[C]\n    D[D]\n    A --> B\n    B --> C\n    C --> D\n    A --> D\n",
        "long_edge",
    );
}

#[test]
fn dense_edges() {
    assert_no_collision(
        "flowchart TB\n    A[A]\n    B[B]\n    C[C]\n    D[D]\n    E[E]\n    F[F]\n    G[G]\n    H[H]\n    I[I]\n    A --> B\n    A --> C\n    B --> D\n    C --> D\n    D --> E\n    D --> F\n    E --> G\n    F --> G\n    G --> H\n    G --> I\n    H --> B\n",
        "dense",
    );
}

/// 回归：多条边使用的绕行通道不得重叠（同 x 且 y 跨度交叠）。
/// 对应 flowchart__cycle2 / flowchart__lc4 的 D→B 与 E→C 双通道重叠问题。
#[test]
fn channel_routes_do_not_overlap() {
    use liemermaid::builder::ir::geograph::RouteSegment;
    let gg = render_gg(
        "flowchart TB\n    A[A]\n    B[B]\n    C[C]\n    D[D]\n    E[E]\n    A --> B\n    B --> C\n    C --> D\n    D --> B\n    E --> C\n",
    );
    // 收集所有边路由中的长垂直段：(edge index, x, y0, y1)
    let mut vsegs: Vec<(usize, f64, f64, f64)> = Vec::new();
    for (ei, e) in gg.edges.iter().enumerate() {
        for seg in e.route.iter() {
            if let RouteSegment::Line { from, to } = seg
                && (from.x - to.x).abs() < 1e-6
                && (from.y - to.y).abs() > 40.0
            {
                vsegs.push((ei, from.x, from.y.min(to.y), from.y.max(to.y)));
            }
        }
    }
    for i in 0..vsegs.len() {
        for j in (i + 1)..vsegs.len() {
            let (a, b) = (&vsegs[i], &vsegs[j]);
            if a.0 != b.0 && (a.1 - b.1).abs() < 2.0 && a.3 > b.2 && a.2 < b.3 {
                panic!(
                    "通道重叠: edge#{} x={} span {:?}.. 与 edge#{} x={} span {:?}..",
                    a.0, a.1, a.2, b.0, b.1, b.2
                );
            }
        }
    }
    // D→B 与 E→C 的通道应分居节点列两侧（D→B 左、E→C 右），
    // 避免回边通道横穿 E→C 的进入段。
    // 判据相对布局中轴（x=0），不写死绝对坐标（节点宽度随文本自适应）。
    let has_left = vsegs.iter().any(|(_, x, _, _)| *x < 0.0);
    let has_right = vsegs.iter().any(|(_, x, _, _)| *x > 0.0);
    assert!(
        has_left && has_right,
        "cycle2 两条通道应分居左右两侧: {vsegs:?}"
    );
}

#[test]
fn diamond_directions() {
    for (name, dir) in [("bt", "BT"), ("lr", "LR"), ("rl", "RL")] {
        assert_no_collision(
            &format!(
                "flowchart {dir}\n    A[A]\n    B{{B}}\n    C[C]\n    D[D]\n    A --> B\n    B --> C\n    B --> D\n"
            ),
            &format!("diamond_{name}"),
        );
    }
}

/// 0.3 容器避障：边不得穿过「两端均非其成员」的子图/复合状态容器框。
/// 容器内边与跨容器边（至少一端为成员）允许穿过容器边界。
#[test]
fn edges_avoid_non_member_containers() {
    for (name, src) in [
        (
            "flowchart subgraph",
            "flowchart TB\n    A[Start]\n    subgraph One\n        B[Process]\n        C[Decision]\n    end\n    subgraph Two\n        D[Output]\n    end\n    A --> B\n    B --> C\n    C --> D\n    C --> A\n",
        ),
        (
            "state composite",
            "stateDiagram-v2\n    [*] --> S\n    state S {\n        [*] --> s1\n        s1 --> s2\n        s2 --> [*]\n    }\n    S --> [*]\n",
        ),
    ] {
        let gg = render_gg(src);
        for e in &gg.edges {
            for c in &gg.containers {
                let member_of = |id: &str| c.member_ids.iter().any(|m| m == id);
                if member_of(&e.source) || member_of(&e.target) {
                    continue;
                }
                let r = c.bounds;
                for p in sample_path(&e.route, 48) {
                    assert!(
                        !(p.x > r.min_x() && p.x < r.max_x() && p.y > r.min_y() && p.y < r.max_y()),
                        "{name}: 边 {}->{} 穿过容器 [{}]: {p:?}",
                        e.source,
                        e.target,
                        c.title.as_deref().unwrap_or("?")
                    );
                }
            }
        }
    }
}
