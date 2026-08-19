use std::collections::HashMap;

use lievisual::text::{RichSpan, layout_text};

use crate::{
    ast::{Node, NodeShape},
    builder::types::OutputConfig,
    vir::{Color, TextAlign, TextBaseline, TextStyle},
};

use super::{
    coord::NodeAnchors,
    types::{GroupId, GroupMetrics, InternalLayout, NodeId, NodeMetrics, Size},
};

const MIN_NODE_WIDTH: f64 = 80.0;
const MIN_NODE_HEIGHT: f64 = 36.0;
const NODE_PAD_X: f64 = 14.0;
const NODE_PAD_Y: f64 = 8.0;
const FONT_SIZE: f64 = 13.0;

const H_GAP: f64 = 60.0;
const V_GAP: f64 = 60.0;

fn shape_multiplier(shape: &Option<NodeShape>) -> (f64, f64, f64, f64) {
    match shape {
        Some(NodeShape::Diamond) => (
            MIN_NODE_WIDTH * 1.4,
            MIN_NODE_HEIGHT * 1.4,
            NODE_PAD_X * 1.6,
            NODE_PAD_Y * 1.6,
        ),
        Some(NodeShape::Hexagon) => (
            MIN_NODE_WIDTH * 1.3,
            MIN_NODE_HEIGHT * 1.2,
            NODE_PAD_X * 1.4,
            NODE_PAD_Y * 1.2,
        ),
        Some(NodeShape::Circle) | Some(NodeShape::DoubleCircle) => (
            MIN_NODE_HEIGHT * 1.3,
            MIN_NODE_HEIGHT * 1.3,
            NODE_PAD_X * 1.3,
            NODE_PAD_Y * 1.3,
        ),
        Some(NodeShape::Cylinder) => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT * 1.2,
            NODE_PAD_X * 1.2,
            NODE_PAD_Y * 1.5,
        ),
        Some(NodeShape::Stadium) => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.5,
            NODE_PAD_Y,
        ),
        Some(NodeShape::Asymmetric) => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.3,
            NODE_PAD_Y,
        ),
        Some(NodeShape::Parallelogram) | Some(NodeShape::ParallelogramAlt) => (
            MIN_NODE_WIDTH * 1.2,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.5,
            NODE_PAD_Y,
        ),
        Some(NodeShape::Trapezoid) | Some(NodeShape::TrapezoidAlt) => (
            MIN_NODE_WIDTH * 1.2,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.4,
            NODE_PAD_Y,
        ),
        _ => (MIN_NODE_WIDTH, MIN_NODE_HEIGHT, NODE_PAD_X, NODE_PAD_Y),
    }
}

/// Pass 2: 测量所有节点的尺寸
pub fn measure_nodes(nodes: &[Node], config: &OutputConfig) -> HashMap<NodeId, NodeMetrics> {
    let mut metrics = HashMap::new();

    for node in nodes {
        let m = measure_node(node, config);
        metrics.insert(node.id.clone(), m);
    }

    metrics
}

fn measure_node(node: &Node, _config: &OutputConfig) -> NodeMetrics {
    let text = node.text.as_deref().unwrap_or(&node.id);

    let text_style = TextStyle::new(Color::BLACK, FONT_SIZE, "sans-serif")
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);

    let layout = layout_text(&[RichSpan::new(text.to_string(), text_style.clone())], None);
    let text_w = layout.width;
    let text_h = layout.height;

    let (min_w, min_h, pad_x, pad_y) = shape_multiplier(&node.shape);

    let width = min_w.max(text_w + 2.0 * pad_x);
    let height = min_h.max(text_h + 2.0 * pad_y);

    let anchors = NodeAnchors::new((width, height));

    NodeMetrics {
        size: Size::new(width, height),
        anchors,
    }
}

/// 计算 Group 的边界尺寸
pub fn measure_groups(
    tree: &super::types::LayoutTree,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
) -> HashMap<GroupId, GroupMetrics> {
    let mut group_metrics = HashMap::new();
    let mut next_id = 0;
    measure_group_recursive(&tree.root, node_metrics, &mut group_metrics, &mut next_id);
    group_metrics
}

fn measure_group_recursive(
    group: &super::types::LogicalGroup,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    group_metrics: &mut HashMap<GroupId, GroupMetrics>,
    next_id: &mut GroupId,
) -> (GroupId, Size) {
    let id = *next_id;
    *next_id += 1;

    let (internal, size) = match group {
        super::types::LogicalGroup::Chain { items } => {
            let item_sizes: Vec<Size> = items
                .iter()
                .filter_map(|item| {
                    item.node_id
                        .as_ref()
                        .and_then(|id| node_metrics.get(id).map(|m| m.size))
                })
                .collect();

            let total_main: f64 = item_sizes.iter().map(|s| s.height).sum::<f64>()
                + (item_sizes.len().saturating_sub(1)) as f64 * V_GAP;
            let max_cross = item_sizes.iter().map(|s| s.width).fold(0.0f64, f64::max);

            let internal = InternalLayout::Chain {
                item_sizes: item_sizes.clone(),
                total_main,
                max_cross,
            };
            let size = Size::new(max_cross, total_main);
            (internal, size)
        }
        super::types::LogicalGroup::Branch { source, arms, sink } => {
            let source_size = node_metrics
                .get(source)
                .map(|m| m.size)
                .unwrap_or(Size::new(MIN_NODE_WIDTH, MIN_NODE_HEIGHT));

            let mut branch_sizes = Vec::new();
            for arm in arms {
                let (_, arm_size) =
                    measure_group_recursive(&arm.body, node_metrics, group_metrics, next_id);
                branch_sizes.push(arm_size);
            }

            let sink_size = sink
                .as_ref()
                .and_then(|s| node_metrics.get(s).map(|m| m.size));

            let branch_total_w: f64 = branch_sizes.iter().map(|s| s.width).sum::<f64>()
                + (branch_sizes.len().saturating_sub(1)) as f64 * H_GAP;
            let cross = source_size
                .width
                .max(branch_total_w)
                .max(sink_size.map(|s| s.width).unwrap_or(0.0));

            let branch_max_h = branch_sizes.iter().map(|s| s.height).fold(0.0f64, f64::max);

            let total_h = source_size.height
                + V_GAP
                + branch_max_h
                + sink_size.map_or(0.0, |s| V_GAP + s.height);

            let internal = InternalLayout::Branch {
                source_size,
                branch_sizes,
                sink_size,
            };
            let size = Size::new(cross, total_h);
            (internal, size)
        }
        super::types::LogicalGroup::Cycle {
            condition,
            body,
            exit,
        } => {
            let condition_size = node_metrics
                .get(condition)
                .map(|m| m.size)
                .unwrap_or(Size::new(MIN_NODE_WIDTH, MIN_NODE_HEIGHT));

            let (_, body_size) =
                measure_group_recursive(body, node_metrics, group_metrics, next_id);

            let exit_size = exit
                .as_ref()
                .and_then(|s| node_metrics.get(s).map(|m| m.size));

            let cross = condition_size
                .width
                .max(body_size.width + H_GAP + body_size.width);

            let cycle_h = condition_size.height.max(body_size.height);
            let total_h = cycle_h + exit_size.map_or(0.0, |s| V_GAP + s.height);

            let internal = InternalLayout::Cycle {
                condition_size,
                body_size,
                exit_size,
            };
            let size = Size::new(cross, total_h);
            (internal, size)
        }
        super::types::LogicalGroup::Leaf { node_id } => {
            let size = node_metrics
                .get(node_id)
                .map(|m| m.size)
                .unwrap_or(Size::new(MIN_NODE_WIDTH, MIN_NODE_HEIGHT));

            let internal = InternalLayout::Chain {
                item_sizes: vec![size],
                total_main: size.height,
                max_cross: size.width,
            };
            (internal, size)
        }
    };

    group_metrics.insert(id, GroupMetrics { size, internal });
    (id, size)
}
