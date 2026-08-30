//! Timeline 的 extract：把 [`crate::ast::TimelineDiagram`] 翻译成 [`Unigraph`]。
//!
//! family = Linear：每个 section 一个节点（`NodeDetail::TimelineSection` 携带事件文本），
//! 无节点间边。布局阶段（engine Linear 分支）按方向（LR 默认 / TD）把各列线性排布，
//! materialize 据列几何画时间轴 / 时间点 / section 块 / 事件块 / 连线。

use crate::{
    ast::{TimelineDiagram, TimelineDirection},
    builder::ir::{
        common::{
            DiagramMeta, LabelOrMeasured, LabelSpec, NodeConstraint, NodeDetail, NodeKind,
            NodeRole, PortSet, SizeHint, StyleRef,
        },
        shape::ShapeKind,
        unigraph::{GraphFamily, UGNode, Unigraph},
    },
};

/// 提取 timeline 为统一拓扑图（Linear 家族，无边的列式布局）。
pub fn extract_timeline(td: &TimelineDiagram) -> Unigraph {
    let nodes: Vec<UGNode> = td
        .sections
        .iter()
        .enumerate()
        .map(|(i, sec)| UGNode {
            id: format!("sec{}", i),
            kind: NodeKind::Atom,
            role: NodeRole::Atom,
            shape: ShapeKind::Rounded,
            label: LabelOrMeasured::Spec(LabelSpec {
                text: sec.name.clone(),
                spans: Vec::new(), // measure 阶段填充 RichSpan
            }),
            ports: PortSet::default(),
            size_hint: SizeHint::ByText,
            style_ref: StyleRef::NodeDefault,
            constraint: NodeConstraint::Free,
            detail: NodeDetail::TimelineSection {
                events: sec.events.clone(),
            },
        })
        .collect();

    Unigraph {
        family: GraphFamily::Linear,
        // 时间轴默认水平（LR），方向决定列推进主轴。
        direction: match td.direction {
            None | Some(TimelineDirection::LR) => crate::ast::Direction::LR,
            Some(TimelineDirection::TD) => crate::ast::Direction::TD,
        },
        nodes,
        edges: Vec::new(),
        subgraphs: Vec::new(),
        sequence_rows: None,
        meta: DiagramMeta {
            title: td.title.clone(),
            show_data: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ir::unigraph::GraphFamily;

    fn parse(src: &str) -> TimelineDiagram {
        crate::MermaidParser::parse_mermaid(src)
            .map(|d| match d {
                crate::ast::Diagram::Timeline(t) => t,
                _ => panic!("not a timeline"),
            })
            .expect("parse")
    }

    #[test]
    fn extract_basic_sections_and_events() {
        // 语法：`section Name` + `time : event1 : event2`（时间标记作为首个事件）。
        let td = parse(
            "timeline\n    title History\n    section A\n    1900 : e1 : e2\n    section B\n    2000 : e3\n",
        );
        let ug = extract_timeline(&td);
        assert_eq!(ug.family, GraphFamily::Linear);
        assert_eq!(ug.nodes.len(), 2, "两个 section 各一个节点");
        assert_eq!(ug.meta.title.as_deref(), Some("History"));
        // 方向默认 LR（水平时间轴）。
        assert_eq!(ug.direction, crate::ast::Direction::LR);
        match &ug.nodes[0].detail {
            NodeDetail::TimelineSection { events } => {
                let label_text = match &ug.nodes[0].label {
                    LabelOrMeasured::Spec(s) => s.text.clone(),
                    LabelOrMeasured::Measured(m) => m.text.clone(),
                };
                assert_eq!(label_text, "A");
                // 事件 = [时间标记] + 冒号分隔事件。
                assert_eq!(
                    events,
                    &vec!["1900".to_string(), "e1".to_string(), "e2".to_string()]
                );
            }
            other => panic!("期望 TimelineSection, got {other:?}"),
        }
        assert!(ug.edges.is_empty(), "timeline 无节点间边");
    }

    #[test]
    fn extract_td_direction() {
        // 方向紧跟 `timeline`（`timeline TD` 或换行后 `TD`）。
        let td = parse("timeline TD\nsection A\n2000 : e1\n");
        let ug = extract_timeline(&td);
        assert_eq!(ug.direction, crate::ast::Direction::TD);
    }
}
