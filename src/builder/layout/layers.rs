use std::collections::{HashMap, HashSet};

use super::types::{LayoutTree, LogicalGroup, NodeId};

/// Pass 3: 在 LogicalGroup 基础上进行层级分配
pub fn assign_layers(tree: &LayoutTree) -> HashMap<NodeId, usize> {
    let mut layers = HashMap::new();
    let mut visited: HashSet<NodeId> = HashSet::new();

    assign_group_layers(&tree.root, 0, &mut layers, &mut visited);

    // 处理孤边中未被分配的节点
    for edge in &tree.orphan_edges {
        if !layers.contains_key(&edge.from) {
            layers.insert(edge.from.clone(), 0);
        }
        if !layers.contains_key(&edge.to) {
            let from_layer = layers.get(&edge.from).copied().unwrap_or(0);
            layers.insert(edge.to.clone(), from_layer + 1);
        }
    }

    layers
}

fn assign_group_layers(
    group: &LogicalGroup,
    start_layer: usize,
    layers: &mut HashMap<NodeId, usize>,
    visited: &mut HashSet<NodeId>,
) -> usize {
    match group {
        LogicalGroup::Chain { items } => {
            let mut layer = start_layer;
            for item in items {
                if let Some(node_id) = &item.node_id {
                    if !visited.contains(node_id) {
                        layers.insert(node_id.clone(), layer);
                        visited.insert(node_id.clone());
                    }
                } else if let Some(sub) = &item.sub_group {
                    let end = assign_group_layers(sub, layer, layers, visited);
                    layer = end;
                    continue;
                }
                layer += 1;
            }
            layer
        }
        LogicalGroup::Branch { source, arms, sink } => {
            if !visited.contains(source) {
                layers.insert(source.clone(), start_layer);
                visited.insert(source.clone());
            }

            let branch_layer = start_layer + 1;
            let mut max_end_layer = branch_layer;

            for arm in arms {
                let end = assign_group_layers(&arm.body, branch_layer, layers, visited);
                max_end_layer = max_end_layer.max(end);
            }

            if let Some(sink_node) = sink {
                if !visited.contains(sink_node) {
                    layers.insert(sink_node.clone(), max_end_layer);
                    visited.insert(sink_node.clone());
                }
                max_end_layer + 1
            } else {
                max_end_layer
            }
        }
        LogicalGroup::Cycle {
            condition,
            body,
            exit,
        } => {
            if !visited.contains(condition) {
                layers.insert(condition.clone(), start_layer);
                visited.insert(condition.clone());
            }

            // 循环体在 condition 的同一层（逻辑上在侧面）
            let body_end = assign_group_layers(body, start_layer, layers, visited);

            if let Some(exit_node) = exit {
                if !visited.contains(exit_node) {
                    layers.insert(exit_node.clone(), start_layer + 1);
                    visited.insert(exit_node.clone());
                }
                start_layer + 2
            } else {
                body_end
            }
        }
        LogicalGroup::Leaf { node_id } => {
            if !visited.contains(node_id) {
                layers.insert(node_id.clone(), start_layer);
                visited.insert(node_id.clone());
            }
            start_layer + 1
        }
    }
}
