//! `GraphAnalysis`（petgraph 数据流分析）的单元测试。
//!
//! 验证：SCC 分组、拓扑序、反馈弧、连通分量对已知图输出正确。

use lievisual::geometry::Size;

use liemermaid::builder::layout::analyze::analyze;
use liemermaid::builder::layout::ir::{LEdge, LNode, LayoutGraph, LineKind, PortHint, ShapeHint};

/// 构造一个简单 LayoutGraph：给定节点数，边为 (src, tgt) 列表。
fn graph_with(nodes: usize, edges: &[(usize, usize)]) -> LayoutGraph {
    let mut lg = LayoutGraph::default();
    for i in 0..nodes {
        lg.nodes.push(LNode {
            id: format!("N{i}"),
            size: Size::new(80.0, 40.0),
            shape_hint: ShapeHint::Rect,
        });
    }
    for &(s, t) in edges {
        lg.edges.push(LEdge {
            source: s,
            target: t,
            source_port: PortHint::Auto,
            target_port: PortHint::Auto,
            line_kind: LineKind::Solid,
        });
    }
    lg
}

#[test]
fn dag_has_no_scc() {
    // A->B->C, A->D->C（DAG）
    let lg = graph_with(4, &[(0, 1), (1, 2), (0, 3), (3, 2)]);
    let analysis = analyze(&lg);
    // DAG 中每个 SCC 都是单节点
    for scc in &analysis.sccs {
        assert_eq!(scc.len(), 1, "DAG 不应有 size>1 的 SCC: {:?}", scc);
    }
    assert!(analysis.feedback_arcs.is_empty(), "DAG 不应有反馈弧");
    // 拓扑序：C(2) 必须在 A(0) 之后
    let pos = |id: usize| analysis.topological_order.iter().position(|&x| x == id).unwrap();
    assert!(pos(2) > pos(0));
    assert!(pos(2) > pos(1));
    assert!(pos(2) > pos(3));
    // 连通分量：所有节点弱连通
    assert_eq!(analysis.connected_components.len(), 1, "全图一个连通分量");
}

#[test]
fn cycle_detects_scc_and_feedback_arc() {
    // A->B, B->C, C->B（B<->C 环）
    let lg = graph_with(3, &[(0, 1), (1, 2), (2, 1)]);
    let analysis = analyze(&lg);
    // 存在一个 size=2 的 SCC（B、C）
    let has_scc2 = analysis.sccs.iter().any(|scc| scc.len() == 2);
    assert!(has_scc2, "B<->C 应构成 size=2 的 SCC: {:?}", analysis.sccs);
    // 反馈弧非空
    assert!(!analysis.feedback_arcs.is_empty(), "环应检测到反馈弧");
}

#[test]
fn disconnected_components() {
    // A->B 与 C->D 两个互不连通的块
    let lg = graph_with(4, &[(0, 1), (2, 3)]);
    let analysis = analyze(&lg);
    assert_eq!(analysis.connected_components.len(), 2, "两个独立连通块");
    // 每个块应包含 {0,1} 和 {2,3}
    let comp_contains = |c: &Vec<usize>, a: usize, b: usize| c.contains(&a) && c.contains(&b);
    let ok = analysis
        .connected_components
        .iter()
        .any(|c| comp_contains(c, 0, 1) && !c.contains(&2))
        && analysis
            .connected_components
            .iter()
            .any(|c| comp_contains(c, 2, 3) && !c.contains(&0));
    assert!(ok, "连通分量分组不正确: {:?}", analysis.connected_components);
}

#[test]
fn isolated_node_is_its_own_component() {
    // 单个孤立节点
    let lg = graph_with(1, &[]);
    let analysis = analyze(&lg);
    assert_eq!(analysis.connected_components.len(), 1);
    assert_eq!(analysis.connected_components[0], vec![0]);
}

#[test]
fn self_loop_ignored_in_analysis() {
    // A 自环，不应形成 SCC 或反馈弧（自环被 analyze 忽略）
    let lg = graph_with(1, &[(0, 0)]);
    let analysis = analyze(&lg);
    assert!(analysis.feedback_arcs.is_empty(), "自环不应被当作反馈弧");
}
