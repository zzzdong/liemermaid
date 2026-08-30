//! class 图的 extract：把 [`crate::ast::ClassDiagram`] 翻译成 [`Unigraph`]。
//!
//! family = Grid（网格布局）。节点 = 类框（结构化 [`NodeDetail::Class`]），
//! 边 = 关系（[`EdgeKind`] 映射到 ClassExtends / ClassComposition / ...）。
//!
//! 继承三角 / 组合菱形 / 聚合菱形等端点装饰由 materialize 据 [`EdgeKind`] 绘制，
//! extract 只填拓扑 + 语义，不填视觉。

use crate::{
    ast::{ClassDiagram, RelationKind, Visibility},
    builder::ir::{
        self,
        common::{
            ArrowKind, ArrowSpec, EdgePriority, LabelSpec, LineKind, NodeDetail, PortHint, PortSet,
            SizeHint, StyleRef,
        },
        shape::ShapeKind,
        unigraph::{EdgeKind, UGEdge, UGNode, Unigraph},
    },
};

/// 关系类别 → EdgeKind 映射（全项目唯一真相源）。
pub fn map_relation_kind(kind: &RelationKind) -> EdgeKind {
    match kind {
        RelationKind::Inheritance => EdgeKind::ClassExtends,
        RelationKind::Composition => EdgeKind::ClassComposition,
        RelationKind::Aggregation => EdgeKind::ClassAggregation,
        RelationKind::Association => EdgeKind::ClassAssociation,
        RelationKind::Dependency => EdgeKind::ClassDependency,
        RelationKind::Realization => EdgeKind::ClassRealization,
        RelationKind::Link => EdgeKind::ClassLink,
        RelationKind::Dashed => EdgeKind::ClassDashed,
    }
}

/// 格式化一个成员为显示行（与旧 render/class.rs 的输出一致）。
fn format_member(m: &crate::ast::ClassMember) -> String {
    let prefix = match m.visibility {
        Some(Visibility::Public) => "+",
        Some(Visibility::Private) => "-",
        Some(Visibility::Protected) => "#",
        Some(Visibility::Package) => "~",
        None => "",
    };
    // 官方 golden：`+String name` / `+int age` / `+eat()` / `+get() : String`
    // —— 可见性符号与类型之间**无空格**（类型与名称之间有一个空格）。
    if m.is_method {
        match &m.type_ {
            Some(ret) => format!("{}{}() : {}", prefix, m.name, ret),
            None => format!("{}{}()", prefix, m.name),
        }
    } else {
        match &m.type_ {
            Some(t) => format!("{}{} {}", prefix, t, m.name),
            None => format!("{}{}", prefix, m.name),
        }
    }
}

/// 提取 class 图为统一拓扑图（family = Grid）。
pub fn extract_class(cd: &ClassDiagram) -> Unigraph {
    let mut nodes: Vec<UGNode> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. 显式声明的类（源码序，去重）。
    for cls in &cd.classes {
        if seen.contains(&cls.name) {
            continue;
        }
        seen.insert(cls.name.clone());
        let display_name = match &cls.generic {
            Some(g) => format!("{}<{}>", cls.name, g),
            None => cls.name.clone(),
        };
        let attrs: Vec<String> = cls
            .members
            .iter()
            .filter(|m| !m.is_method)
            .map(format_member)
            .collect();
        let methods: Vec<String> = cls
            .members
            .iter()
            .filter(|m| m.is_method)
            .map(format_member)
            .collect();
        nodes.push(UGNode {
            id: cls.name.clone(),
            kind: ir::common::NodeKind::Atom,
            role: ir::common::NodeRole::Atom,
            shape: ShapeKind::Rectangle,
            label: ir::common::LabelOrMeasured::Spec(LabelSpec {
                text: display_name,
                spans: Vec::new(),
            }),
            ports: PortSet::default(),
            size_hint: SizeHint::ByText,
            style_ref: StyleRef::NodeDefault,
            constraint: ir::common::NodeConstraint::Free,
            detail: NodeDetail::Class {
                annotation: cls.annotation.clone(),
                attrs,
                methods,
            },
        });
    }

    // 2. 关系边；端点若未显式声明为类则补齐（防御，避免丢边）。
    let mut edges: Vec<UGEdge> = Vec::new();
    for (i, rel) in cd.relations.iter().enumerate() {
        for id in [&rel.source, &rel.target] {
            if !seen.contains(id.as_str()) {
                seen.insert(id.clone());
                nodes.push(UGNode {
                    id: id.clone(),
                    kind: ir::common::NodeKind::Atom,
                    role: ir::common::NodeRole::Atom,
                    shape: ShapeKind::Rectangle,
                    label: ir::common::LabelOrMeasured::Spec(LabelSpec {
                        text: id.clone(),
                        spans: Vec::new(),
                    }),
                    ports: PortSet::default(),
                    size_hint: SizeHint::ByText,
                    style_ref: StyleRef::NodeDefault,
                    constraint: ir::common::NodeConstraint::Free,
                    detail: NodeDetail::Class {
                        annotation: None,
                        attrs: Vec::new(),
                        methods: Vec::new(),
                    },
                });
            }
        }

        // 继承 / 组合 / 聚合的端点装饰（三角 / 菱形）在 source 端，由 materialize
        // 据 kind 绘制；关联 / 依赖的箭头在 target 端。
        let end_arrow = match rel.kind {
            RelationKind::Association | RelationKind::Dependency => ArrowKind::Arrow,
            _ => ArrowKind::None,
        };
        let line_kind = match rel.kind {
            RelationKind::Dependency | RelationKind::Dashed | RelationKind::Realization => {
                LineKind::Dotted
            }
            _ => LineKind::Solid,
        };
        edges.push(UGEdge {
            id: format!("r{}", i),
            source: rel.source.clone(),
            target: rel.target.clone(),
            source_port: PortHint::Auto,
            target_port: PortHint::Auto,
            kind: map_relation_kind(&rel.kind),
            label_text: rel.label.clone(),
            label: None,
            priority: EdgePriority::Primary,
            // 官方 class 关系线是曲线（path 贝塞尔），非正交折线。
            routing_hint: ir::common::RoutingHint::Spline,
            arrow: ArrowSpec {
                start: ArrowKind::None,
                end: end_arrow,
            },
            line_kind,
            repulsion: 1.0,
            cardinality: (None, None),
            cardinality_text: (
                rel.cardinality_first.clone(),
                rel.cardinality_second.clone(),
            ),
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
    use crate::ast::{Class, ClassMember};

    fn cls(name: &str, members: Vec<ClassMember>) -> Class {
        Class {
            name: name.to_string(),
            generic: None,
            annotation: None,
            members,
        }
    }

    #[test]
    fn extracts_nodes_and_details() {
        let cd = ClassDiagram {
            classes: vec![
                cls(
                    "Animal",
                    vec![ClassMember {
                        visibility: Some(Visibility::Protected),
                        name: "age".into(),
                        type_: Some("int".into()),
                        is_method: false,
                    }],
                ),
                cls(
                    "Dog",
                    vec![ClassMember {
                        visibility: Some(Visibility::Public),
                        name: "bark".into(),
                        type_: Some("void".into()),
                        is_method: true,
                    }],
                ),
            ],
            relations: vec![crate::ast::Relation {
                source: "Dog".into(),
                target: "Animal".into(),
                kind: RelationKind::Inheritance,
                cardinality_first: None,
                cardinality_second: None,
                label: None,
            }],
        };

        let ug = extract_class(&cd);
        assert_eq!(ug.family, crate::builder::ir::unigraph::GraphFamily::Grid);
        assert_eq!(ug.nodes.len(), 2);
        assert_eq!(ug.nodes[0].id, "Animal");
        assert_eq!(ug.edges.len(), 1);
        assert_eq!(ug.edges[0].kind, EdgeKind::ClassExtends);

        // detail：属性 / 方法分栏正确。
        let NodeDetail::Class { attrs, methods, .. } = &ug.nodes[0].detail else {
            panic!("expected Class detail");
        };
        assert_eq!(attrs, &["#int age".to_string()]);
        assert!(methods.is_empty());
    }

    #[test]
    fn maps_relation_kinds() {
        assert_eq!(
            map_relation_kind(&RelationKind::Inheritance),
            EdgeKind::ClassExtends
        );
        assert_eq!(
            map_relation_kind(&RelationKind::Composition),
            EdgeKind::ClassComposition
        );
        assert_eq!(
            map_relation_kind(&RelationKind::Aggregation),
            EdgeKind::ClassAggregation
        );
        assert_eq!(
            map_relation_kind(&RelationKind::Association),
            EdgeKind::ClassAssociation
        );
        assert_eq!(
            map_relation_kind(&RelationKind::Dependency),
            EdgeKind::ClassDependency
        );
    }

    #[test]
    fn dependency_is_dotted() {
        let cd = ClassDiagram {
            classes: vec![cls("A", vec![]), cls("B", vec![])],
            relations: vec![crate::ast::Relation {
                source: "A".into(),
                target: "B".into(),
                kind: RelationKind::Dependency,
                cardinality_first: None,
                cardinality_second: None,
                label: None,
            }],
        };
        let ug = extract_class(&cd);
        assert_eq!(ug.edges[0].line_kind, LineKind::Dotted);
    }
}
