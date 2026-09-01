//! ER 图的 extract：把 [`crate::ast::ErDiagram`] 翻译成 [`Unigraph`]。
//!
//! family = Grid（网格布局）。节点 = 实体框（[`NodeDetail::Entity`]），
//! 边 = 关系（[`EdgeKind::Generic`] + 两端基数 [`ErCardinality`]）。
//!
//! 基数符号（`||` / `|o` / `}|` / `}o`）由 materialize 据 `cardinality` 绘制，
//! extract 只填拓扑 + 语义。

use crate::{
    ast::{Cardinality, ErDiagram},
    builder::ir::{
        self,
        common::{
            ArrowKind, ArrowSpec, EdgePriority, EntityAttr, ErCardinality, LabelSpec, LineKind,
            NodeDetail, PortHint, PortSet, SizeHint, StyleRef,
        },
        shape::ShapeKind,
        unigraph::{EdgeKind, UGEdge, UGNode, Unigraph},
    },
};

/// ast::Cardinality → IR 基数（全项目唯一真相源）。
pub fn map_cardinality(c: &Cardinality) -> ErCardinality {
    match c {
        Cardinality::ZeroOrOne => ErCardinality::ZeroOrOne,
        Cardinality::ExactlyOne => ErCardinality::ExactlyOne,
        Cardinality::ZeroOrMany => ErCardinality::ZeroOrMany,
        Cardinality::OneOrMany => ErCardinality::OneOrMany,
    }
}

/// 提取 ER 图为统一拓扑图（family = Grid）。
pub fn extract_er(ed: &ErDiagram) -> Unigraph {
    let mut nodes: Vec<UGNode> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 实体顺序：显式声明在前 + 关系隐含实体按出现顺序追加（与旧 render/er.rs 一致）。
    let push_entity = |nodes: &mut Vec<UGNode>,
                       seen: &mut std::collections::HashSet<String>,
                       name: &str,
                       attrs: Vec<EntityAttr>| {
        if seen.contains(name) {
            return;
        }
        seen.insert(name.to_string());
        nodes.push(UGNode {
            id: name.to_string(),
            kind: ir::common::NodeKind::Atom,
            role: ir::common::NodeRole::Atom,
            shape: ShapeKind::Rectangle,
            label: ir::common::LabelOrMeasured::Spec(LabelSpec {
                text: name.to_string(),
                spans: Vec::new(),
            }),
            ports: PortSet::default(),
            size_hint: SizeHint::ByText,
            style_ref: StyleRef::NodeDefault,
            constraint: ir::common::NodeConstraint::Free,
            detail: NodeDetail::Entity { attrs },
        });
    };

    for e in &ed.entities {
        let attrs = e
            .attributes
            .iter()
            .map(|a| EntityAttr {
                type_: a.type_.clone(),
                name: a.name.clone(),
                constraint: a.constraint.clone(),
            })
            .collect();
        push_entity(&mut nodes, &mut seen, &e.name, attrs);
    }
    for r in &ed.relationships {
        for name in [&r.first_entity, &r.second_entity] {
            push_entity(&mut nodes, &mut seen, name, Vec::new());
        }
    }

    // 关系边（Generic + 两端基数）。
    let mut edges: Vec<UGEdge> = Vec::new();
    for (i, r) in ed.relationships.iter().enumerate() {
        edges.push(UGEdge {
            id: format!("r{}", i),
            source: r.first_entity.clone(),
            target: r.second_entity.clone(),
            source_port: PortHint::Auto,
            target_port: PortHint::Auto,
            kind: EdgeKind::Generic,
            label_text: r.label.clone(),
            label: None,
            priority: EdgePriority::Primary,
            routing_hint: ir::common::RoutingHint::Orthogonal,
            arrow: ArrowSpec {
                start: ArrowKind::None,
                end: ArrowKind::None,
            },
            line_kind: LineKind::Solid,
            repulsion: 1.0,
            cardinality: (
                Some(map_cardinality(&r.cardinality_first)),
                Some(map_cardinality(&r.cardinality_second)),
            ),
            cardinality_text: (None, None),
        });
    }

    Unigraph {
        family: ir::unigraph::GraphFamily::Grid,
        direction: crate::ast::Direction::TB,
        nodes,
        edges,
        subgraphs: Vec::new(),
        sequence_rows: None,
        meta: ir::common::DiagramMeta {
            title: None,
            show_data: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_cardinality() {
        assert_eq!(
            map_cardinality(&Cardinality::ZeroOrOne),
            ErCardinality::ZeroOrOne
        );
        assert_eq!(
            map_cardinality(&Cardinality::ExactlyOne),
            ErCardinality::ExactlyOne
        );
        assert_eq!(
            map_cardinality(&Cardinality::ZeroOrMany),
            ErCardinality::ZeroOrMany
        );
        assert_eq!(
            map_cardinality(&Cardinality::OneOrMany),
            ErCardinality::OneOrMany
        );
    }

    #[test]
    fn extracts_entities_and_cardinality() {
        let ed = ErDiagram {
            entities: vec![crate::ast::ErEntity {
                name: "Customer".into(),
                attributes: vec![crate::ast::ErAttribute {
                    type_: "int".into(),
                    name: "id".into(),
                    constraint: None,
                }],
            }],
            relationships: vec![crate::ast::ErRelationship {
                first_entity: "Customer".into(),
                second_entity: "Order".into(),
                cardinality_first: Cardinality::ExactlyOne,
                cardinality_second: Cardinality::ZeroOrMany,
                label: Some("places".into()),
            }],
        };

        let ug = extract_er(&ed);
        assert_eq!(ug.family, crate::builder::ir::unigraph::GraphFamily::Grid);
        // 实体：Customer（显式）+ Order（隐含）。
        assert_eq!(ug.nodes.len(), 2);
        assert_eq!(ug.nodes[0].id, "Customer");
        assert_eq!(ug.nodes[1].id, "Order");
        assert_eq!(ug.edges.len(), 1);
        assert_eq!(ug.edges[0].kind, EdgeKind::Generic);
        assert_eq!(
            ug.edges[0].cardinality,
            (
                Some(ErCardinality::ExactlyOne),
                Some(ErCardinality::ZeroOrMany)
            )
        );
    }
}
