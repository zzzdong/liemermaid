use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::ast::Flowchart;

use super::types::{BranchArm, ChainItem, GroupEdge, LayoutTree, LogicalGroup, NodeId};

/// 收集流程图中所有参与布局的节点（顶层 + 各 subgraph 内部，按 id 去重）
pub fn all_flowchart_nodes(fc: &Flowchart) -> Vec<crate::ast::Node> {
    let mut seen: HashSet<NodeId> = HashSet::new();
    let mut out: Vec<crate::ast::Node> = Vec::new();
    let push = |n: &crate::ast::Node, seen: &mut HashSet<NodeId>, out: &mut Vec<crate::ast::Node>| {
        if seen.insert(n.id.clone()) {
            out.push(n.clone());
        }
    };
    for node in &fc.nodes {
        push(node, &mut seen, &mut out);
    }
    for sg in &fc.subgraphs {
        for node in &sg.nodes {
            push(node, &mut seen, &mut out);
        }
    }
    out
}

/// 使用 petgraph 构建流程图的有向图（含 subgraph 内部节点与边）
fn build_petgraph(fc: &Flowchart) -> (DiGraph<NodeId, ()>, HashMap<NodeId, NodeIndex>) {
    let mut graph = DiGraph::new();
    let mut id_to_idx = HashMap::new();

    for node in all_flowchart_nodes(fc) {
        if !id_to_idx.contains_key(&node.id) {
            let idx = graph.add_node(node.id.clone());
            id_to_idx.insert(node.id.clone(), idx);
        }
    }

    let add_edge = |edge: &crate::ast::Edge, id_to_idx: &mut HashMap<NodeId, NodeIndex>, graph: &mut DiGraph<NodeId, ()>| {
        if let (Some(&from), Some(&to)) = (id_to_idx.get(&edge.source), id_to_idx.get(&edge.target)) {
            graph.add_edge(from, to, ());
        }
    };
    for edge in &fc.edges {
        add_edge(edge, &mut id_to_idx, &mut graph);
    }
    for sg in &fc.subgraphs {
        for edge in &sg.edges {
            add_edge(edge, &mut id_to_idx, &mut graph);
        }
    }

    (graph, id_to_idx)
}

/// 在 petgraph 图上用 BFS 分配层级，检测回边
fn find_back_edge_indices(graph: &DiGraph<NodeId, ()>) -> HashMap<NodeIndex, NodeIndex> {
    let mut layers: HashMap<NodeIndex, usize> = HashMap::new();
    let mut queue = VecDeque::new();

    for node in graph.node_indices() {
        if graph.neighbors_directed(node, Direction::Incoming).count() == 0 {
            layers.insert(node, 0);
            queue.push_back(node);
        }
    }

    if queue.is_empty()
        && let Some(first) = graph.node_indices().next()
    {
        layers.insert(first, 0);
        queue.push_back(first);
    }

    while let Some(cur) = queue.pop_front() {
        let cur_layer = layers[&cur];
        for target in graph.neighbors(cur) {
            let new_layer = cur_layer + 1;
            let existing = layers.get(&target).copied().unwrap_or(usize::MAX);
            if new_layer < existing {
                layers.insert(target, new_layer);
                queue.push_back(target);
            }
        }
    }

    let mut back = HashMap::new();
    for edge_ref in graph.edge_references() {
        let from = edge_ref.source();
        let to = edge_ref.target();
        let from_layer = layers.get(&from).copied().unwrap_or(0);
        let to_layer = layers.get(&to).copied().unwrap_or(0);
        if from_layer > to_layer {
            back.insert(from, to);
        }
    }
    back
}

/// 从 petgraph 图导出回边集合（供 flowchart.rs 使用）
pub fn compute_flowchart_back_edges(fc: &Flowchart) -> HashSet<(NodeId, NodeId)> {
    let (graph, _) = build_petgraph(fc);
    let back = find_back_edge_indices(&graph);
    back.iter()
        .map(|(from, to)| (graph[*from].clone(), graph[*to].clone()))
        .collect()
}

/// 构建 petgraph 图供外部使用
pub fn build_flowchart_graph(fc: &Flowchart) -> (DiGraph<NodeId, ()>, HashMap<NodeId, NodeIndex>) {
    build_petgraph(fc)
}

pub fn recognize_structure(fc: &Flowchart) -> LayoutTree {
    let (graph, _id_to_idx) = build_petgraph(fc);
    let back_edge_indices = find_back_edge_indices(&graph);

    // 从 petgraph 导出 out_edges HashMap（保持递归函数接口不变）
    let mut out_edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for idx in graph.node_indices() {
        let succs: Vec<NodeId> = graph.neighbors(idx).map(|n| graph[n].clone()).collect();
        out_edges.insert(graph[idx].clone(), succs);
    }

    let mut back_edges: HashMap<NodeId, NodeId> = HashMap::new();
    for (from_idx, to_idx) in &back_edge_indices {
        back_edges.insert(graph[*from_idx].clone(), graph[*to_idx].clone());
    }

    let all_nodes: Vec<NodeId> = graph.node_indices().map(|idx| graph[idx].clone()).collect();

    // 使用 petgraph 查找入度为 0 的入口节点
    let entries: Vec<NodeId> = graph
        .node_indices()
        .filter(|&idx| graph.neighbors_directed(idx, Direction::Incoming).count() == 0)
        .map(|idx| graph[idx].clone())
        .collect();
    // 如果 petgraph 没找到入口（全连通环），取第一个节点
    let entries = if entries.is_empty() {
        all_nodes.first().cloned().into_iter().collect()
    } else {
        entries
    };

    let mut visited: HashSet<NodeId> = HashSet::new();

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
        .map(|e| GroupEdge {
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

fn walk_to_terminal(
    node: &NodeId,
    out_edges: &HashMap<NodeId, Vec<NodeId>>,
    visited: &HashSet<NodeId>,
    back_edges: &HashMap<NodeId, NodeId>,
) -> Option<NodeId> {
    let mut cur = node.clone();
    let mut seen = HashSet::new();
    seen.insert(cur.clone());

    loop {
        if visited.contains(&cur) {
            return Some(cur);
        }

        let succs = match out_edges.get(&cur) {
            Some(s) => s.clone(),
            None => return Some(cur),
        };

        let forward: Vec<&NodeId> = succs
            .iter()
            .filter(|t| back_edges.get(&cur) != Some(*t))
            .collect();

        match forward.len() {
            0 => return Some(cur),
            1 => {
                let next = forward[0].clone();
                if !seen.insert(next.clone()) {
                    return None;
                }
                cur = next;
            }
            _ => return None,
        }
    }
}

fn find_common_sink(
    successors: &[NodeId],
    out_edges: &HashMap<NodeId, Vec<NodeId>>,
    visited: &HashSet<NodeId>,
    back_edges: &HashMap<NodeId, NodeId>,
) -> Option<NodeId> {
    let terminals: Vec<Option<NodeId>> = successors
        .iter()
        .filter(|s| !visited.contains(*s))
        .map(|s| walk_to_terminal(s, out_edges, visited, back_edges))
        .collect();

    if terminals.len() < 2 {
        return None;
    }

    let first = terminals[0].clone()?;

    if terminals
        .iter()
        .all(|t| t.as_ref().is_some_and(|id| *id == first))
    {
        Some(first)
    } else {
        None
    }
}

fn recognize_from_entry_stop_at(
    entry: &NodeId,
    stop_at: Option<&NodeId>,
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

        if stop_at.is_some_and(|sa| cur == *sa) {
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
            if stop_at.is_some_and(|sa| *next == *sa) {
                chain_items.push(ChainItem::leaf(cur.clone()));
                visited.insert(cur.clone());
                break;
            }
            chain_items.push(ChainItem::leaf(cur.clone()));
            visited.insert(cur.clone());
            cur = next.clone();
            continue;
        }

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

        let common_sink = find_common_sink(&forward_targets, out_edges, visited, back_edges);

        if let Some(ref sink) = common_sink {
            visited.insert(sink.clone());
        }

        let mut arms = Vec::new();

        for target in successors {
            if visited.contains(target) {
                continue;
            }
            let arm_body = if common_sink.is_some() {
                recognize_from_entry_stop_at(
                    target,
                    common_sink.as_ref(),
                    out_edges,
                    back_edges,
                    visited,
                )
            } else {
                recognize_from_entry(target, out_edges, back_edges, visited)
            };
            arms.push(BranchArm {
                label: None,
                body: arm_body,
            });
        }

        return LogicalGroup::Branch {
            source: node.clone(),
            arms,
            sink: common_sink,
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
