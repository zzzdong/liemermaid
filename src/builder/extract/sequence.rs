//! Sequence 的 extract：把 [`crate::ast::SequenceDiagram`] 翻译成 [`Unigraph`]。
//!
//! family = Sequence：
//! - participant → 节点（`NodeRole::Lifeline`，id = 参与者名，label = alias 或名）；
//! - 语句（消息 / 备注 / 分组块）→ `Unigraph.sequence_rows`（[`SequenceRow`]），
//!   按源码序保留纵向行序与块嵌套；消息同时产出 `EdgeKind::SequenceMessage` 边，
//!   备注产出 `NodeDetail::SequenceNote` 节点（engine 据行序与目标列定位）。
//!
//! 消息的激活标记（`->>+B`）与泳道属性留待后续（引擎步骤）。

use std::collections::HashMap;

use crate::{
    ast::{
        MessageActivation, MessageArrow, NotePlacement, SequenceBlock, SequenceBlockKind,
        SequenceDiagram, SequenceItem, SequenceStatement,
    },
    builder::ir::{
        common::{
            ArrowKind, ArrowSpec, DiagramMeta, EdgePriority, LabelOrMeasured, LabelSpec, LineKind,
            NodeConstraint, NodeDetail, NodeKind, NodeRole, PortHint, PortSet, RoutingHint,
            SequenceNotePlacement, SizeHint, StyleRef,
        },
        shape::ShapeKind,
        unigraph::{EdgeKind, GraphFamily, SequenceRow, UGEdge, UGNode, Unigraph},
    },
};

/// 消息箭头 → IR 箭头规格。
pub fn map_message_arrow(arrow: &MessageArrow) -> ArrowSpec {
    let (start, end) = match arrow {
        MessageArrow::Solid
        | MessageArrow::SolidTip
        | MessageArrow::Dashed
        | MessageArrow::DashedTip
        | MessageArrow::Open => (ArrowKind::None, ArrowKind::Arrow),
        MessageArrow::Cross => (ArrowKind::None, ArrowKind::Cross),
        MessageArrow::Both => (ArrowKind::Arrow, ArrowKind::Arrow),
    };
    ArrowSpec { start, end }
}

/// 消息箭头 → IR 线型（实线 / 虚线）。
pub fn map_message_line(arrow: &MessageArrow) -> LineKind {
    match arrow {
        MessageArrow::Dashed | MessageArrow::DashedTip => LineKind::Dotted,
        _ => LineKind::Solid,
    }
}

/// 分组块标签（`loop [label]` / `alt [label]` / `opt [label]` / `par [label]`）。
fn block_label(block: &SequenceBlock) -> String {
    let prefix = match block.kind {
        SequenceBlockKind::Loop => "loop",
        SequenceBlockKind::Alt => "alt",
        SequenceBlockKind::Opt => "opt",
        SequenceBlockKind::Par => "par",
        SequenceBlockKind::Critical => "critical",
        SequenceBlockKind::Break => "break",
        SequenceBlockKind::Rect => "rect",
    };
    match &block.label {
        Some(l) if !l.trim().is_empty() => format!("{} [{}]", prefix, l.trim()),
        _ => prefix.to_string(),
    }
}

/// 保证某参与者存在（缺失时补建节点，如消息引用了未声明的参与者）。
fn ensure_participant(
    nodes: &mut Vec<UGNode>,
    idx_of: &mut HashMap<String, usize>,
    name: &str,
    alias: Option<&str>,
) -> usize {
    if let Some(&i) = idx_of.get(name) {
        return i;
    }
    let i = nodes.len();
    idx_of.insert(name.to_string(), i);
    let display = alias.unwrap_or(name).to_string();
    nodes.push(UGNode {
        id: name.to_string(),
        kind: NodeKind::Atom,
        role: NodeRole::Lifeline,
        shape: ShapeKind::Rounded,
        label: LabelOrMeasured::Spec(LabelSpec {
            text: display,
            spans: Vec::new(),
        }),
        ports: PortSet::default(),
        size_hint: SizeHint::ByText,
        style_ref: StyleRef::NodeDefault,
        constraint: NodeConstraint::Free,
        detail: NodeDetail::None,
    });
    i
}

/// 递归收集语句行：消息 → 边 + `SequenceRow::Message`；备注 → 节点 + `SequenceRow::Note`；
/// 分组块 → `BlockStart`/`BlockEnd` 包裹块内语句（块 id 递增）。
#[allow(clippy::too_many_arguments)]
fn collect_items(
    items: &[SequenceItem],
    nodes: &mut Vec<UGNode>,
    idx_of: &mut HashMap<String, usize>,
    edges: &mut Vec<UGEdge>,
    rows: &mut Vec<SequenceRow>,
    edge_counter: &mut usize,
    note_counter: &mut usize,
    block_counter: &mut usize,
) {
    for item in items {
        match item {
            SequenceItem::Message(m) => {
                // 消息引用未声明参与者时补建（与旧渲染默认下标 0 对齐的兜底）。
                ensure_participant(nodes, idx_of, &m.from, None);
                ensure_participant(nodes, idx_of, &m.to, None);
                let eid = format!("m{}", *edge_counter);
                edges.push(UGEdge {
                    id: eid.clone(),
                    source: m.from.clone(),
                    target: m.to.clone(),
                    source_port: PortHint::Auto,
                    target_port: PortHint::Auto,
                    kind: EdgeKind::SequenceMessage,
                    label_text: m.text.clone(),
                    label: None,
                    priority: EdgePriority::Primary,
                    routing_hint: RoutingHint::Orthogonal,
                    arrow: map_message_arrow(&m.arrow),
                    line_kind: map_message_line(&m.arrow),
                    repulsion: 1.0,
                    cardinality: (None, None),
                    cardinality_text: (None, None),
                });
                *edge_counter += 1;
                rows.push(SequenceRow::Message(eid));
                // 激活标记（`->>+` / `->>-`）紧跟在消息行之后（engine 取其前一行的 y
                // 作为激活条起点）。作用对象按官方语义：
                // - `A->>+B`：**激活目标** B；
                // - `A-->>-B`：**取消激活源** A（注意不是 B）。
                if let Some(act) = m.activation {
                    let actor = match act {
                        MessageActivation::Activate => m.to.clone(),
                        MessageActivation::Deactivate => m.from.clone(),
                    };
                    rows.push(SequenceRow::Activation {
                        actor,
                        on: matches!(act, MessageActivation::Activate),
                    });
                }
            }
            SequenceItem::Note(n) => {
                let nid = format!("note{}", *note_counter);
                *note_counter += 1;
                let placement = match n.placement {
                    NotePlacement::Over => SequenceNotePlacement::Over,
                    NotePlacement::LeftOf => SequenceNotePlacement::LeftOf,
                    NotePlacement::RightOf => SequenceNotePlacement::RightOf,
                };
                nodes.push(UGNode {
                    id: nid.clone(),
                    kind: NodeKind::Atom,
                    role: NodeRole::Virtual,
                    shape: ShapeKind::Rounded,
                    label: LabelOrMeasured::Spec(LabelSpec {
                        text: String::new(),
                        spans: Vec::new(),
                    }),
                    ports: PortSet::default(),
                    size_hint: SizeHint::ByText,
                    style_ref: StyleRef::NodeDefault,
                    constraint: NodeConstraint::Free,
                    detail: NodeDetail::SequenceNote {
                        text: n.text.clone(),
                        targets: n.targets.clone(),
                        placement,
                    },
                });
                rows.push(SequenceRow::Note(nid));
            }
            SequenceItem::Block(b) => {
                let bid = format!("block{}", *block_counter);
                *block_counter += 1;
                rows.push(SequenceRow::BlockStart(bid.clone(), block_label(b)));
                for (i, branch) in b.branches.iter().enumerate() {
                    // 分支之间插入分隔行（alt 的 else / par 的 and / critical 的 option），
                    // 文本与官方 sectionTitle 对齐：`[条件]`。
                    if i > 0 {
                        let div_label = branch
                            .label
                            .as_deref()
                            .map(|l| format!("[{}]", l.trim()))
                            .unwrap_or_default();
                        rows.push(SequenceRow::BlockDivider(bid.clone(), div_label));
                    }
                    collect_items(
                        &branch.items,
                        nodes,
                        idx_of,
                        edges,
                        rows,
                        edge_counter,
                        note_counter,
                        block_counter,
                    );
                }
                rows.push(SequenceRow::BlockEnd(bid));
            }
        }
    }
}

/// 提取 sequence 为统一拓扑图（Sequence 家族）。
pub fn extract_sequence(seq: &SequenceDiagram) -> Unigraph {
    let mut nodes: Vec<UGNode> = Vec::new();
    let mut idx_of: HashMap<String, usize> = HashMap::new();

    // 声明的参与者（含 alias）。
    for p in &seq.participants {
        ensure_participant(&mut nodes, &mut idx_of, &p.name, p.alias.as_deref());
    }

    let mut edges: Vec<UGEdge> = Vec::new();
    let mut rows: Vec<SequenceRow> = Vec::new();
    let mut edge_counter = 0usize;
    let mut note_counter = 0usize;
    let mut block_counter = 0usize;

    // 顶层语句统一转成 SequenceItem 后递归收集。
    let mut items: Vec<SequenceItem> = Vec::new();
    for stmt in &seq.statements {
        match stmt {
            SequenceStatement::Message(m) => items.push(SequenceItem::Message(m.clone())),
            SequenceStatement::Note(n) => items.push(SequenceItem::Note(n.clone())),
            SequenceStatement::Block(b) => items.push(SequenceItem::Block(b.clone())),
        }
    }
    collect_items(
        &items,
        &mut nodes,
        &mut idx_of,
        &mut edges,
        &mut rows,
        &mut edge_counter,
        &mut note_counter,
        &mut block_counter,
    );

    Unigraph {
        family: GraphFamily::Sequence,
        direction: crate::ast::Direction::LR,
        nodes,
        edges,
        subgraphs: Vec::new(),
        sequence_rows: Some(rows),
        meta: DiagramMeta {
            title: None,
            show_data: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ir::unigraph::GraphFamily;

    fn parse(src: &str) -> SequenceDiagram {
        crate::MermaidParser::parse_mermaid(src)
            .map(|d| match d {
                crate::ast::Diagram::Sequence(s) => s,
                _ => panic!("not a sequence"),
            })
            .expect("parse")
    }

    #[test]
    fn extract_participants_as_lifeline_nodes() {
        let seq = parse(
            "sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n    A->>B: hi\n",
        );
        let ug = extract_sequence(&seq);
        assert_eq!(ug.family, GraphFamily::Sequence);
        assert_eq!(ug.nodes.len(), 2, "两个参与者节点");
        assert!(ug.nodes.iter().all(|n| n.role == NodeRole::Lifeline));
        let alice = ug.nodes.iter().find(|n| n.id == "A").unwrap();
        let text = match &alice.label {
            LabelOrMeasured::Spec(s) => s.text.clone(),
            LabelOrMeasured::Measured(m) => m.text.clone(),
        };
        assert_eq!(text, "Alice");
    }

    #[test]
    fn extract_messages_as_ordered_rows() {
        let seq = parse("sequenceDiagram\n    A->>B: one\n    B-->>A: two\n    A->>B\n");
        let ug = extract_sequence(&seq);
        assert_eq!(ug.edges.len(), 3, "三条消息边，顺序保留");
        assert_eq!(ug.edges[0].kind, EdgeKind::SequenceMessage);
        assert_eq!(ug.edges[0].label_text.as_deref(), Some("one"));
        // 行序 = 源码序。
        let rows = ug.sequence_rows.clone().unwrap();
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], SequenceRow::Message(ref id) if id == "m0"));
        assert!(matches!(rows[2], SequenceRow::Message(ref id) if id == "m2"));
        // 虚线消息（-->>）→ Dotted。
        assert_eq!(ug.edges[1].line_kind, LineKind::Dotted);
    }

    #[test]
    fn extract_notes_and_blocks() {
        let seq = parse(
            "sequenceDiagram\n    A->>B: x\n    Note over A,B: shared note\n    loop retry\n        A->>B: again\n        B-->>A: ack\n    end\n    Note right of A: side note\n",
        );
        let ug = extract_sequence(&seq);
        // 2 参与者 + 2 备注节点。
        let note_count = ug
            .nodes
            .iter()
            .filter(|n| matches!(n.detail, NodeDetail::SequenceNote { .. }))
            .count();
        assert_eq!(note_count, 2, "两个备注节点");
        assert_eq!(ug.edges.len(), 3, "顶层 1 条 + 循环块内 2 条");
        // 行序：消息、备注、块起、块内消息×2、块止、备注。
        let rows = ug.sequence_rows.clone().unwrap();
        let kinds: Vec<&str> = rows
            .iter()
            .map(|r| match r {
                SequenceRow::Message(_) => "msg",
                SequenceRow::Note(_) => "note",
                SequenceRow::BlockStart(_, l) => {
                    assert_eq!(l, "loop [retry]");
                    "block-start"
                }
                SequenceRow::BlockEnd(_) => "block-end",
                SequenceRow::BlockDivider(..) => "block-divider",
                SequenceRow::Activation { on, .. } => {
                    if *on {
                        "activate"
                    } else {
                        "deactivate"
                    }
                }
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "msg",
                "note",
                "block-start",
                "msg",
                "msg",
                "block-end",
                "note"
            ]
        );
    }

    #[test]
    fn activations_emit_rows_after_their_message() {
        let seq = parse(
            "sequenceDiagram\n    A->>B: x\n    A->>+B: start\n    B-->>-A: done\n    A->>B: y\n",
        );
        let ug = extract_sequence(&seq);
        let rows = ug.sequence_rows.clone().unwrap();
        let kinds: Vec<&str> = rows
            .iter()
            .map(|r| match r {
                SequenceRow::Message(_) => "msg",
                SequenceRow::Note(_) => "note",
                SequenceRow::BlockStart(..) => "block-start",
                SequenceRow::BlockDivider(..) => "block-divider",
                SequenceRow::BlockEnd(_) => "block-end",
                SequenceRow::Activation { on, .. } => {
                    if *on {
                        "activate"
                    } else {
                        "deactivate"
                    }
                }
            })
            .collect();
        assert_eq!(
            kinds,
            vec!["msg", "msg", "activate", "msg", "deactivate", "msg"]
        );
        // 激活作用于消息**目标**参与者。
        let act = rows
            .iter()
            .find_map(|r| match r {
                SequenceRow::Activation { actor, on } if *on => Some(actor.as_str()),
                _ => None,
            })
            .unwrap();
        assert_eq!(act, "B");
        // `B-->>-A` 取消激活的是**源** B（官方语义，非目标 A）。
        let deact = rows
            .iter()
            .find_map(|r| match r {
                SequenceRow::Activation { actor, on } if !*on => Some(actor.as_str()),
                _ => None,
            })
            .unwrap();
        assert_eq!(deact, "B");
    }

    #[test]
    fn note_carries_text_targets_and_placement() {
        let seq = parse(
            "sequenceDiagram\n    A->>B: x\n    Note over A,B: shared note\n    loop retry\n        A->>B: again\n        B-->>A: ack\n    end\n    Note right of A: side note\n",
        );
        let ug = extract_sequence(&seq);
        let note = ug
            .nodes
            .iter()
            .find(|n| matches!(n.detail, NodeDetail::SequenceNote { .. }))
            .unwrap();
        match &note.detail {
            NodeDetail::SequenceNote {
                text,
                targets,
                placement,
            } => {
                assert_eq!(text, "shared note");
                assert_eq!(targets, &vec!["A".to_string(), "B".to_string()]);
                assert_eq!(*placement, SequenceNotePlacement::Over);
            }
            other => panic!("期望 SequenceNote, got {other:?}"),
        }
    }
}
