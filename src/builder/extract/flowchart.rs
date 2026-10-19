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
        | ArrowType::MultiCross => LineKind::Solid,
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

/// 递归预序遍历子图（外层先于内层）。
fn walk_subgraphs<'a>(sg: &'a crate::ast::Subgraph, f: &mut impl FnMut(&'a crate::ast::Subgraph)) {
    f(sg);
    for child in &sg.subgraphs {
        walk_subgraphs(child, f);
    }
}

/// 收集子图（含嵌套）的全部直接节点。
fn collect_subgraph_nodes<'a>(sg: &'a crate::ast::Subgraph, out: &mut Vec<&'a crate::ast::Node>) {
    out.extend(sg.nodes.iter());
    for child in &sg.subgraphs {
        collect_subgraph_nodes(child, out);
    }
}

/// 收集子图（含嵌套）的全部边。
fn collect_subgraph_edges<'a>(sg: &'a crate::ast::Subgraph, out: &mut Vec<&'a crate::ast::Edge>) {
    out.extend(sg.edges.iter());
    for child in &sg.subgraphs {
        collect_subgraph_edges(child, out);
    }
}

/// 容器成员 = 本层直接节点 + 所有后代节点（外层容器须包住内层容器）。
fn subgraph_member_ids(sg: &crate::ast::Subgraph, out: &mut Vec<String>) {
    out.extend(sg.nodes.iter().map(|n| n.id.clone()));
    for child in &sg.subgraphs {
        subgraph_member_ids(child, out);
    }
}

/// 提取 flowchart 为统一拓扑图。
pub fn extract_flowchart(fc: &Flowchart) -> Unigraph {
    // 节点形状/文本真相源：顶层 fc.nodes + 所有 subgraph 内部节点（按 id 合并）。
    // parser 把 subgraph 内部节点只放进 subgraph.nodes，顶层 fc.nodes 仅由边端点补充
    // （shape 信息会丢失），因此必须合并 subgraph.nodes 才能保住 `C{Decision}` 的菱形等。
    //
    // 重复声明语义（与官方一致）：**同一集合内后出现的定义覆盖先出现的**。
    // 先前用 `entry().or_insert()` 保留首见定义，导致 `A[First]` 后 `A[Second]`
    // 仍渲染 First（与 docs/syntax-flowchart.md 的“最后一次为准”相反）。
    // subgraph 内部声明仍优先于顶层边端点补出的占位节点，故先填顶层、再覆盖 subgraph。
    use std::collections::HashMap;
    let mut shape_of: HashMap<&str, &Option<NodeShape>> = HashMap::new();
    let mut text_of: HashMap<&str, &Option<String>> = HashMap::new();
    for n in &fc.nodes {
        shape_of.insert(n.id.as_str(), &n.shape);
        text_of.insert(n.id.as_str(), &n.text);
    }
    for sg in &fc.subgraphs {
        walk_subgraphs(sg, &mut |sub| {
            for n in &sub.nodes {
                shape_of.insert(n.id.as_str(), &n.shape);
                text_of.insert(n.id.as_str(), &n.text);
            }
        });
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
    let mut sub_nodes: Vec<&crate::ast::Node> = Vec::new();
    for sg in &fc.subgraphs {
        collect_subgraph_nodes(sg, &mut sub_nodes);
    }
    for n in sub_nodes {
        emit_node(&mut nodes, &n.id, &shape_of, &text_of);
    }

    let mut edges = Vec::new();
    // parser 把写在 `subgraph … end` 块内的边放入 `subgraphs[i].edges`（顶层 fc.edges
    // 只含子图外的边）。原先只遍历 fc.edges 导致**子图内部连线整体丢失**，这里含嵌套一并收集。
    let mut edge_idx = 0usize;
    let mut sub_edges: Vec<&crate::ast::Edge> = Vec::new();
    for sg in &fc.subgraphs {
        collect_subgraph_edges(sg, &mut sub_edges);
    }
    for e in sub_edges {
        edges.push(flow_edge(e, edge_idx, fc.direction));
        edge_idx += 1;
    }
    for e in &fc.edges {
        edges.push(flow_edge(e, edge_idx, fc.direction));
        edge_idx += 1;
    }

    // 子图：预序（外层先于内层）生成容器；成员含全部后代节点，
    // 使外层包围盒包住内层容器。
    let mut subgraphs: Vec<ir::unigraph::UGSubgraph> = Vec::new();
    for sg in &fc.subgraphs {
        walk_subgraphs(sg, &mut |sub| {
            let mut member_ids = Vec::new();
            subgraph_member_ids(sub, &mut member_ids);
            subgraphs.push(ir::unigraph::UGSubgraph {
                id: format!("sub{}", subgraphs.len()),
                title: sub.title.clone(),
                member_ids,
                direction: sub.direction,
                kind: ir::common::ContainerKind::Subgraph,
            });
        });
    }

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
        assert!(
            pairs.contains(&("B".into(), "C".into())),
            "缺子图内部边 B→C"
        );
        assert!(pairs.contains(&("A".into(), "B".into())), "缺跨子图边 A→B");
        // 边 id 全图唯一且连续。
        let ids: Vec<String> = ug.edges.iter().map(|e| e.id.clone()).collect();
        assert_eq!(ids, vec!["e0", "e1"]);
    }

    /// 嵌套子图：内外两层容器都要保留，外层成员包含内层节点。
    #[test]
    fn nested_subgraphs_keep_hierarchy() {
        let fc = parse(
            "flowchart TD\nsubgraph outer\nsubgraph inner\nA[In] --> B[Inner]\nend\nC[Outer] --> A\nend\nD --> C\n",
        );
        let ug = extract_flowchart(&fc);
        assert_eq!(ug.subgraphs.len(), 2, "内层与外层各一个容器");
        let outer = &ug.subgraphs[0];
        let inner = &ug.subgraphs[1];
        assert_eq!(outer.title.as_deref(), Some("outer"));
        assert_eq!(inner.title.as_deref(), Some("inner"));
        assert!(inner.member_ids.iter().any(|id| id == "A"));
        assert!(
            outer.member_ids.iter().any(|id| id == "A"),
            "外层成员应包含内层节点"
        );
        assert!(outer.member_ids.len() > inner.member_ids.len());
        assert_eq!(ug.nodes.len(), 4);
        assert_eq!(ug.edges.len(), 3, "内层边与跨层边都不得丢失");
    }

    /// 重复声明：官方语义为“最后一次定义为准”。
    #[test]
    fn duplicate_node_declaration_last_wins() {
        let fc = parse("flowchart TD\nA[First]\nA[Second]\nA --> B[End]\n");
        let ug = extract_flowchart(&fc);
        let a = ug.nodes.iter().find(|n| n.id == "A").expect("node A");
        match &a.label {
            crate::builder::ir::common::LabelOrMeasured::Spec(s) => {
                assert_eq!(s.text, "Second", "后出现的定义应覆盖先出现的");
            }
            other => panic!("expected Spec label, got {other:?}"),
        }
        assert_eq!(ug.nodes.len(), 2, "同名节点只应出现一次");
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
