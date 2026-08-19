// 内部布局辅助函数接收大量上下文参数（metrics / config / next_id / positions），
// 参数数量是该领域 API 的合理设计，统一 allow 而非逐个拆分。
#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;

use lievisual::geometry::Point;

use crate::{ast::Direction, builder::types::OutputConfig};

use super::{
    coord::NodeAnchors,
    types::{
        ChainItem, GroupId, GroupMetrics, InternalLayout, LayoutTree, LogicalGroup, NodeId,
        NodeMetrics, NodePosition, Size,
    },
};

const MARGIN: f64 = 40.0;
const H_GAP: f64 = 60.0;
const V_GAP: f64 = 60.0;

pub fn compute_positions(
    tree: &LayoutTree,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    group_metrics: &HashMap<GroupId, GroupMetrics>,
    config: &OutputConfig,
    direction: Direction,
) -> HashMap<NodeId, NodePosition> {
    let mut positions = HashMap::new();
    let mut next_id = 0;

    position_group(
        &tree.root,
        node_metrics,
        group_metrics,
        &mut next_id,
        config,
        direction,
        0.0,
        &mut positions,
    );

    positions
}

fn position_group(
    group: &LogicalGroup,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    group_metrics: &HashMap<GroupId, GroupMetrics>,
    next_id: &mut GroupId,
    config: &OutputConfig,
    direction: Direction,
    start_main: f64,
    positions: &mut HashMap<NodeId, NodePosition>,
) -> f64 {
    let gid = *next_id;
    *next_id += 1;

    let gmetrics = group_metrics.get(&gid);
    let is_horizontal = direction == Direction::LR || direction == Direction::RL;

    if is_horizontal {
        position_group_horizontal(
            group,
            node_metrics,
            group_metrics,
            next_id,
            config,
            direction,
            start_main,
            positions,
        )
    } else {
        position_group_vertical(
            group,
            node_metrics,
            gmetrics,
            next_id,
            config,
            direction,
            start_main,
            positions,
        )
    }
}

fn position_group_vertical(
    group: &LogicalGroup,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    gmetrics: Option<&GroupMetrics>,
    next_id: &mut GroupId,
    config: &OutputConfig,
    direction: Direction,
    start_main: f64,
    positions: &mut HashMap<NodeId, NodePosition>,
) -> f64 {
    match group {
        LogicalGroup::Chain { items } => position_chain_vertical(
            items,
            node_metrics,
            gmetrics,
            next_id,
            config,
            direction,
            start_main,
            positions,
        ),
        LogicalGroup::Branch { source, arms, sink } => position_branch_vertical(
            source,
            arms,
            sink,
            node_metrics,
            gmetrics,
            config,
            start_main,
            positions,
        ),
        LogicalGroup::Cycle {
            condition,
            body,
            exit,
        } => position_cycle_vertical(
            condition,
            body,
            exit,
            node_metrics,
            next_id,
            config,
            start_main,
            positions,
        ),
        LogicalGroup::Leaf { node_id } => {
            let nm = node_metrics.get(node_id).unwrap();
            let size = nm.size;
            let center = Point::new(config.width / 2.0, start_main + size.height / 2.0);
            positions.insert(
                node_id.clone(),
                NodePosition {
                    center,
                    anchors: NodeAnchors::new((size.width, size.height)),
                },
            );
            start_main + size.height
        }
    }
}

fn position_chain_vertical(
    items: &[ChainItem],
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    gmetrics: Option<&GroupMetrics>,
    next_id: &mut GroupId,
    config: &OutputConfig,
    direction: Direction,
    start_main: f64,
    positions: &mut HashMap<NodeId, NodePosition>,
) -> f64 {
    let canvas_center = config.width / 2.0;
    let mut cur_main = start_main;

    // BT (Bottom-Top): 反转链，让第一个节点在最下方
    let chain_items: Vec<&ChainItem> = if direction == Direction::BT {
        items.iter().rev().collect()
    } else {
        items.iter().collect()
    };

    for item in chain_items {
        if let Some(node_id) = &item.node_id {
            let nm = node_metrics
                .get(node_id)
                .unwrap_or_else(|| unreachable!("Node metrics not found for {}", node_id));
            let size = nm.size;
            let center = Point::new(canvas_center, cur_main + size.height / 2.0);
            positions.insert(
                node_id.clone(),
                NodePosition {
                    center,
                    anchors: NodeAnchors::new((size.width, size.height)),
                },
            );
            cur_main += size.height + V_GAP;
        } else if let Some(sub) = &item.sub_group {
            cur_main = position_group_vertical(
                sub,
                node_metrics,
                gmetrics,
                next_id,
                config,
                direction.clone(),
                cur_main,
                positions,
            );
        }
    }

    cur_main - V_GAP
}

fn position_branch_vertical(
    source: &NodeId,
    arms: &[super::types::BranchArm],
    sink: &Option<NodeId>,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    gmetrics: Option<&GroupMetrics>,
    config: &OutputConfig,
    start_main: f64,
    positions: &mut HashMap<NodeId, NodePosition>,
) -> f64 {
    let canvas_center = config.width / 2.0;
    let source_nm = node_metrics.get(source).unwrap();
    let source_size = source_nm.size;

    // source 居中
    let source_center = Point::new(canvas_center, start_main + source_size.height / 2.0);
    positions.insert(
        source.clone(),
        NodePosition {
            center: source_center,
            anchors: NodeAnchors::new((source_size.width, source_size.height)),
        },
    );

    // branches
    let branch_main = start_main + source_size.height + V_GAP;

    // 使用 GroupMetrics 获取更准确的 arm 尺寸
    // branch_sizes 包含每个 arm body 的总尺寸
    let arm_sizes: Vec<Size> = if let Some(gm) = gmetrics {
        if let InternalLayout::Branch { branch_sizes, .. } = &gm.internal {
            branch_sizes.clone()
        } else {
            compute_arm_sizes_from_first_node(arms, node_metrics)
        }
    } else {
        compute_arm_sizes_from_first_node(arms, node_metrics)
    };

    let total_w: f64 = arm_sizes.iter().map(|s| s.width).sum::<f64>()
        + (arm_sizes.len().saturating_sub(1)) as f64 * H_GAP;
    let start_x = canvas_center - total_w / 2.0;
    let mut cur_x = start_x;

    // 计算所有 arm 第一个节点的最大高度，用于 Y 轴对齐
    let first_node_max_h = arms
        .iter()
        .map(|arm| {
            let first = get_first_node_id(&arm.body);
            node_metrics
                .get(&first)
                .map(|m| m.size.height)
                .unwrap_or(36.0)
        })
        .fold(0.0f64, f64::max);

    for (i, arm) in arms.iter().enumerate() {
        let arm_size = arm_sizes.get(i).copied().unwrap_or(Size::new(140.0, 50.0));
        let arm_center_x = cur_x + arm_size.width / 2.0;
        // 所有 arm 的第一个节点在同一水平线上对齐
        let arm_center_y = branch_main + first_node_max_h / 2.0;
        position_arm(
            &arm.body,
            node_metrics,
            arm_center_x,
            arm_center_y,
            positions,
        );
        cur_x += arm_size.width + H_GAP;
    }

    // sink 定位（始终居中于所有 branch 的下方）
    let branch_max_h = arms
        .iter()
        .map(|arm| {
            let first = get_first_node_id(&arm.body);
            node_metrics
                .get(&first)
                .map(|m| m.size.height)
                .unwrap_or(36.0)
        })
        .fold(0.0f64, f64::max);
    if let Some(sink_node) = sink {
        let sink_nm = node_metrics.get(sink_node).unwrap_or(source_nm);
        let ss = sink_nm.size;
        let sink_main = branch_main + branch_max_h + V_GAP;
        let sink_center = Point::new(canvas_center, sink_main + ss.height / 2.0);
        positions.insert(
            sink_node.clone(),
            NodePosition {
                center: sink_center,
                anchors: NodeAnchors::new((ss.width, ss.height)),
            },
        );
        return sink_main + ss.height;
    }

    branch_main
}

fn compute_arm_sizes_from_first_node(
    arms: &[super::types::BranchArm],
    node_metrics: &HashMap<NodeId, NodeMetrics>,
) -> Vec<Size> {
    arms.iter()
        .map(|arm| {
            let first = get_first_node_id(&arm.body);
            node_metrics
                .get(&first)
                .map(|m| m.size)
                .unwrap_or(Size::new(140.0, 50.0))
        })
        .collect()
}

fn position_cycle_vertical(
    condition: &NodeId,
    body: &LogicalGroup,
    exit: &Option<NodeId>,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    _next_id: &mut GroupId,
    config: &OutputConfig,
    start_main: f64,
    positions: &mut HashMap<NodeId, NodePosition>,
) -> f64 {
    let canvas_center = config.width / 2.0;
    let cond_nm = node_metrics.get(condition).unwrap();
    let cond_size = cond_nm.size;

    // condition 居中
    let cond_center = Point::new(canvas_center, start_main + cond_size.height / 2.0);
    positions.insert(
        condition.clone(),
        NodePosition {
            center: cond_center,
            anchors: NodeAnchors::new((cond_size.width, cond_size.height)),
        },
    );

    // body 在 condition 左侧，与 condition 垂直居中对齐
    let body_first = get_first_node_id(body);
    let body_nm = node_metrics.get(&body_first).unwrap_or(cond_nm);
    let body_size = body_nm.size;
    let body_center_x = MARGIN + body_size.width / 2.0;
    let body_center_y = cond_center.y; // 与 condition 同一垂直中心
    position_arm(body, node_metrics, body_center_x, body_center_y, positions);

    // exit 在 condition 下方
    let cycle_h = cond_size.height.max(body_size.height);

    if let Some(exit_node) = exit {
        let exit_nm = node_metrics.get(exit_node).unwrap_or(cond_nm);
        let exit_size = exit_nm.size;
        let y = start_main + cycle_h + V_GAP + exit_size.height / 2.0;
        positions.insert(
            exit_node.clone(),
            NodePosition {
                center: Point::new(canvas_center, y),
                anchors: NodeAnchors::new((exit_size.width, exit_size.height)),
            },
        );
        y + exit_size.height / 2.0
    } else {
        start_main + cycle_h
    }
}

fn position_group_horizontal(
    group: &LogicalGroup,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    _group_metrics: &HashMap<GroupId, GroupMetrics>,
    next_id: &mut GroupId,
    config: &OutputConfig,
    direction: Direction,
    start_main: f64,
    positions: &mut HashMap<NodeId, NodePosition>,
) -> f64 {
    match group {
        LogicalGroup::Chain { items } => {
            let canvas_center = config.height / 2.0;
            let mut cur_main = start_main;

            // RL (Right-Left): 反转链，让第一个节点在最右侧
            let chain_items: Vec<&ChainItem> = if direction == Direction::RL {
                items.iter().rev().collect()
            } else {
                items.iter().collect()
            };

            for item in chain_items {
                if let Some(node_id) = &item.node_id {
                    let nm = node_metrics.get(node_id).unwrap();
                    let size = nm.size;
                    let center = Point::new(cur_main + size.width / 2.0, canvas_center);
                    positions.insert(
                        node_id.clone(),
                        NodePosition {
                            center,
                            anchors: NodeAnchors::new((size.width, size.height)),
                        },
                    );
                    cur_main += size.width + H_GAP;
                }
            }

            cur_main - H_GAP
        }
        LogicalGroup::Leaf { node_id } => {
            let nm = node_metrics.get(node_id).unwrap();
            let size = nm.size;
            let center = Point::new(start_main + size.width / 2.0, config.height / 2.0);
            positions.insert(
                node_id.clone(),
                NodePosition {
                    center,
                    anchors: NodeAnchors::new((size.width, size.height)),
                },
            );
            start_main + size.width
        }
        _ => position_group_vertical(
            group,
            node_metrics,
            None,
            next_id,
            config,
            direction,
            start_main,
            positions,
        ),
    }
}

fn position_arm(
    group: &LogicalGroup,
    node_metrics: &HashMap<NodeId, NodeMetrics>,
    center_x: f64,
    center_y: f64,
    positions: &mut HashMap<NodeId, NodePosition>,
) {
    match group {
        LogicalGroup::Leaf { node_id } => {
            let nm = node_metrics.get(node_id).unwrap();
            let size = nm.size;
            positions.insert(
                node_id.clone(),
                NodePosition {
                    center: Point::new(center_x, center_y),
                    anchors: NodeAnchors::new((size.width, size.height)),
                },
            );
        }
        LogicalGroup::Chain { items } => {
            let mut cur_y = center_y;
            for (j, item) in items.iter().enumerate() {
                if let Some(node_id) = &item.node_id {
                    let nm = node_metrics.get(node_id).unwrap();
                    let size = nm.size;
                    let node_center_y = if j == 0 {
                        cur_y
                    } else {
                        cur_y + size.height / 2.0
                    };
                    positions.insert(
                        node_id.clone(),
                        NodePosition {
                            center: Point::new(center_x, node_center_y),
                            anchors: NodeAnchors::new((size.width, size.height)),
                        },
                    );
                    cur_y = node_center_y + size.height / 2.0 + V_GAP;
                }
            }
        }
        _ => {
            let node_id = get_first_node_id(group);
            let nm = node_metrics.get(&node_id).unwrap();
            let size = nm.size;
            positions.insert(
                node_id,
                NodePosition {
                    center: Point::new(center_x, center_y),
                    anchors: NodeAnchors::new((size.width, size.height)),
                },
            );
        }
    }
}

fn get_first_node_id(group: &LogicalGroup) -> NodeId {
    match group {
        LogicalGroup::Leaf { node_id } => node_id.clone(),
        LogicalGroup::Chain { items } => items
            .first()
            .and_then(|i| i.node_id.clone())
            .unwrap_or_default(),
        LogicalGroup::Branch { source, .. } => source.clone(),
        LogicalGroup::Cycle { condition, .. } => condition.clone(),
    }
}
