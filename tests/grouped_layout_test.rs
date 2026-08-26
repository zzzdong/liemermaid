//! `GroupedDirected`（递归求解 + 平移回贴）测试。

use lievisual::geometry::Size;

use liemermaid::builder::layout::config::LayoutConfig;
use liemermaid::builder::layout::ir::{
    GroupChild, LEdge, LGroup, LNode, LayoutGraph, LineKind, PortHint, ShapeHint,
};
use liemermaid::builder::layout::solver::DirectedSolver;

/// 构造带子图的 LayoutGraph：
/// - 顶层独立节点 A(0)
/// - 组0 含 X(1)、Y(2)
/// - 边 A->X（跨组）、X->Y（组内）
fn grouped_graph() -> LayoutGraph {
    let mut lg = LayoutGraph::default();
    for (id, _w, _h) in [("A", 80.0, 40.0), ("X", 80.0, 40.0), ("Y", 80.0, 40.0)] {
        lg.nodes.push(LNode {
            id: id.to_string(),
            size: Size::new(80.0, 40.0),
            shape_hint: ShapeHint::Rect,
        });
    }
    // 组0：X(1)、Y(2)
    lg.groups.push(LGroup {
        title: Some("Group".into()),
        children: vec![GroupChild::Node(1), GroupChild::Node(2)],
    });
    // A->X 跨组，X->Y 组内
    lg.edges.push(LEdge {
        source: 0,
        target: 1,
        source_port: PortHint::Auto,
        target_port: PortHint::Auto,
        line_kind: LineKind::Solid,
    });
    lg.edges.push(LEdge {
        source: 1,
        target: 2,
        source_port: PortHint::Auto,
        target_port: PortHint::Auto,
        line_kind: LineKind::Solid,
    });
    lg
}

#[test]
fn grouped_positions_len_matches_nodes() {
    let lg = grouped_graph();
    let placed = DirectedSolver::solve(&lg, &LayoutConfig::default());
    assert_eq!(placed.positions.len(), 3, "3 个节点位置");
    assert_eq!(placed.group_bounds.len(), 1, "1 个子图容器包围盒");
}

#[test]
fn group_container_contains_members() {
    let lg = grouped_graph();
    let placed = DirectedSolver::solve(&lg, &LayoutConfig::default());
    let bound = placed.group_bounds[0];
    // 成员 X、Y 的中心应在容器内
    for &idx in &[1, 2] {
        let p = placed.positions[idx];
        assert!(
            p.x >= bound.min_x()
                && p.x <= bound.max_x()
                && p.y >= bound.min_y()
                && p.y <= bound.max_y(),
            "成员 {idx} 应在容器包围盒内: pos={:?} bound={:?}",
            p,
            bound
        );
    }
}

#[test]
fn all_grouped_members_have_valid_positions() {
    let lg = grouped_graph();
    let placed = DirectedSolver::solve(&lg, &LayoutConfig::default());
    // 所有节点都有非零位置（没有落在原点）
    for (i, p) in placed.positions.iter().enumerate() {
        assert!(
            p.x.abs() > 0.01 || p.y.abs() > 0.01,
            "节点 {i} 位置不应是原点: {p:?}"
        );
    }
}
