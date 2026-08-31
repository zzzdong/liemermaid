//! P1.3 覆盖测试：验证 `materialize` 能覆盖 `ShapeKind` 全部变体、且 `paint` 能将其
//! 翻译成 `lievisual::Scene` 而不 panic；同时验证带箭头的边在 paint 阶段生成多图元
//! （折线 + 箭头标记）。
//!
//! 不比对像素（视觉细节留 P2/P4 收敛），只断言「全 ShapeKind 都能贯通 + 边箭头多节点」。

use liemermaid::builder::ir::common::{ArrowKind, ArrowSpec};
use liemermaid::builder::ir::geograph::{GGEdge, GGNode, Geograph};
use liemermaid::builder::ir::scenegraph::{SceneItem, StyleIntent};
use liemermaid::builder::ir::shape::{EdgeEnds, ShapeGeometry, ShapeKind};
use liemermaid::builder::ir::unigraph::EdgeKind;
use liemermaid::builder::materialize;
use liemermaid::builder::paint;
use lievisual::geometry::{Point, Size};

/// 构造一个最小 Geograph：单个节点 + 指定 ShapeKind + 一条带箭头的边（自环，用于验证箭头）。
fn gg_with(shape: ShapeKind) -> Geograph {
    let node = GGNode {
        id: "n1".to_string(),
        role: liemermaid::builder::ir::common::NodeRole::Atom,
        center: Point::new(50.0, 50.0),
        size: Size::new(80.0, 40.0),
        shape,
        ports: liemermaid::builder::ir::common::ResolvedPorts {
            top: Point::new(50.0, 30.0),
            bottom: Point::new(50.0, 70.0),
            left: Point::new(10.0, 50.0),
            right: Point::new(90.0, 50.0),
        },
        label: None,
        detail: liemermaid::builder::ir::common::NodeDetail::None,
    };
    let edge = GGEdge {
        id: "e1".to_string(),
        source: "n1".to_string(),
        target: "n1".to_string(),
        route: liemermaid::builder::ir::geograph::line_route(&[
            Point::new(10.0, 50.0),
            Point::new(90.0, 50.0),
        ]),
        label_text: None,
        label_anchor: None,
        kind: EdgeKind::Flow,
        arrow: ArrowSpec {
            start: ArrowKind::None,
            end: ArrowKind::Arrow,
        },
        routing_hint: liemermaid::builder::ir::common::RoutingHint::Orthogonal,
        line_kind: liemermaid::builder::ir::common::LineKind::Solid,
        cardinality: (None, None),
        cardinality_text: (None, None),
    };
    liemermaid::builder::ir::geograph::Geograph {
        size: Size::new(100.0, 100.0),
        background: lievisual::geometry::Color::WHITE,
        nodes: vec![node],
        edges: vec![edge],
        containers: vec![],
        title: None,
        show_data: false,
        activations: vec![],
        sequence_dividers: vec![],
    }
}

#[test]
fn all_shape_kinds_materialize_and_paint_without_panic() {
    let kinds = [
        ShapeKind::Rectangle,
        ShapeKind::Rounded,
        ShapeKind::Stadium,
        ShapeKind::Subroutine,
        ShapeKind::Diamond,
        ShapeKind::Hexagon,
        ShapeKind::Circle,
        ShapeKind::DoubleCircle,
        ShapeKind::Cylinder,
        ShapeKind::Asymmetric,
        ShapeKind::Parallelogram,
        ShapeKind::Trapezoid,
        ShapeKind::Bar,
        ShapeKind::StartDot,
        ShapeKind::EndDot,
        ShapeKind::PieSlice,
        ShapeKind::QuadrantCell,
    ];

    for &kind in &kinds {
        let gg = gg_with(kind);
        // materialize 不 panic，且产出 1 形状项 + 1 边项。
        let sg = materialize::run(&gg, &StyleIntent::default());
        let shapes = sg
            .items
            .iter()
            .filter(|i| matches!(i, SceneItem::Shape { .. }))
            .count();
        // EndDot / DoubleCircle 双环 = 外圈 + 内圈 2 个形状；其余 1 个。
        let expected = if matches!(kind, ShapeKind::EndDot | ShapeKind::DoubleCircle) {
            2
        } else {
            1
        };
        assert_eq!(
            shapes, expected,
            "ShapeKind::{:?} 应产出 {expected} 个形状项",
            kind
        );

        // paint 不 panic，且图元树能成功构建。
        let scene = paint::run(&sg);
        assert!(
            !scene.nodes.is_empty(),
            "ShapeKind::{:?} paint 应产出图元",
            kind
        );
    }
}

#[test]
fn edge_with_arrow_yields_polyline_plus_marker() {
    // 带 Arrow 终端的边：paint 应把单条 Edge 展开为 Group（折线 polyline + 箭头三角）。
    // 此时 scene.nodes = [形状节点, 边 Group 节点] = 2。
    let gg = gg_with(ShapeKind::Rectangle);
    let sg = materialize::run(&gg, &StyleIntent::default());
    let scene = paint::run(&sg);
    // 形状（1）+ 带箭头边（被 Group 包裹，算 1 个节点）= 2。
    assert_eq!(
        scene.nodes.len(),
        2,
        "带箭头边应被 group 包裹为 1 节点 + 1 形状节点"
    );

    // 验证 SG 里存在「终点=Arrow、起点=None」的 Edge 项（materialize 已把 ArrowSpec 映射为 (start,end) 二元组）。
    let has_arrow_edge = sg.items.iter().any(
        |i| matches!(i, SceneItem::Edge { ends, .. } if *ends == (EdgeEnds::None, EdgeEnds::Arrow)),
    );
    assert!(has_arrow_edge, "带箭头边应映射为 (None, Arrow) 终点标记");
}

#[test]
fn each_shape_kind_produces_distinct_geometry_variant() {
    // 验证 materialize 对每种 ShapeKind 产出的 ShapeGeometry 变体正确（非统一 fallback）。
    use liemermaid::builder::ir::shape::ShapeGeometry::*;
    /// (形状, 期望的几何变体判定) 对照表。
    type Expectation = (ShapeKind, fn(&ShapeGeometry) -> bool);
    let expectations: [Expectation; 17] = [
        (ShapeKind::Rectangle, |g| matches!(g, Rect { .. })),
        (ShapeKind::Rounded, |g| matches!(g, RoundedRect { .. })),
        (ShapeKind::Stadium, |g| matches!(g, Stadium { .. })),
        (ShapeKind::Subroutine, |g| matches!(g, RoundedRect { .. })),
        (ShapeKind::Diamond, |g| matches!(g, Polygon { .. })),
        (ShapeKind::Hexagon, |g| matches!(g, Polygon { .. })),
        (ShapeKind::Circle, |g| matches!(g, Ellipse { .. })),
        (ShapeKind::DoubleCircle, |g| matches!(g, Ellipse { .. })),
        (ShapeKind::Cylinder, |g| matches!(g, Polygon { .. })),
        (ShapeKind::Asymmetric, |g| matches!(g, Polygon { .. })),
        (ShapeKind::Parallelogram, |g| matches!(g, Polygon { .. })),
        (ShapeKind::Trapezoid, |g| matches!(g, Polygon { .. })),
        (ShapeKind::Bar, |g| matches!(g, Rect { .. })),
        (ShapeKind::StartDot, |g| matches!(g, Ellipse { .. })),
        (ShapeKind::EndDot, |g| matches!(g, Ellipse { .. })),
        (ShapeKind::PieSlice, |g| matches!(g, Pie { .. })),
        (ShapeKind::QuadrantCell, |g| matches!(g, Rect { .. })),
    ];
    for (kind, pred) in expectations {
        let gg = gg_with(kind);
        let sg = materialize::run(&gg, &StyleIntent::default());
        let geom = sg.items.iter().find_map(|i| match i {
            SceneItem::Shape { geometry, .. } => Some(geometry),
            _ => None,
        });
        let geom = geom.expect("应有 Shape 项");
        assert!(pred(geom), "ShapeKind::{:?} 的几何变体不符合预期", kind);
    }
}
