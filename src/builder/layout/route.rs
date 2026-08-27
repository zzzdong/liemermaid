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

use crate::builder::ir::geograph::{GGEdge, GGNode, Geograph};

use super::spatial::{SpatialGrid, segment_intersects_rect};

/// 出线 stub 长度（端口法向延伸距离）。
const STUB: f64 = 18.0;
/// 节点回避偏移步进。
const OFFSET_STEP: f64 = 12.0;
/// 回避最大尝试次数。
const MAX_OFFSET_TRIES: usize = 8;

/// 为 GG 中所有边生成正交路由。
pub fn route_edges(gg: &mut Geograph) {
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

    for edge in &mut gg.edges {
        let (Some(s), Some(t)) = (node_map.get(&edge.source), node_map.get(&edge.target)) else {
            continue;
        };
        let s = *s;
        let t = *t;
        let route = orthogonal_route(s, t, &node_rects, &grid);
        edge.route = route;
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

/// 生成单条边的正交路由 polyline（纯曼哈顿）。
fn orthogonal_route(
    s: &GGNode,
    t: &GGNode,
    node_rects: &[(usize, Rect, String)],
    grid: &SpatialGrid,
) -> Vec<Point> {
    // 自适应端口：source 朝 target 方向出，target 朝 source 方向入
    let sp = pick_port(s.center, t.center, true);
    let tp = pick_port(t.center, s.center, false);
    let start = port_point(s, sp);
    let end = port_point(t, tp);

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

    pts
}

/// 选端口：朝 `toward` 方向。is_source=true 表示这是起点（出端口），
/// false 表示终点（入端口，朝向 source 方向即「反向」）。
fn pick_port(from: Point, toward: Point, is_source: bool) -> Port {
    let dx = toward.x - from.x;
    let dy = toward.y - from.y;
    if is_source {
        if dx.abs() >= dy.abs() {
            if dx >= 0.0 { Port::Right } else { Port::Left }
        } else {
            if dy >= 0.0 { Port::Bottom } else { Port::Top }
        }
    } else {
        // 终点入端口：朝向 source，方向与起点镜像
        if dx.abs() >= dy.abs() {
            if dx >= 0.0 { Port::Right } else { Port::Left }
        } else {
            if dy >= 0.0 { Port::Bottom } else { Port::Top }
        }
    }
}

#[derive(Clone, Copy)]
enum Port {
    Top,
    Bottom,
    Left,
    Right,
}

fn port_point(n: &GGNode, p: Port) -> Point {
    let (hw, hh) = (n.size.width / 2.0, n.size.height / 2.0);
    match p {
        Port::Top => Point::new(n.center.x, n.center.y - hh),
        Port::Bottom => Point::new(n.center.x, n.center.y + hh),
        Port::Left => Point::new(n.center.x - hw, n.center.y),
        Port::Right => Point::new(n.center.x + hw, n.center.y),
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

/// 节点回避：整体平移「主干」（中间点），沿主轴步进，直到不穿非端点节点。
/// 水平主导时统一平移所有中间点 y；垂直主导时统一平移 x。保持曼哈顿。
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
    let mut tries = 0;
    while tries < MAX_OFFSET_TRIES {
        // 采样所有中间段（index 1..len-1），先收集避免与下方可变借冲突
        let segs: Vec<(Point, Point)> = (1..pts.len() - 1).map(|k| (pts[k - 1], pts[k])).collect();
        let hit = segs
            .iter()
            .any(|(a, b)| segment_hits_foreign(*a, *b, endpoint_ids, node_rects, grid));
        if !hit {
            break;
        }
        // 整体平移主轴坐标（所有中间点）
        let n = pts.len();
        for p in pts.iter_mut().take(n - 1).skip(1) {
            if horizontal_main {
                p.y += OFFSET_STEP;
            } else {
                p.x += OFFSET_STEP;
            }
        }
        tries += 1;
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
        ArrowSpec, NodeId, NodeRole, ResolvedPorts, RoutingHint,
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
            route: vec![],
            label_anchor: None,
            kind: EdgeKind::Flow,
            arrow: ArrowSpec { start: crate::builder::ir::common::ArrowKind::None, end: crate::builder::ir::common::ArrowKind::Arrow },
            routing_hint: RoutingHint::Orthogonal,
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
        route_edges(&mut gg);
        let route = &gg.edges[0].route;
        // 正交折线应 >= 3 个点（含 stub + 中间拐点）
        assert!(route.len() >= 3, "正交路由点过少: {:?}", route);

        // 路由不应穿过 C 的包围盒（除非 C 是端点，但 C 不是）
        let c_rect = Rect::new(100.0 - 32.0, 0.0 - 22.0, 100.0 + 32.0, 0.0 + 22.0);
        for w in route.windows(2) {
            assert!(
                !segment_intersects_rect(w[0], w[1], &c_rect),
                "路由穿越中间阻挡节点 C: {:?} -> {:?}",
                w[0],
                w[1]
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
        route_edges(&mut gg);
        let route = &gg.edges[0].route;
        // 垂直主导：A 出 Bottom 端口(0,20)，B 入 Top 端口(0,130)
        assert!((route[0].x - 0.0).abs() < 1.0, "起点 x 应≈0");
        assert!(route[0].y > 0.0 && route[0].y < 40.0, "起点应在 A 底部出线, got {:?}", route[0]);
        let last = route.last().unwrap();
        assert!(last.y > 110.0 && last.y < 150.0, "终点应在 B 顶部入线, got {:?}", last);
    }
}

