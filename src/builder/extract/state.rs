//! state 图的 extract：把 [`crate::ast::StateDiagram`] 翻译成 [`Unigraph`](crate::builder::ir::Unigraph)。
//!
//! family=Directed（状态转移是有向 DAG），与 flowchart 复用同一套 Sugiyama 分层 +
//! 交叉减少 + 正交路由。
//!
//! 节点收集顺序与旧 `layout/state_nodes.rs::collect_state_nodes` 保持一致：
//! 显式 states 声明序 + transitions 补充序（按 id 去重）。`[*]` 映射为
//! `__start__`（StartDot）/ `__end__`（EndDot），`<<fork>>`/`<<join>>` 映射为 `Bar`。
//!
//! 复合状态（`State::Composite`）**展开为子图容器**：inner 的子状态作为普通节点，
//! 复合状态本身作为 [`UGSubgraph`] 容器（标题 + 成员），inner 的 `[*] --> X` /
//! `Y --> [*]` 映射为复合状态的入口/出口节点，供外层转移边重写。

use std::collections::{HashMap, HashSet};

use crate::{
    ast::{State, StateDiagram, StateStmt},
    builder::ir::{
        self,
        common::{
            ArrowKind, ArrowSpec, EdgePriority, LabelSpec, PortHint, PortSet, SizeHint, StyleRef,
        },
        shape::ShapeKind,
        unigraph::{EdgeKind, UGEdge, UGNode, UGSubgraph, Unigraph},
    },
};
use lievisual::geometry::Size;

/// 状态图默认从上到下布局（无方向声明）。
const DEFAULT_DIRECTION: crate::ast::Direction = crate::ast::Direction::TB;

// 状态特殊节点固定尺寸（对齐官方 golden）：
// - start：`<circle class="state-start" r="7" width="14" height="14"/>` → 14×14
// - end：外圈 + 实心内圈（官方外圈 r≈10）
// - fork / join：细长横条
const START_SIZE: Size = Size::new(14.0, 14.0);
const END_SIZE: Size = Size::new(20.0, 20.0);
const BAR_SIZE: Size = Size::new(100.0, 10.0);
/// state 节点 padding（官方实测：如 `Idle` 文本 25.8×24 → 41.8×40，故 padding=8）。
/// 与 flowchart 的圆角节点（padding 30/15）区分，二者同用 `ShapeKind::Rounded`。
const STATE_PAD: f64 = 8.0;

/// 节点形状 / 尺寸决策。
fn node_meta(_id: &str, is_start: bool, is_end: bool, is_bar: bool) -> (ShapeKind, SizeHint) {
    if is_start {
        (ShapeKind::StartDot, SizeHint::Fixed(START_SIZE))
    } else if is_end {
        (ShapeKind::EndDot, SizeHint::Fixed(END_SIZE))
    } else if is_bar {
        (ShapeKind::Bar, SizeHint::Fixed(BAR_SIZE))
    } else {
        (
            ShapeKind::Rounded,
            SizeHint::Padded { pad_x: STATE_PAD, pad_y: STATE_PAD },
        )
    }
}

/// 入队一个节点（去重）。无显式描述时用节点 id 作为默认标签（官方行为），
/// start/end/fork 等无文本形状保持空。
fn push_node(
    nodes: &mut Vec<UGNode>,
    seen: &mut HashSet<String>,
    id: String,
    label: Option<String>,
    shape: ShapeKind,
    size_hint: SizeHint,
) {
    if seen.contains(&id) {
        return;
    }
    seen.insert(id.clone());
    let text = match shape {
        ShapeKind::StartDot | ShapeKind::EndDot | ShapeKind::Bar => label.unwrap_or_default(),
        _ => label.unwrap_or_else(|| id.clone()),
    };
    nodes.push(UGNode {
        id,
        kind: ir::common::NodeKind::Atom,
        role: ir::common::NodeRole::Atom,
        shape,
        label: ir::common::LabelOrMeasured::Spec(LabelSpec { text, spans: Vec::new() }),
        ports: PortSet::default(),
        size_hint,
        style_ref: StyleRef::NodeDefault,
        constraint: ir::common::NodeConstraint::Free,
        detail: ir::common::NodeDetail::None,
    });
}

/// 求出「先被转移引用、之后才被 `<<fork>>`/`<<join>>` 声明」的状态 id 集合。
///
/// mermaid 的状态库按源码顺序建节点：一个 id 一旦由转移创建为普通状态，后续的
/// `<<fork>>` / `<<join>>` 声明**不会**把它改成横条
/// （官方 `state__fork_join`：`fork_state` 是横条，`join_state` 是带标签的普通框）。
/// 这些 id 在 extract 中降级为普通状态。
fn transition_seen_first(sd: &StateDiagram) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut first_is_trans: HashSet<String> = HashSet::new();
    for stmt in &sd.order {
        match stmt {
            StateStmt::Decl(i) => {
                let Some(s) = sd.states.get(*i) else { continue };
                let id = match s {
                    State::Simple { id, .. }
                    | State::Composite { id, .. }
                    | State::Fork { id }
                    | State::Join { id } => id.as_str(),
                    State::Start | State::End => continue,
                };
                // 该 id 首次出现是转移 → 声明无法覆盖类型。
                if first_is_trans.contains(id) {
                    out.insert(id.to_string());
                }
            }
            StateStmt::Trans(i) => {
                let Some(t) = sd.transitions.get(*i) else { continue };
                for id in [&t.from, &t.to] {
                    if *id != "[*]" {
                        first_is_trans.insert(id.clone());
                    }
                }
            }
        }
    }
    out
}

/// 提取 state 图为统一拓扑图（复合状态展开为子图容器）。
pub fn extract_state(sd: &StateDiagram) -> Unigraph {
    let mut nodes: Vec<UGNode> = Vec::new();
    let mut edges: Vec<UGEdge> = Vec::new();
    let mut subgraphs: Vec<UGSubgraph> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut edge_counter: usize = 0;

    // 收集显式 Simple 状态的描述文本（作为 label，含嵌套）。
    let mut labels: HashMap<String, Option<String>> = HashMap::new();
    fn collect_labels(sd: &StateDiagram, labels: &mut HashMap<String, Option<String>>) {
        for s in &sd.states {
            match s {
                State::Simple { id, description } => {
                    labels.insert(id.clone(), description.clone());
                }
                State::Composite { inner, .. } => collect_labels(inner, labels),
                _ => {}
            }
        }
    }
    collect_labels(sd, &mut labels);

    // 复合状态 id → (入口节点, 出口节点)。
    let mut composite_entry_exit: HashMap<String, (Option<String>, Option<String>)> = HashMap::new();

    // 递归收集一个 StateDiagram。返回 (入口节点, 出口节点)。
    fn collect(
        sd: &StateDiagram,
        nodes: &mut Vec<UGNode>,
        edges: &mut Vec<UGEdge>,
        subgraphs: &mut Vec<UGSubgraph>,
        seen: &mut HashSet<String>,
        labels: &HashMap<String, Option<String>>,
        composite_entry_exit: &mut HashMap<String, (Option<String>, Option<String>)>,
        edge_counter: &mut usize,
        is_inner: bool,
        degraded: &HashSet<String>,
    ) -> (Option<String>, Option<String>) {
        let mut entry: Option<String> = None;
        let mut exit: Option<String> = None;

        // 1. 显式 states。
        for s in &sd.states {
            match s {
                State::Simple { id, .. } => {
                    let (shape, hint) = node_meta(id, false, false, false);
                    push_node(nodes, seen, id.clone(), labels.get(id).cloned().flatten(), shape, hint);
                }
                State::Composite { id, inner } => {
                    let start_idx = nodes.len();
                    let (e, x) = collect(
                        inner, nodes, edges, subgraphs, seen, labels, composite_entry_exit,
                        edge_counter, true, degraded,
                    );
                    let member_ids: Vec<String> =
                        nodes[start_idx..].iter().map(|n| n.id.clone()).collect();
                    subgraphs.push(UGSubgraph {
                        id: id.clone(),
                        title: Some(id.clone()),
                        member_ids,
                        kind: ir::common::ContainerKind::StateComposite,
                    });
                    composite_entry_exit.insert(id.clone(), (e.clone(), x.clone()));
                    entry = e;
                    exit = x;
                }
                State::Fork { id } | State::Join { id } => {
                    if degraded.contains(id) {
                        // 官方行为：该 id 已先被转移创建为普通状态，`<<fork>>`/`<<join>>`
                        // 声明不再生效 → 退化为带标签的普通状态框（id 作默认标签）。
                        let (shape, hint) = node_meta(id, false, false, false);
                        push_node(
                            nodes, seen, id.clone(),
                            labels.get(id).cloned().flatten(), shape, hint,
                        );
                    } else {
                        let (shape, hint) = node_meta(id, false, false, true);
                        push_node(nodes, seen, id.clone(), None, shape, hint);
                    }
                }
                State::Start => {
                    push_node(
                        nodes, seen, "__start__".to_string(), None, ShapeKind::StartDot,
                        SizeHint::Fixed(START_SIZE),
                    );
                }
                State::End => {
                    push_node(
                        nodes, seen, "__end__".to_string(), None, ShapeKind::EndDot,
                        SizeHint::Fixed(END_SIZE),
                    );
                }
            }
        }

        // 2. transitions。
        for t in &sd.transitions {
            let from_is_star = t.from == "[*]";
            let to_is_star = t.to == "[*]";

            // 复合状态内部的 [*] 是入口/出口标记，不生成边。
            if is_inner && from_is_star {
                entry = Some(t.to.clone());
                let (shape, hint) = node_meta(&t.to, false, false, false);
                push_node(nodes, seen, t.to.clone(), labels.get(&t.to).cloned().flatten(), shape, hint);
                continue;
            }
            if is_inner && to_is_star {
                exit = Some(t.from.clone());
                let (shape, hint) = node_meta(&t.from, false, false, false);
                push_node(
                    nodes, seen, t.from.clone(), labels.get(&t.from).cloned().flatten(), shape, hint,
                );
                continue;
            }

            // 外层 [*] → 全局 start/end。
            let mut from = if from_is_star { "__start__".to_string() } else { t.from.clone() };
            let mut to = if to_is_star { "__end__".to_string() } else { t.to.clone() };

            // 引用复合状态的边，重写到其入口/出口节点：
            // source 是复合状态 → 从出口节点离开；target 是复合状态 → 进入入口节点。
            if let Some((_, Some(x))) = composite_entry_exit.get(&from) {
                from = x.clone();
            }
            if let Some((Some(e), _)) = composite_entry_exit.get(&to) {
                to = e.clone();
            }

            let (shape_f, hint_f) = node_meta(&from, from_is_star, false, false);
            push_node(nodes, seen, from.clone(), labels.get(&from).cloned().flatten(), shape_f, hint_f);
            let (shape_t, hint_t) = node_meta(&to, false, to_is_star, false);
            push_node(nodes, seen, to.clone(), labels.get(&to).cloned().flatten(), shape_t, hint_t);

            edges.push(UGEdge {
                id: format!("t{}", *edge_counter),
                source: from,
                target: to,
                source_port: PortHint::Bottom,
                target_port: PortHint::Top,
                kind: EdgeKind::StateTransition,
                label_text: t.label.clone(),
                label: None,
                priority: EdgePriority::Primary,
                // 官方 state 转移线是曲线（transition path 含 C 贝塞尔）。
                routing_hint: ir::common::RoutingHint::Spline,
                arrow: ArrowSpec { start: ArrowKind::None, end: ArrowKind::Arrow },
                line_kind: ir::common::LineKind::Solid,
                repulsion: 1.0,
                cardinality: (None, None),
                cardinality_text: (None, None),
            });
            *edge_counter += 1;
        }

        (entry, exit)
    }

    let degraded = transition_seen_first(sd);
    collect(
        sd, &mut nodes, &mut edges, &mut subgraphs, &mut seen, &labels,
        &mut composite_entry_exit, &mut edge_counter, false, &degraded,
    );

    // 保证所有被边引用的节点都存在。
    for e in &edges {
        if !seen.contains(&e.source) {
            let (shape, hint) = node_meta(&e.source, false, false, false);
            push_node(
                &mut nodes, &mut seen, e.source.clone(), labels.get(&e.source).cloned().flatten(),
                shape, hint,
            );
        }
        if !seen.contains(&e.target) {
            let (shape, hint) = node_meta(&e.target, false, false, false);
            push_node(
                &mut nodes, &mut seen, e.target.clone(), labels.get(&e.target).cloned().flatten(),
                shape, hint,
            );
        }
    }

    Unigraph {
        family: ir::unigraph::GraphFamily::Directed,
        direction: DEFAULT_DIRECTION,
        nodes,
        edges,
        subgraphs,
        sequence_rows: None,
        meta: ir::common::DiagramMeta { title: None, show_data: false },
    }
}
