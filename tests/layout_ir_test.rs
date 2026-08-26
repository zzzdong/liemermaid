//! `LayoutGraph` 转换（`ToLayoutGraph`）的单元测试。
//!
//! 验证：节点/边顺序=源码顺序、子图映射为组、跨组边收集、`LineKind` 映射、`title` 映射。

use liemermaid::ast::{ArrowType, Edge, Flowchart, Node, Subgraph};
use liemermaid::builder::layout::convert::{Measure, ToLayoutGraph};
use liemermaid::builder::layout::ir::LineKind;
use liemermaid::builder::types::OutputConfig;

fn measure() -> Measure<'static> {
    // `Measure` 只借用 config；这里用默认配置，生命周期用 static 兜底。
    // 由于 `Measure::new` 借用，需要保证 config 存活——用 Box 泄漏模拟。
    let cfg: &'static OutputConfig = Box::leak(Box::new(OutputConfig::default()));
    Measure::new(cfg)
}

#[test]
fn flowchart_node_order_matches_source() {
    let fc = Flowchart {
        direction: None,
        nodes: vec![
            Node {
                id: "A".into(),
                shape: None,
                text: None,
            },
            Node {
                id: "B".into(),
                shape: None,
                text: None,
            },
            Node {
                id: "C".into(),
                shape: None,
                text: None,
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
    };
    let lg = fc.to_layout_graph(&measure());
    let ids: Vec<&str> = lg.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, vec!["A", "B", "C"], "节点顺序必须等于源码顺序");
    assert_eq!(lg.edges.len(), 2, "两条边");
    assert_eq!(lg.edges[0].source, 0);
    assert_eq!(lg.edges[0].target, 1);
    assert_eq!(lg.edges[1].source, 1);
    assert_eq!(lg.edges[1].target, 2);
}

#[test]
fn flowchart_subgraph_maps_to_group() {
    let fc = Flowchart {
        direction: None,
        nodes: vec![
            Node {
                id: "A".into(),
                shape: None,
                text: None,
            },
            Node {
                id: "X".into(),
                shape: None,
                text: None,
            },
            Node {
                id: "Y".into(),
                shape: None,
                text: None,
            },
        ],
        edges: vec![Edge {
            source: "A".into(),
            target: "X".into(),
            arrow_type: ArrowType::Solid,
            label: None,
        }],
        subgraphs: vec![Subgraph {
            title: Some("Group".into()),
            nodes: vec![
                Node {
                    id: "X".into(),
                    shape: None,
                    text: None,
                },
                Node {
                    id: "Y".into(),
                    shape: None,
                    text: None,
                },
            ],
            edges: vec![Edge {
                source: "X".into(),
                target: "Y".into(),
                arrow_type: ArrowType::Solid,
                label: None,
            }],
        }],
    };
    let lg = fc.to_layout_graph(&measure());
    assert_eq!(lg.groups.len(), 1, "一个子图 → 一个 LGroup");
    assert_eq!(
        lg.groups[0].title.as_deref(),
        Some("Group"),
        "子图标题映射到 LGroup.title"
    );
    // 组内节点：X(idx1)、Y(idx2)
    use liemermaid::builder::layout::ir::GroupChild;
    let member_ids: Vec<usize> = lg.groups[0]
        .children
        .iter()
        .filter_map(|c| match c {
            GroupChild::Node(i) => Some(*i),
            _ => None,
        })
        .collect();
    assert_eq!(member_ids, vec![1, 2], "组内成员映射到正确的节点索引");
    // A→X 跨组边应进 cross_group_edges
    assert_eq!(lg.cross_group_edges.len(), 1, "A→X 是跨组边");
    assert_eq!(lg.cross_group_edges[0].source, 0);
    assert_eq!(lg.cross_group_edges[0].target, 1);
}

#[test]
fn line_kind_mapping() {
    let fc = Flowchart {
        direction: None,
        nodes: vec![
            Node {
                id: "A".into(),
                shape: None,
                text: None,
            },
            Node {
                id: "B".into(),
                shape: None,
                text: None,
            },
        ],
        edges: vec![Edge {
            source: "A".into(),
            target: "B".into(),
            arrow_type: ArrowType::Dotted,
            label: None,
        }],
        subgraphs: vec![],
    };
    let lg = fc.to_layout_graph(&measure());
    assert_eq!(
        lg.edges[0].line_kind,
        LineKind::Dashed,
        "虚线箭头 → LineKind::Dashed"
    );
}

#[test]
fn self_loop_and_invisible() {
    let fc = Flowchart {
        direction: None,
        nodes: vec![Node {
            id: "A".into(),
            shape: None,
            text: None,
        }],
        edges: vec![Edge {
            source: "A".into(),
            target: "A".into(),
            arrow_type: ArrowType::Solid,
            label: None,
        }],
        subgraphs: vec![],
    };
    let lg = fc.to_layout_graph(&measure());
    // 自环边应保留在 edges（solver/路由层处理 SelfLoop）
    assert_eq!(lg.edges.len(), 1);
    assert_eq!(lg.edges[0].source, 0);
    assert_eq!(lg.edges[0].target, 0);
}
