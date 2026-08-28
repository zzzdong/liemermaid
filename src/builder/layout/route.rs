//! `EdgeRouter`：正交（曼哈顿）边路由 + 基于空间网格的节点回避。
//!
//! 输入 [`crate::builder::ir::geograph::Geograph`]，原地更新每条边的 `route`
//! （polyline 控制点序列）。路由目标：
//! 1. 自适应端口：source 取「朝 target 方向」的端口，target 取「朝 source 方向」的端口，
//!    出线段沿端口法向延伸一小段 stub，避免贴着节点边直出。
//! 2. 正交连接：stub 端点之间走曼哈顿折线（先主轴后交叉轴，或反之，按相对位置选）。
//! 3. 节点回避：用 [`crate::builder::layout::spatial::SpatialGrid`] 把节点包围盒登记，
//!    若某段穿过非端点节点，则沿主轴方向做最小步进偏移，直到不冲突或达上限。
//!
//! 本阶段不做边-边完整避让（仅节点回避）；边-边排斥在后续迭代接入（仍用同一网格）。

use std::collections::HashMap;

use lievisual::geometry::{Point, Rect};

use crate::builder::ir::geograph::{GGNode, Geograph, RoutePath, RouteSegment};

use super::spatial::{SpatialGrid, segment_intersects_rect};

/// 出线 stub 长度（端口法向延伸距离）。
const STUB: f64 = 18.0;
/// 端口槽位：该边在节点对应端口边（Top/Bottom/Left/Right）上的序号与总数。
#[derive(Clone, Copy)]
pub(crate) struct Slot {
    pub(crate) idx: usize,
    pub(crate) total: usize,
}
/// 节点回避偏移步进。
const OFFSET_STEP: f64 = 12.0;
/// 回避最大尝试次数。
const MAX_OFFSET_TRIES: usize = 8;

/// 为 GG 中所有边生成正交路由。
pub fn route_edges(gg: &mut Geograph, direction: crate::ast::Direction) {
    // 节点索引
    let node_map: HashMap<&String, &GGNode> = gg.nodes.iter().map(|n| (&n.id, n)).collect();

    // 空间网格：登记每个节点包围盒（携带 owner id）
    let mut grid = SpatialGrid::new(80.0);
    let node_rects: Vec<(usize, Rect, String)> = gg
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let r = node_rect(n);
            grid.insert_rect(i, &r);
            (i, r, n.id.clone())
        })
        .collect();

    // —— 端口分散：同一节点多条出/入边在节点边上分散出发，而非都从中点 ——
    // 对每个节点，按端口边（Top/Bottom/Left/Right）分组，给每条边分配槽位 (idx,total)。
    use std::collections::HashMap as Map;
    let mut src_count: Map<(String, Port), usize> = Map::new();
    let mut tgt_count: Map<(String, Port), usize> = Map::new();
    let mut src_idx: Map<(String, Port), usize> = Map::new();
    let mut tgt_idx: Map<(String, Port), usize> = Map::new();

    // 先统计数量（需要稳定的边顺序，用 gg.edges 索引）。key 克隆字符串，避免借用 gg.edges。
    for e in &gg.edges {
        let (Some(s), Some(t)) = (node_map.get(&e.source), node_map.get(&e.target)) else {
            continue;
        };
        let sp = pick_port(s.center, t.center, true, direction);
        let tp = pick_port(t.center, s.center, false, direction);
        src_count.entry((e.source.clone(), sp)).and_modify(|c| *c += 1).or_insert(1);
        tgt_count.entry((e.target.clone(), tp)).and_modify(|c| *c += 1).or_insert(1);
    }
    // 计算所有节点 top 的最小 y（最高节点的顶），用于 back edge 绕行在所有节点之上。
    let all_nodes_min_y = gg
        .nodes
        .iter()
        .map(|n| n.center.y - n.size.height / 2.0)
        .fold(f64::INFINITY, f64::min);

    // mutual 对（u↔v）的绕行侧分配记录：key = (source, target)，value = ±1。
    let mut mutual_side: std::collections::HashMap<(String, String), f64> =
        std::collections::HashMap::new();

    // 预计算 mutual 对集合（u↔v 与 v↔u 同时存在）：两条相对边都走"垂直双线"。
    let mut mutual_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for e in &gg.edges {
        if gg.edges.iter().any(|o| o.source == e.target && o.target == e.source) {
            mutual_pairs.insert((e.source.clone(), e.target.clone()));
        }
    }

    // 分配槽位并路由。
    for e in gg.edges.iter_mut() {
        let (Some(s), Some(t)) = (node_map.get(&e.source), node_map.get(&e.target)) else {
            continue;
        };
        let s = *s;
        let t = *t;
        let sp = pick_port(s.center, t.center, true, direction);
        let tp = pick_port(t.center, s.center, false, direction);
        let s_total = src_count[&(e.source.clone(), sp)];
        let t_total = tgt_count[&(e.target.clone(), tp)];
        let s_i = src_idx.entry((e.source.clone(), sp)).or_insert(0);
        let t_i = tgt_idx.entry((e.target.clone(), tp)).or_insert(0);
        let s_slot = Slot { idx: *s_i, total: s_total };
        let t_slot = Slot { idx: *t_i, total: t_total };
        *s_i += 1;
        *t_i += 1;

        // 是否属于 mutual 对。
        let is_mutual = mutual_pairs.contains(&(e.source.clone(), e.target.clone()));
        // back edge 检测：source 节点在主轴方向上位于 target 「下游」（逆主轴）。
        // - TB/BT 主轴 = y：source.y > t.y 表示 source 在 target 下方（下游）。
        // - LR/RL 主轴 = x：source.x > t.x 表示 source 在 target 右侧（下游）。
        use crate::ast::Direction;
        let is_back_edge = match direction {
            // TB/TD：主轴向下，source 在 target 下方（y 更大）是下游 → back。
            Direction::TB | Direction::TD => s.center.y > t.center.y,
            // BT：主轴向上，source 在 target 上方（y 更小）是下游 → back。
            Direction::BT => s.center.y < t.center.y,
            // LR：主轴向右，source 在 target 右侧（x 更大）是下游 → back。
            Direction::LR => s.center.x > t.center.x,
            // RL：主轴向左，source 在 target 左侧（x 更小）是下游 → back。
            Direction::RL => s.center.x < t.center.x,
        };

        let route = if is_mutual {
            // mutual 对分配侧：u→v 先遇到 +1（右侧），v→u 后遇到 -1（左侧）。
            let side = mutual_side_for(&e.source, &e.target, &mut mutual_side);
            mutual_dual_route(s, t, side)
        } else if is_back_edge {
            let side = mutual_side_for(&e.source, &e.target, &mut mutual_side);
            back_edge_route(s, t, &(sp, s_slot), &(tp, t_slot), all_nodes_min_y, side, direction)
        } else {
            match e.routing_hint {
                crate::builder::ir::common::RoutingHint::Spline => {
                    spline_route(s, t, &(sp, s_slot), &(tp, t_slot))
                }
                _ => orthogonal_route(s, t, &(sp, s_slot), &(tp, t_slot), &node_rects, &grid),
            }
        };
        e.route = route;
        // 边标签锚点：取路由中段的中点（为标签占位预留）。有 label_text 才需要。
        if e.label_text.is_some() {
            e.label_anchor = Some(e.route.midpoint());
        }
    }
}

/// 检测 mutual 对并为每条边分配绕行侧。
/// `side`: +1 = 从 source 右出（左绕），-1 = 从 source 左出（右绕）。
/// mutual 对（u↔v）两条相对边：先遇到的 +1，后遇到的 -1，形成左右对称。
fn mutual_side_for(
    source: &String,
    target: &String,
    seen: &mut std::collections::HashMap<(String, String), f64>,
) -> f64 {
    let key = (source.clone(), target.clone());
    if let Some(&s) = seen.get(&key) {
        return s;
    }
    let reverse_key = (target.clone(), source.clone());
    // 若反向边已分配，则本条分配相反侧。
    if let Some(&rev) = seen.get(&reverse_key) {
        let side = -rev;
        seen.insert(key, side);
        side
    } else {
        seen.insert(key, 1.0);
        1.0
    }
}

/// Spline 路由：单段三次贝塞尔（官方 mermaid 风格）。
///
/// 端口锚点（含槽位分散）作为 P0/P3；控制点 P1/P2 沿源/目标端口的主轴方向延伸，
/// 弧度与端口距离成比例，产生平滑弧线（节点不大时视觉上"自然弯曲"）。
fn spline_route(
    s: &GGNode,
    t: &GGNode,
    s_slot: &(Port, Slot),
    t_slot: &(Port, Slot),
) -> RoutePath {
    let sp = s_slot.0;
    let tp = t_slot.0;
    let p0 = port_point_at(s, sp, s_slot.1);
    let p3 = port_point_at(t, tp, t_slot.1);

    // 端口出/入方向：沿端口法向（离开节点）。
    let (ox, oy) = port_normal(sp);
    let (ix, iy) = port_normal(tp);

    // 弧度：与首末水平/垂直距离成比例（官方 dagre 风格），clamp 到合理范围。
    // 上限放大让 fan-out 长边弧度更明显（避免控制点 clamp 后弧线太"直"）。
    let span = ((p3.x - p0.x).abs()).max((p3.y - p0.y).abs());
    let arc = (span * 0.5).clamp(28.0, 140.0);

    // 水平展开：c1 向 target 方向偏 h_bias/2，c2 向 source 方向偏 h_bias/2。
    // 让 fan-out 多条出边呈对称扇形（避免弧线互相交叉）。
    let h_bias = (p3.x - p0.x) * 0.5;
    let p1 = Point::new(p0.x + ox * arc + h_bias * 0.5, p0.y + oy * arc);
    let p2 = Point::new(p3.x + ix * arc - h_bias * 0.5, p3.y + iy * arc);

    let mut r = RoutePath::new();
    r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    r
}

/// Mutual 对（u↔v）的双线路由：两条相对边从 source/target 的**左右两侧端口**
/// 分别出发/进入，形成官方 mermaid 的"双出"风格——两条线都垂直（沿主轴），
/// x 偏移 = side * off（一条偏左、一条偏右），在 source/target 之间平行分开。
///
/// 关键：端口点取在节点边界的水平偏移位置（而非节点中点），使两条线不穿过节点。
fn mutual_dual_route(s: &GGNode, t: &GGNode, side: f64) -> RoutePath {
    // 水平偏移：节点半宽的 0.4（约 24px），双线在节点宽度内平行分开。
    let off = s.size.width.min(t.size.width) * 0.4;
    // source 侧：side>0 → source 从底边偏右出；side<0 → source 从底边偏左出。
    // （TB 布局下 mutual 对两条边方向相反：一条向下、一条向上。）
    let p0 = if s.center.y < t.center.y {
        // source 在上（forward 边，B→C）：从 source 底边出（向下）。
        Point::new(s.center.x + side * off, s.center.y + s.size.height / 2.0)
    } else {
        // source 在下（back 边，C→B）：从 source 顶边出（向上）。
        Point::new(s.center.x + side * off, s.center.y - s.size.height / 2.0)
    };
    let p3 = if t.center.y > s.center.y {
        // target 在下（C）：从 target 顶边入（从上进入）。
        Point::new(t.center.x + side * off, t.center.y - t.size.height / 2.0)
    } else {
        // target 在上（B）：从 target 底边入（从下进入）。
        Point::new(t.center.x + side * off, t.center.y + t.size.height / 2.0)
    };
    // 轻微弧线（沿主轴小幅延伸），保持柔和视觉但不横向摆动（非 S 形）。
    let dy = (p3.y - p0.y).abs();
    let ext = dy * 0.3 + 10.0;
    let p1 = Point::new(p0.x, p0.y + if p3.y >= p0.y { ext } else { -ext });
    let p2 = Point::new(p3.x, p3.y + if p3.y >= p0.y { -ext } else { ext });

    let mut r = RoutePath::new();
    r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    r
}

/// Back edge 路由：源节点**侧边**出，目标节点**对侧边**入，
/// 控制点大幅**水平偏移**，让弧线整体在 source/target 之间的空白区域走（侧向绕行）。
///
/// 视觉上呈现"侧向 U 形"（类似官方 mermaid 的 back edge 双线紧贴风格），不绕到
/// 最高节点顶部之上（避免占用过多垂直空间）。
///
/// 适用：TB 布局下 source.center.y > target.center.y（source 在 target 下游）。
fn back_edge_route(
    s: &GGNode,
    t: &GGNode,
    _s_slot: &(Port, Slot),
    _t_slot: &(Port, Slot),
    _all_nodes_min_y: f64,
    side: f64,
    direction: crate::ast::Direction,
) -> RoutePath {
    use crate::ast::Direction;
    let (sp, tp) = match direction {
        Direction::TB | Direction::TD => (Port::Top, Port::Bottom),
        Direction::BT => (Port::Bottom, Port::Top),
        Direction::LR => (Port::Right, Port::Left),
        Direction::RL => (Port::Left, Port::Right),
    };
    // 垂直方向的"相对双弧"：source 从顶边出（向上），target 从底边入（向下）。
    // 控制点 x 小幅偏移（±节点半宽*0.5），让 mutual 对（u↔v）两条相对边
    // 在 source/target 之间一左一右小幅分开，形成官方 mermaid 的"双线紧贴"风格。
    let p0 = port_point_at(s, Port::Top, Slot { idx: 0, total: 1 });
    let p3 = port_point_at(t, Port::Bottom, Slot { idx: 0, total: 1 });

    // 垂直跨度：source 顶到 target 底的垂直距离。
    let dy = (p3.y - p0.y).abs();
    // x 偏移：节点半宽的 0.5（小幅左右分开），确保两条相对边不重合但也不远离。
    let x_off = s.size.width.max(t.size.width) * 0.5 * 0.5;
    // y 延伸：向上绕过 target 一点（dy 的小比例），让弧线在 source/target 之间弯曲。
    let y_ext = dy * 0.4 + 20.0;

    // 控制点：source 侧向上延伸（y 更小），target 侧向下延伸（y 更大），
    // 且一左一右错开（side 决定 x 方向）。
    let p1 = Point::new(p0.x + side * x_off, p0.y - y_ext);
    let p2 = Point::new(p3.x - side * x_off, p3.y + y_ext);

    let mut r = RoutePath::new();
    r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    r
}

/// 端口法向（离开节点方向）单位向量。
fn port_normal(p: Port) -> (f64, f64) {
    match p {
        Port::Top => (0.0, -1.0),
        Port::Bottom => (0.0, 1.0),
        Port::Left => (-1.0, 0.0),
        Port::Right => (1.0, 0.0),
    }
}

/// 节点包围盒（含少量 padding 以避免边贴边）。
fn node_rect(n: &GGNode) -> Rect {
    let half_w = n.size.width / 2.0 + 2.0;
    let half_h = n.size.height / 2.0 + 2.0;
    Rect::new(
        n.center.x - half_w,
        n.center.y - half_h,
        n.center.x + half_w,
        n.center.y + half_h,
    )
}

/// 生成单条边的正交路由（折线段序列）。
fn orthogonal_route(
    s: &GGNode,
    t: &GGNode,
    s_slot: &(Port, Slot),
    t_slot: &(Port, Slot),
    node_rects: &[(usize, Rect, String)],
    grid: &SpatialGrid,
) -> RoutePath {
    // 自适应端口：source 朝 target 方向出，target 朝 source 方向入
    let sp = s_slot.0;
    let tp = t_slot.0;
    // 端口点在节点边上按槽位分散（多条出/入边不都从中点出发）。
    let start = port_point_at(s, sp, s_slot.1);
    let end = port_point_at(t, tp, t_slot.1);

    let dx = (t.center.x - s.center.x).abs();
    let dy = (t.center.y - s.center.y).abs();
    let horizontal_main = dx >= dy;

    // 出线 stub（沿端口法向）
    let mut start_stub = offset_along(sp, start, STUB);
    let mut end_stub = offset_along(tp, end, STUB);

    // 主干统一到同一主轴坐标，保证曼哈顿：
    // - 水平主导：start_stub / mid / end_stub 共享同一 y
    // - 垂直主导：共享同一 x
    let mut pts = if horizontal_main {
        let my = (start_stub.y + end_stub.y) / 2.0;
        start_stub.y = my;
        end_stub.y = my;
        let mid = Point::new(end_stub.x, my);
        vec![start, start_stub, mid, end_stub, end]
    } else {
        let mx = (start_stub.x + end_stub.x) / 2.0;
        start_stub.x = mx;
        end_stub.x = mx;
        let mid = Point::new(mx, end_stub.y);
        vec![start, start_stub, mid, end_stub, end]
    };

    // 节点回避：整体平移主干（保持曼哈顿），避开非端点节点
    avoid_nodes(&mut pts, horizontal_main, &[s.id.clone(), t.id.clone()], node_rects, grid);

    points_to_route(&pts)
}

/// 选端口：朝 `toward` 方向。is_source=true 表示这是起点（出端口），
/// false 表示终点（入端口，朝向 source 方向即「反向」）。
///
/// 端口选择偏向**垂直方向**（dy 主导）：让 fan-out 节点的多条出边都从 Bottom/Top 出，
/// 沿底边/顶边均匀分散，避免折线或斜向出口。仅当水平距离显著大于垂直距离（1.5x）
/// 时才选水平端口（Left/Right）。
/// 选端口：朝 `toward` 方向。is_source=true 表示这是起点（出端口），
/// false 表示终点（入端口，朝向 source 方向即「反向」）。
///
/// 端口选择根据**布局方向**（TB/BT vs LR/RL）：
/// - TB/BT：优先垂直端口（Top/Bottom），让 fan-out/fan-in 节点从底/顶边出边分散；
/// - LR/RL：优先水平端口（Left/Right），让节点从侧边出边到对侧节点。
/// 这样 LR 布局的边走水平方向（左右流动），TB 布局走垂直方向（上下流动），
/// 不会出现 LR 图"上下端面进出"的诡异画法。
fn pick_port(
    from: Point,
    toward: Point,
    is_source: bool,
    direction: crate::ast::Direction,
) -> Port {
    let dx = toward.x - from.x;
    let dy = toward.y - from.y;
    use crate::ast::Direction;
    let main_axis_vertical = matches!(direction, Direction::TB | Direction::BT);
    if main_axis_vertical {
        // TB/BT：垂直端口优先。
        if dy >= 0.0 { Port::Bottom } else { Port::Top }
    } else {
        // LR/RL：水平端口优先。
        if dx >= 0.0 { Port::Right } else { Port::Left }
    }
}

/// 把折线点序列转成路由段序列（跳过重复点；相邻段合并共线）。
fn points_to_route(points: &[Point]) -> RoutePath {
    let mut pts: Vec<Point> = Vec::new();
    for &p in points {
        if let Some(last) = pts.last()
            && (p.x - last.x).abs() < 1e-9
            && (p.y - last.y).abs() < 1e-9
        {
            continue;
        }
        // 与倒数第二点共线则替换（用更远点，保持方向）。
        if pts.len() >= 2 {
            let a = pts[pts.len() - 2];
            let b = *pts.last().unwrap();
            if ((b.x - a.x) * (p.y - b.y) - (b.y - a.y) * (p.x - b.x)).abs() < 1e-6 {
                *pts.last_mut().unwrap() = p;
                continue;
            }
        }
        pts.push(p);
    }
    crate::builder::ir::geograph::line_route(&pts)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Port {
    Top,
    Bottom,
    Left,
    Right,
}

/// 节点边上按槽位分散的端口点。
///
/// 基准是节点对应边的中点；`slot` 决定沿该边方向的偏移，使同一节点的多条出/入边
/// 分散开（而非都从边中点出发）。偏移范围限制在节点边长内（不超出节点包围盒）。
fn port_point_at(n: &GGNode, p: Port, slot: Slot) -> Point {
    let (hw, hh) = (n.size.width / 2.0, n.size.height / 2.0);
    let (base, span) = match p {
        // 沿边方向 = 沿 x（Bottom/Top），可偏移范围 = 节点宽度内。
        Port::Top | Port::Bottom => {
            let y = if matches!(p, Port::Top) { n.center.y - hh } else { n.center.y + hh };
            (Point::new(n.center.x, y), n.size.width)
        }
        // 沿边方向 = 沿 y（Left/Right），可偏移范围 = 节点高度内。
        Port::Left | Port::Right => {
            let x = if matches!(p, Port::Left) { n.center.x - hw } else { n.center.x + hw };
            (Point::new(x, n.center.y), n.size.height)
        }
    };
    if slot.total <= 1 {
        return base;
    }
    // 分散间距：总可用长度 / 边数，但 clamp 到合理最小，避免过于贴近角。
    let spacing = (span / slot.total as f64).min(28.0);
    // 居中对称：idx 从 -(total-1)/2 .. +(total-1)/2
    let offset = (slot.idx as f64 - (slot.total as f64 - 1.0) / 2.0) * spacing;
    match p {
        Port::Top | Port::Bottom => Point::new(base.x + offset, base.y),
        Port::Left | Port::Right => Point::new(base.x, base.y + offset),
    }
}

/// 沿端口法向外移 dist。
fn offset_along(p: Port, pt: Point, dist: f64) -> Point {
    match p {
        Port::Top => Point::new(pt.x, pt.y - dist),
        Port::Bottom => Point::new(pt.x, pt.y + dist),
        Port::Left => Point::new(pt.x - dist, pt.y),
        Port::Right => Point::new(pt.x + dist, pt.y),
    }
}

/// 节点回避：整体平移「主干」（中间点），沿主轴双向步进，直到不穿非端点节点。
/// 水平主导时统一平移所有中间点 y；垂直主导时统一平移 x。保持曼哈顿。
///
/// 策略：分别朝 + 方向与 - 方向各试 [`MAX_OFFSET_TRIES`] 步，取「冲突数最少」的落点，
/// 处理长边跨层（主干初始位置落在中间层节点内）的情况。
fn avoid_nodes(
    pts: &mut [Point],
    horizontal_main: bool,
    endpoint_ids: &[String],
    node_rects: &[(usize, Rect, String)],
    grid: &SpatialGrid,
) {
    if pts.len() < 3 {
        return;
    }
    let n = pts.len();

    // 计算当前主轴偏移量为 d 时的冲突数（未修改 pts，只读）。
    let conflicts_at = |offset: f64| -> usize {
        let mut cnt = 0;
        for k in 1..n - 1 {
            let a = shift(pts[k - 1], horizontal_main, offset);
            let b = shift(pts[k], horizontal_main, offset);
            if segment_hits_foreign(a, b, endpoint_ids, node_rects, grid) {
                cnt += 1;
            }
        }
        cnt
    };

    // 当前偏移。
    let cur = 0.0f64;
    let base = conflicts_at(cur);

    // 朝 + 方向搜索。
    let mut best_offset = 0.0f64;
    let mut best_conflicts = base;
    for step in 1..=MAX_OFFSET_TRIES {
        let off = cur + step as f64 * OFFSET_STEP;
        let c = conflicts_at(off);
        if c < best_conflicts {
            best_conflicts = c;
            best_offset = off;
            if c == 0 {
                break;
            }
        }
    }
    // 朝 - 方向搜索（+ 方向无解时）。
    if best_conflicts > 0 {
        for step in 1..=MAX_OFFSET_TRIES {
            let off = cur - step as f64 * OFFSET_STEP;
            let c = conflicts_at(off);
            if c < best_conflicts {
                best_conflicts = c;
                best_offset = off;
                if c == 0 {
                    break;
                }
            }
        }
    }

    // 应用最佳偏移（仅当确实改善）。只平移中间点（index 1..n-1），
    // 端点（index 0 / n-1）保持固定在节点端口上。
    if best_offset != 0.0 {
        for p in pts.iter_mut().take(n - 1).skip(1) {
            shift_mut(p, horizontal_main, best_offset);
        }
    }
}

fn shift(p: Point, horizontal_main: bool, offset: f64) -> Point {
    if horizontal_main {
        Point::new(p.x, p.y + offset)
    } else {
        Point::new(p.x + offset, p.y)
    }
}

fn shift_mut(p: &mut Point, horizontal_main: bool, offset: f64) {
    if horizontal_main {
        p.y += offset;
    } else {
        p.x += offset;
    }
}

/// 段是否穿过「非端点」节点包围盒（用网格缩小候选集）。
fn segment_hits_foreign(
    a: Point,
    b: Point,
    endpoint_ids: &[String],
    node_rects: &[(usize, Rect, String)],
    grid: &SpatialGrid,
) -> bool {
    let candidates = grid.query_segment(a, b);
    for cid in candidates {
        if let Some((_, rect, owner)) = node_rects.get(cid).map(|(i, r, o)| (*i, *r, o)) {
            // 跳过端点节点（边自然穿过自己的端点节点）
            if endpoint_ids.iter().any(|e| e == owner) {
                continue;
            }
            if segment_intersects_rect(a, b, &rect) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ir::geograph::{GGEdge, GGNode, Geograph};
    use crate::builder::ir::shape::ShapeKind;
    use crate::builder::ir::unigraph::EdgeKind;
    use crate::builder::ir::common::{
        ArrowSpec, NodeRole, ResolvedPorts, RoutingHint,
    };

    fn mk_node(id: &str, x: f64, y: f64, w: f64, h: f64) -> GGNode {
        let p = Point::new(x, y);
        GGNode {
            id: id.to_string(),
            role: NodeRole::Atom,
            center: p,
            size: lievisual::geometry::Size::new(w, h),
            shape: ShapeKind::Rectangle,
            ports: ResolvedPorts {
                top: Point::new(x, y - h / 2.0),
                bottom: Point::new(x, y + h / 2.0),
                left: Point::new(x - w / 2.0, y),
                right: Point::new(x + w / 2.0, y),
            },
            label: None,
        }
    }

    fn mk_edge(s: &str, t: &str) -> GGEdge {
        GGEdge {
            id: format!("{}-{}", s, t),
            source: s.to_string(),
            target: t.to_string(),
            route: crate::builder::ir::geograph::RoutePath::new(),
            label_text: None,
            label_anchor: None,
            kind: EdgeKind::Flow,
            arrow: ArrowSpec { start: crate::builder::ir::common::ArrowKind::None, end: crate::builder::ir::common::ArrowKind::Arrow },
            routing_hint: RoutingHint::Orthogonal,
            line_kind: crate::builder::ir::common::LineKind::Solid,
        }
    }

    #[test]
    fn orthogonal_route_has_orthogonal_segments_and_avoids_node() {
        // A 在左、B 在右，中间 C 阻挡
        // A(0,0) - B(200,0)，C(100,0) 正好在连线上
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![
                mk_node("A", 0.0, 0.0, 60.0, 40.0),
                mk_node("B", 200.0, 0.0, 60.0, 40.0),
                mk_node("C", 100.0, 0.0, 60.0, 40.0),
            ],
            edges: vec![mk_edge("A", "B")],
            containers: vec![],
        };
        route_edges(&mut gg, crate::ast::Direction::TB);
        let route = &gg.edges[0].route;
        // 正交折线应 >= 3 个段（含 stub + 中间拐点）
        assert!(route.len() >= 3, "正交路由段过少: {:?}", route);

        // 路由不应穿过 C 的包围盒（除非 C 是端点，但 C 不是）
        let c_rect = Rect::new(100.0 - 32.0, 0.0 - 22.0, 100.0 + 32.0, 0.0 + 22.0);
        for seg in route.iter() {
            let (a, b) = (seg.start(), seg.end());
            assert!(
                !segment_intersects_rect(a, b, &c_rect),
                "路由穿越中间阻挡节点 C: {:?} -> {:?}",
                a,
                b
            );
        }
    }

    #[test]
    fn route_preserves_endpoints() {
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![
                mk_node("A", 0.0, 0.0, 60.0, 40.0),
                mk_node("B", 0.0, 150.0, 60.0, 40.0),
            ],
            edges: vec![mk_edge("A", "B")],
            containers: vec![],
        };
        route_edges(&mut gg, crate::ast::Direction::TB);
        let route = &gg.edges[0].route;
        // 垂直主导：A 出 Bottom 端口(0,20)，B 入 Top 端口(0,130)
        let start = route.start();
        assert!((start.x - 0.0).abs() < 1.0, "起点 x 应≈0");
        assert!(start.y > 0.0 && start.y < 40.0, "起点应在 A 底部出线, got {:?}", start);
        let end = route.end();
        assert!(end.y > 110.0 && end.y < 150.0, "终点应在 B 顶部入线, got {:?}", end);
    }
}

