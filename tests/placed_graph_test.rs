//! `DirectedSolver` 输出的 `PlacedGraph` 不变量测试。

use lievisual::geometry::Size;

use liemermaid::builder::layout::config::LayoutConfig;
use liemermaid::builder::layout::ir::{
    LEdge, LNode, LayoutGraph, LineKind, PortHint, ShapeHint,
};
use liemermaid::builder::layout::solver::DirectedSolver;

fn simple_graph(nodes: usize, edges: &[(usize, usize)]) -> LayoutGraph {
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
fn positions_len_matches_nodes() {
    let lg = simple_graph(4, &[(0, 1), (1, 2), (0, 3)]);
    let placed = DirectedSolver::solve(&lg, &LayoutConfig::default());
    assert_eq!(placed.positions.len(), lg.nodes.len(), "positions 与 nodes 同序同长");
}

#[test]
fn edge_routes_len_matches_edges() {
    let lg = simple_graph(4, &[(0, 1), (1, 2), (0, 3)]);
    let placed = DirectedSolver::solve(&lg, &LayoutConfig::default());
    assert_eq!(placed.edge_routes.len(), lg.edges.len(), "edge_routes 与 edges 同序同长");
    // 每条边至少有点
    for r in &placed.edge_routes {
        assert!(r.len() >= 2, "每条边路由至少两个点: {:?}", r);
    }
}

#[test]
fn directed_layering_tb() {
    // A->B->C：B 应在 A 下方，C 在 B 下方（TB，y 递增）
    let lg = simple_graph(3, &[(0, 1), (1, 2)]);
    let placed = DirectedSolver::solve(&lg, &LayoutConfig::default());
    assert!(placed.positions[1].y > placed.positions[0].y, "B 在 A 下方");
    assert!(placed.positions[2].y > placed.positions[1].y, "C 在 B 下方");
    // 同层 Y 对齐（A 只有一层，验证 A、B、C 不同层）
    assert!(
        (placed.positions[0].y - placed.positions[1].y).abs() > 1.0,
        "A、B 应不同层"
    );
}

#[test]
fn normalized_min_is_zero() {
    let lg = simple_graph(3, &[(0, 1), (1, 2)]);
    let mut placed = DirectedSolver::solve(&lg, &LayoutConfig::default());
    placed.normalize();
    let min_x = placed.positions.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let min_y = placed.positions.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    assert!((min_x - 0.0).abs() < 1e-6, "normalize 后 min_x≈0, got {min_x}");
    assert!((min_y - 0.0).abs() < 1e-6, "normalize 后 min_y≈0, got {min_y}");
}

#[test]
fn lr_direction_swaps_axes() {
    let lg = simple_graph(3, &[(0, 1), (1, 2)]);
    let cfg = LayoutConfig { direction: liemermaid::ast::Direction::LR, ..Default::default() };
    let placed = DirectedSolver::solve(&lg, &cfg);
    // LR 下：B 应在 A 右侧（x 递增）
    assert!(placed.positions[1].x > placed.positions[0].x, "LR 下 B 在 A 右侧");
    assert!(placed.positions[2].x > placed.positions[1].x, "LR 下 C 在 B 右侧");
}
