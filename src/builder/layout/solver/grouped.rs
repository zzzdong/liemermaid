//! `GroupedDirected`：带子图 / 复合状态的有向图布局。
//!
//! 策略：**递归求解 + 纯平移回贴**（拒绝仿射）。
//! - 每个组先递归求解内部子图 → 子 `PlacedGraph`
//! - 组的容器尺寸 = 子图 bbox + padding + 标题高
//! - 构建「外部图」（组折叠为 super-node + 独立节点 + 跨组边）
//! - 对外部图做 `DirectedSolver` → super-node 位置
//! - 把子图内部坐标**纯平移**贴回（偏移 = super-node 中心 - 子图 bbox 中心）

use std::collections::{HashMap, HashSet};

use lievisual::geometry::{Point, Rect, Size};

use super::super::config::LayoutConfig;
use super::super::ir::{GroupChild, LEdge, LGroup, LNode, LayoutGraph, LineKind, PlacedGraph, PortHint};
use super::directed::DirectedSolver;
use super::LayoutSolver;

/// 子图容器标题区高度（额外计入容器尺寸）。
const SUBGRAPH_TITLE_H: f64 = 22.0;

/// `GroupedDirected`：有子图的有向图布局。
pub struct GroupedDirected;

impl GroupedDirected {
    pub fn solve(lg: &LayoutGraph, config: &LayoutConfig) -> PlacedGraph {
        // 1. 递归求解每个组的内部子图，并计算容器尺寸
        let n_groups = lg.groups.len();
        let mut sub_placed: Vec<PlacedGraph> = Vec::with_capacity(n_groups);
        let mut container_sizes: Vec<Size> = Vec::with_capacity(n_groups);

        let mut sub_member_lists: Vec<Vec<usize>> = Vec::with_capacity(n_groups);
        for gi in 0..n_groups {
            let (sub, member_list) = extract_subgraph(lg, gi);
            // 空组：给一个最小容器
            if sub.nodes.is_empty() {
                sub_placed.push(PlacedGraph {
                    positions: vec![],
                    edge_routes: vec![],
                    edge_kinds: vec![],
                    group_bounds: vec![],
                    size: Size::new(0.0, 0.0),
                });
                sub_member_lists.push(member_list);
                container_sizes.push(Size::new(
                    2.0 * config.group_padding,
                    2.0 * config.group_padding,
                ));
                continue;
            }
            let placed = DirectedSolver.solve(&sub, config);
            // 容器尺寸需计入节点实际宽高（节点中心跨度不含 size）
            let (_min_x, _min_y, w, h) = bbox_with_node_sizes(&placed, lg, &member_list);
            let container = Size::new(
                w + 2.0 * config.group_padding,
                h + 2.0 * config.group_padding + SUBGRAPH_TITLE_H,
            );
            sub_placed.push(placed);
            sub_member_lists.push(member_list);
            container_sizes.push(container);
        }

        // 2. 构建外部图：独立节点 + super-node
        // 确定哪些节点属于某个组
        let mut node_in_group: HashMap<usize, usize> = HashMap::new(); // node_idx -> group_idx
        for (gi, group) in lg.groups.iter().enumerate() {
            collect_members(group, lg, &mut node_in_group, gi);
        }

        // 外部节点：先独立节点（源码序），再 super-node（组序）
        let mut external = LayoutGraph::default();
        let mut external_idx_of_node: HashMap<usize, usize> = HashMap::new(); // 独立 node_idx -> external idx
        let mut external_idx_of_group: HashMap<usize, usize> = HashMap::new(); // group_idx -> external idx

        for (i, node) in lg.nodes.iter().enumerate() {
            if !node_in_group.contains_key(&i) {
                let ext_idx = external.nodes.len();
                external_idx_of_node.insert(i, ext_idx);
                external.nodes.push(LNode {
                    id: node.id.clone(),
                    size: node.size,
                    shape_hint: node.shape_hint,
                });
            }
        }
        for (gi, size) in container_sizes.iter().enumerate() {
            let ext_idx = external.nodes.len();
            external_idx_of_group.insert(gi, ext_idx);
            external.nodes.push(LNode {
                id: format!("__group_{gi}__"),
                size: *size,
                shape_hint: super::super::ir::ShapeHint::Rect,
            });
        }

        // 外部边：原 lg.edges 中两端都在外部节点池（独立 或 super-node）的
        for e in &lg.edges {
            let s_ext = external_idx_of_node.get(&e.source).copied().or_else(|| {
                node_in_group
                    .get(&e.source)
                    .and_then(|&g| external_idx_of_group.get(&g).copied())
            });
            let t_ext = external_idx_of_node.get(&e.target).copied().or_else(|| {
                node_in_group
                    .get(&e.target)
                    .and_then(|&g| external_idx_of_group.get(&g).copied())
            });
            if let (Some(s), Some(t)) = (s_ext, t_ext)
                && s != t
            {
                external.edges.push(LEdge {
                    source: s,
                    target: t,
                    source_port: PortHint::Auto,
                    target_port: PortHint::Auto,
                    line_kind: e.line_kind,
                });
            }
        }
        // 跨组边（lg.cross_group_edges）
        for e in &lg.cross_group_edges {
            let s_ext = external_idx_of_node.get(&e.source).copied().or_else(|| {
                node_in_group
                    .get(&e.source)
                    .and_then(|&g| external_idx_of_group.get(&g).copied())
            });
            let t_ext = external_idx_of_node.get(&e.target).copied().or_else(|| {
                node_in_group
                    .get(&e.target)
                    .and_then(|&g| external_idx_of_group.get(&g).copied())
            });
            if let (Some(s), Some(t)) = (s_ext, t_ext)
                && s != t
            {
                external.edges.push(LEdge {
                    source: s,
                    target: t,
                    source_port: PortHint::Auto,
                    target_port: PortHint::Auto,
                    line_kind: e.line_kind,
                });
            }
        }

        // 3. 外部求解（此时外部图无组，DirectedSolver 不会递归到 GroupedDirected）
        let external_placed = DirectedSolver.solve(&external, config);

        // 4. 平移回贴：把每个子图平移到对应 super-node
        // 计算每个组 super-node 在 external_placed 中的中心
        let mut final_positions: Vec<Option<Point>> = vec![None; lg.nodes.len()];

        // 独立节点直接取外部位置
        for (i, ext_idx) in &external_idx_of_node {
            final_positions[*i] = Some(external_placed.positions[*ext_idx]);
        }

        // 子图内部节点：平移
        for (gi, placed) in sub_placed.iter().enumerate() {
            let Some(&ext_idx) = external_idx_of_group.get(&gi) else {
                continue;
            };
            let super_center = external_placed.positions[ext_idx];
            // 子图 bbox 真实中心（sugiyama 有 padding，min 不一定为 0）
            let (min_x, min_y, w, h) = bbox_size(placed);
            let sub_center = Point::new(min_x + w / 2.0, min_y + h / 2.0);
            let dx = super_center.x - sub_center.x;
            let dy = super_center.y - sub_center.y;
            // 子图 placed.positions[k] 对应 sub_member_lists[gi][k]（lg 节点下标）
            let member_list = &sub_member_lists[gi];
            for (k, sub_pos) in placed.positions.iter().enumerate() {
                if k < member_list.len() {
                    let lg_idx = member_list[k];
                    final_positions[lg_idx] = Some(Point::new(sub_pos.x + dx, sub_pos.y + dy));
                }
            }
        }

        // 边：组内/顶层边 + 跨组边
        // 端点裁剪到节点边框（沿朝向目标方向），避免从节点中心出发穿过节点。
        // 跨组边加避障：检测穿过中间节点矩形或子图容器，绕到容器外侧。

        // 先算 group_bounds（跨组边避障需要容器矩形）
        let mut group_bounds: Vec<Rect> = Vec::with_capacity(n_groups);
        for (gi, size) in container_sizes.iter().enumerate() {
            if let Some(&ext_idx) = external_idx_of_group.get(&gi) {
                let c = external_placed.positions[ext_idx];
                group_bounds.push(Rect::new(
                    c.x - size.width / 2.0,
                    c.y - size.height / 2.0,
                    c.x + size.width / 2.0,
                    c.y + size.height / 2.0,
                ));
            } else {
                group_bounds.push(Rect::ZERO);
            }
        }

        let mut final_edges = Vec::with_capacity(lg.edges.len() + lg.cross_group_edges.len());

        // 组内/顶层边
        for e in &lg.edges {
            let s = e.source;
            let t = e.target;
            let sp = final_positions.get(s).copied().flatten();
            let tp = final_positions.get(t).copied().flatten();
            match (sp, tp) {
                (Some(a), Some(b)) => {
                    let start = clip_to_border(a, b, lg.nodes[s].size);
                    let end = clip_to_border(b, a, lg.nodes[t].size);
                    final_edges.push(vec![start, end]);
                }
                _ => final_edges.push(vec![]),
            }
        }

        // 跨组边：端点裁剪到边框 + 避障（绕开中间节点和子图容器）
        for e in &lg.cross_group_edges {
            let s = e.source;
            let t = e.target;
            let sp = final_positions.get(s).copied().flatten();
            let tp = final_positions.get(t).copied().flatten();
            match (sp, tp) {
                (Some(a), Some(b)) => {
                    let start = clip_to_border(a, b, lg.nodes[s].size);
                    let end = clip_to_border(b, a, lg.nodes[t].size);
                    // 避障：检测穿过中间节点/子图容器，绕行
                    let route = route_cross_group(
                        start,
                        end,
                        s,
                        t,
                        &positions_now(&final_positions),
                        lg,
                        &group_bounds,
                    );
                    final_edges.push(route);
                }
                _ => final_edges.push(vec![]),
            }
        }

        // 组装
        let positions: Vec<Point> = final_positions
            .into_iter()
            .map(|p| p.unwrap_or(Point::new(0.0, 0.0)))
            .collect();

        // 先算 size（借用），再 move 进结构体
        let size = compute_total_size(&positions, &final_edges, &group_bounds);
        // 边线型：组内/顶层边在前，跨组边在后，与 final_edges 顺序一致
        let edge_kinds: Vec<LineKind> = lg
            .edges
            .iter()
            .chain(lg.cross_group_edges.iter())
            .map(|e| e.line_kind)
            .collect();
        let mut placed = PlacedGraph {
            positions,
            edge_routes: final_edges,
            edge_kinds,
            group_bounds,
            size,
        };
        placed.normalize();
        placed
    }
}

/// 收集组的全部成员（含嵌套）节点下标，记录 node_idx → 最内层 group_idx。
fn collect_members(
    group: &LGroup,
    lg: &LayoutGraph,
    node_in_group: &mut HashMap<usize, usize>,
    group_idx: usize,
) {
    for child in &group.children {
        match child {
            GroupChild::Node(i) => {
                node_in_group.entry(*i).or_insert(group_idx);
            }
            GroupChild::Group(gi) => {
                if let Some(sub) = lg.groups.get(*gi) {
                    collect_members(sub, lg, node_in_group, group_idx);
                }
            }
        }
    }
}

/// 收集组的全部成员节点下标（含嵌套）。
fn collect_member_indices(group: &LGroup, lg: &LayoutGraph, out: &mut HashSet<usize>) {
    for child in &group.children {
        match child {
            GroupChild::Node(i) => {
                out.insert(*i);
            }
            GroupChild::Group(gi) => {
                if let Some(sub) = lg.groups.get(*gi) {
                    collect_member_indices(sub, lg, out);
                }
            }
        }
    }
}

/// 抽取组 gi 的子 LayoutGraph（成员节点 + 嵌套组 + 组内边），索引重映射。
///
/// 返回 `(子图, member_list)`：`member_list[k]` = 子图第 k 个节点对应的 lg 节点下标，
/// 供平移回贴时把子图坐标映射回原图节点。
fn extract_subgraph(lg: &LayoutGraph, gi: usize) -> (LayoutGraph, Vec<usize>) {
    let mut sub = LayoutGraph::default();
    let mut member_indices: HashSet<usize> = HashSet::new();
    if let Some(group) = lg.groups.get(gi) {
        collect_member_indices(group, lg, &mut member_indices);
    }
    // 成员节点（源码序）
    let mut member_list: Vec<usize> = member_indices.iter().copied().collect();
    member_list.sort_unstable();
    let mut sub_idx_of: HashMap<usize, usize> = HashMap::new();
    for &ni in &member_list {
        sub_idx_of.insert(ni, sub.nodes.len());
        sub.nodes.push(lg.nodes[ni].clone());
    }
    // 组内边：两端都在 member 内
    for e in &lg.edges {
        if member_indices.contains(&e.source) && member_indices.contains(&e.target) {
            let s = sub_idx_of[&e.source];
            let t = sub_idx_of[&e.target];
            sub.edges.push(LEdge {
                source: s,
                target: t,
                source_port: e.source_port,
                target_port: e.target_port,
                line_kind: e.line_kind,
            });
        }
    }
    // 嵌套组：组内若有 GroupChild::Group，把其作为子图的 groups（递归处理）
    if let Some(group) = lg.groups.get(gi) {
        for child in &group.children {
            if let GroupChild::Group(sub_gi) = child {
                // 收集嵌套组及其成员（组内）
                let mut nested_members: HashSet<usize> = HashSet::new();
                if let Some(nested) = lg.groups.get(*sub_gi) {
                    collect_member_indices(nested, lg, &mut nested_members);
                }
                // 嵌套组成员若不在本层 member 就不应出现（理论上都在）
                let children: Vec<GroupChild> = nested_members
                    .iter()
                    .filter_map(|ni| sub_idx_of.get(ni).copied())
                    .map(GroupChild::Node)
                    .collect();
                if !children.is_empty() {
                    sub.groups.push(LGroup {
                        title: lg.groups[*sub_gi].title.clone(),
                        children,
                    });
                }
            }
        }
    }
    (sub, member_list)
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

/// 从 `final_positions`（含 None）取出确定位置用于避障检测。
fn positions_now(final_positions: &[Option<Point>]) -> Vec<Point> {
    final_positions
        .iter()
        .map(|p| p.unwrap_or(Point::new(0.0, 0.0)))
        .collect()
}

/// 跨组边路由：端点已裁剪到边框；若穿过中间节点或子图容器，则加绕行点。
fn route_cross_group(
    start: Point,
    end: Point,
    src_idx: usize,
    tgt_idx: usize,
    positions: &[Point],
    lg: &LayoutGraph,
    group_bounds: &[Rect],
) -> Vec<Point> {
    // 收集所有需要避障的矩形：中间节点 + 子图容器
    let mut obstacles: Vec<Rect> = Vec::new();
    for (i, n) in lg.nodes.iter().enumerate() {
        if i == src_idx || i == tgt_idx {
            continue;
        }
        let c = positions[i];
        obstacles.push(Rect::new(
            c.x - n.size.width / 2.0,
            c.y - n.size.height / 2.0,
            c.x + n.size.width / 2.0,
            c.y + n.size.height / 2.0,
        ));
    }
    for b in group_bounds {
        obstacles.push(*b);
    }

    // 检测线段 start→end 是否穿过任一障碍
    for rect in &obstacles {
        if segment_intersects_rect(start, end, *rect) {
            // 绕行：在障碍外侧加一个点（取穿过方向的反侧）
            let mid = Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
            // 绕行方向：优先取水平外扩（避开上下障碍），dx 同号方向外
            let dx = if mid.x >= rect.center().x { 1.0 } else { -1.0 };
            let detour = Point::new(mid.x + dx * (rect.width() / 2.0 + 30.0), mid.y);
            return vec![start, detour, end];
        }
    }
    vec![start, end]
}

/// 线段与矩形是否相交（粗略）。
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

/// 计算子图含节点实际宽高的 bbox：`(min_x, min_y, w, h)`。
///
/// `placed.positions[k]` 是第 k 个成员节点中心，其尺寸取 `lg.nodes[member_list[k]].size`。
fn bbox_with_node_sizes(
    placed: &PlacedGraph,
    lg: &LayoutGraph,
    member_list: &[usize],
) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (k, center) in placed.positions.iter().enumerate() {
        let size = member_list
            .get(k)
            .and_then(|&lg_idx| lg.nodes.get(lg_idx))
            .map(|n| n.size)
            .unwrap_or(Size::new(80.0, 40.0));
        min_x = min_x.min(center.x - size.width / 2.0);
        min_y = min_y.min(center.y - size.height / 2.0);
        max_x = max_x.max(center.x + size.width / 2.0);
        max_y = max_y.max(center.y + size.height / 2.0);
    }
    if !min_x.is_finite() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

/// 返回子图节点中心的 bbox：`(min_x, min_y, w, h)`。
fn bbox_size(placed: &PlacedGraph) -> (f64, f64, f64, f64) {
    if placed.positions.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in &placed.positions {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

fn compute_total_size(positions: &[Point], routes: &[Vec<Point>], bounds: &[Rect]) -> Size {
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
    for b in bounds {
        min_x = min_x.min(b.min_x());
        min_y = min_y.min(b.min_y());
        max_x = max_x.max(b.max_x());
        max_y = max_y.max(b.max_y());
    }
    if !min_x.is_finite() {
        return Size::new(0.0, 0.0);
    }
    Size::new(max_x - min_x, max_y - min_y)
}
