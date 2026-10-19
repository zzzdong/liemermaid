//! LayoutEngine —— 布局求解入口（P1.1 Sugiyama 分层 + 交叉减少）。
//!
//! 编排：`extract` 产出的 UG → `directed::sugiyama_layers`（分层 + 降交叉 + 方向反转）
//! → 按 `direction` 做主轴坐标分配 → 端口解析 → 直线边路由（P1.2 升级为正交路由）。
//!
//! 产出 `(Geograph, StyleIntent)`：几何与视觉意图在 layout 结束时分离，
//! 使 UG 可在此后 drop，materialize / paint 不持有 UG 引用。

use std::collections::{HashMap, HashSet};

use lievisual::geometry::Point;

use crate::ast::Direction;
use crate::builder::ir::{
    self, StyleIntent,
    common::*,
    geograph::{GGContainer, GGEdge, GGNode, Geograph},
    shape::ShapeKind,
};
use crate::builder::layout::directed::sugiyama_layers;
use crate::builder::layout::route::route_edges;
use crate::builder::theme;

/// 层间（主轴）间距，对齐官方 mermaid 的 `rankSpacing: 50`
/// （golden 实测：`flowchart__chain` 节点中心 y = 35 / 139 / 243，间隔 104 = 节点高 54 + 50）。
const LAYER_GAP: f64 = 50.0;
/// 同层节点间距，对齐官方 mermaid 的 `nodeSpacing: 50`。
const NODE_GAP: f64 = 72.0;
/// 当相邻两层之间存在带标签的边时，在层间距上额外预留的空间
/// （标签行高 + 白底上下留白），避免标签与节点重叠。
const EDGE_LABEL_GAP: f64 = 28.0;

/// Grid 家族（class / er）的 BFS 分层：入度为 0 的根在第 0 层，逐层推进。
///
/// 层内顺序保持源码序（不做交叉减少，class 图按声明顺序排布）。
/// 全成环（无入度 0 节点）时退化为单层。
fn grid_layers(ug: &ir::Unigraph) -> Vec<Vec<NodeId>> {
    let ids: Vec<NodeId> = ug.nodes.iter().map(|n| n.id.clone()).collect();
    let n = ids.len();
    if n == 0 {
        return Vec::new();
    }

    // 入度（忽略自环）。
    let mut in_deg: HashMap<NodeId, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
    for e in &ug.edges {
        if e.source != e.target {
            *in_deg.entry(e.target.clone()).or_insert(0) += 1;
        }
    }
    // 邻接表。
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for id in &ids {
        adj.entry(id.clone()).or_default();
    }
    for e in &ug.edges {
        if e.source != e.target {
            adj.entry(e.source.clone())
                .or_default()
                .push(e.target.clone());
        }
    }

    let mut frontier: Vec<NodeId> = ids
        .iter()
        .filter(|id| in_deg.get(*id).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();
    // 全成环：全部第 0 层。
    if frontier.is_empty() {
        return vec![ids.clone()];
    }

    let mut layer_of: HashMap<NodeId, usize> = HashMap::new();
    let mut depth = 0usize;
    while !frontier.is_empty() {
        let mut next: Vec<NodeId> = Vec::new();
        for id in frontier {
            if layer_of.contains_key(&id) {
                continue;
            }
            layer_of.insert(id.clone(), depth);
            if let Some(outs) = adj.get(&id) {
                for t in outs {
                    if !layer_of.contains_key(t) {
                        next.push(t.clone());
                    }
                }
            }
        }
        // 兜底：本轮未覆盖的节点（如环内节点）补入，避免死循环。
        if next.is_empty() && layer_of.len() < n {
            for id in &ids {
                if !layer_of.contains_key(id) {
                    next.push(id.clone());
                    break;
                }
            }
        }
        depth += 1;
        frontier = next;
    }

    // 按层收集，层内保持 ids 源码序。
    let max_layer = layer_of.values().cloned().max().unwrap_or(0);
    let mut layers: Vec<Vec<NodeId>> = vec![Vec::new(); max_layer + 1];
    for id in &ids {
        let l = *layer_of.get(id).unwrap_or(&0);
        layers[l].push(id.clone());
    }
    layers
}

/// Directed / Grid 家族的坐标分配：分层 → 主轴推进 → 同层轴展开。
fn layered_centers(ug: &ir::Unigraph, sizes: &HashMap<NodeId, Size>) -> HashMap<NodeId, Point> {
    // —— 分层（Directed: Sugiyama 分层 + 交叉减少；Grid: BFS 网格分层）——
    let layers = match ug.family {
        ir::unigraph::GraphFamily::Grid => grid_layers(ug),
        _ => sugiyama_layers(ug),
    };
    let n_layers = layers.len();

    // 主轴 = 层推进方向；同层轴 = 同层节点展开方向。
    let horizontal_main = matches!(ug.direction, Direction::LR | Direction::RL);

    // 每层「主尺寸」（层内节点在「同层轴」上的累计长度）与「跨尺寸」（层在主轴方向的厚度）。
    // 层厚（cross）用「主轴方向尺寸」：TB=高度、LR=宽度；同层长（main）用「同层轴方向尺寸」。
    let mut layer_main_len = vec![0.0f64; n_layers];
    let mut layer_cross_thick = vec![0.0f64; n_layers];
    for (li, layer) in layers.iter().enumerate() {
        let mut main_len = 0.0f64;
        let mut max_cross = 0.0f64;
        for id in layer {
            let size = sizes
                .get(id)
                .cloned()
                .unwrap_or(Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H));
            let (cross, main) = if horizontal_main {
                (size.width, size.height)
            } else {
                (size.height, size.width)
            };
            main_len += main + NODE_GAP;
            max_cross = max_cross.max(cross);
        }
        main_len = main_len.max(NODE_GAP) - NODE_GAP;
        layer_main_len[li] = main_len;
        layer_cross_thick[li] = max_cross;
    }

    // —— 层间标签预留：相邻两层之间有带标签的边时，额外拉开该层间距，给标签留空间 ——
    let layer_of: HashMap<&NodeId, usize> = layers
        .iter()
        .enumerate()
        .flat_map(|(li, layer)| layer.iter().map(move |id| (id, li)))
        .collect();
    // 节点 → 最内层容器（复合状态 / 子图）。用于层间间距的容器边界补偿：
    // 官方 golden 实测，跨容器边界的层间距 = 基础间距 + 容器留白
    // （`flowchart__subgraph`：Start→Process 75 = 50+25，Decision→Output
    // 100 = 50+25+25；`state__composite`：起点→框内圆点 87.5 ≈ 50+35），
    // 同容器内部 flowchart 为 50、state 复合状态为 75（50+25）。
    let container_of: HashMap<&NodeId, &ir::unigraph::UGSubgraph> = {
        let mut map: HashMap<&NodeId, &ir::unigraph::UGSubgraph> = HashMap::new();
        for sg in &ug.subgraphs {
            for m in &sg.member_ids {
                // 嵌套容器取成员集更小者（最内层）。
                let better = map
                    .get(m)
                    .map(|old: &&ir::unigraph::UGSubgraph| {
                        sg.member_ids.len() < old.member_ids.len()
                    })
                    .unwrap_or(true);
                if better {
                    map.insert(m, sg);
                }
            }
        }
        map
    };
    // 容器纵向留白 / 同容器内部额外间距（与 compute_containers 的 pad_y 保持一致）。
    fn container_pad_y(kind: ir::common::ContainerKind) -> f64 {
        match kind {
            ir::common::ContainerKind::StateComposite => 35.0,
            _ => 25.0,
        }
    }
    fn container_inner_extra(kind: ir::common::ContainerKind) -> f64 {
        match kind {
            ir::common::ContainerKind::StateComposite => 25.0,
            _ => 0.0,
        }
    }
    let boundary_extra =
        |a: Option<&ir::unigraph::UGSubgraph>, b: Option<&ir::unigraph::UGSubgraph>| -> f64 {
            match (a, b) {
                (None, None) => 0.0,
                (Some(x), None) | (None, Some(x)) => container_pad_y(x.kind),
                (Some(x), Some(y)) if x.id == y.id => container_inner_extra(x.kind),
                (Some(x), Some(y)) => container_pad_y(x.kind) + container_pad_y(y.kind),
            }
        };
    let mut gap_extra = vec![0.0f64; n_layers.saturating_sub(1)];
    for e in &ug.edges {
        let (Some(&si), Some(&ti)) = (layer_of.get(&e.source), layer_of.get(&e.target)) else {
            continue;
        };
        if si.abs_diff(ti) != 1 {
            continue;
        }
        let lo = si.min(ti);
        // 只看相邻层的边（跨层边由路由处理，标签取中点，仍受最近层间距影响）。
        let has_label = e.label_text.as_deref().is_some_and(|s| !s.is_empty());
        if has_label {
            gap_extra[lo] = gap_extra[lo].max(EDGE_LABEL_GAP);
        }
        let ca = container_of.get(&e.source).copied();
        let cb = container_of.get(&e.target).copied();
        gap_extra[lo] = gap_extra[lo].max(boundary_extra(ca, cb));
    }

    // 主轴起点（居中）：总主轴长度
    let total_gap: f64 =
        gap_extra.iter().sum::<f64>() + LAYER_GAP * (n_layers as f64 - 1.0).max(0.0);
    let total_main: f64 = layer_main_len.iter().sum::<f64>() + total_gap;
    let mut main_cursor = -total_main / 2.0;

    // 各层主轴坐标（层中心在主轴上的位置）
    let mut layer_main_coord = vec![0.0f64; n_layers];
    for li in 0..n_layers {
        let cross_thick = layer_cross_thick[li];
        layer_main_coord[li] = main_cursor + cross_thick / 2.0;
        let extra = if li + 1 < n_layers {
            gap_extra[li]
        } else {
            0.0
        };
        main_cursor += cross_thick + LAYER_GAP + extra;
    }

    // 同层轴坐标分配（居中展开）
    let mut centers: HashMap<NodeId, Point> = HashMap::new();
    for (li, layer) in layers.iter().enumerate() {
        let main_len = layer_main_len[li];
        let mut cross_cursor = -main_len / 2.0;
        for id in layer {
            let size = sizes
                .get(id)
                .cloned()
                .unwrap_or(Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H));
            // 同层轴方向尺寸：LR=高度（垂直展开）、TB=宽度（水平展开）。
            let main = if horizontal_main {
                size.height
            } else {
                size.width
            };
            let half_main = main / 2.0;
            let main_coord = layer_main_coord[li];
            let cross_coord = cross_cursor + half_main;
            let (x, y) = if horizontal_main {
                (main_coord, cross_coord)
            } else {
                (cross_coord, main_coord)
            };
            centers.insert(id.clone(), Point::new(x, y));
            cross_cursor += main + NODE_GAP;
        }
    }
    centers
}

// ==================== 子图方向（`subgraph X / direction LR`）====================

/// 局部重排的布局单元：一个直接节点，或一个子图块（内部布局保持不变，整体平移）。
struct SubgraphUnit {
    key: NodeId,
    size: Size,
    center: Point,
    /// 该单元包含的节点（块为其全部后代节点）。
    nodes: Vec<NodeId>,
    is_block: bool,
}

/// `a` 的成员集合是否包含于 `b`（允许相等）。
fn member_subset(a: &ir::unigraph::UGSubgraph, b: &ir::unigraph::UGSubgraph) -> bool {
    let set: HashSet<&str> = b.member_ids.iter().map(String::as_str).collect();
    a.member_ids.iter().all(|n| set.contains(n.as_str()))
}

/// 直接子容器：成员集合最大的那些真子集（排除被其他子容器包含的孙容器）。
fn direct_child_containers(ug: &ir::Unigraph, idx: usize) -> Vec<usize> {
    let mut cands: Vec<usize> = (0..ug.subgraphs.len())
        .filter(|&j| {
            j != idx
                && !ug.subgraphs[j].member_ids.is_empty()
                && member_subset(&ug.subgraphs[j], &ug.subgraphs[idx])
        })
        .collect();
    // 成员多的优先；只保留“不被已保留容器包含”的极大集合。
    cands.sort_by_key(|&j| std::cmp::Reverse(ug.subgraphs[j].member_ids.len()));
    let mut kept: Vec<usize> = Vec::new();
    for j in cands {
        if !kept
            .iter()
            .any(|&k| member_subset(&ug.subgraphs[j], &ug.subgraphs[k]))
        {
            kept.push(j);
        }
    }
    kept
}

/// 单元集合的包围盒 `(min, max)`。
fn units_bbox(units: &[SubgraphUnit]) -> Option<(Point, Point)> {
    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for u in units {
        min.x = min.x.min(u.center.x - u.size.width / 2.0);
        min.y = min.y.min(u.center.y - u.size.height / 2.0);
        max.x = max.x.max(u.center.x + u.size.width / 2.0);
        max.y = max.y.max(u.center.y + u.size.height / 2.0);
    }
    (min.x.is_finite() && max.x.is_finite()).then_some((min, max))
}

/// 合成节点（仅用于对单元图复用 `layered_centers`，尺寸由 `unit_sizes` 提供）。
fn synthetic_ug_node(id: &str) -> ir::unigraph::UGNode {
    ir::unigraph::UGNode {
        id: id.to_string(),
        kind: ir::common::NodeKind::Atom,
        role: ir::common::NodeRole::Atom,
        shape: ShapeKind::Rectangle,
        label: ir::common::LabelOrMeasured::Spec(ir::common::LabelSpec {
            text: String::new(),
            spans: Vec::new(),
        }),
        ports: ir::common::PortSet::default(),
        size_hint: ir::common::SizeHint::ByText,
        style_ref: ir::common::StyleRef::NodeDefault,
        constraint: ir::common::NodeConstraint::Free,
        detail: ir::common::NodeDetail::None,
    }
}

/// 合成边（仅参与分层，不参与绘制）。
fn synthetic_ug_edge(id: &str, source: &str, target: &str) -> ir::unigraph::UGEdge {
    ir::unigraph::UGEdge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        source_port: ir::common::PortHint::Auto,
        target_port: ir::common::PortHint::Auto,
        kind: ir::unigraph::EdgeKind::Flow,
        label_text: None,
        label: None,
        priority: ir::common::EdgePriority::Primary,
        routing_hint: ir::common::RoutingHint::Spline,
        arrow: ir::common::ArrowSpec::default(),
        line_kind: ir::common::LineKind::Solid,
        repulsion: 1.0,
        cardinality: (None, None),
        cardinality_text: (None, None),
    }
}

/// 对显式声明 `direction` 的子图做局部重排。
///
/// 全局分层不知道容器边界，子图成员会被摊到各层；这里在全局坐标之上，
/// 按“直接节点 + 直接子容器块”构造单元图，用请求方向重新分层并**整体平移**：
/// - 内层容器先处理（按成员数升序），外层再把内层当作一个块移动，嵌套方向都保留；
/// - 重排后的单元整体平移回原子图的包围盒左上角，避免整图漂移。
fn apply_subgraph_directions(
    ug: &ir::Unigraph,
    sizes: &HashMap<NodeId, Size>,
    centers: &mut HashMap<NodeId, Point>,
) {
    if ug.subgraphs.iter().all(|sg| sg.direction.is_none()) {
        return;
    }
    let mut order: Vec<usize> = (0..ug.subgraphs.len()).collect();
    order.sort_by_key(|&i| ug.subgraphs[i].member_ids.len());
    for idx in order {
        if let Some(dir) = ug.subgraphs[idx].direction {
            relayout_subgraph(ug, idx, dir, sizes, centers);
        }
    }
}

fn relayout_subgraph(
    ug: &ir::Unigraph,
    idx: usize,
    direction: Direction,
    sizes: &HashMap<NodeId, Size>,
    centers: &mut HashMap<NodeId, Point>,
) {
    let sg = &ug.subgraphs[idx];
    let children = direct_child_containers(ug, idx);

    // —— 构造单元：直接子容器（块）+ 直接节点 ——
    let mut units: Vec<SubgraphUnit> = Vec::new();
    let mut node_unit: HashMap<NodeId, usize> = HashMap::new();
    for &child in &children {
        let c = &ug.subgraphs[child];
        let mut min = Point::new(f64::INFINITY, f64::INFINITY);
        let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut nodes = Vec::new();
        for id in &c.member_ids {
            let (Some(size), Some(center)) = (sizes.get(id), centers.get(id)) else {
                continue;
            };
            min.x = min.x.min(center.x - size.width / 2.0);
            min.y = min.y.min(center.y - size.height / 2.0);
            max.x = max.x.max(center.x + size.width / 2.0);
            max.y = max.y.max(center.y + size.height / 2.0);
            nodes.push(id.clone());
        }
        if nodes.is_empty() {
            continue;
        }
        let ui = units.len();
        // 前缀使用标识符不可能包含的字符，避免与真实节点 id 冲突。
        units.push(SubgraphUnit {
            key: format!("\u{1}sub{}", child),
            size: Size::new(max.x - min.x, max.y - min.y),
            center: Point::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0),
            nodes: nodes.clone(),
            is_block: true,
        });
        for id in nodes {
            node_unit.insert(id, ui);
        }
    }
    for id in &sg.member_ids {
        if node_unit.contains_key(id) {
            continue;
        }
        let (Some(size), Some(center)) = (sizes.get(id), centers.get(id)) else {
            continue;
        };
        let ui = units.len();
        units.push(SubgraphUnit {
            key: id.clone(),
            size: *size,
            center: *center,
            nodes: vec![id.clone()],
            is_block: false,
        });
        node_unit.insert(id.clone(), ui);
    }
    if units.len() < 2 {
        return;
    }

    // —— 单元图：复用分层 + 坐标分配 ——
    let mut mini = ir::Unigraph {
        family: ir::unigraph::GraphFamily::Directed,
        direction,
        ..Default::default()
    };
    mini.nodes = units.iter().map(|u| synthetic_ug_node(&u.key)).collect();
    for e in &ug.edges {
        let (Some(&su), Some(&tu)) = (node_unit.get(&e.source), node_unit.get(&e.target)) else {
            continue;
        };
        if su == tu {
            continue;
        }
        let id = format!("ue{}", mini.edges.len());
        mini.edges
            .push(synthetic_ug_edge(&id, &units[su].key, &units[tu].key));
    }
    // 无内部连接时用链式边强制沿请求方向排开（否则全部落在第 0 层，
    // LR 会退化成竖向堆叠）。
    if mini.edges.is_empty() {
        for w in units.windows(2) {
            let id = format!("ue{}", mini.edges.len());
            mini.edges
                .push(synthetic_ug_edge(&id, &w[0].key, &w[1].key));
        }
    }
    let unit_sizes: HashMap<NodeId, Size> = units.iter().map(|u| (u.key.clone(), u.size)).collect();
    let new_centers = layered_centers(&mini, &unit_sizes);
    if new_centers.len() != units.len() {
        return;
    }

    // —— 整体平移回原包围盒左上角，避免子图漂移 ——
    let (Some((old_min, _)), Some((new_min, _))) = (units_bbox(&units), {
        let moved: Vec<SubgraphUnit> = units
            .iter()
            .map(|u| SubgraphUnit {
                key: u.key.clone(),
                size: u.size,
                center: new_centers[&u.key],
                nodes: Vec::new(),
                is_block: false,
            })
            .collect();
        units_bbox(&moved)
    }) else {
        return;
    };
    let offset = Point::new(old_min.x - new_min.x, old_min.y - new_min.y);

    for u in &units {
        let base = new_centers[&u.key];
        let target = Point::new(base.x + offset.x, base.y + offset.y);
        if u.is_block {
            let delta = Point::new(target.x - u.center.x, target.y - u.center.y);
            for id in &u.nodes {
                if let Some(c) = centers.get_mut(id) {
                    c.x += delta.x;
                    c.y += delta.y;
                }
            }
        } else if let Some(id) = u.nodes.first() {
            centers.insert(id.clone(), target);
        }
    }
}

/// Linear 家族（timeline 等）的坐标分配：所有节点沿主轴单行排布并居中。
/// LR（默认）：水平一行，y=0（时间轴）；TD：垂直一列，x=0。
///
/// 列距取官方 timeline 实测 200（块宽 190 + 列间隙 10），与文本宽度解耦——
/// 官方每列块宽固定，列心等距分布。
fn linear_centers(ug: &ir::Unigraph, sizes: &HashMap<NodeId, Size>) -> HashMap<NodeId, Point> {
    let horizontal = matches!(ug.direction, Direction::LR | Direction::RL);
    let n = ug.nodes.len().max(1);
    let pitch = theme::timeline::COL_PITCH;
    let total = pitch * (n as f64 - 1.0);
    let mut cursor = -total / 2.0;
    let mut centers: HashMap<NodeId, Point> = HashMap::new();
    for node in &ug.nodes {
        let coord = cursor;
        let (x, y) = if horizontal {
            (coord, 0.0)
        } else {
            (0.0, coord)
        };
        centers.insert(node.id.clone(), Point::new(x, y));
        cursor += pitch;
    }
    let _ = sizes;
    centers
}

// ==================== Sequence（泳道 + 消息时序）====================
// 布局常量对齐官方 mermaid golden（`sequence__activation`）：
// 参与者盒 150×65（`minActorWidth` / `actorMargin=50` 使列中心相距 200），
// 首条消息距盒底 46，消息行距 46（golden 行 y = 111 / 157 / 201 / 247）。
const SEQ_BOX_H: f64 = 65.0;
const SEQ_BOX_MIN_W: f64 = 150.0;
const SEQ_PAD_X: f64 = 16.0;
const SEQ_COL_GAP: f64 = 50.0; // actorMargin
const SEQ_MSG_SPACING: f64 = 46.0;
const SEQ_NOTE_H: f64 = 36.0;
const SEQ_NOTE_GAP: f64 = 14.0;
const SEQ_ROW_GAP: f64 = 46.0; // 首行距参与者盒底
const SEQ_BLOCK_LABEL_H: f64 = 24.0;
const SEQ_BLOCK_PAD: f64 = 10.0;
const SEQ_BLOCK_INDENT: f64 = 24.0;
/// 分支分隔行高度（分隔虚线 + 下方居中的分支标题）。
const SEQ_DIVIDER_H: f64 = 40.0;

/// 备注框横向几何（x 起点, 宽），据放置位置与目标列计算。
fn seq_note_box_x(
    note: &ir::UGNode,
    col_centers: &[f64],
    col_widths: &[f64],
    col_of: &HashMap<&str, usize>,
    sizes: &HashMap<NodeId, Size>,
) -> (f64, f64) {
    use ir::common::SequenceNotePlacement as NP;
    let ir::common::NodeDetail::SequenceNote {
        targets, placement, ..
    } = &note.detail
    else {
        return (0.0, 160.0);
    };
    let text_w = sizes.get(&note.id).map(|s| s.width).unwrap_or(120.0);
    match placement {
        NP::Over => {
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            for t in targets {
                if let Some(&ci) = col_of.get(t.as_str()) {
                    min_x = min_x.min(col_centers[ci] - col_widths[ci] / 2.0);
                    max_x = max_x.max(col_centers[ci] + col_widths[ci] / 2.0);
                }
            }
            if targets.is_empty() {
                return (0.0, 160.0);
            }
            let span = max_x - min_x;
            let w = (span + 30.0).max(160.0);
            (min_x - (w - span) / 2.0, w)
        }
        NP::LeftOf => {
            let min_x = targets
                .iter()
                .filter_map(|t| col_of.get(t.as_str()))
                .map(|&ci| col_centers[ci] - col_widths[ci] / 2.0)
                .fold(f64::INFINITY, f64::min);
            let w = (text_w + 10.0).clamp(100.0, 200.0);
            (min_x - w - 10.0, w)
        }
        NP::RightOf => {
            let max_x = targets
                .iter()
                .filter_map(|t| col_of.get(t.as_str()))
                .map(|&ci| col_centers[ci] + col_widths[ci] / 2.0)
                .fold(f64::NEG_INFINITY, f64::max);
            let w = (text_w + 10.0).clamp(100.0, 200.0);
            (max_x + 10.0, w)
        }
    }
}

/// Sequence 家族的专属布局：参与者列（生命线 x） + 语句行（消息/备注 y） +
/// 消息水平路由 + 分组块容器几何。
/// Sequence 家族布局的产出：节点 / 边 / 分组容器 / 激活条 / 分支分隔线。
type SequenceGeometry = (
    Vec<GGNode>,
    Vec<GGEdge>,
    Vec<GGContainer>,
    Vec<ir::geograph::GGActivation>,
    Vec<ir::geograph::GGSequenceDivider>,
);

fn sequence_geometry(
    ug: &ir::Unigraph,
    sizes: &HashMap<NodeId, Size>,
    labels: &HashMap<NodeId, ir::common::MeasuredLabel>,
) -> SequenceGeometry {
    use ir::unigraph::SequenceRow;

    // —— 参与者列 ——
    let participants: Vec<&ir::UGNode> = ug
        .nodes
        .iter()
        .filter(|n| n.role == NodeRole::Lifeline)
        .collect();
    let mut col_widths: Vec<f64> = Vec::with_capacity(participants.len());
    for p in &participants {
        let tw = labels
            .get(&p.id)
            .map(|l| l.size.width)
            .unwrap_or(SEQ_BOX_MIN_W);
        col_widths.push(SEQ_BOX_MIN_W.max(tw + SEQ_PAD_X * 2.0));
    }
    let mut col_centers: Vec<f64> = Vec::with_capacity(participants.len());
    let mut cur_x = 0.0;
    for w in &col_widths {
        col_centers.push(cur_x + w / 2.0);
        cur_x += w + SEQ_COL_GAP;
    }
    let col_of: HashMap<&str, usize> = participants
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), i))
        .collect();
    let box_top = 0.0;
    let box_bottom = box_top + SEQ_BOX_H;

    // —— 语句行：按 sequence_rows 推进 y ——
    let rows = ug.sequence_rows.clone().unwrap_or_default();
    let mut row_y: HashMap<&str, f64> = HashMap::new(); // 消息边 id / 备注节点 id → 行 y
    let mut cur_y = box_bottom + SEQ_ROW_GAP;
    // 分组块几何（id, label, depth, y_top, y_bot）。
    let mut block_geo: Vec<(String, String, usize, f64, f64)> = Vec::new();
    // 分支分隔线（块 id, 分支标题, 虚线 y）。
    let mut dividers: Vec<(String, String, f64)> = Vec::new();
    // 激活跨度（actor, y0, y1），由下方语句行循环产出（块内必然赋值）。
    let act_spans_out: Vec<(String, f64, f64)>;
    {
        struct Frame {
            id: String,
            label: String,
            depth: usize,
            top: f64,
            /// 框内最后一条内容行（消息 / 备注）的 y；嵌套块闭合时外层据此取框底。
            last_content: Option<f64>,
        }
        let mut stack: Vec<Frame> = Vec::new();
        // 激活条：actor → 已开始但未结束的起点 y 栈（支持嵌套激活）。
        let mut open_act: HashMap<&str, Vec<f64>> = HashMap::new();
        // 已闭合的激活跨度 (actor, y0, y1)。
        let mut act_spans: Vec<(String, f64, f64)> = Vec::new();
        // 上一行的内容 y（`Activation` 行不占行高，起点取所属消息行的 y）。
        // 初值取参与者盒底：官方 golden 中分组框顶边距盒底仅 10px
        // （loop 框顶 75 = 盒底 65 + 10），若初值取 `box_bottom + SEQ_ROW_GAP`
        // 则首行是 BlockStart 时框顶会整体下移一个行距。
        let mut prev_y = box_bottom;
        for row in &rows {
            match row {
                SequenceRow::Message(eid) => {
                    row_y.insert(eid.as_str(), cur_y);
                    prev_y = cur_y;
                    for f in &mut stack {
                        f.last_content = Some(cur_y);
                    }
                    cur_y += SEQ_MSG_SPACING;
                }
                SequenceRow::Note(nid) => {
                    row_y.insert(nid.as_str(), cur_y);
                    prev_y = cur_y;
                    for f in &mut stack {
                        f.last_content = Some(cur_y);
                    }
                    cur_y += SEQ_NOTE_H + SEQ_NOTE_GAP;
                }
                SequenceRow::Activation { actor, on } => {
                    if *on {
                        open_act.entry(actor.as_str()).or_default().push(prev_y);
                    } else if let Some(starts) = open_act.get_mut(actor.as_str())
                        && let Some(y0) = starts.pop()
                    {
                        act_spans.push((actor.clone(), y0, prev_y));
                    }
                }
                SequenceRow::BlockStart(id, label) => {
                    // 官方 golden：分组框顶边距上一内容（参与者盒底 / 上一条消息）仅 10px
                    // （loop 框顶 75 = 盒底 65 + 10），而非一个完整行距。
                    let top = prev_y + SEQ_BLOCK_PAD;
                    stack.push(Frame {
                        id: id.clone(),
                        label: label.clone(),
                        depth: stack.len(),
                        top,
                        last_content: None,
                    });
                    prev_y = top;
                    // 框内首条消息距框顶 = 标签高 + 一个行距（官方实测为 81，此处
                    // 取结构性等价的 24 + 46 = 70）。
                    cur_y = top + SEQ_BLOCK_LABEL_H + SEQ_MSG_SPACING;
                }
                SequenceRow::BlockDivider(bid, label) => {
                    // 分隔虚线画在行顶，分支标题在其下方居中（materialize 绘制）。
                    dividers.push((bid.clone(), label.clone(), cur_y));
                    prev_y = cur_y;
                    cur_y += SEQ_DIVIDER_H;
                }
                SequenceRow::BlockEnd(_) => {
                    if let Some(f) = stack.pop() {
                        // 官方 golden：框底距框内最后一条消息 10px（loop 框底 210 =
                        // done 200 + 10），块后第一条消息距框底一个行距（finalize
                        // 254 = 210 + 44）——即「框底 ≠ 下一行 y」，旧实现让下一行
                        // 正好落在框底边上，视觉上被分组框罩住。
                        let bottom = f.last_content.unwrap_or(f.top) + SEQ_BLOCK_PAD;
                        block_geo.push((f.id, f.label, f.depth, f.top, bottom));
                        cur_y = bottom + SEQ_MSG_SPACING;
                    }
                    prev_y = cur_y;
                }
            }
        }
        // 未显式取消的激活延伸到内容底部。
        for (actor, starts) in open_act {
            for y0 in starts {
                act_spans.push((actor.to_string(), y0, cur_y));
            }
        }
        act_spans_out = act_spans;
    }
    let content_bottom = cur_y;

    // —— GGNode：参与者盒（顶部） + 备注盒 ——
    let mut gg_nodes: Vec<GGNode> = Vec::new();
    for (i, p) in participants.iter().enumerate() {
        let center = Point::new(col_centers[i], box_top + SEQ_BOX_H / 2.0);
        let size = Size::new(col_widths[i], SEQ_BOX_H);
        gg_nodes.push(GGNode {
            id: p.id.clone(),
            role: p.role,
            center,
            size,
            shape: p.shape,
            ports: resolve_ports(center, size),
            label: labels.get(&p.id).cloned(),
            detail: p.detail.clone(),
        });
    }
    for n in &ug.nodes {
        if !matches!(n.detail, ir::common::NodeDetail::SequenceNote { .. }) {
            continue;
        }
        let (nx, nw) = seq_note_box_x(n, &col_centers, &col_widths, &col_of, sizes);
        let ny = row_y
            .get(n.id.as_str())
            .copied()
            .unwrap_or(box_bottom + SEQ_ROW_GAP);
        let center = Point::new(nx + nw / 2.0, ny + SEQ_NOTE_H / 2.0);
        let size = Size::new(nw, SEQ_NOTE_H);
        gg_nodes.push(GGNode {
            id: n.id.clone(),
            role: n.role,
            center,
            size,
            shape: ShapeKind::Rounded,
            ports: resolve_ports(center, size),
            label: None,
            detail: n.detail.clone(),
        });
    }

    // —— GGEdge：消息水平路由（自环右侧小环） ——
    let mut gg_edges: Vec<GGEdge> = Vec::new();
    for e in &ug.edges {
        let y = row_y
            .get(e.id.as_str())
            .copied()
            .unwrap_or(box_bottom + SEQ_ROW_GAP);
        let (Some(&sci), Some(&tci)) =
            (col_of.get(e.source.as_str()), col_of.get(e.target.as_str()))
        else {
            continue;
        };
        let (x0, x1) = (col_centers[sci], col_centers[tci]);
        let route = if x0 == x1 {
            let r = 24.0;
            ir::geograph::line_route(&[
                Point::new(x0, y),
                Point::new(x0 + r, y),
                Point::new(x0 + r, y - r * 0.7),
                Point::new(x0 + r * 0.5, y - r * 0.7),
                Point::new(x0 + r * 0.5, y),
            ])
        } else {
            ir::geograph::line_route(&[Point::new(x0, y), Point::new(x1, y)])
        };
        gg_edges.push(GGEdge {
            id: e.id.clone(),
            source: e.source.clone(),
            target: e.target.clone(),
            route,
            label_text: e.label_text.clone(),
            label_anchor: if e.label_text.as_deref().is_some_and(|s| !s.is_empty()) {
                Some(Point::new((x0 + x1) / 2.0, y - 6.0))
            } else {
                None
            },
            kind: e.kind,
            arrow: e.arrow,
            routing_hint: e.routing_hint,
            line_kind: e.line_kind,
            cardinality: e.cardinality,
            cardinality_text: e.cardinality_text.clone(),
        });
    }

    // —— 分组块容器：横跨全部列（按深度缩进），纵向取块成员行范围 ——
    let mut gg_containers: Vec<GGContainer> = Vec::new();
    let min_x_all = col_centers
        .first()
        .map(|c| c - col_widths[0] / 2.0)
        .unwrap_or(0.0);
    let max_x_all = col_centers
        .last()
        .map(|c| c + col_widths[col_widths.len() - 1] / 2.0)
        .unwrap_or(0.0);
    for (bid, label, depth, y_top, y_bot) in &block_geo {
        let indent = *depth as f64 * SEQ_BLOCK_INDENT;
        gg_containers.push(GGContainer {
            id: bid.clone(),
            kind: ir::common::ContainerKind::SequenceBlock,
            title: Some(label.clone()),
            bounds: lievisual::geometry::Rect::new(
                min_x_all - SEQ_BLOCK_PAD + indent,
                *y_top,
                max_x_all + SEQ_BLOCK_PAD + indent,
                *y_bot,
            ),
            member_ids: Vec::new(),
        });
    }

    // —— 激活条：列 x + 纵向跨度 ——
    let mut activations: Vec<ir::geograph::GGActivation> = Vec::new();
    for (actor, y0, y1) in act_spans_out {
        let Some(&ci) = col_of.get(actor.as_str()) else {
            continue;
        };
        activations.push(ir::geograph::GGActivation {
            actor,
            x: col_centers[ci],
            y0,
            y1,
        });
    }

    let _ = content_bottom; // 生命线底端由 materialize 从边几何推导
    let sequence_dividers = dividers
        .into_iter()
        .map(|(block_id, label, y)| ir::geograph::GGSequenceDivider { block_id, label, y })
        .collect();
    (
        gg_nodes,
        gg_edges,
        gg_containers,
        activations,
        sequence_dividers,
    )
}

// ==================== Hierarchy（git 分支列）====================
// 常量对齐 `theme::gitgraph`（提交点半径 / 分支行距 / 提交列距 / 边距）。
const GIT_COMMIT_RADIUS: f64 = theme::gitgraph::COMMIT_RADIUS;
const GIT_BRANCH_SPACING: f64 = theme::gitgraph::BRANCH_SPACING;
const GIT_COMMIT_SPACING: f64 = theme::gitgraph::COMMIT_SPACING;
const GIT_LEFT_MARGIN: f64 = theme::gitgraph::LEFT_MARGIN;
const GIT_TOP_MARGIN: f64 = theme::gitgraph::TOP_MARGIN;

/// Hierarchy 家族（gitgraph）的专属布局：提交按声明序沿 x 推进，分支按容器序映射到 y 行。
/// 提交点半径固定；边路由留空（materialize 从节点中心画分支线 / 合并曲线）。
fn hierarchy_geometry(ug: &ir::Unigraph) -> (Vec<GGNode>, Vec<GGEdge>, Vec<GGContainer>) {
    // 分支行映射：subgraphs 顺序 = 分支首见序（main 在第 0 行）。
    let row_of: HashMap<&str, usize> = ug
        .subgraphs
        .iter()
        .enumerate()
        .map(|(i, sg)| (sg.title.as_deref().unwrap_or(""), i))
        .collect();

    let mut gg_nodes = Vec::new();
    for (i, n) in ug.nodes.iter().enumerate() {
        let ir::common::NodeDetail::GitCommit { branch, .. } = &n.detail else {
            continue;
        };
        let row = row_of.get(branch.as_str()).copied().unwrap_or(0);
        let center = Point::new(
            GIT_LEFT_MARGIN + i as f64 * GIT_COMMIT_SPACING,
            GIT_TOP_MARGIN + row as f64 * GIT_BRANCH_SPACING,
        );
        let d = GIT_COMMIT_RADIUS * 2.0;
        let size = Size::new(d, d);
        gg_nodes.push(GGNode {
            id: n.id.clone(),
            role: n.role,
            center,
            size,
            shape: n.shape,
            ports: resolve_ports(center, size),
            label: None,
            detail: n.detail.clone(),
        });
    }

    let mut gg_edges = Vec::new();
    for e in &ug.edges {
        gg_edges.push(GGEdge {
            id: e.id.clone(),
            source: e.source.clone(),
            target: e.target.clone(),
            route: ir::geograph::RoutePath::new(),
            label_text: e.label_text.clone(),
            label_anchor: None,
            kind: e.kind,
            arrow: e.arrow,
            routing_hint: e.routing_hint,
            line_kind: e.line_kind,
            cardinality: e.cardinality,
            cardinality_text: e.cardinality_text.clone(),
        });
    }

    // 分支容器：bounds 占位（materialize 只用 title / kind / member_ids）。
    let gg_containers: Vec<GGContainer> = ug
        .subgraphs
        .iter()
        .map(|sg| GGContainer {
            id: sg.id.clone(),
            kind: sg.kind,
            title: sg.title.clone(),
            bounds: lievisual::geometry::Rect::new(0.0, 0.0, 1.0, 1.0),
            member_ids: sg.member_ids.clone(),
        })
        .collect();

    (gg_nodes, gg_edges, gg_containers)
}

/// 把 UG 布局成 GG，并抽取 StyleIntent。
pub fn run(ug: &ir::Unigraph) -> Result<(Geograph, StyleIntent), String> {
    // 索引节点尺寸（measure 已写入 MeasuredLabel）
    let mut sizes: HashMap<NodeId, Size> = HashMap::new();
    let mut shapes: HashMap<NodeId, ShapeKind> = HashMap::new();
    let mut labels: HashMap<NodeId, ir::common::MeasuredLabel> = HashMap::new();
    for n in &ug.nodes {
        let size = match &n.label {
            LabelOrMeasured::Measured(m) => m.size,
            _ => Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H),
        };
        sizes.insert(n.id.clone(), size);
        shapes.insert(n.id.clone(), n.shape);
        if let LabelOrMeasured::Measured(m) = &n.label {
            labels.insert(n.id.clone(), m.clone());
        }
    }

    // —— 坐标分配 / GG 构建：Sequence 家族走专属布局；其余走线性或分层 ——
    let mut gg_nodes;
    let gg_edges;
    let containers;
    let activations;
    let sequence_dividers;
    let mut needs_routing = true;
    if ug.family == ir::unigraph::GraphFamily::Sequence {
        (
            gg_nodes,
            gg_edges,
            containers,
            activations,
            sequence_dividers,
        ) = sequence_geometry(ug, &sizes, &labels);
        needs_routing = false;
    } else if ug.family == ir::unigraph::GraphFamily::Hierarchy {
        activations = Vec::new();
        sequence_dividers = Vec::new();
        (gg_nodes, gg_edges, containers) = hierarchy_geometry(ug);
        needs_routing = false;
    } else {
        activations = Vec::new();
        sequence_dividers = Vec::new();
        let mut centers = if ug.family == ir::unigraph::GraphFamily::Linear {
            linear_centers(ug, &sizes)
        } else if ug.family == ir::unigraph::GraphFamily::Radial {
            // Radial（pie 等）：所有节点叠于原点，扇区角度由 materialize 据数据计算。
            ug.nodes
                .iter()
                .map(|n| (n.id.clone(), Point::new(0.0, 0.0)))
                .collect()
        } else {
            layered_centers(ug, &sizes)
        };
        // 子图显式 direction：在全局坐标之上按容器内部方向局部重排。
        apply_subgraph_directions(ug, &sizes, &mut centers);

        // —— 构建 GGNode（含端口）——
        let mut nodes = Vec::new();
        for n in &ug.nodes {
            let center = centers.get(&n.id).cloned().unwrap_or(Point::new(0.0, 0.0));
            let size = sizes
                .get(&n.id)
                .cloned()
                .unwrap_or(Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H));
            let shape = shapes.get(&n.id).cloned().unwrap_or(ShapeKind::Rectangle);
            nodes.push(GGNode {
                id: n.id.clone(),
                role: n.role,
                center,
                size,
                shape,
                ports: resolve_ports(center, size),
                label: labels.get(&n.id).cloned(),
                detail: n.detail.clone(),
            });
        }
        gg_nodes = nodes;

        // —— 边（空路由，待统一重路由）——
        let mut edges = Vec::new();
        let node_lookup: HashMap<&NodeId, &GGNode> = gg_nodes.iter().map(|n| (&n.id, n)).collect();
        for e in &ug.edges {
            let (Some(s), Some(t)) = (node_lookup.get(&e.source), node_lookup.get(&e.target))
            else {
                continue;
            };
            let _ = (s, t);
            edges.push(GGEdge {
                id: e.id.clone(),
                source: e.source.clone(),
                target: e.target.clone(),
                route: ir::geograph::RoutePath::new(),
                label_text: e.label_text.clone(),
                label_anchor: None,
                kind: e.kind,
                arrow: e.arrow,
                routing_hint: e.routing_hint,
                line_kind: e.line_kind,
                cardinality: e.cardinality,
                cardinality_text: e.cardinality_text.clone(),
            });
        }
        gg_edges = edges;

        // —— 子图容器 ——
        containers = compute_containers(ug, &gg_nodes);
        // 容器框与外部节点解重叠：层内排布并不知道容器范围，紧挨 subgraph 的
        // 外部节点可能被容器框罩住一角，迫使连向它的边绕行容器（产生不必要
        // 的折线）。将重叠的外部节点沿主轴法向平移出容器后再路由。
        let container_members: std::collections::HashSet<String> = ug
            .subgraphs
            .iter()
            .flat_map(|sg| sg.member_ids.iter().cloned())
            .collect();
        separate_nodes_from_containers(
            ug.direction,
            &container_members,
            &mut gg_nodes,
            &containers,
        );
    }

    // —— StyleIntent 抽取 ——
    let style = StyleIntent {
        node_styles: ug
            .nodes
            .iter()
            .map(|n| (n.id.clone(), n.style_ref.clone()))
            .collect(),
        edge_styles: ug
            .edges
            .iter()
            .map(|e| (e.id.clone(), e.kind, e.arrow))
            .collect(),
        container_styles: Vec::new(),
    };

    let mut gg = Geograph {
        size: Size::new(0.0, 0.0),
        background: theme::BACKGROUND,
        nodes: gg_nodes,
        edges: gg_edges,
        containers: Vec::<GGContainer>::new(),
        title: ug.meta.title.clone(),
        show_data: ug.meta.show_data,
        activations,
        sequence_dividers,
    };
    if needs_routing {
        // 正交/样条边路由（节点 + 容器回避）：容器包围盒仅依赖节点几何，
        // 故在路由前计算；路由把容器框视为障碍，使「两端均非容器成员」的
        // 边绕开子图，避免关系线穿容器。
        route_edges(&mut gg, ug.direction, &containers);
    }

    // ER/通用边的关系标签锚点：路由完成后取路径弧长中点（与 materialize
    // 渲染关系名的位置一致）。此前 label_anchor 恒为 None，导致 ER 关系名
    // （"places"/"contains" 等）完全不渲染。
    for e in &mut gg.edges {
        if e.label_text.as_deref().is_some_and(|s| !s.is_empty()) {
            e.label_anchor = Some(e.route.midpoint());
        }
    }
    gg.containers = containers;

    let bbox = compute_bbox(&gg.nodes, &gg.edges);
    gg.size = bbox;
    Ok((gg, style))
}

/// 将与子图容器框重叠的外部节点沿主轴法向平移出容器（层坐标不变）。
///
/// 容器框由成员节点推导，但层内排布并不知道容器范围：紧挨 subgraph 的外部
/// 节点（例如与容器同层带的 Retry）可能被容器框罩住一角，迫使连向它的边
/// 绕行容器（产生不必要的折线）。这里把与容器框相交的外部节点沿主轴法向
/// 平移到容器外侧（取节点中心相对容器中心的侧向），成员节点与容器 bounds
/// 均保持不变。
fn separate_nodes_from_containers(
    direction: crate::ast::Direction,
    members: &std::collections::HashSet<String>,
    nodes: &mut [GGNode],
    containers: &[GGContainer],
) {
    use crate::ast::Direction;
    const GAP: f64 = 12.0;
    // 垂直主轴（TB/TD/BT）：容器沿水平方向占据层带 → 水平推开；水平主轴反之。
    let vertical = matches!(direction, Direction::TB | Direction::TD | Direction::BT);
    for c in containers {
        let b = c.bounds;
        for n in nodes.iter_mut() {
            if members.contains(&n.id) {
                continue;
            }
            let (hw, hh) = (n.size.width / 2.0, n.size.height / 2.0);
            let n_rect = lievisual::geometry::Rect::new(
                n.center.x - hw,
                n.center.y - hh,
                n.center.x + hw,
                n.center.y + hh,
            );
            // AABB 相交（贴边不算）。
            let overlaps = n_rect.max_x() > b.min_x()
                && n_rect.min_x() < b.max_x()
                && n_rect.max_y() > b.min_y()
                && n_rect.min_y() < b.max_y();
            if !overlaps {
                continue;
            }
            // 沿主轴法向平移出容器：节点中心在容器中心正向侧 → 向正向推。
            if vertical {
                if n.center.x >= (b.min_x() + b.max_x()) / 2.0 {
                    n.center.x = b.max_x() + GAP + hw;
                } else {
                    n.center.x = b.min_x() - GAP - hw;
                }
            } else if n.center.y >= (b.min_y() + b.max_y()) / 2.0 {
                n.center.y = b.max_y() + GAP + hh;
            } else {
                n.center.y = b.min_y() - GAP - hh;
            }
            n.ports = resolve_ports(n.center, n.size);
        }
    }
}

/// 据子图成员节点坐标计算容器包围盒（含留白与标题区）。
///
/// padding 按容器类型区分（实测官方 golden）：
/// - flowchart subgraph：横向 35 / 纵向 25（`flowchart__subgraph` 的 One/Two 簇），
///   标题行位于顶部留白内（官方 label 在簇顶、居中）；
/// - state 复合状态：四周 35（`state__composite` 的 Outer 簇：内容 43..104 ×
///   109.5..445.5，簇 8..139.4 × 72..480），标题行同样在顶部留白内。
fn compute_containers(ug: &ir::Unigraph, nodes: &[GGNode]) -> Vec<GGContainer> {
    let lookup: HashMap<&str, &GGNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    ug.subgraphs
        .iter()
        .filter_map(|sg| {
            // 找到该容器所有已布局成员节点。
            let members: Vec<&GGNode> = sg
                .member_ids
                .iter()
                .filter_map(|id| lookup.get(id.as_str()).copied())
                .collect();
            if members.is_empty() {
                return None;
            }
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for m in &members {
                min_x = min_x.min(m.center.x - m.size.width / 2.0);
                min_y = min_y.min(m.center.y - m.size.height / 2.0);
                max_x = max_x.max(m.center.x + m.size.width / 2.0);
                max_y = max_y.max(m.center.y + m.size.height / 2.0);
            }
            // state 复合状态四周留白 35；flowchart subgraph 横 35 / 纵 25。
            let (pad_x, pad_y) = match sg.kind {
                ir::common::ContainerKind::StateComposite => (35.0, 35.0),
                _ => (35.0, 25.0),
            };
            Some(GGContainer {
                id: sg.id.clone(),
                kind: sg.kind,
                title: sg.title.clone(),
                // kurbo Rect::new 接受两个角点 (x0,y0,x1,y1)，宽高由角点推导。
                bounds: lievisual::geometry::Rect::new(
                    min_x - pad_x,
                    min_y - pad_y,
                    max_x + pad_x,
                    max_y + pad_y,
                ),
                member_ids: sg.member_ids.clone(),
            })
        })
        .collect()
}

/// 据节点 center + size 解析四向端口坐标。
fn resolve_ports(center: Point, size: Size) -> ir::common::ResolvedPorts {
    ir::common::ResolvedPorts {
        top: Point::new(center.x, center.y - size.height / 2.0),
        bottom: Point::new(center.x, center.y + size.height / 2.0),
        left: Point::new(center.x - size.width / 2.0, center.y),
        right: Point::new(center.x + size.width / 2.0, center.y),
    }
}

/// 计算所有节点 + 边点的包围盒尺寸（留白边距）。
fn compute_bbox(nodes: &[GGNode], edges: &[GGEdge]) -> Size {
    let mut min_x = 0.0f64;
    let mut min_y = 0.0f64;
    let mut max_x = 0.0f64;
    let mut max_y = 0.0f64;
    for n in nodes {
        min_x = min_x.min(n.center.x - n.size.width / 2.0);
        min_y = min_y.min(n.center.y - n.size.height / 2.0);
        max_x = max_x.max(n.center.x + n.size.width / 2.0);
        max_y = max_y.max(n.center.y + n.size.height / 2.0);
    }
    for e in edges {
        for p in e.route.anchors() {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    let pad = 20.0f64;
    Size::new(max_x - min_x + pad * 2.0, max_y - min_y + pad * 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{extract, measure};

    fn run_linear(src: &str) -> Geograph {
        let diagram = crate::MermaidParser::parse_mermaid(src).expect("parse");
        let timeline = match &diagram {
            crate::ast::Diagram::Timeline(t) => t,
            _ => panic!("not timeline"),
        };
        let ug = extract::timeline::extract_timeline(timeline);
        let ug = measure::measure_all(ug);
        let (gg, _) = run(&ug).expect("layout");
        gg
    }

    #[test]
    fn linear_lr_lays_out_sections_in_row() {
        let gg = run_linear("timeline\nsection A\n2000 : X\nsection B\n3000 : Y\n");
        assert_eq!(gg.nodes.len(), 2);
        let (a, b) = (&gg.nodes[0], &gg.nodes[1]);
        // LR（默认）：同一水平线（y≈0），x 递增。
        assert!(
            (a.center.y - b.center.y).abs() < 1e-6,
            "LR 各列应在同一水平线"
        );
        assert!(b.center.x > a.center.x, "LR 列应沿 x 递增");
    }

    #[test]
    fn linear_td_lays_out_sections_in_column() {
        let gg = run_linear("timeline TD\nsection A\n2000 : X\nsection B\n3000 : Y\n");
        assert_eq!(gg.nodes.len(), 2);
        let (a, b) = (&gg.nodes[0], &gg.nodes[1]);
        // TD：同一垂直线（x≈0），y 递增。
        assert!(
            (a.center.x - b.center.x).abs() < 1e-6,
            "TD 各列应在同一垂直线"
        );
        assert!(b.center.y > a.center.y, "TD 列应沿 y 递增");
    }

    fn run_flowchart(src: &str) -> Geograph {
        let diagram = crate::MermaidParser::parse_mermaid(src).expect("parse");
        let ug = extract::run(&diagram).expect("extract");
        let ug = measure::measure_all(ug);
        let (gg, _) = run(&ug).expect("layout");
        gg
    }

    fn node<'a>(gg: &'a Geograph, id: &str) -> &'a GGNode {
        gg.nodes.iter().find(|n| n.id == id).expect("node")
    }

    /// 子图 `direction LR` 覆盖全局 TB：成员应横向排列（同一 y、x 递增）。
    #[test]
    fn subgraph_direction_overrides_global_direction() {
        let gg = run_flowchart(
            "flowchart TB\nsubgraph S\ndirection LR\nA[AA] --> B[BB] --> C[CC]\nend\nD[DD] --> A\n",
        );
        let (a, b, c) = (node(&gg, "A"), node(&gg, "B"), node(&gg, "C"));
        assert!(
            (a.center.y - b.center.y).abs() < 1.0 && (b.center.y - c.center.y).abs() < 1.0,
            "LR 子图内节点应同 y：A={:?} B={:?} C={:?}",
            a.center,
            b.center,
            c.center
        );
        assert!(
            a.center.x < b.center.x && b.center.x < c.center.x,
            "LR 子图内节点应沿 x 递增"
        );
    }

    /// 嵌套子图各自方向：内层 TD 在外层 LR 重排后仍保持竖向。
    #[test]
    fn nested_subgraph_directions_are_preserved() {
        let gg = run_flowchart(
            "flowchart TB\nsubgraph outer\ndirection LR\nsubgraph inner\ndirection TD\nX[XX] --> Y[YY]\nend\nA[AA] --> B[BB]\nB --> X\nend\nC --> A\n",
        );
        let (x, y) = (node(&gg, "X"), node(&gg, "Y"));
        assert!(
            (x.center.x - y.center.x).abs() < 1.0,
            "内层 TD 应保持竖向：X={:?} Y={:?}",
            x.center,
            y.center
        );
        assert!(y.center.y > x.center.y);
    }

    /// RL / BT 方向：层序反转，成员应沿请求方向反向排列。
    #[test]
    fn subgraph_direction_rl_and_bt() {
        let gg = run_flowchart(
            "flowchart TB\nsubgraph S\ndirection RL\nA[AA] --> B[BB]\nend\nC --> A\n",
        );
        let (a, b) = (node(&gg, "A"), node(&gg, "B"));
        assert!((a.center.y - b.center.y).abs() < 1.0, "RL 子图应同 y");
        assert!(b.center.x < a.center.x, "RL 应从右到左排列");

        let gg = run_flowchart(
            "flowchart TB\nsubgraph S\ndirection BT\nA[AA] --> B[BB]\nend\nC --> A\n",
        );
        let (a, b) = (node(&gg, "A"), node(&gg, "B"));
        assert!((a.center.x - b.center.x).abs() < 1.0, "BT 子图应同 x");
        assert!(b.center.y < a.center.y, "BT 应从下到上排列");
    }

    /// 全局 LR + 子图 TD：子图内应竖向排列。
    #[test]
    fn subgraph_direction_td_overrides_global_lr() {
        let gg = run_flowchart(
            "flowchart LR\nsubgraph S\ndirection TD\nA[AA] --> B[BB]\nend\nC[CC] --> A\n",
        );
        let (a, b) = (node(&gg, "A"), node(&gg, "B"));
        assert!((a.center.x - b.center.x).abs() < 1.0, "TD 子图内节点应同 x");
        assert!(b.center.y > a.center.y, "TD 子图内节点应沿 y 递增");
    }
}
