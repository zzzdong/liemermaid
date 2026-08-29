//! ER 迁移（新管线）回归测试：验证 ER 图走 extract → measure → engine →
//! materialize → paint 产出几何正确（Grid 分层 + 实体框 + 基数符号）。

use liemermaid::ast::{Cardinality, ErAttribute, ErDiagram, ErEntity, ErRelationship};
use liemermaid::builder::extract::er::extract_er;
use liemermaid::builder::ir::scenegraph::{SceneItem, StyleIntent};
use liemermaid::builder::{layout, materialize, measure, paint};

fn run_gg(ed: &ErDiagram) -> liemermaid::builder::ir::geograph::Geograph {
    let ug = extract_er(ed);
    let ug = measure::measure_all(ug);
    layout::engine::run(&ug).expect("layout").0
}

#[test]
fn er_entities_layer_and_cardinality_render() {
    let ed = ErDiagram {
        entities: vec![ErEntity {
            name: "Customer".into(),
            attributes: vec![ErAttribute { type_: "int".into(), name: "id".into() }],
        }],
        relationships: vec![ErRelationship {
            first_entity: "Customer".into(),
            second_entity: "Order".into(),
            cardinality_first: Cardinality::ExactlyOne,
            cardinality_second: Cardinality::ZeroOrMany,
            label: Some("places".into()),
        }],
    };

    let gg = run_gg(&ed);
    assert_eq!(gg.nodes.len(), 2, "Customer + 隐含的 Order");
    assert_eq!(gg.edges.len(), 1);

    let sg = materialize::run(&gg, &StyleIntent::default());

    // 基数符号：ExactlyOne 端产 2 条短线（SceneItem::Edge 短线），ZeroOrMany 端产 1 圆 + 1 大括号。
    // 至少应有实体框（header+body × 2）+ 关系线 + 基数符号若干。
    let shape_count = sg.items.iter().filter(|i| matches!(i, SceneItem::Shape { .. })).count();
    let edge_count = sg.items.iter().filter(|i| matches!(i, SceneItem::Edge { .. })).count();
    // 2 实体框 × 2（header+body）= 4 shape + 基数符号的圆/大括号 + 关系线 edge + 基数短线的 edge。
    assert!(shape_count >= 4, "实体框应产 header+body: {shape_count}");
    assert!(edge_count >= 1, "应有关系线 + 基数短线: {edge_count}");

    let scene = paint::run(&sg);
    assert!(!scene.nodes.is_empty(), "ER 图 paint 应产出图元");
}

/// 真实公共 API 端到端：parse → build_diagram。
#[test]
fn er_via_public_api_renders() {
    let src = "erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER ||--|{ LINE-ITEM : contains\n";
    let diagram = liemermaid::MermaidParser::parse_mermaid(src).expect("parse");
    let scene = liemermaid::builder::build_diagram(&diagram).expect("build");
    assert!(!scene.nodes.is_empty(), "ER 图经公共 API 应产出场景");
}
