//! `directed` family solver：Sugiyama 风格分层 + 通用交叉减少。
//!
//! 输入 [`crate::builder::ir::Unigraph`]，输出分层序列 `Vec<Vec<NodeId>>`
//! （已按 [`crate::builder::layout::crossing::minimize_crossings`] 降低相邻层交叉）。
//! 坐标分配（含 `direction` 主轴旋转）由 `engine` 负责。
//!
//! 分层采用最长路径松弛（DAG 分层），自环/弱环在松弛迭代次数上限内收敛到稳定层号；
//! `direction` 为 `BT` / `RL` 时整体反转层序（首层变末层），使主轴方向正确。

use std::collections::HashMap;

use crate::ast::Direction;
use crate::builder::ir::{common::NodeId, unigraph::Unigraph};

use super::crossing::{LayerEdge, minimize_crossings};

#[allow(unused_imports)]
use crate::builder::ir::common::*;

/// 反馈边（back edge）检测：用 DFS 找环，指向「当前 DFS 栈中的祖先」的边即为 back edge。
///
/// 保留非反馈边（forward edges）构成 DAG，分层时跳过反馈边（避免环把层号推高）。
/// 反馈边（如 cycle 中的 `C→B`，其反向 `B→C` 保留）由路由单独画跨层绕行曲线。
fn detect_back_edges(ug: &Unigraph) -> std::collections::HashSet<(NodeId, NodeId)> {
    // 邻接表
    let mut adj: std::collections::HashMap<NodeId, Vec<NodeId>> = std::collections::HashMap::new();
    for n in &ug.nodes {
        adj.entry(n.id.clone()).or_default();
    }
    for e in &ug.edges {
        adj.entry(e.source.clone())
            .or_default()
            .push(e.target.clone());
    }
    let mut back: std::collections::HashSet<(NodeId, NodeId)> = std::collections::HashSet::new();
    // 三色标记：0=未访问，1=在栈中，2=已结束。
    let mut color: std::collections::HashMap<NodeId, u8> = std::collections::HashMap::new();
    for n in &ug.nodes {
        color.insert(n.id.clone(), 0u8);
    }

    // 迭代 DFS（显式栈模拟递归，避免大图栈溢出）。
    // stack 元素：node。同时用 iter_idx 记录每个节点的下一条邻接边。
    let mut stack: Vec<NodeId> = Vec::new();
    let mut iter_idx: std::collections::HashMap<NodeId, usize> = std::collections::HashMap::new();
    // 从每个未访问节点开始。
    let ids: Vec<NodeId> = ug.nodes.iter().map(|n| n.id.clone()).collect();
    for start in &ids {
        if color.get(start).copied().unwrap_or(0) != 0 {
            continue;
        }
        color.insert(start.clone(), 1);
        stack.push(start.clone());
        while let Some(top) = stack.last() {
            let top_node = top.clone();
            let idx = iter_idx.entry(top_node.clone()).or_insert(0);
            let neighbors = adj.get(top).cloned().unwrap_or_default();
            if *idx >= neighbors.len() {
                // 完成节点 top。
                color.insert(top_node.clone(), 2);
                stack.pop();
                continue;
            }
            let v = neighbors[*idx].clone();
            *idx += 1;
            match color.get(&v).copied().unwrap_or(0) {
                1 => {
                    // v 在栈中 → top→v 是 back edge。
                    back.insert((top_node.clone(), v.clone()));
                }
                0 => {
                    color.insert(v.clone(), 1);
                    stack.push(v.clone());
                }
                _ => {} // 2 = 已结束，跳过
            }
        }
    }
    back
}

/// 对 DAG 做最长路径分层：层号 = 沿有向边的最大前驱层 + 1。
///
/// 关键约束：
/// 1. 构图反向边（u↔v 同时存在）跳过，不参与分层（避免环膨胀）。
/// 2. 层号上限 = 节点数 - 1，兜底防止其他长链把层号推到过高。
fn assign_layers(ug: &Unigraph) -> Vec<Vec<NodeId>> {
    let ids: Vec<NodeId> = ug.nodes.iter().map(|n| n.id.clone()).collect();
    let back_edges = detect_back_edges(ug);
    let max_allowed = ids.len().saturating_sub(1).max(1);
    let mut layer_of: HashMap<NodeId, usize> = HashMap::new();
    for id in &ids {
        layer_of.insert(id.clone(), 0);
    }
    // 松弛：仅对非 back edge 做最长路径抬升，层号上限截断。
    let mut changed = true;
    let mut guard = 0;
    while changed && guard < ids.len() + 1 {
        changed = false;
        guard += 1;
        for e in &ug.edges {
            if back_edges.contains(&(e.source.clone(), e.target.clone())) {
                continue;
            }
            let sl = *layer_of.get(&e.source).unwrap_or(&0);
            let tl = *layer_of.get(&e.target).unwrap_or(&0);
            if tl <= sl {
                let new_tl = (sl + 1).min(max_allowed);
                if new_tl > tl {
                    layer_of.insert(e.target.clone(), new_tl);
                    changed = true;
                }
            }
        }
    }
    // 按层收集（保留空层，跨层边路由距离正确）。
    let max_layer = layer_of.values().cloned().max().unwrap_or(0);
    let mut layers: Vec<Vec<NodeId>> = vec![Vec::new(); max_layer + 1];
    for id in &ids {
        let l = *layer_of.get(id).unwrap_or(&0);
        layers[l].push(id.clone());
    }
    layers
}

/// Sugiyama 分层 + 交叉减少：返回层内顺序已优化的分层序列。
///
/// `direction` 为 `BT` / `RL` 时，将分层整体反转（首层 ↔ 末层），使主轴方向正确。
pub fn sugiyama_layers(ug: &Unigraph) -> Vec<Vec<NodeId>> {
    let layers = assign_layers(ug);

    // 组装跨相邻层边对（供 crossing 原语使用）
    let layer_of: HashMap<&NodeId, usize> = layers
        .iter()
        .enumerate()
        .flat_map(|(li, l)| l.iter().map(move |id| (id, li)))
        .collect();
    let mut layer_edges: Vec<LayerEdge> = Vec::new();
    for e in &ug.edges {
        let (Some(&si), Some(&ti)) = (layer_of.get(&e.source), layer_of.get(&e.target)) else {
            continue;
        };
        if si + 1 == ti {
            layer_edges.push(LayerEdge {
                source: e.source.clone(),
                target: e.target.clone(),
            });
        }
    }

    let mut optimized = minimize_crossings(&layers, &layer_edges);

    // 方向反转：BT / RL 让层序倒过来（首层变末层）
    match ug.direction {
        Direction::BT | Direction::RL => {
            optimized.reverse();
        }
        _ => {}
    }
    optimized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ir::unigraph::{EdgeKind, UGEdge, UGNode};

    fn mk_node(id: &str) -> UGNode {
        UGNode {
            id: id.to_string(),
            kind: NodeKind::Atom,
            role: NodeRole::Atom,
            shape: crate::builder::ir::shape::ShapeKind::Rectangle,
            label: LabelOrMeasured::Spec(LabelSpec {
                text: id.to_string(),
                spans: vec![],
            }),
            ports: PortSet::default(),
            size_hint: SizeHint::default(),
            style_ref: StyleRef::default(),
            constraint: NodeConstraint::default(),
            detail: NodeDetail::None,
        }
    }

    fn mk_edge(s: &str, t: &str) -> UGEdge {
        UGEdge {
            id: format!("{}-{}", s, t),
            source: s.to_string(),
            target: t.to_string(),
            source_port: PortHint::Bottom,
            target_port: PortHint::Top,
            kind: EdgeKind::Flow,
            label_text: None,
            label: None,
            priority: EdgePriority::Primary,
            routing_hint: RoutingHint::Orthogonal,
            arrow: ArrowSpec {
                start: ArrowKind::None,
                end: ArrowKind::Arrow,
            },
            line_kind: LineKind::Solid,
            repulsion: 1.0,
            cardinality: (None, None),
            cardinality_text: (None, None),
        }
    }

    fn count_crossings_ug(layers: &[Vec<NodeId>], edges: &[UGEdge]) -> usize {
        let pos =
            |li: usize, id: &str| -> Option<usize> { layers[li].iter().position(|x| x == id) };
        let mut count = 0;
        for li in 0..layers.len() - 1 {
            let mut segs: Vec<(usize, usize)> = Vec::new();
            for e in edges {
                if let (Some(sp), Some(tp)) = (pos(li, &e.source), pos(li + 1, &e.target)) {
                    segs.push((sp, tp));
                }
            }
            for i in 0..segs.len() {
                for j in i + 1..segs.len() {
                    let (s1, t1) = segs[i];
                    let (s2, t2) = segs[j];
                    if (s1 < s2 && t1 > t2) || (s1 > s2 && t1 < t2) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    #[test]
    fn sugiyama_reduces_crossings_on_3_layers() {
        // 构造 3 层、带严重交叉的 UG。
        // L0: A,B   L1: C,D   L2: E,F
        // 边 A->C, B->D, A->D, B->C (L0-L1 交叉)
        //   C->E, D->F, C->F, D->E (L1-L2 交叉)
        let mut ug = Unigraph::default();
        for id in ["A", "B", "C", "D", "E", "F"] {
            ug.nodes.push(mk_node(id));
        }
        ug.edges.push(mk_edge("A", "C"));
        ug.edges.push(mk_edge("B", "D"));
        ug.edges.push(mk_edge("A", "D"));
        ug.edges.push(mk_edge("B", "C"));
        ug.edges.push(mk_edge("C", "E"));
        ug.edges.push(mk_edge("D", "F"));
        ug.edges.push(mk_edge("C", "F"));
        ug.edges.push(mk_edge("D", "E"));

        // 基线交叉：按节点原始顺序分层
        let baseline = vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["C".to_string(), "D".to_string()],
            vec!["E".to_string(), "F".to_string()],
        ];
        let baseline_cross = count_crossings_ug(&baseline, &ug.edges);
        assert!(baseline_cross > 0, "基线应存在交叉");

        let out = sugiyama_layers(&ug);
        let out_cross = count_crossings_ug(&out, &ug.edges);
        assert!(
            out_cross <= baseline_cross,
            "Sugiyama 交叉({})应不高于基线({})",
            out_cross,
            baseline_cross
        );
    }

    #[test]
    fn sugiyama_reverses_layers_for_bt() {
        let mut ug = Unigraph::default();
        for id in ["A", "B", "C"] {
            ug.nodes.push(mk_node(id));
        }
        ug.edges.push(mk_edge("A", "B"));
        ug.edges.push(mk_edge("B", "C"));
        // TB：首层应为 [A]
        ug.direction = Direction::TB;
        let out = sugiyama_layers(&ug);
        assert_eq!(out[0], vec!["A".to_string()]);
        // BT：首层应为 [C]（反转）
        ug.direction = Direction::BT;
        let out2 = sugiyama_layers(&ug);
        assert_eq!(out2[0], vec!["C".to_string()]);
    }
}
