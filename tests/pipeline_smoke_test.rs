//! P0.3 端到端管线冒烟测试：验证 三层 IR（UG → GG → SG）贯通 + 最终产出 lievisual::Scene。
//!
//! 不比对 golden（视觉细节留 P1.3 完善），只断言：
//! - 管线全程不 panic；
//! - SG 的 items 数量符合预期（节点数×2[形状+文本] + 边数[线段]）；
//! - 最终 Scene 的图元数量与 SG 一致。

use liemermaid::builder::extract;
use liemermaid::builder::layout::engine;
use liemermaid::builder::materialize;
use liemermaid::builder::measure;
use liemermaid::builder::paint;
use liemermaid::builder::ir::scenegraph::SceneItem;
use liemermaid::MermaidParser;

#[test]
fn flowchart_pipeline_smoke() {
    let src = "graph TD\nA[Start]\nB[Proc]\nC[End]\nA --> B\nB --> C\nC --> A";
    let diagram = MermaidParser::parse_mermaid(src).expect("parse failed");

    // Stage 1: extract
    let ug = extract::run(&diagram).expect("extract failed");
    assert_eq!(ug.nodes.len(), 3, "应有 3 个节点");
    assert_eq!(ug.edges.len(), 3, "应有 3 条边");

    // Stage 1.5: measure
    let ug = measure::measure_all(ug);
    for n in &ug.nodes {
        assert!(n.label.is_measured(), "measure 后节点标签应已测量");
    }

    // Stage 2: layout
    let (gg, style) = engine::run(&ug).expect("layout failed");
    assert_eq!(gg.nodes.len(), 3);
    assert_eq!(gg.edges.len(), 3);
    assert_eq!(style.node_styles.len(), 3);
    assert_eq!(style.edge_styles.len(), 3);

    // Stage 3: materialize
    let sg = materialize::run(&gg, &style);
    // 预期 items：3 节点 × (形状 + 文本) + 3 边 = 9
    let shapes = sg.items.iter().filter(|i| matches!(i, SceneItem::Shape { .. })).count();
    let labels = sg.items.iter().filter(|i| matches!(i, SceneItem::Label { .. })).count();
    let edges = sg.items.iter().filter(|i| matches!(i, SceneItem::Edge { .. })).count();
    assert_eq!(shapes, 3, "应有 3 个形状项");
    assert_eq!(labels, 3, "应有 3 个文本项");
    assert_eq!(edges, 3, "应有 3 个边项");

    // Stage 4: paint
    let scene = paint::run(&sg);
    assert!(!scene.nodes.is_empty(), "Scene 应至少含一个图元");
    // 形状+文本为 z=0/2，边为 z=1；总图元数应等于 SG items 数（无 Group 包裹）
    assert_eq!(scene.nodes.len(), sg.items.len(), "Scene 图元数应与 SG items 一致");
}

#[test]
fn flowchart_pipeline_unsupported_diagram_errors() {
    // P0.3 仅 flowchart 支持；其他图类型应返回错误而非 panic。
    let src = "sequenceDiagram\nA->>B: hi";
    let diagram = MermaidParser::parse_mermaid(src).expect("parse failed");
    let res = extract::run(&diagram);
    assert!(res.is_err(), "非 flowchart 图类型在 P0.3 应返回 UnsupportedDiagram 错误");
}
