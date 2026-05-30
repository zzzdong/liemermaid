use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::{Edge, Flowchart};

use super::types::{BranchArm, ChainItem, GroupEdge, LayoutTree, LogicalGroup, NodeId};

fn build_graph(
    edges: &[Edge],
) -> (HashMap<NodeId, usize>, HashMap<NodeId, Vec<NodeId>>) {
    let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
    let mut out_edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    for edge in edges {
        out_edges
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        *in_degree.entry(edge.target.clone()).or_insert(0) += 1;
        in_degree.entry(edge.source.clone()).or_insert(0);
    }

    (in_degree, out_edges)
}

fn find_back_edges(
    out_edges: &HashMap<NodeId, Vec<NodeId>>,
    all_nodes: &[NodeId],
) -> HashMap<NodeId, NodeId> {
    let mut layers: HashMap<NodeId, usize> = HashMap::new();
    let mut queue = VecDeque::new();
    let mut in_deg: HashMap<NodeId, usize> = HashMap::new();

    for n in all_nodes {
        in_deg.entry(n.clone()).or_insert(0);
    }
    for (_, targets) in out_edges {
        for t in targets {
            *in_deg.entry(t.clone()).or_insert(0) += 1;
        }
    }

    for n in all_nodes {
        if in_deg.get(n).copied().unwrap_or(0) == 0 {
            layers.insert(n.clone(), 0);
            queue.push_back(n.clone());
        }
    }
    if queue.is_empty() {
        if let Some(first) = all_nodes.first() {
            layers.insert(first.clone(), 0);
            queue.push_back(first.clone());
        }
    }

    while let Some(cur) = queue.pop_front() {
        let cur_layer = layers[&cur];
        if let Some(targets) = out_edges.get(&cur) {
            for t in targets {
                let new_layer = cur_layer + 1;
                let existing = layers.get(t).copied().unwrap_or(usize::MAX);
                if new_layer < existing {
                    layers.insert(t.clone(), new_layer);
                    queue.push_back(t.clone());
                }
            }
        }
    }

    let mut back_edges = HashMap::new();
    for (from, targets) in out_edges {
        let from_layer = layers.get(from).copied().unwrap_or(0);
        for to in targets {
            let to_layer = layers.get(to).copied().unwrap_or(0);
            if from_layer > to_layer {
                back_edges.insert(from.clone(), to.clone());
            }
        }
    }
    back_edges
}

pub fn recognize_structure(fc: &Flowchart) -> LayoutTree {
    let (in_degree, out_edges) = build_graph(&fc.edges);
    let all_nodes: Vec<NodeId> = fc.nodes.iter().map(|n| n.id.clone()).collect();
    let back_edges = find_back_edges(&out_edges, &all_nodes);

    let mut visited: HashSet<NodeId> = HashSet::new();

    let mut entries: Vec<NodeId> = all_nodes
        .iter()
        .filter(|n| in_degree.get(*n).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();
    if entries.is_empty() {
        if let Some(first) = all_nodes.first() {
            entries.push(first.clone());
        }
    }

    let root = if entries.len() == 1 {
        recognize_from_entry(&entries[0], &out_edges, &back_edges, &mut visited)
    } else {
        let mut items: Vec<ChainItem> = Vec::new();
        for entry in &entries {
            if visited.contains(entry) {
                continue;
            }
            let g = recognize_from_entry(entry, &out_edges, &back_edges, &mut visited);
            push_to_chain(&mut items, g);
        }
        for node in &all_nodes {
            if !visited.contains(node) {
                items.push(ChainItem::leaf(node.clone()));
                visited.insert(node.clone());
            }
        }
        LogicalGroup::Chain { items }
    };

    let orphan_edges = fc
        .edges
        .iter()
        .enumerate()
        .map(|(_i, e)| GroupEdge {
            from: e.source.clone(),
            to: e.target.clone(),
            edge: e.clone(),
            from_group: 0,
            to_group: 0,
        })
        .collect();

    LayoutTree { root, orphan_edges }
}

fn push_to_chain(items: &mut Vec<ChainItem>, group: LogicalGroup) {
    match group {
        LogicalGroup::Leaf { node_id } => items.push(ChainItem::leaf(node_id)),
        LogicalGroup::Chain { items: sub } => items.extend(sub),
        other => items.push(ChainItem::group(other)),
    }
}

fn recognize_from_entry(
    entry: &NodeId,
    out_edges: &HashMap<NodeId, Vec<NodeId>>,
    back_edges: &HashMap<NodeId, NodeId>,
    visited: &mut HashSet<NodeId>,
) -> LogicalGroup {
    let mut chain_items: Vec<ChainItem> = Vec::new();
    let mut cur = entry.clone();

    loop {
        if visited.contains(&cur) {
            break;
        }

        let succs = out_edges.get(&cur).cloned().unwrap_or_default();

        if succs.is_empty() {
            chain_items.push(ChainItem::leaf(cur.clone()));
            visited.insert(cur.clone());
            break;
        }

        if succs.len() == 1 {
            let next = &succs[0];
            if visited.contains(next) {
                chain_items.push(ChainItem::leaf(cur.clone()));
                visited.insert(cur.clone());
                break;
            }
            chain_items.push(ChainItem::leaf(cur.clone()));
            visited.insert(cur.clone());
            cur = next.clone();
            continue;
        }

        // Multi-successor
        let sub = recognize_multi_successor(&cur, &succs, out_edges, back_edges, visited);
        chain_items.push(ChainItem::group(sub));
        break;
    }

    if chain_items.len() == 1 {
        let item = chain_items.into_iter().next().unwrap();
        if let Some(node_id) = item.node_id {
            LogicalGroup::Leaf { node_id }
        } else if let Some(g) = item.sub_group {
            *g
        } else {
            LogicalGroup::Leaf {
                node_id: "unknown".to_string(),
            }
        }
    } else {
        LogicalGroup::Chain { items: chain_items }
    }
}

fn recognize_multi_successor(
    node: &NodeId,
    successors: &[NodeId],
    out_edges: &HashMap<NodeId, Vec<NodeId>>,
    back_edges: &HashMap<NodeId, NodeId>,
    visited: &mut HashSet<NodeId>,
) -> LogicalGroup {
    let mut back_targets: Vec<NodeId> = Vec::new();
    let mut forward_targets: Vec<NodeId> = Vec::new();

    for s in successors {
        let other_succs = out_edges.get(s).cloned().unwrap_or_default();
        let has_back = other_succs.iter().any(|t| back_edges.get(t) == Some(node));
        let is_back = back_edges.get(s) == Some(node);

        if has_back || is_back {
            back_targets.push(s.clone());
        } else {
            forward_targets.push(s.clone());
        }
    }

    if back_targets.len() == 1 && forward_targets.len() == 1 {
        visited.insert(node.clone());
        let body = recognize_from_entry(&back_targets[0], out_edges, back_edges, visited);

        let exit = if !visited.contains(&forward_targets[0]) {
            let exit_group =
                recognize_from_entry(&forward_targets[0], out_edges, back_edges, visited);
            extract_leaf_node(&exit_group)
        } else {
            forward_targets[0].clone()
        };

        return LogicalGroup::Cycle {
            condition: node.clone(),
            body: Box::new(body),
            exit: Some(exit),
        };
    }

    if !forward_targets.is_empty() {
        visited.insert(node.clone());
        let mut arms = Vec::new();
        let mut sink_candidates: Vec<NodeId> = Vec::new();

        for target in successors {
            if visited.contains(target) {
                continue;
            }
            let arm_body = recognize_from_entry(target, out_edges, back_edges, visited);
            let sink = extract_leaf_node(&arm_body);
            sink_candidates.push(sink);
            arms.push(BranchArm {
                label: None,
                body: arm_body,
            });
        }

        let sink = if sink_candidates.len() > 1
            && sink_candidates.iter().all(|s| s == &sink_candidates[0])
        {
            Some(sink_candidates[0].clone())
        } else {
            None
        };

        return LogicalGroup::Branch {
            source: node.clone(),
            arms,
            sink,
        };
    }

    LogicalGroup::Leaf {
        node_id: node.clone(),
    }
}

fn extract_leaf_node(group: &LogicalGroup) -> NodeId {
    match group {
        LogicalGroup::Leaf { node_id } => node_id.clone(),
        LogicalGroup::Chain { items } => items
            .last()
            .and_then(|i| i.node_id.clone())
            .unwrap_or_default(),
        LogicalGroup::Branch { sink, .. } => sink.clone().unwrap_or_default(),
        LogicalGroup::Cycle { exit, .. } => exit.clone().unwrap_or_default(),
    }
}
