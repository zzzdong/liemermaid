//! `GridSolver`：class / er 图的分层网格布局。
//!
//! 职责：把纯拓扑的 `LayoutGraph`（class / er 的节点 + 关系边）求解为 `PlacedGraph`：
//! - 按「入度为 0 的根 → BFS 深度」分层（同层水平排布、跨层居中）
//! - 边路由：端点裁剪到节点边框 + 穿过中间节点时正交绕行
//!
//! 求解器不接触 AST，只读 `LayoutGraph`（节点尺寸已由 `convert` 测量）。

use lievisual::geometry::{Point, Rect, Size};

use super::super::config::LayoutConfig;
use super::super::ir::{LayoutGraph, LineKind, PlacedGraph};
use super::LayoutSolver;

/// `GridSolver`：class / er 的网格布局。
pub struct GridSolver;

impl LayoutSolver for GridSolver {
    fn solve(&self, lg: &LayoutGraph, config: &LayoutConfig) -> PlacedGraph {
        let n = lg.nodes.len();
        let mut positions = vec![Point::new(0.0, 0.0); n];
        if n == 0 {
            return PlacedGraph {
                positions,
                edge_routes: vec![],
                edge_kinds: vec![],
                group_bounds: vec![],
                size: Size::new(0.0, 0.0),
            };
        }

        // ---- 1. BFS 分层（入度为 0 的根在第 0 层） ----
        let mut in_deg = vec![0usize; n];
        for e in &lg.edges {
            if e.target < n {
                in_deg[e.target] += 1;
            }
        }
        let mut layer = vec![usize::MAX; n];
        let mut frontier: Vec<usize> = (0..n)
            .filter(|&i| in_deg[i] == 0 || lg.edges.is_empty())
            .collect();
        // 全成环时退化为全部第 0 层
        if frontier.is_empty() {
            frontier = (0..n).collect();
        }
        let mut depth = 0usize;
        let mut visited = 0usize;
        while !frontier.is_empty() && visited < n {
            let mut next = Vec::new();
            for i in frontier {
                if layer[i] != usize::MAX {
                    continue;
                }
                layer[i] = depth;
                visited += 1;
                for e in &lg.edges {
                    if e.source == i && layer[e.target] == usize::MAX {
                        next.push(e.target);
                    }
                }
            }
            // 兜底：本轮未覆盖的孤立/后节点（避免死循环）
            if next.is_empty() && visited < n {
                for i in 0..n {
                    if layer[i] == usize::MAX {
                        next.push(i);
                        break;
                    }
                }
            }
            depth += 1;
            frontier = next;
        }

        // ---- 2. 每层水平排布（同层节点按源码序），跨层居中 ----
        let node_gap = config.node_gap;
        let layer_gap = config.layer_gap;
        let mut max_layer = 0usize;
        for l in &layer {
            max_layer = max_layer.max(*l);
        }
        // 每层总宽
        let mut layer_w = vec![0.0f64; max_layer + 1];
        let mut layer_nodes: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
        for i in 0..n {
            let l = layer[i];
            layer_nodes[l].push(i);
            layer_w[l] += lg.nodes[i].size.width;
        }
        for l in 0..=max_layer {
            if layer_nodes[l].len() > 1 {
                layer_w[l] += (layer_nodes[l].len() - 1) as f64 * node_gap;
            }
        }
        let max_w = layer_w.iter().cloned().fold(0.0f64, f64::max);

        let margin = 40.0;
        let mut cur_y = margin;
        for l in 0..=max_layer {
            if layer_nodes[l].is_empty() {
                continue;
            }
            let mut max_h: f64 = 0.0;
            for &i in &layer_nodes[l] {
                max_h = max_h.max(lg.nodes[i].size.height);
            }
            let offset = ((max_w - layer_w[l]) / 2.0).max(0.0);
            let mut cur_x = margin + offset;
            for &i in &layer_nodes[l] {
                let sz = lg.nodes[i].size;
                positions[i] = Point::new(cur_x + sz.width / 2.0, cur_y + sz.height / 2.0);
                cur_x += sz.width + node_gap;
            }
            cur_y += max_h + layer_gap;
        }

        // ---- 3. 边路由：裁剪到边框 + 避障绕行 ----
        let edge_routes: Vec<Vec<Point>> = lg
            .edges
            .iter()
            .map(|e| {
                if e.source >= n || e.target >= n {
                    return vec![];
                }
                let a = positions[e.source];
                let b = positions[e.target];
                let start = clip_to_border(a, b, lg.nodes[e.source].size);
                let end = clip_to_border(b, a, lg.nodes[e.target].size);
                route_avoid(start, end, e.source, e.target, &positions, lg)
            })
            .collect();

        let size = compute_size(&positions, &edge_routes);
        let edge_kinds: Vec<LineKind> = lg.edges.iter().map(|e| e.line_kind).collect();
        PlacedGraph {
            positions,
            edge_routes,
            edge_kinds,
            group_bounds: vec![],
            size,
        }
    }
}

/// 把 `from`（节点中心）沿 `toward` 方向裁剪到节点矩形边框。
fn clip_to_border(from: Point, toward: Point, size: Size) -> Point {
    let half_w = size.width / 2.0;
    let half_h = size.height / 2.0;
    let vx = toward.x - from.x;
    let vy = toward.y - from.y;
    let tx = if vx.abs() > 1e-9 {
        half_w / vx.abs()
    } else {
        f64::INFINITY
    };
    let ty = if vy.abs() > 1e-9 {
        half_h / vy.abs()
    } else {
        f64::INFINITY
    };
    let t = tx.min(ty);
    if !t.is_finite() {
        return from;
    }
    Point::new(from.x + vx * t, from.y + vy * t)
}

/// 路由：若线段穿过中间节点，加一个正交绕行点。
fn route_avoid(
    start: Point,
    end: Point,
    src_idx: usize,
    tgt_idx: usize,
    positions: &[Point],
    lg: &LayoutGraph,
) -> Vec<Point> {
    // 构造障碍（其余节点矩形）
    let mut obstacle: Option<Rect> = None;
    for (i, n) in lg.nodes.iter().enumerate() {
        if i == src_idx || i == tgt_idx {
            continue;
        }
        let c = positions[i];
        let rect = Rect::new(
            c.x - n.size.width / 2.0,
            c.y - n.size.height / 2.0,
            c.x + n.size.width / 2.0,
            c.y + n.size.height / 2.0,
        );
        if segment_intersects_rect(start, end, rect) {
            obstacle = Some(rect);
            break;
        }
    }
    match obstacle {
        Some(rect) => {
            let mid = Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
            // 绕行方向：水平外扩（避开上下遮挡）
            let dx = if mid.x >= rect.center().x {
                1.0
            } else {
                -1.0
            };
            let detour = Point::new(mid.x + dx * (rect.width() / 2.0 + 30.0), mid.y);
            vec![start, detour, end]
        }
        None => vec![start, end],
    }
}

fn segment_intersects_rect(a: Point, b: Point, rect: Rect) -> bool {
    let seg_min_x = a.x.min(b.x);
    let seg_min_y = a.y.min(b.y);
    let seg_max_x = a.x.max(b.x);
    let seg_max_y = a.y.max(b.y);
    if seg_max_x < rect.min_x()
        || seg_min_x > rect.max_x()
        || seg_max_y < rect.min_y()
        || seg_min_y > rect.max_y()
    {
        return false;
    }
    let edges = [
        (
            Point::new(rect.min_x(), rect.min_y()),
            Point::new(rect.max_x(), rect.min_y()),
        ),
        (
            Point::new(rect.max_x(), rect.min_y()),
            Point::new(rect.max_x(), rect.max_y()),
        ),
        (
            Point::new(rect.max_x(), rect.max_y()),
            Point::new(rect.min_x(), rect.max_y()),
        ),
        (
            Point::new(rect.min_x(), rect.max_y()),
            Point::new(rect.min_x(), rect.min_y()),
        ),
    ];
    for (p1, p2) in edges {
        if segments_intersect(a, b, p1, p2) {
            return true;
        }
    }
    false
}

fn segments_intersect(p1: Point, p2: Point, p3: Point, p4: Point) -> bool {
    let d1 = cross(p3, p4, p1);
    let d2 = cross(p3, p4, p2);
    let d3 = cross(p1, p2, p3);
    let d4 = cross(p1, p2, p4);
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

fn cross(a: Point, b: Point, p: Point) -> f64 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

fn compute_size(positions: &[Point], routes: &[Vec<Point>]) -> Size {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in positions {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    for r in routes {
        for p in r {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    if !min_x.is_finite() {
        return Size::new(0.0, 0.0);
    }
    Size::new(max_x - min_x, max_y - min_y)
}
