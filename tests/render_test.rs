//! 端到端渲染测试：验证新布局管线（AST → LayoutGraph → PlacedGraph → SceneNode → SVG）。

use lievisual::scene::Scene;

use liemermaid::ast::{ArrowType, Diagram, Edge, Flowchart, Node};
use liemermaid::builder::layout::config::LayoutConfig;
use liemermaid::builder::layout::layout_diagram;
use liemermaid::builder::render::render_placed;
use liemermaid::builder::types::OutputConfig;
use liemermaid::scene_ext::render_scene_svg;

fn small_flowchart() -> Diagram {
    Diagram::Flowchart(Flowchart {
        direction: None,
        nodes: vec![
            Node {
                id: "A".into(),
                shape: None,
                text: Some("Start".into()),
            },
            Node {
                id: "B".into(),
                shape: None,
                text: Some("Process".into()),
            },
            Node {
                id: "C".into(),
                shape: None,
                text: Some("End".into()),
            },
        ],
        edges: vec![
            Edge {
                source: "A".into(),
                target: "B".into(),
                arrow_type: ArrowType::Solid,
                label: None,
            },
            Edge {
                source: "B".into(),
                target: "C".into(),
                arrow_type: ArrowType::Solid,
                label: None,
            },
        ],
        subgraphs: vec![],
    })
}

#[test]
fn end_to_end_produces_svg() {
    let diagram = small_flowchart();
    let layout_cfg = LayoutConfig::default();
    let output_cfg = OutputConfig::default();

    let placed = layout_diagram(&diagram, &layout_cfg, &output_cfg);
    assert_eq!(placed.positions.len(), 3, "3 个节点被布局");

    let nodes = render_placed(&placed, &diagram, &output_cfg);
    assert!(!nodes.is_empty(), "渲染层应产出节点");

    let mut scene = Scene::new(800.0, 600.0);
    scene.nodes = nodes;
    let svg = render_scene_svg(&scene);
    assert!(!svg.is_empty(), "SVG 不应为空");
    assert!(svg.contains("svg"), "SVG 应含 svg 标签");
}

#[test]
fn diamond_and_rounded_shapes_render() {
    // 含菱形决策 + 圆角节点
    let diagram = Diagram::Flowchart(Flowchart {
        direction: None,
        nodes: vec![
            Node {
                id: "D".into(),
                shape: Some(liemermaid::ast::NodeShape::Diamond),
                text: Some("?".into()),
            },
            Node {
                id: "X".into(),
                shape: Some(liemermaid::ast::NodeShape::Rounded),
                text: Some("ok".into()),
            },
        ],
        edges: vec![Edge {
            source: "D".into(),
            target: "X".into(),
            arrow_type: ArrowType::Solid,
            label: None,
        }],
        subgraphs: vec![],
    });
    let layout_cfg = LayoutConfig::default();
    let output_cfg = OutputConfig::default();
    let placed = layout_diagram(&diagram, &layout_cfg, &output_cfg);
    let nodes = render_placed(&placed, &diagram, &output_cfg);
    let mut scene = Scene::new(800.0, 600.0);
    scene.nodes = nodes;
    let svg = render_scene_svg(&scene);
    assert!(!svg.is_empty());
}
