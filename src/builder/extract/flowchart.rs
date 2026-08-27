//! flowchart 的 extract：把 [`crate::ast::Flowchart`] 翻译成 [`Unigraph`](crate::builder::ir::Unigraph)。
//!
//! 这是 `extract` 家族的第一个实现（P0.3 最小子集）：矩形/菱形/圆等节点 + 直线边。
//! subgraphs 暂忽略（P2 再处理容器），其他图类型在 P3 实现。

use crate::{
    ast::{ArrowType, Flowchart, NodeShape},
    builder::ir::{
        self,
        common::{ArrowKind, ArrowSpec, EdgePriority, LabelOrMeasured, LabelSpec, PortHint, PortSet, SizeHint, StyleRef},
        shape::ShapeKind,
        unigraph::{EdgeKind, UGEdge, UGNode, Unigraph},
    },
};

/// 把 ast 的 NodeShape 映射到 IR 的 ShapeKind（全项目唯一真相源）。
pub fn map_shape(shape: &Option<NodeShape>) -> ShapeKind {
    match shape {
        None => ShapeKind::Rectangle,
        Some(NodeShape::Rectangle) => ShapeKind::Rectangle,
        Some(NodeShape::Rounded) => ShapeKind::Rounded,
        Some(NodeShape::Stadium) => ShapeKind::Stadium,
        Some(NodeShape::Subroutine) => ShapeKind::Subroutine,
        Some(NodeShape::Diamond) => ShapeKind::Diamond,
        Some(NodeShape::Hexagon) => ShapeKind::Hexagon,
        Some(NodeShape::Circle) => ShapeKind::Circle,
        Some(NodeShape::DoubleCircle) => ShapeKind::DoubleCircle,
        Some(NodeShape::Cylinder) => ShapeKind::Cylinder,
        Some(NodeShape::Asymmetric) => ShapeKind::Asymmetric,
        Some(NodeShape::Parallelogram) | Some(NodeShape::ParallelogramAlt) => {
            ShapeKind::Parallelogram
        }
        Some(NodeShape::Trapezoid) | Some(NodeShape::TrapezoidAlt) => ShapeKind::Trapezoid,
    }
}

/// 把 ast 的 ArrowType 映射到 IR 的 ArrowSpec（终点标记；起点暂统一 None）。
/// Dotted/Thick 仅影响线型，线型细化留 P1.3（此处统一画实线箭头）。
pub fn map_arrow(arrow: &ArrowType) -> ArrowSpec {
    let end = match arrow {
        ArrowType::Solid | ArrowType::Thick | ArrowType::Dotted => ArrowKind::Arrow,
        ArrowType::Labeled(_) => ArrowKind::Arrow,
        ArrowType::NoArrow | ArrowType::Invisible => ArrowKind::None,
        ArrowType::Both => ArrowKind::Circle, // 近似：Both 用 Circle 表示双向，P1.3 细化
        ArrowType::Circle => ArrowKind::Circle,
        ArrowType::Cross => ArrowKind::Cross,
        ArrowType::MultiCircle => ArrowKind::Circle,
        ArrowType::MultiCross => ArrowKind::Cross,
    };
    ArrowSpec { start: ArrowKind::None, end }
}

/// 提取 flowchart 为统一拓扑图。
pub fn extract_flowchart(fc: &Flowchart) -> Unigraph {
    let mut nodes = Vec::new();
    for n in &fc.nodes {
        let shape = map_shape(&n.shape);
        let label = LabelSpec {
            text: n.text.clone().unwrap_or_else(|| n.id.clone()),
            spans: Vec::new(), // measure 阶段填充 RichSpan
        };
        nodes.push(UGNode {
            id: n.id.clone(),
            kind: ir::common::NodeKind::Atom,
            role: ir::common::NodeRole::Atom,
            shape,
            label: ir::common::LabelOrMeasured::Spec(label),
            ports: PortSet::default(),
            size_hint: SizeHint::ByText,
            style_ref: StyleRef::NodeDefault,
            constraint: ir::common::NodeConstraint::Free,
        });
    }

    let mut edges = Vec::new();
    for (i, e) in fc.edges.iter().enumerate() {
        // 线方向：TB/TD 默认自上而下（source 底部 → target 顶部）；LR 自左向右。
        let (src_port, tgt_port) = match fc.direction {
            Some(crate::ast::Direction::LR) | Some(crate::ast::Direction::RL) => {
                (PortHint::Right, PortHint::Left)
            }
            _ => (PortHint::Bottom, PortHint::Top),
        };
        edges.push(UGEdge {
            id: format!("e{}", i),
            source: e.source.clone(),
            target: e.target.clone(),
            source_port: src_port,
            target_port: tgt_port,
            kind: EdgeKind::Flow,
            label: None,
            priority: EdgePriority::Primary,
            routing_hint: ir::common::RoutingHint::Orthogonal,
            arrow: map_arrow(&e.arrow_type),
            repulsion: 1.0,
        });
    }

    Unigraph {
        family: ir::unigraph::GraphFamily::Directed,
        direction: fc.direction.clone().unwrap_or(crate::ast::Direction::TB),
        nodes,
        edges,
        meta: ir::common::DiagramMeta { title: None },
    }
}
