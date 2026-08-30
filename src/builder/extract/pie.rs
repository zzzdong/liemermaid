//! Pie 的 extract：把 [`crate::ast::PieDiagram`] 翻译成 [`Unigraph`]。
//!
//! family = Radial：每个数据项一个节点（`NodeDetail::PieSlice` 携带标签与数值），
//! 节点不占位置（engine 全部叠于原点），materialize 据数据计算扇区角度并绘制。

use crate::{
    ast::PieDiagram,
    builder::ir::{
        common::{
            DiagramMeta, LabelOrMeasured, LabelSpec, NodeConstraint, NodeDetail, NodeKind,
            NodeRole, PortSet, SizeHint, StyleRef,
        },
        shape::ShapeKind,
        unigraph::{GraphFamily, UGNode, Unigraph},
    },
    error::{DiagramError, DiagramResult},
};

/// 提取 pie 为统一拓扑图（Radial 家族）。
pub fn extract_pie(pie: &PieDiagram) -> DiagramResult<Unigraph> {
    let mut nodes: Vec<UGNode> = Vec::new();
    for (i, d) in pie.data.iter().enumerate() {
        let value: f64 = d
            .value
            .parse()
            .map_err(|_| DiagramError::LayoutError(format!("invalid pie value: {}", d.value)))?;
        nodes.push(UGNode {
            id: format!("slice{}", i),
            kind: NodeKind::Atom,
            role: NodeRole::Atom,
            shape: ShapeKind::PieSlice,
            label: LabelOrMeasured::Spec(LabelSpec {
                text: d.label.clone(),
                spans: Vec::new(),
            }),
            ports: PortSet::default(),
            size_hint: SizeHint::ByText,
            style_ref: StyleRef::NodeDefault,
            constraint: NodeConstraint::Free,
            detail: NodeDetail::PieSlice {
                label: d.label.clone(),
                value,
            },
        });
    }
    Ok(Unigraph {
        family: GraphFamily::Radial,
        direction: crate::ast::Direction::TB,
        nodes,
        edges: Vec::new(),
        subgraphs: Vec::new(),
        sequence_rows: None,
        meta: DiagramMeta {
            title: pie.title.clone(),
            show_data: pie.show_data,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> PieDiagram {
        crate::MermaidParser::parse_mermaid(src)
            .map(|d| match d {
                crate::ast::Diagram::Pie(p) => p,
                _ => panic!("not a pie"),
            })
            .expect("parse")
    }

    #[test]
    fn extract_slices_and_meta() {
        let pie = parse("pie\ntitle My Pie\n\"A\": 30\n\"B\": 50");
        let ug = extract_pie(&pie).expect("extract");
        assert_eq!(ug.family, GraphFamily::Radial);
        assert_eq!(ug.nodes.len(), 2);
        match &ug.nodes[0].detail {
            NodeDetail::PieSlice { label, value } => {
                assert_eq!(label, "A");
                assert_eq!(*value, 30.0);
            }
            other => panic!("期望 PieSlice, got {other:?}"),
        }
        assert_eq!(ug.meta.title.as_deref(), Some("My Pie"));
        assert!(!ug.meta.show_data);
    }

    #[test]
    fn extract_invalid_value_errors() {
        let pie = parse("pie\n\"A\": abc");
        assert!(extract_pie(&pie).is_err());
    }
}
