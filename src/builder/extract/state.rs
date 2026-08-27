//! state 图的 extract：把 [`crate::ast::StateDiagram`] 翻译成 [`Unigraph`](crate::builder::ir::Unigraph)。
//!
//! family=Directed（状态转移是有向 DAG），与 flowchart 复用同一套 Sugiyama 分层 +
//! 交叉减少 + 正交路由。
//!
//! 节点收集顺序与旧 `layout/state_nodes.rs::collect_state_nodes` 保持一致：
//! 显式 states 声明序 + transitions 补充序（按 id 去重）。`[*]` 映射为
//! `__start__`（StartDot）/ `__end__`（EndDot），`<<fork>>`/`<<join>>` 映射为 `Bar`。
//! 复合状态（`State::Composite`）作为一个整体节点，不展开内部子状态。

use crate::{
    ast::{State, StateDiagram},
    builder::ir::{
        self,
        common::{ArrowKind, ArrowSpec, EdgePriority, LabelSpec, PortHint, PortSet, SizeHint, StyleRef},
        shape::ShapeKind,
        unigraph::{EdgeKind, UGEdge, UGNode, Unigraph},
    },
};
use lievisual::geometry::Size;

/// 状态图默认从上到下布局（无方向声明）。
const DEFAULT_DIRECTION: crate::ast::Direction = crate::ast::Direction::TB;

// 状态特殊节点固定尺寸（与旧 collect_state_nodes 一致）。
const START_SIZE: Size = Size::new(32.0, 32.0);
const END_SIZE: Size = Size::new(36.0, 36.0);
const BAR_SIZE: Size = Size::new(100.0, 10.0);

/// 提取 state 图（顶层 states + 顶层 transitions，复合状态不展开）为统一拓扑图。
pub fn extract_state(sd: &StateDiagram) -> Unigraph {
    let mut nodes: Vec<UGNode> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 记录显式 Simple 状态的描述文本（作为 label）。
    let mut labels: std::collections::HashMap<String, Option<String>> = Default::default();
    for s in &sd.states {
        if let State::Simple { id, description } = s {
            labels.insert(id.clone(), description.clone());
        }
    }

    // 节点形状 / 尺寸决策。
    let node_meta = |_id: &str, is_start: bool, is_end: bool, is_bar: bool| {
        if is_start {
            (ShapeKind::StartDot, SizeHint::Fixed(START_SIZE))
        } else if is_end {
            (ShapeKind::EndDot, SizeHint::Fixed(END_SIZE))
        } else if is_bar {
            (ShapeKind::Bar, SizeHint::Fixed(BAR_SIZE))
        } else {
            (ShapeKind::Rounded, SizeHint::ByText)
        }
    };

    // 入队一个节点（去重）。
    fn push_node(
        nodes: &mut Vec<UGNode>,
        seen: &mut std::collections::HashSet<String>,
        id: String,
        label: Option<String>,
        shape: ShapeKind,
        size_hint: SizeHint,
    ) {
        if seen.contains(&id) {
            return;
        }
        seen.insert(id.clone());
        nodes.push(UGNode {
            id,
            kind: ir::common::NodeKind::Atom,
            role: ir::common::NodeRole::Atom,
            shape,
            label: ir::common::LabelOrMeasured::Spec(LabelSpec {
                text: label.unwrap_or_default(),
                spans: Vec::new(),
            }),
            ports: PortSet::default(),
            size_hint,
            style_ref: StyleRef::NodeDefault,
            constraint: ir::common::NodeConstraint::Free,
        });
    }

    // 1. 显式 states 声明（Simple / Composite / Fork / Join / Start / End）
    for s in &sd.states {
        match s {
            State::Simple { id, .. } | State::Composite { id, .. } => {
                let label = labels.get(id).cloned().flatten();
                let (shape, hint) = node_meta(id, false, false, false);
                push_node(&mut nodes, &mut seen, id.clone(), label, shape, hint);
            }
            State::Fork { id } | State::Join { id } => {
                let (shape, hint) = node_meta(id, false, false, true);
                push_node(&mut nodes, &mut seen, id.clone(), None, shape, hint);
            }
            State::Start => {
                push_node(
                    &mut nodes,
                    &mut seen,
                    "__start__".to_string(),
                    None,
                    ShapeKind::StartDot,
                    SizeHint::Fixed(START_SIZE),
                );
            }
            State::End => {
                push_node(
                    &mut nodes,
                    &mut seen,
                    "__end__".to_string(),
                    None,
                    ShapeKind::EndDot,
                    SizeHint::Fixed(END_SIZE),
                );
            }
        }
    }

    // 2. transitions 补充出现的节点（`[*]` → start/end）
    for t in &sd.transitions {
        let (from, f_start) = if t.from == "[*]" {
            ("__start__", true)
        } else {
            (t.from.as_str(), false)
        };
        let (to, t_end) = if t.to == "[*]" {
            ("__end__", true)
        } else {
            (t.to.as_str(), false)
        };
        let (shape_f, hint_f) = node_meta(from, f_start, false, false);
        push_node(
            &mut nodes,
            &mut seen,
            from.to_string(),
            labels.get(from).cloned().flatten(),
            shape_f,
            hint_f,
        );
        let (shape_t, hint_t) = node_meta(to, false, t_end, false);
        push_node(
            &mut nodes,
            &mut seen,
            to.to_string(),
            labels.get(to).cloned().flatten(),
            shape_t,
            hint_t,
        );
    }

    // 3. 转移边（→ StateTransition，带 label 文本；测量在 Stage 1.5 完成）
    let mut edges = Vec::new();
    for (i, t) in sd.transitions.iter().enumerate() {
        let from = if t.from == "[*]" {
            "__start__".to_string()
        } else {
            t.from.clone()
        };
        let to = if t.to == "[*]" {
            "__end__".to_string()
        } else {
            t.to.clone()
        };
        edges.push(UGEdge {
            id: format!("t{}", i),
            source: from,
            target: to,
            source_port: PortHint::Bottom,
            target_port: PortHint::Top,
            kind: EdgeKind::StateTransition,
            label_text: t.label.clone(),
            label: None,
            priority: EdgePriority::Primary,
            routing_hint: ir::common::RoutingHint::Orthogonal,
            arrow: ArrowSpec {
                start: ArrowKind::None,
                end: ArrowKind::Arrow,
            },
            line_kind: ir::common::LineKind::Solid,
            repulsion: 1.0,
        });
    }

    // 保证所有被边引用的节点都存在（即使 states/transitions 都缺失，也避免丢边）。
    for e in &edges {
        if !seen.contains(&e.source) {
            push_node(
                &mut nodes,
                &mut seen,
                e.source.clone(),
                labels.get(&e.source).cloned().flatten(),
                ShapeKind::Rounded,
                SizeHint::ByText,
            );
        }
        if !seen.contains(&e.target) {
            push_node(
                &mut nodes,
                &mut seen,
                e.target.clone(),
                labels.get(&e.target).cloned().flatten(),
                ShapeKind::Rounded,
                SizeHint::ByText,
            );
        }
    }

    Unigraph {
        family: ir::unigraph::GraphFamily::Directed,
        direction: DEFAULT_DIRECTION,
        nodes,
        edges,
        subgraphs: Vec::new(),
        meta: ir::common::DiagramMeta { title: None },
    }
}
