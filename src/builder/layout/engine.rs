//! LayoutEngine —— 布局求解入口（P1.1 Sugiyama 分层 + 交叉减少）。
//!
//! 编排：`extract` 产出的 UG → `directed::sugiyama_layers`（分层 + 降交叉 + 方向反转）
//! → 按 `direction` 做主轴坐标分配 → 端口解析 → 直线边路由（P1.2 升级为正交路由）。
//!
//! 产出 `(Geograph, StyleIntent)`：几何与视觉意图在 layout 结束时分离，
//! 使 UG 可在此后 drop，materialize / paint 不持有 UG 引用。

use std::collections::HashMap;

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
const NODE_GAP: f64 = 50.0;
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
    let mut gap_extra = vec![0.0f64; n_layers.saturating_sub(1)];
    for e in &ug.edges {
        let (Some(&si), Some(&ti)) = (layer_of.get(&e.source), layer_of.get(&e.target)) else {
            continue;
        };
        // 只看相邻层的边（跨层边由路由处理，标签取中点，仍受最近层间距影响）。
        let has_label = e.label_text.as_deref().is_some_and(|s| !s.is_empty());
        if has_label && si.abs_diff(ti) == 1 {
            let lo = si.min(ti);
            gap_extra[lo] = gap_extra[lo].max(EDGE_LABEL_GAP);
        }
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

/// Linear 家族（timeline 等）的坐标分配：所有节点沿主轴单行排布并居中。
/// LR（默认）：水平一行，y=0（时间轴）；TD：垂直一列，x=0。
fn linear_centers(ug: &ir::Unigraph, sizes: &HashMap<NodeId, Size>) -> HashMap<NodeId, Point> {
    let horizontal = matches!(ug.direction, Direction::LR | Direction::RL);
    let mut total = 0.0f64;
    for n in &ug.nodes {
        let size = sizes
            .get(&n.id)
            .cloned()
            .unwrap_or(Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H));
        let main = if horizontal { size.width } else { size.height };
        total += main + NODE_GAP;
    }
    total = total.max(NODE_GAP) - NODE_GAP;
    let mut cursor = -total / 2.0;
    let mut centers: HashMap<NodeId, Point> = HashMap::new();
    for n in &ug.nodes {
        let size = sizes
            .get(&n.id)
            .cloned()
            .unwrap_or(Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H));
        let main = if horizontal { size.width } else { size.height };
        let coord = cursor + main / 2.0;
        let (x, y) = if horizontal {
            (coord, 0.0)
        } else {
            (0.0, coord)
        };
        centers.insert(n.id.clone(), Point::new(x, y));
        cursor += main + NODE_GAP;
    }
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
fn sequence_geometry(
    ug: &ir::Unigraph,
    sizes: &HashMap<NodeId, Size>,
    labels: &HashMap<NodeId, ir::common::MeasuredLabel>,
) -> (
    Vec<GGNode>,
    Vec<GGEdge>,
    Vec<GGContainer>,
    Vec<ir::geograph::GGActivation>,
) {
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
    // 激活跨度（actor, y0, y1），由下方语句行循环产出（块内必然赋值）。
    let act_spans_out: Vec<(String, f64, f64)>;
    {
        struct Frame {
            id: String,
            label: String,
            depth: usize,
            top: f64,
        }
        let mut stack: Vec<Frame> = Vec::new();
        // 激活条：actor → 已开始但未结束的起点 y 栈（支持嵌套激活）。
        let mut open_act: HashMap<&str, Vec<f64>> = HashMap::new();
        // 已闭合的激活跨度 (actor, y0, y1)。
        let mut act_spans: Vec<(String, f64, f64)> = Vec::new();
        // 上一行的起始 y（`Activation` 行不占行高，起点取所属消息行的 y）。
        let mut prev_y = cur_y;
        for row in &rows {
            match row {
                SequenceRow::Message(eid) => {
                    row_y.insert(eid.as_str(), cur_y);
                    prev_y = cur_y;
                    cur_y += SEQ_MSG_SPACING;
                }
                SequenceRow::Note(nid) => {
                    row_y.insert(nid.as_str(), cur_y);
                    prev_y = cur_y;
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
                    stack.push(Frame {
                        id: id.clone(),
                        label: label.clone(),
                        depth: stack.len(),
                        top: cur_y,
                    });
                    prev_y = cur_y;
                    cur_y += SEQ_BLOCK_LABEL_H;
                }
                SequenceRow::BlockEnd(_) => {
                    if let Some(f) = stack.pop() {
                        cur_y += SEQ_BLOCK_PAD;
                        block_geo.push((f.id, f.label, f.depth, f.top, cur_y));
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
    (gg_nodes, gg_edges, gg_containers, activations)
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
            label_text: None,
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
    let gg_nodes;
    let gg_edges;
    let containers;
    let activations;
    let mut needs_routing = true;
    if ug.family == ir::unigraph::GraphFamily::Sequence {
        (gg_nodes, gg_edges, containers, activations) = sequence_geometry(ug, &sizes, &labels);
        needs_routing = false;
    } else if ug.family == ir::unigraph::GraphFamily::Hierarchy {
        activations = Vec::new();
        (gg_nodes, gg_edges, containers) = hierarchy_geometry(ug);
        needs_routing = false;
    } else {
        activations = Vec::new();
        let centers = if ug.family == ir::unigraph::GraphFamily::Linear {
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
    };
    if needs_routing {
        // 正交/样条边路由（节点 + 容器回避）：容器包围盒仅依赖节点几何，
        // 故在路由前计算；路由把容器框视为障碍，使「两端均非容器成员」的
        // 边绕开子图，避免关系线穿容器。
        route_edges(&mut gg, ug.direction, &containers);
    }
    gg.containers = containers;

    let bbox = compute_bbox(&gg.nodes, &gg.edges);
    gg.size = bbox;
    Ok((gg, style))
}

/// 据子图成员节点坐标计算容器包围盒（含留白与标题区）。
fn compute_containers(ug: &ir::Unigraph, nodes: &[GGNode]) -> Vec<GGContainer> {
    let lookup: HashMap<&str, &GGNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    const PAD: f64 = 12.0;
    const TITLE_H: f64 = 24.0;
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
            Some(GGContainer {
                id: sg.id.clone(),
                kind: sg.kind,
                title: sg.title.clone(),
                // kurbo Rect::new 接受两个角点 (x0,y0,x1,y1)，宽高由角点推导。
                bounds: lievisual::geometry::Rect::new(
                    min_x - PAD,
                    min_y - PAD - TITLE_H,
                    max_x + PAD,
                    max_y + PAD,
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
}
