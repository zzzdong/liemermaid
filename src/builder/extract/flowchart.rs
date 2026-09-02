//! flowchart 的 extract：把 [`crate::ast::Flowchart`] 翻译成 [`Unigraph`]。
//!
//! 这是 `extract` 家族的第一个实现（P0.3 最小子集）：矩形/菱形/圆等节点 + 直线边。
//! subgraphs 暂忽略（P2 再处理容器），其他图类型在 P3 实现。

use crate::{
    ast::{ArrowType, Flowchart, NodeShape},
    builder::ir::{
        self,
        common::{
            ArrowKind, ArrowSpec, EdgePriority, LabelSpec, LineKind, PortHint, PortSet, SizeHint,
            StyleRef,
        },
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

/// 把 ast 的 ArrowType 映射到 IR 的 ArrowSpec（起点 / 终点各自的标记类型）。
pub fn map_arrow(arrow: &ArrowType) -> ArrowSpec {
    let (start, end) = match arrow {
        ArrowType::Solid | ArrowType::Thick | ArrowType::Dotted => {
            (ArrowKind::None, ArrowKind::Arrow)
        }
        ArrowType::Labeled(_) => (ArrowKind::None, ArrowKind::Arrow),
        ArrowType::NoArrow => (ArrowKind::None, ArrowKind::None),
        ArrowType::Invisible => (ArrowKind::None, ArrowKind::None),
        ArrowType::Both => (ArrowKind::Arrow, ArrowKind::Arrow),
        ArrowType::Circle => (ArrowKind::None, ArrowKind::Circle),
        ArrowType::Cross => (ArrowKind::None, ArrowKind::Cross),
        ArrowType::MultiCircle => (ArrowKind::Circle, ArrowKind::Circle),
        ArrowType::MultiCross => (ArrowKind::Cross, ArrowKind::Cross),
    };
    ArrowSpec { start, end }
}

/// 把 ast 的 ArrowType 映射到 IR 的 LineKind（实线 / 虚线 / 粗线 / 不可见）。
pub fn map_line(arrow: &ArrowType) -> LineKind {
    match arrow {
        ArrowType::Solid
        | ArrowType::NoArrow
        | ArrowType::Both
        | ArrowType::Circle
        | ArrowType::Cross
        | ArrowType::MultiCircle
        | ArrowType::MultiCross
        | ArrowType::Labeled(_) => LineKind::Solid,
        ArrowType::Dotted => LineKind::Dotted,
        ArrowType::Thick => LineKind::Thick,
        ArrowType::Invisible => LineKind::Invisible,
    }
}

/// 单条边 → UGEdge（id 用连续序号，保证全图唯一）。
fn flow_edge(e: &crate::ast::Edge, i: usize, direction: Option<crate::ast::Direction>) -> UGEdge {
    // 线方向：TB/TD 默认自上而下（source 底部 → target 顶部）；LR 自左向右。
    let (src_port, tgt_port) = match direction {
        Some(crate::ast::Direction::LR) | Some(crate::ast::Direction::RL) => {
            (PortHint::Right, PortHint::Left)
        }
        _ => (PortHint::Bottom, PortHint::Top),
    };
    UGEdge {
        id: format!("e{}", i),
        source: e.source.clone(),
        target: e.target.clone(),
        source_port: src_port,
        target_port: tgt_port,
        kind: EdgeKind::Flow,
        label_text: e.label.clone(),
        label: None,
        priority: EdgePriority::Primary,
        routing_hint: ir::common::RoutingHint::Spline,
        arrow: map_arrow(&e.arrow_type),
        line_kind: map_line(&e.arrow_type),
        repulsion: 1.0,
        cardinality: (None, None),
        cardinality_text: (None, None),
    }
}

/// 提取 flowchart 为统一拓扑图。
pub fn extract_flowchart(fc: &Flowchart) -> Unigraph {
    // 节点形状/文本真相源：顶层 fc.nodes + 所有 subgraph 内部节点（按 id 合并）。
    // parser 把 subgraph 内部节点只放进 subgraph.nodes，顶层 fc.nodes 仅由边端点补充
    // （shape 信息会丢失），因此必须合并 subgraph.nodes 才能保住 `C{Decision}` 的菱形等。
    use std::collections::HashMap;
    // subgraph 内部节点声明更完整（含 shape/text，如 `C{Decision}`），先填；
    // 顶层 fc.nodes 的补充节点（shape=None）用 or_insert 不覆盖 subgraph 的声明。
    let mut shape_of: HashMap<&str, &Option<NodeShape>> = HashMap::new();
    let mut text_of: HashMap<&str, &Option<String>> = HashMap::new();
    for sg in &fc.subgraphs {
        for n in &sg.nodes {
            shape_of.entry(n.id.as_str()).or_insert(&n.shape);
            text_of.entry(n.id.as_str()).or_insert(&n.text);
        }
    }
    for n in &fc.nodes {
        shape_of.entry(n.id.as_str()).or_insert(&n.shape);
        text_of.entry(n.id.as_str()).or_insert(&n.text);
    }

    let mut nodes = Vec::new();
    // 按 fc.nodes + subgraph 内部节点都纳入，去重构建。
    let mut seen = std::collections::HashSet::new();
    let mut emit_node = |nodes: &mut Vec<UGNode>,
                         id: &str,
                         shape_of: &HashMap<&str, &Option<NodeShape>>,
                         text_of: &HashMap<&str, &Option<String>>| {
        if seen.contains(id) {
            return;
        }
        seen.insert(id.to_string());
        let shape = map_shape(shape_of.get(id).copied().unwrap_or(&None));
        let label = LabelSpec {
            text: text_of
                .get(id)
                .and_then(|o| o.as_ref())
                .cloned()
                .unwrap_or_else(|| id.to_string()),
            spans: Vec::new(), // measure 阶段填充 RichSpan
        };
        nodes.push(UGNode {
            id: id.to_string(),
            kind: ir::common::NodeKind::Atom,
            role: ir::common::NodeRole::Atom,
            shape,
            label: ir::common::LabelOrMeasured::Spec(label),
            ports: PortSet::default(),
            size_hint: SizeHint::ByText,
            style_ref: StyleRef::NodeDefault,
            constraint: ir::common::NodeConstraint::Free,
            detail: ir::common::NodeDetail::None,
        });
    };
    for n in &fc.nodes {
        emit_node(&mut nodes, &n.id, &shape_of, &text_of);
    }
    for sg in &fc.subgraphs {
        for n in &sg.nodes {
            emit_node(&mut nodes, &n.id, &shape_of, &text_of);
        }
    }

    let mut edges = Vec::new();
    // parser 把写在 `subgraph … end` 块内的边放入 `subgraphs[i].edges`（顶层 fc.edges
    // 只含子图外的边）。原先只遍历 fc.edges 导致**子图内部连线整体丢失**，这里一并收集。
    let mut edge_idx = 0usize;
    for sg in &fc.subgraphs {
        for e in &sg.edges {
            edges.push(flow_edge(e, edge_idx, fc.direction));
            edge_idx += 1;
        }
    }
    for e in &fc.edges {
        edges.push(flow_edge(e, edge_idx, fc.direction));
        edge_idx += 1;
    }

    // 子图：收集每个 subgraph 的成员节点 id（含嵌套 subgraph 展平后的节点）。
    let subgraphs: Vec<ir::unigraph::UGSubgraph> = fc
        .subgraphs
        .iter()
        .enumerate()
        .map(|(i, sg)| ir::unigraph::UGSubgraph {
            id: format!("sub{}", i),
            title: sg.title.clone(),
            member_ids: sg.nodes.iter().map(|n| n.id.clone()).collect(),
            kind: ir::common::ContainerKind::Subgraph,
        })
        .collect();

    Unigraph {
        family: ir::unigraph::GraphFamily::Directed,
        direction: fc.direction.unwrap_or(crate::ast::Direction::TB),
        nodes,
        edges,
        subgraphs,
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

    fn parse(src: &str) -> Flowchart {
        crate::MermaidParser::parse_mermaid(src)
            .map(|d| match d {
                crate::ast::Diagram::Flowchart(f) => f,
                _ => panic!("not a flowchart"),
            })
            .expect("parse")
    }

    #[test]
    fn subgraph_internal_edges_are_collected() {
        // 回归：`B --> C` 写在 `subgraph One … end` 块内，parser 把它放入
        // subgraphs[0].edges 而非顶层 fc.edges；extract 原先只遍历 fc.edges，
        // 导致子图内部连线整体丢失。
        let fc = parse(
            "flowchart TD\nA[Start]\nsubgraph One\nB[Process]\nC[Decision]\nB --> C\nend\nA --> B\n",
        );
        let ug = extract_flowchart(&fc);
        // 顶层 1 条跨子图边 + 子图内部 1 条。
        assert_eq!(ug.edges.len(), 2, "子图内部边不得丢失");
        let pairs: Vec<(String, String)> = ug
            .edges
            .iter()
            .map(|e| (e.source.clone(), e.target.clone()))
            .collect();
        assert!(pairs.contains(&("B".into(), "C".into())), "缺子图内部边 B→C");
        assert!(pairs.contains(&("A".into(), "B".into())), "缺跨子图边 A→B");
        // 边 id 全图唯一且连续。
        let ids: Vec<String> = ug.edges.iter().map(|e| e.id.clone()).collect();
        assert_eq!(ids, vec!["e0", "e1"]);
    }

    #[test]
    fn flowchart_dispatch_is_case_insensitive_and_trims_leading_space() {
        // 回归：parser/mod.rs 曾用区分大小写的 starts_with 并把未 trim 的原始输入
        // 传给 parse_flowchart，`Flowchart TD` 与前导空格的图均解析失败。
        for src in [
            "flowchart TD\nA[Start] --> B[End]\n",
            "Flowchart TD\nA[Start] --> B[End]\n",
            "  flowchart TD\nA[Start] --> B[End]\n",
            "graph LR\nA[Start] --> B[End]\n",
        ] {
            let fc = parse(src);
            assert_eq!(fc.edges.len(), 1, "src: {src:?}");
        }
    }
}
