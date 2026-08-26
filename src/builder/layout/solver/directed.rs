//! `DirectedSolver`：有向图（flowchart / state）的分层布局求解器。
//!
//! 复用现有成熟的 `SugiyamaLayout` 作为**纯黑盒**求解器，所有启发式编排
//! （SCC / 连通分量 / 拓扑序）都在这层通过「带序号的节点 id」控制层内初始顺序，
//! **不侵入 `SugiyamaLayout` 内部**。
//!
//! 关键技巧：`SugiyamaLayout::build_layer_index` 按节点 id 字符串排序层内节点。
//! 我们在入图时给每个节点构造 `"{order:04}:{real_id}"` 形式的 id，使字符串排序
//! 的次序 = 启发式编排的次序，从而影响层内初始排列。`NodeIndex` 顺序仍 =
//! `LayoutGraph.nodes` 顺序，映射回 `PlacedGraph` 时天然正确。

use std::collections::HashMap;

use lievisual::geometry::{Point, Size};
use petgraph::graph::{DiGraph, NodeIndex};

use crate::ast::Direction;

use super::super::analyze::{GraphAnalysis, analyze};
use super::super::config::LayoutConfig;
use super::super::ir::{LNode, LayoutGraph, PlacedGraph};
use super::super::sugiyama::{NodeSize, SugiyamaConfig, SugiyamaLayout};

/// `DirectedSolver`：无子图的有向图分层布局。
pub struct DirectedSolver;

/// 构建 petgraph 有向图。
///
/// 节点添加顺序 = `LayoutGraph.nodes` 顺序（保证 `NodeIndex` ↔ 节点下标对应）；
/// 节点 id 用启发式序号的带前缀形式，使 `SugiyamaLayout` 层内排序受启发式控制。
fn build_graph(
    lg: &LayoutGraph,
    analysis: &GraphAnalysis,
) -> (DiGraph<String, ()>, Vec<NodeIndex>) {
    // 启发式编排：连通分量聚拢 → SCC 聚拢 → 拓扑序 → 源码序兜底
    let order = heuristic_order(lg, analysis);

    // order[i] = 第 i 个被"优先"的节点下标。逆映射：node_index -> priority
    let mut priority = vec![0usize; lg.nodes.len()];
    for (prio, &node_idx) in order.iter().enumerate() {
        priority[node_idx] = prio;
    }

    let mut graph: DiGraph<String, ()> = DiGraph::with_capacity(lg.nodes.len(), lg.edges.len());
    // 节点添加顺序 = LayoutGraph.nodes 顺序
    let mut idx_map: Vec<NodeIndex> = Vec::with_capacity(lg.nodes.len());
    for (i, n) in lg.nodes.iter().enumerate() {
        let _ = n;
        let id = format!("{:04}:{}", priority[i], i);
        idx_map.push(graph.add_node(id));
    }

    for e in &lg.edges {
        if e.source < lg.nodes.len() && e.target < lg.nodes.len() {
            let a = idx_map[e.source];
            let b = idx_map[e.target];
            if a != b && graph.find_edge(a, b).is_none() {
                graph.add_edge(a, b, ());
            }
        }
    }
    (graph, idx_map)
}

/// 启发式编排：决定每个节点的「优先级」（越小越先）。
///
/// 目标：连通分量内节点聚拢（分量间留白）、同 SCC 节点相邻、有环时按拓扑序尽量有序。
fn heuristic_order(lg: &LayoutGraph, analysis: &GraphAnalysis) -> Vec<usize> {
    let n = lg.nodes.len();
    let mut result: Vec<usize> = Vec::with_capacity(n);
    let mut seen = vec![false; n];

    // 1. 按连通分量：分量内聚拢
    for comp in &analysis.connected_components {
        // 2. 分量内按 SCC 聚拢
        let mut scc_touched: Vec<usize> = Vec::new();
        // 收集该分量内节点涉及的 SCC
        let mut in_comp_scc: Vec<usize> = Vec::new();
        for (scc_idx, scc) in analysis.sccs.iter().enumerate() {
            if scc.iter().any(|&node| comp.contains(&node)) {
                in_comp_scc.push(scc_idx);
            }
        }
        // 按 SCC 内最小节点下标排序 SCC，保证确定性
        in_comp_scc.sort_by_key(|&si| analysis.sccs[si].iter().copied().min().unwrap_or(0));
        for si in in_comp_scc {
            let mut members = analysis.sccs[si].clone();
            // SCC 内按源码序，保证确定性
            members.sort_unstable();
            for m in members {
                if comp.contains(&m) && !seen[m] {
                    result.push(m);
                    seen[m] = true;
                    scc_touched.push(m);
                }
            }
        }
        // 分量内不属于任何多节点 SCC 的节点，按拓扑序
        for &node in comp {
            if !seen[node] {
                result.push(node);
                seen[node] = true;
            }
        }
    }
    // 兜底：所有节点
    for (i, is_seen) in seen.iter_mut().enumerate() {
        if !*is_seen {
            result.push(i);
            *is_seen = true;
        }
    }
    result
}

impl DirectedSolver {
    /// 把 `LayoutGraph` 求解为 `PlacedGraph`。
    pub fn solve(lg: &LayoutGraph, config: &LayoutConfig) -> PlacedGraph {
        // 有组 → 交给 GroupedDirected
        if !lg.groups.is_empty() {
            return super::grouped::GroupedDirected::solve(lg, config);
        }

        let analysis = analyze(lg);
        let (graph, idx_map) = build_graph(lg, &analysis);

        let mut sizes: HashMap<NodeIndex, NodeSize> = HashMap::new();
        for (i, n) in lg.nodes.iter().enumerate() {
            sizes.insert(
                idx_map[i],
                NodeSize {
                    width: n.size.width,
                    height: n.size.height,
                },
            );
        }

        let sug_config = SugiyamaConfig {
            node_gap: config.node_gap,
            layer_gap: config.layer_gap,
            crossing_iterations: config.crossing_iterations,
            ..Default::default()
        };
        let sugiyama = SugiyamaLayout::new(sug_config, &graph);
        let result = sugiyama.layout(&sizes);

        // 映射回 PlacedGraph：positions[i] = LayoutGraph.nodes[i] 的中心
        let mut positions: Vec<Point> = vec![Point::new(0.0, 0.0); lg.nodes.len()];
        for ni in graph.node_indices() {
            if let Some(p) = result.positions.get(&ni)
                && ni.index() < lg.nodes.len()
            {
                positions[ni.index()] = *p;
            }
        }

        // edge_routes 与 LayoutGraph.edges 同序
        // 新路由层：不依赖 sugiyama 的正交折线，改为「贝塞尔曲线 + 绕开节点避障」。
        // - 端点裁剪到节点边框（朝向目标方向）。
        // - 正向边：两点贝塞尔（简单）。
        // - 回边（feedback arc）：绕到与正向边相反侧，生成 S 曲线。
        // - 避障：若两点边穿过中间节点矩形，则加绕行点。
        let mut edge_routes: Vec<Vec<Point>> = Vec::with_capacity(lg.edges.len());
        for e in &lg.edges {
            if e.source >= lg.nodes.len() || e.target >= lg.nodes.len() {
                edge_routes.push(vec![]);
                continue;
            }
            // 自环（source == target）单独生成小环路径
            if e.source == e.target {
                edge_routes.push(self_loop_route(&lg.nodes[e.source], positions[e.source]));
                continue;
            }
            let a = idx_map[e.source];
            let b = idx_map[e.target];
            let is_feedback = result.feedback_arcs.contains(&(a, b));
            // 端点裁剪到节点边框
            let start = clip_to_border(
                positions[e.source],
                positions[e.target],
                &lg.nodes[e.source].size,
            );
            let end = clip_to_border(
                positions[e.target],
                positions[e.source],
                &lg.nodes[e.target].size,
            );
            if is_feedback {
                // 回边：绕到反侧（远离正向边一侧）的 S 曲线
                edge_routes.push(feedback_s_curve(
                    start,
                    end,
                    &lg.nodes[e.source].size,
                    &lg.nodes[e.target].size,
                ));
            } else {
                // 正向边：先两点，检测是否穿过中间节点，必要时绕行
                edge_routes.push(route_forward(
                    start, end, e.source, e.target, &positions, lg,
                ));
            }
        }

        let size = compute_bbox_size(&positions, &edge_routes);

        let mut placed = PlacedGraph {
            positions,
            edge_routes,
            group_bounds: vec![],
            size,
        };

        // 方向变换（sugiyama 恒为 TB）
        apply_direction(&mut placed, &config.direction);
        placed
    }
}

/// 回边 S 曲线：4 点（起、控制1、控制2、终）形成平滑绕行 S 曲线。
///
/// 绕行侧：取「源和目标之间区域的相反侧」——源在右时绕左侧，源在左时绕右侧，
/// 从而避开主流向（正向边）所在的侧，减少与其他边的交叉。
fn feedback_s_curve(start: Point, end: Point, src_size: &Size, tgt_size: &Size) -> Vec<Point> {
    // 绕行外侧 x：源在右则绕到左（更小），源在左则绕到右（更大）
    let offset = 60.0 + (src_size.width.max(tgt_size.width)) * 0.5;
    let outer_x = if start.x >= end.x {
        (start.x.min(end.x)) - offset
    } else {
        (start.x.max(end.x)) + offset
    };
    let mid_y = (start.y + end.y) / 2.0;
    // 让 c1 和 c2 略错开（避免重合和正交），形成平滑 S
    let c1 = Point::new(outer_x, mid_y - 12.0);
    let c2 = Point::new(outer_x + 20.0, mid_y + 12.0);
    vec![start, c1, c2, end]
}

/// 把 `from`（节点中心）沿 `toward` 方向裁剪到节点边框。
///
/// 返回 `from + t*(toward-from)`，`t` 为最小正参数使点落在矩形边框上。
fn clip_to_border(from: Point, toward: Point, size: &Size) -> Point {
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

/// 正向边路由：两点边，若穿过中间节点矩形则加绕行点。
///
/// 返回 2 点（正常）或 4 点（需绕行）的路径。
fn route_forward(
    start: Point,
    end: Point,
    src_idx: usize,
    tgt_idx: usize,
    positions: &[Point],
    lg: &LayoutGraph,
) -> Vec<Point> {
    // 检测线段 start→end 是否穿过其他节点矩形
    let seg = (start, end);
    for (i, n) in lg.nodes.iter().enumerate() {
        if i == src_idx || i == tgt_idx {
            continue;
        }
        let center = positions[i];
        let rect = lievisual::geometry::Rect::new(
            center.x - n.size.width / 2.0,
            center.y - n.size.height / 2.0,
            center.x + n.size.width / 2.0,
            center.y + n.size.height / 2.0,
        );
        if segment_intersects_rect(seg.0, seg.1, rect) {
            // 穿过中间节点：水平外扩绕行，绕过节点矩形一侧。
            // 生成 4 点：[start, (outer_x,start.y), (outer_x,end.y), end]，
            // 先水平到节点外侧，再垂直，再水平回终点。
            // 外侧 x：取节点矩形外、且尽量远离穿过的方向。
            let rect_left = rect.min_x();
            let rect_right = rect.max_x();
            // 选择绕行侧：优先取"start/end x 更靠内的一侧的外侧"（远离中心）
            let center_x = (rect_left + rect_right) / 2.0;
            let outer_x = if start.x <= center_x {
                rect_left - 40.0
            } else {
                rect_right + 40.0
            };
            return vec![
                start,
                Point::new(outer_x, start.y),
                Point::new(outer_x, end.y),
                end,
            ];
        }
    }
    vec![start, end]
}

/// 线段是否与矩形相交（粗略检测）。
fn segment_intersects_rect(a: Point, b: Point, rect: lievisual::geometry::Rect) -> bool {
    // 先检查 bbox 粗略相交
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
    // 线段与矩形四条边的相交检测（简化为采样检测中点附近）
    // 更稳妥：检查线段是否与矩形边相交
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

/// 两条线段是否相交。
fn segments_intersect(p1: Point, p2: Point, p3: Point, p4: Point) -> bool {
    let d1 = cross(p3, p4, p1);
    let d2 = cross(p3, p4, p2);
    let d3 = cross(p1, p2, p3);
    let d4 = cross(p1, p2, p4);
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

/// 三点叉积（判断 p 相对线段 a→b 的方位）。
fn cross(a: Point, b: Point, p: Point) -> f64 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// 生成自环路径：从节点右侧伸出一个小环，绕回节点右侧。
///
/// 返回一组折线点（起点 → 外凸 → 终点），渲染层据此画出自环。
fn self_loop_route(node: &LNode, center: Point) -> Vec<Point> {
    let half_w = node.size.width / 2.0;
    let half_h = node.size.height / 2.0;
    let extend = (half_w * 0.6 + 10.0).max(20.0); // 向右伸出距离
    let loop_h = half_h * 0.5; // 环的竖直跨度
    let right_x = center.x + half_w;
    // 起点：节点右边缘中部偏上；终点：节点右边缘中部偏下
    let start = Point::new(right_x, center.y - loop_h * 0.4);
    let end = Point::new(right_x, center.y + loop_h * 0.6);
    let outer_x = center.x + half_w + extend;
    vec![
        start,
        Point::new(outer_x, start.y),
        Point::new(outer_x, end.y),
        end,
    ]
}

/// 计算内容实际占据的 bbox 尺寸。
fn compute_bbox_size(positions: &[Point], edge_routes: &[Vec<Point>]) -> Size {
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
    for route in edge_routes {
        for p in route {
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

/// 方向变换：sugiyama 内部恒为 TB（y 为层主轴）。LR/BT/RL 做坐标映射。
///
/// 等价于 dagre 的 rankdir 语义（flowchart.rs 的 transform_sugiyama_direction 思想）：
/// - TD: 不变
/// - BT: 上下镜像 (x,y) -> (x,-y)
/// - LR: 转置 (x,y) -> (y,x)
/// - RL: 转置+镜像 (-y,x)
fn apply_direction(placed: &mut PlacedGraph, direction: &Direction) {
    let map = |p: &mut Point| match direction {
        Direction::TB | Direction::TD => {}
        Direction::BT => p.y = -p.y,
        Direction::LR => {
            let (x, y) = (p.x, p.y);
            p.x = y;
            p.y = x;
        }
        Direction::RL => {
            let (x, y) = (p.x, p.y);
            p.x = -y;
            p.y = x;
        }
    };

    // 映射后整体平移使坐标非负（与 dagre bounding box 一致）
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for p in placed.positions.iter_mut() {
        map(p);
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
    }
    for route in placed.edge_routes.iter_mut() {
        for p in route.iter_mut() {
            map(p);
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
        }
    }
    for b in placed.group_bounds.iter_mut() {
        // group_bounds 不在本层生成（无组时为空），此处仅防御
        let _ = b;
    }

    if min_x.is_finite() {
        let (dx, dy) = (-min_x, -min_y);
        for p in placed.positions.iter_mut() {
            p.x += dx;
            p.y += dy;
        }
        for route in placed.edge_routes.iter_mut() {
            for p in route.iter_mut() {
                p.x += dx;
                p.y += dy;
            }
        }
    }
}
