//! 数据流分析：把 petgraph 的图分析结果（SCC / 拓扑序 / 反馈弧 / 连通分量）
//! 显式产出，作为 `DirectedSolver` 启发式编排的**输入事实**。

use petgraph::algo::{tarjan_scc, toposort};
use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;

use super::ir::{LayoutGraph, LineKind};

/// 图分析结果：分组信息 + 顺序信息。
///
/// 节点索引统一使用 `LayoutGraph.nodes` 下标（与 [`super::ir::LNode`] 对应）。
#[derive(Debug, Clone)]
pub struct GraphAnalysis {
    /// petgraph 有向图（节点索引 = `LayoutGraph.nodes` 下标）。
    pub graph: DiGraph<usize, ()>,
    /// SCC 分组：每个强连通分量是环 / 紧密耦合的一组节点（降序，petgraph 默认）。
    pub sccs: Vec<Vec<usize>>,
    /// 拓扑序（有环时反馈弧被忽略，未覆盖的节点按序号补齐）。
    pub topological_order: Vec<usize>,
    /// 反馈弧（构成环的边）。
    pub feedback_arcs: Vec<(usize, usize)>,
    /// 连通分量（弱连通），用于启发式按块排布。
    pub connected_components: Vec<Vec<usize>>,
}

/// 对 `LayoutGraph` 做 petgraph 数据流分析。
pub fn analyze(lg: &LayoutGraph) -> GraphAnalysis {
    let n = lg.nodes.len();
    let mut graph: DiGraph<usize, ()> = DiGraph::with_capacity(n, lg.edges.len());

    // 建立节点索引映射：LayoutGraph.nodes 下标 → petgraph NodeIndex
    let idx_map: Vec<petgraph::graph::NodeIndex> = (0..n).map(|_| graph.add_node(0)).collect();

    // 加边（忽略自环，保留普通边与反馈弧检测）
    for edge in &lg.edges {
        // 忽略自环与不可见边（它们不参与分层拓扑）
        if edge.source == edge.target || edge.line_kind == LineKind::Invisible {
            continue;
        }
        if edge.source >= n || edge.target >= n {
            continue;
        }
        let s = idx_map[edge.source];
        let t = idx_map[edge.target];
        if graph.find_edge(s, t).is_some() {
            // 已有同向边，去重
            continue;
        }
        graph.add_edge(s, t, ());
    }

    // 将 graph 节点权重设为 LayoutGraph.nodes 下标
    for (i, &ni) in idx_map.iter().enumerate() {
        graph[ni] = i;
    }

    // SCC（petgraph 返回 Vec<Vec<NodeIndex>>）
    let scc_components = tarjan_scc(&graph);
    let sccs: Vec<Vec<usize>> = scc_components
        .into_iter()
        .map(|comp| comp.into_iter().map(|ni| graph[ni]).collect())
        .collect();

    // 反馈弧：用 DFS 回边检测找出构成环的边（见 sugiyama 做法）。
    let feedback_arcs = detect_feedback_arcs(&graph);

    // 拓扑序（有环时 toposort 返回 Err，Cycle 内部字段是私有的）。
    // 破环策略：能拓扑排序的节点先按拓扑序，环内 / 未覆盖节点按下标补齐——
    // 既保证「无环部分有序」，又保证确定性（有环时回退到源码顺序锚定）。
    let topological_order = {
        let mut order: Vec<usize> = Vec::with_capacity(n);
        match toposort(&graph, None) {
            Ok(order_pg) => {
                order = order_pg.into_iter().map(|ni| graph[ni]).collect();
            }
            Err(_) => {
                // 无法用私有字段访问 Cycle，直接按源码下标补齐所有节点。
                // 后续 `DirectedSolver` 的启发式会基于 SCC 再做层内重排，
                // 这里仅提供确定性回退。
                order.extend(0..n);
            }
        }
        order
    };

    // 连通分量（弱连通）
    let connected_components = compute_connected_components(&graph, n);

    GraphAnalysis {
        graph,
        sccs,
        topological_order,
        feedback_arcs,
        connected_components,
    }
}

/// 用 DFS 回边检测反馈弧（构成环的边）。
fn detect_feedback_arcs(graph: &DiGraph<usize, ()>) -> Vec<(usize, usize)> {
    use petgraph::Direction;
    use petgraph::visit::NodeIndexable;
    let mut visited = vec![false; graph.node_bound()];
    let mut on_stack = vec![false; graph.node_bound()];
    let mut fas = Vec::new();

    fn dfs(
        graph: &DiGraph<usize, ()>,
        u: petgraph::graph::NodeIndex,
        visited: &mut [bool],
        on_stack: &mut [bool],
        fas: &mut Vec<(usize, usize)>,
    ) {
        visited[u.index()] = true;
        on_stack[u.index()] = true;
        let mut outgoing: Vec<_> = graph
            .edges_directed(u, Direction::Outgoing)
            .map(|e| e.target())
            .collect();
        outgoing.sort_by_key(|v| v.index());
        for v in outgoing {
            if on_stack[v.index()] {
                fas.push((graph[u], graph[v]));
            } else if !visited[v.index()] {
                dfs(graph, v, visited, on_stack, fas);
            }
        }
        on_stack[u.index()] = false;
    }

    for ni in graph.node_indices() {
        if !visited[ni.index()] {
            dfs(graph, ni, &mut visited, &mut on_stack, &mut fas);
        }
    }
    fas
}

/// 弱连通分量分组（DFS/BFS 遍历）。
fn compute_connected_components(graph: &DiGraph<usize, ()>, n: usize) -> Vec<Vec<usize>> {
    use petgraph::Direction;
    use petgraph::visit::NodeIndexable;
    let mut comps: Vec<Vec<usize>> = Vec::new();
    let mut visited = vec![false; graph.node_bound()];
    for ni in graph.node_indices() {
        if visited[ni.index()] {
            continue;
        }
        let mut stack = vec![ni];
        let mut comp = Vec::new();
        visited[ni.index()] = true;
        while let Some(u) = stack.pop() {
            comp.push(graph[u]);
            for e in graph.edges_directed(u, Direction::Outgoing) {
                let v = e.target();
                if !visited[v.index()] {
                    visited[v.index()] = true;
                    stack.push(v);
                }
            }
            for e in graph.edges_directed(u, Direction::Incoming) {
                let v = e.source();
                if !visited[v.index()] {
                    visited[v.index()] = true;
                    stack.push(v);
                }
            }
        }
        comp.sort_unstable();
        comps.push(comp);
    }
    // 若图中没有任何边，每个孤立节点自成一个分量（上面遍历已覆盖）。
    let _ = n;
    comps
}
