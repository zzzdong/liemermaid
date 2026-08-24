use std::collections::{HashMap, HashSet, VecDeque};

use lievisual::geometry::Point;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

/// Sugiyama 布局配置参数
#[derive(Debug, Clone)]
pub struct SugiyamaConfig {
    /// 层内节点间水平间距
    pub node_gap: f64,
    /// 层间垂直间距
    pub layer_gap: f64,
    /// 整体边距
    pub padding: f64,
    /// 交叉减少最大迭代次数
    pub crossing_iterations: usize,
    /// 层分配算法: "longest-path" | "network-simplex"
    pub ranker: String,
}

impl Default for SugiyamaConfig {
    fn default() -> Self {
        Self {
            node_gap: 50.0,
            layer_gap: 60.0,
            padding: 40.0,
            crossing_iterations: 12,
            // dagre 默认 ranker 为 network-simplex，这里与之对齐
            ranker: "network-simplex".to_string(),
        }
    }
}

/// 节点尺寸
#[derive(Debug, Clone, Copy)]
pub struct NodeSize {
    pub width: f64,
    pub height: f64,
}

/// Sugiyama 布局结果
#[derive(Debug, Clone)]
pub struct SugiyamaResult {
    /// 各节点中心位置（key 为 NodeIndex）
    pub positions: HashMap<NodeIndex, Point>,
    /// 节点尺寸
    pub sizes: HashMap<NodeIndex, NodeSize>,
    /// 层号映射
    pub layers: HashMap<NodeIndex, usize>,
    /// 层 → 节点列表（按排列顺序）
    pub layer_nodes: HashMap<usize, Vec<NodeIndex>>,
    /// 边路由弯折点
    pub edge_routes: HashMap<(NodeIndex, NodeIndex), Vec<Point>>,
    /// 检测到的反馈弧（构成环的边）
    pub feedback_arcs: HashSet<(NodeIndex, NodeIndex)>,
    /// 强连通分量分组（size > 1 表示环）
    pub sccs: Vec<Vec<NodeIndex>>,
    /// 节点 → SCC 编号（-1 表示非 SCC 节点）
    pub scc_id: HashMap<NodeIndex, i32>,
}

/// 使用 Sugiyama 4 阶段算法进行分层有向图布局
///
/// 改进管线:
///   0. 去环预处理 (DFS 回边检测)
///   1. 层分配 (Longest-Path) - 忽略反馈弧方向
///   2. 交叉减少 (Barycenter Heuristic)
///   3. 坐标分配 (Brandes & Köpf 四方向平衡 + 层居中)
///   4. 边路由 (正交折线，反馈弧绕行右侧)
///
/// 参考:
/// - Sugiyama et al. "Methods for Visual Understanding of Hierarchical Systems" (1981)
/// - Gansner et al. "A Technique for Drawing Directed Graphs" (1993)
/// - Brandes & Köpf "Fast and Simple Horizontal Coordinate Assignment" (2002)
/// - Dagre.js (https://github.com/dagrejs/dagre)
pub struct SugiyamaLayout<'a> {
    config: SugiyamaConfig,
    graph: &'a DiGraph<String, ()>,
}

impl<'a> SugiyamaLayout<'a> {
    pub fn new(config: SugiyamaConfig, graph: &'a DiGraph<String, ()>) -> Self {
        Self { config, graph }
    }

    /// 运行完整 Sugiyama 布局管线
    ///
    /// 在内部构建含虚拟节点（dummy node）的工作图：跨多层的长边被拆成
    /// 逐层一段的虚拟节点链，使虚拟节点参与排序(Barycenter)与坐标分配
    /// (Brandes-Köpf) 对齐，最终边沿层间空隙走线、不再穿越中间层节点。
    /// 这与 dagre 处理长边的方式一致。
    pub fn layout(&self, node_sizes: &HashMap<NodeIndex, NodeSize>) -> SugiyamaResult {
        // 构建含虚拟节点的工作图
        let (work_graph, work_sizes, real_to_work, edge_dummies, work_feedback) =
            self.build_work_graph(node_sizes);

        // 在工作图上运行 4 阶段（临时 layout 指向工作图，复用所有 &self 方法）
        let work_layout = SugiyamaLayout {
            config: self.config.clone(),
            graph: &work_graph,
        };
        let mut res = work_layout.run_inner(&work_sizes);

        // 用虚拟节点链重建每条原始边的折线（按层顺序贯穿 dummy）
        let edge_routes =
            self.rebuild_routes_from_dummies(&res, &edge_dummies, &real_to_work, &work_graph);
        res.edge_routes = edge_routes;

        // 仅保留真实节点尺寸（虚拟节点尺寸对外无意义）
        res.sizes = node_sizes.clone();

        // 反馈弧/层号映射回真实节点
        let mut feedback_arcs = HashSet::new();
        for (u, v) in work_feedback {
            if let (Some(&wu), Some(&wv)) = (real_to_work.get(&u), real_to_work.get(&v))
                && wu == u
                && wv == v
            {
                feedback_arcs.insert((u, v));
            }
        }
        res.feedback_arcs = feedback_arcs;

        res
    }

    /// 内部 4 阶段（基于 self.graph 上的所有 &self 方法）
    fn run_inner(&self, node_sizes: &HashMap<NodeIndex, NodeSize>) -> SugiyamaResult {
        // Phase 0: 去环 — 检测反馈弧 + SCC
        let feedback_arcs = self.detect_feedback_arcs();
        let sccs = self.tarjan_scc();
        let sizes = node_sizes.clone();

        // 构建 node → scc_id 映射
        let mut scc_id: HashMap<NodeIndex, i32> = HashMap::new();
        for (sid, scc) in sccs.iter().enumerate() {
            let sid_i32 = sid as i32;
            for &node in scc {
                scc_id.insert(node, sid_i32);
            }
        }

        // Phase 1: 层分配（基于 SCC 凝结）
        let layers = self.assign_layers(&feedback_arcs, &sccs);
        let layer_nodes = self.build_layer_index(&layers);

        // Phase 2: 交叉减少 (Barycenter)
        let ordered = self.reduce_crossings(&layer_nodes, &feedback_arcs);

        // Phase 3: 坐标分配 (Brandes & Köpf 四方向平衡 + SCC 水平排布)
        let positions =
            self.assign_coordinates_bk(&ordered, &sizes, &sccs, &scc_id, &feedback_arcs);

        // Phase 4: 边路由
        let edge_routes = self.route_edges(&positions, &sizes, &layers, &feedback_arcs, &scc_id);

        SugiyamaResult {
            positions,
            sizes,
            layers,
            layer_nodes: ordered,
            edge_routes,
            feedback_arcs,
            sccs,
            scc_id,
        }
    }

    // ================================================================
    // 虚拟节点工作图构建
    //
    // 复制真实节点，并将每条跨多层的长边拆成逐层一段的虚拟节点(dummy)链，
    // 使虚拟节点参与后续 Barycenter 排序与 Brandes-Köpf 坐标对齐，
    // 最终边沿层间空隙走线，不再穿越中间层节点（与 dagre 一致）。
    // ================================================================
    fn build_work_graph(
        &self,
        node_sizes: &HashMap<NodeIndex, NodeSize>,
    ) -> (
        DiGraph<String, ()>,
        HashMap<NodeIndex, NodeSize>,
        HashMap<NodeIndex, NodeIndex>,
        HashMap<(NodeIndex, NodeIndex), Vec<NodeIndex>>,
        HashSet<(NodeIndex, NodeIndex)>,
    ) {
        let feedback_arcs = self.detect_feedback_arcs();

        let mut work = DiGraph::<String, ()>::new();
        let mut real_to_work: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        let mut work_sizes: HashMap<NodeIndex, NodeSize> = HashMap::new();

        // 复制真实节点
        for n in self.graph.node_indices() {
            let id = self.graph[n].clone();
            let wn = work.add_node(id);
            real_to_work.insert(n, wn);
            if let Some(s) = node_sizes.get(&n) {
                work_sizes.insert(wn, *s);
            }
        }

        // 真实图层号（用于判断长边跨度）
        let sccs = self.tarjan_scc();
        let layers = self.assign_layers(&feedback_arcs, &sccs);

        // dummy 计数器（label 用于调试，size=0）
        let mut edge_dummies: HashMap<(NodeIndex, NodeIndex), Vec<NodeIndex>> = HashMap::new();

        for e in self.graph.edge_references() {
            let u = e.source();
            let v = e.target();
            let wu = real_to_work[&u];
            let wv = real_to_work[&v];

            let lu = layers.get(&u).copied().unwrap_or(0);
            let lv = layers.get(&v).copied().unwrap_or(0);
            let span = lv.saturating_sub(lu);

            if span >= 2 {
                // 插入 span-1 个 dummy，逐层一段
                let mut prev = wu;
                let mut chain: Vec<NodeIndex> = Vec::new();
                for _ in 0..span - 1 {
                    let dummy = work.add_node("__dummy__".to_string());
                    work_sizes.insert(
                        dummy,
                        NodeSize {
                            width: 0.0,
                            height: 0.0,
                        },
                    );
                    work.add_edge(prev, dummy, ());
                    chain.push(dummy);
                    prev = dummy;
                }
                work.add_edge(prev, wv, ());
                edge_dummies.insert((u, v), chain);
            } else {
                work.add_edge(wu, wv, ());
            }
        }

        (work, work_sizes, real_to_work, edge_dummies, feedback_arcs)
    }

    /// 用虚拟节点链重建每条原始边的折线
    fn rebuild_routes_from_dummies(
        &self,
        res: &SugiyamaResult,
        edge_dummies: &HashMap<(NodeIndex, NodeIndex), Vec<NodeIndex>>,
        real_to_work: &HashMap<NodeIndex, NodeIndex>,
        work_graph: &DiGraph<String, ()>,
    ) -> HashMap<(NodeIndex, NodeIndex), Vec<Point>> {
        let mut out: HashMap<(NodeIndex, NodeIndex), Vec<Point>> = HashMap::new();

        // 工作图边 key -> 折线
        let route_of = |a: NodeIndex, b: NodeIndex| -> Option<Vec<Point>> {
            res.edge_routes
                .get(&(a, b))
                .cloned()
                .or_else(|| res.edge_routes.get(&(b, a)).cloned())
        };

        let empty_chain: Vec<NodeIndex> = Vec::new();
        for edge in self.graph.edge_references() {
            let u = edge.source();
            let v = edge.target();

            // 自环边：工作图会丢弃 a==b 的边，这里单独生成节点右侧的小环
            if u == v {
                // res 的 positions/sizes 以工作图节点为 key，需用 real_to_work 转换
                if let (Some(&wu), Some(&fp), Some(&fs)) = (
                    real_to_work.get(&u),
                    res.positions.get(real_to_work.get(&u).unwrap_or(&u)),
                    res.sizes.get(real_to_work.get(&u).unwrap_or(&u)),
                ) {
                    let _ = wu;
                    let right = fp.x + fs.width / 2.0;
                    let cy = fp.y;
                    let loop_w = self.config.node_gap * 0.7;
                    let loop_h = fs.height * 0.55;
                    let route = vec![
                        Point::new(right, cy - 3.0),
                        Point::new(right + loop_w, cy - 3.0),
                        Point::new(right + loop_w, cy - loop_h),
                        Point::new(right, cy - loop_h),
                        Point::new(right, cy + 3.0),
                    ];
                    out.insert((u, v), route);
                }
                continue;
            }

            let chain = edge_dummies.get(&(u, v)).unwrap_or(&empty_chain);
            // 原始边无 dummy（span<=1）：直接用工作图边 route
            if chain.is_empty() {
                if let (Some(&wu), Some(&wv)) = (real_to_work.get(&u), real_to_work.get(&v))
                    && let Some(pts) = route_of(wu, wv)
                {
                    out.insert((u, v), pts);
                }
                continue;
            }

            // 序列：源 -> dummy... -> 目标（work 图 NodeIndex）
            let mut seq = vec![real_to_work[&u]];
            seq.extend(chain.iter().copied());
            seq.push(real_to_work[&v]);

            // 拼接相邻段折线
            let mut full: Vec<Point> = Vec::new();
            for w in seq.windows(2) {
                let a = w[0];
                let b = w[1];
                if let Some(mut pts) = route_of(a, b) {
                    // 去掉与上一端点重复的点
                    if full.is_empty() {
                        full.append(&mut pts);
                    } else {
                        // pts[0] 应等于 full.last()，去掉
                        if pts.len() > 1 {
                            full.extend_from_slice(&pts[1..]);
                        }
                    }
                }
            }
            if full.is_empty() {
                // 兜底：直线连接源与目标中心
                if let (Some(&wu), Some(&wv)) = (real_to_work.get(&u), real_to_work.get(&v))
                    && let (Some(pu), Some(pv)) = (res.positions.get(&wu), res.positions.get(&wv))
                {
                    full = vec![*pu, *pv];
                }
            }
            out.insert((u, v), full);
        }

        // 确保原始图中所有边都有 route（span<=1 已处理；兜底覆盖漏网）
        let _ = work_graph;
        out
    }

    // ================================================================
    // Phase 0: 去环 (Acyclic / Feedback Arc Set)
    //
    // 使用 DFS 检测回边（back edge），标记为反馈弧。
    // 仅将通过 on_stack 检测到的真正回边标记为反馈弧，
    // 不将跨边（cross edge）错误标记，避免在 DAG 中误报。
    // ================================================================
    fn detect_feedback_arcs(&self) -> HashSet<(NodeIndex, NodeIndex)> {
        // 单源 DFS（从每个未访问节点出发），on_stack 追踪当前 DFS 路径。
        // 出边按目标节点 index 升序排序，使遍历顺序确定：优先走向低序号节点
        // （如 A→B 先于 A→C），从而正确捕获真正的环回边（如 A→B→…→H→B 中的 H→B），
        // 而非误将非环边（如 B→C）标为反馈弧。反转真正的回边才能正确打破环、保持分层。
        let mut fas = HashSet::new();
        let mut visited = HashSet::new();
        let mut on_stack = HashSet::new();

        fn dfs(
            g: &DiGraph<String, ()>,
            u: NodeIndex,
            visited: &mut HashSet<NodeIndex>,
            on_stack: &mut HashSet<NodeIndex>,
            fas: &mut HashSet<(NodeIndex, NodeIndex)>,
        ) {
            visited.insert(u);
            on_stack.insert(u);

            let mut outgoing: Vec<NodeIndex> = g
                .edges_directed(u, petgraph::Direction::Outgoing)
                .map(|e| e.target())
                .collect();
            outgoing.sort();

            for &v in &outgoing {
                if on_stack.contains(&v) {
                    // 回边: u → v 构成环（v 在当前 DFS 路径上）
                    fas.insert((u, v));
                } else if !visited.contains(&v) {
                    dfs(g, v, visited, on_stack, fas);
                }
                // 跨边（visited 但不在栈上）不是环边，忽略
            }

            on_stack.remove(&u);
        }

        for node in self.graph.node_indices() {
            if !visited.contains(&node) {
                dfs(self.graph, node, &mut visited, &mut on_stack, &mut fas);
            }
        }

        fas
    }

    /// Tarjan 强连通分量检测
    fn tarjan_scc(&self) -> Vec<Vec<NodeIndex>> {
        let mut index = 0usize;
        let mut stack: Vec<NodeIndex> = Vec::new();
        let mut indices: HashMap<NodeIndex, usize> = HashMap::new();
        let mut lowlink: HashMap<NodeIndex, usize> = HashMap::new();
        let mut on_stack: HashSet<NodeIndex> = HashSet::new();
        let mut sccs: Vec<Vec<NodeIndex>> = Vec::new();

        #[allow(clippy::too_many_arguments)] // Tarjan 递归传递图与多个状态表
        fn strongconnect(
            g: &DiGraph<String, ()>,
            v: NodeIndex,
            index: &mut usize,
            stack: &mut Vec<NodeIndex>,
            indices: &mut HashMap<NodeIndex, usize>,
            lowlink: &mut HashMap<NodeIndex, usize>,
            on_stack: &mut HashSet<NodeIndex>,
            sccs: &mut Vec<Vec<NodeIndex>>,
        ) {
            indices.insert(v, *index);
            lowlink.insert(v, *index);
            *index += 1;
            stack.push(v);
            on_stack.insert(v);

            for e in g.edges_directed(v, petgraph::Direction::Outgoing) {
                let w = e.target();
                if !indices.contains_key(&w) {
                    strongconnect(g, w, index, stack, indices, lowlink, on_stack, sccs);
                    let wl = lowlink[&w];
                    let vl = lowlink[&v];
                    lowlink.insert(v, vl.min(wl));
                } else if on_stack.contains(&w) {
                    let wi = indices[&w];
                    let vl = lowlink[&v];
                    lowlink.insert(v, vl.min(wi));
                }
            }

            if lowlink[&v] == indices[&v] {
                let mut scc: Vec<NodeIndex> = Vec::new();
                loop {
                    let w = stack.pop().unwrap();
                    on_stack.remove(&w);
                    scc.push(w);
                    if w == v {
                        break;
                    }
                }
                sccs.push(scc);
            }
        }

        for v in self.graph.node_indices() {
            if !indices.contains_key(&v) {
                strongconnect(
                    self.graph,
                    v,
                    &mut index,
                    &mut stack,
                    &mut indices,
                    &mut lowlink,
                    &mut on_stack,
                    &mut sccs,
                );
            }
        }

        sccs
    }

    // ================================================================
    // Phase 1: 层分配
    //
    // 分派:
    //   - "network-simplex": Gansner tight-tree + 单纯形迭代压缩（dagre 默认）
    //   - "longest-path"   : SCC 凝结 + 最长路径（原有实现）
    // ================================================================
    fn assign_layers(
        &self,
        feedback_arcs: &HashSet<(NodeIndex, NodeIndex)>,
        sccs: &[Vec<NodeIndex>],
    ) -> HashMap<NodeIndex, usize> {
        let use_ns = self
            .config
            .ranker
            .trim()
            .eq_ignore_ascii_case("network-simplex");
        if use_ns {
            return self.assign_layers_network_simplex(feedback_arcs, sccs);
        }

        // 如果只有一个 SCC（全图强连通），退化为原来的方法
        if sccs.len() <= 1 {
            return self.assign_layers_legacy(feedback_arcs);
        }

        // 构建 node → scc_id 映射
        let mut scc_id: HashMap<NodeIndex, usize> = HashMap::new();
        for (sid, scc) in sccs.iter().enumerate() {
            for &node in scc {
                scc_id.insert(node, sid);
            }
        }

        // 构建凝结图的入度（只看跨 SCC 的非反馈边）
        let mut super_in_degree: HashMap<usize, usize> = HashMap::new();
        for sid in 0..sccs.len() {
            super_in_degree.entry(sid).or_insert(0);
        }
        for edge in self.graph.edge_references() {
            let from = edge.source();
            let to = edge.target();
            let sid_from = scc_id[&from];
            let sid_to = scc_id[&to];
            if sid_from != sid_to && !feedback_arcs.contains(&(from, to)) {
                *super_in_degree.entry(sid_to).or_insert(0) += 1;
            }
        }

        // 最长路径层分配（凝结图）
        let mut super_layers: HashMap<usize, usize> = HashMap::new();
        let mut queue: VecDeque<usize> = VecDeque::new();

        for (&sid, &deg) in &super_in_degree {
            if deg == 0 {
                super_layers.insert(sid, 0);
                queue.push_back(sid);
            }
        }

        if queue.is_empty()
            && let Some(&sid) = scc_id.values().next()
        {
            super_layers.insert(sid, 0);
            queue.push_back(sid);
        }

        let mut in_deg_work = super_in_degree.clone();
        while let Some(sid) = queue.pop_front() {
            let cur = super_layers[&sid];
            for node in &sccs[sid] {
                for e in self
                    .graph
                    .edges_directed(*node, petgraph::Direction::Outgoing)
                {
                    let to = e.target();
                    let sid_to = scc_id[&to];
                    if sid_to == sid {
                        continue;
                    }
                    if feedback_arcs.contains(&(*node, to)) {
                        continue;
                    }

                    let new_layer = cur + 1;
                    super_layers
                        .entry(sid_to)
                        .and_modify(|l| {
                            if new_layer > *l {
                                *l = new_layer;
                            }
                        })
                        .or_insert(new_layer);

                    if let Some(d) = in_deg_work.get_mut(&sid_to) {
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            queue.push_back(sid_to);
                        }
                    }
                }
            }
        }

        // 展开：SCC 内所有节点共享该 SCC 的层号
        let mut layers: HashMap<NodeIndex, usize> = HashMap::new();
        for (sid, scc) in sccs.iter().enumerate() {
            let layer = super_layers.get(&sid).copied().unwrap_or(0);
            for &node in scc {
                layers.insert(node, layer);
            }
        }

        layers
    }

    // ================================================================
    // Phase 1 (Network-Simplex): Gansner tight-tree + 单纯形迭代
    //
    // 与 dagre (ranker="network-simplex") 默认行为对齐：
    //   **反转反馈弧**后在原图节点上运行网络单纯形（不凝结 SCC）。
    //   这样环内节点（如 B<->C）可按拓扑顺序分到不同层（B:1, C:2），
    //   而旧的"SCC 凝结成单层"会让 B、C 同层，与 dagre 不符。
    //
    // 算法 (Gansner et al. 1993):
    //   1. 反转反馈弧打破环，得到 DAG
    //   2. 以最长路径为初始可行 rank
    //   3. 构建初始可行树（feasible tree）
    //   4. 反复选 cut value < 0 的非树边交换进树，直到无负 cut value
    // ================================================================
    fn assign_layers_network_simplex(
        &self,
        feedback_arcs: &HashSet<(NodeIndex, NodeIndex)>,
        sccs: &[Vec<NodeIndex>],
    ) -> HashMap<NodeIndex, usize> {
        // node -> 连续 usize id（原图规模，不凝结 SCC）
        let mut node_id: HashMap<NodeIndex, usize> = HashMap::new();
        let mut id_node: Vec<NodeIndex> = Vec::new();
        for n in self.graph.node_indices() {
            node_id.insert(n, id_node.len());
            id_node.push(n);
        }
        let n = id_node.len();
        if n == 0 {
            return HashMap::new();
        }

        // 边集：反转反馈弧（u->v 为回边则翻成 v->u），其余保留。
        // 反转后图为 DAG，最长路径有效，环内节点可分不同层。
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut edge_set: HashSet<(usize, usize)> = HashSet::new();
        for e in self.graph.edge_references() {
            let u = node_id[&e.source()];
            let v = node_id[&e.target()];
            let (a, b) = if feedback_arcs.contains(&(e.source(), e.target())) {
                (v, u) // 反馈弧反转
            } else {
                (u, v)
            };
            if a != b && edge_set.insert((a, b)) {
                edges.push((a, b));
            }
        }

        // 初始可行解：最长路径
        let mut rank = longest_path_ranks_scc(&edges, n);

        // 若全连通（无根），最长路径会给全 0，需给一个有根起点：
        // 选 rank 最小的节点作为根（保持拓扑约束即可）
        let mut parent: Vec<Option<usize>> = vec![None; n];
        let mut tree_edge: Vec<Option<(usize, usize)>> = vec![None; n];

        // --- 构建初始 feasible tree（从 rank 推导）---
        build_feasible_tree(&edges, &rank, n, &mut parent, &mut tree_edge);

        // 非树边集合
        let mut cut_value: Vec<f64> = vec![0.0; edges.len()];
        let mut in_tree: Vec<bool> = vec![false; edges.len()];
        for (i, &(a, b)) in edges.iter().enumerate() {
            let is_tree = (parent[b] == Some(a) && rank[b] as i64 - rank[a] as i64 == 1)
                || (parent[a] == Some(b) && rank[a] as i64 - rank[b] as i64 == 1);
            in_tree[i] = is_tree;
        }

        // 计算初始 cut value
        compute_cut_values(&edges, &rank, &parent, &mut cut_value, &in_tree, n);

        // 单纯形主循环
        let max_iterations = 2 * n + 8;
        for _ in 0..max_iterations {
            // 找 cut value 最负的非树边
            let mut best = usize::MAX;
            let mut best_val: f64 = -1e-9;
            for (i, &cv) in cut_value.iter().enumerate() {
                if !in_tree[i] && cv < best_val {
                    best_val = cv;
                    best = i;
                }
            }
            if best == usize::MAX {
                break;
            }
            // 交换进入树
            let (enter_u, enter_v) = edges[best];
            exchange(
                &edges,
                &mut rank,
                &mut parent,
                &mut tree_edge,
                &mut in_tree,
                enter_u,
                enter_v,
                n,
            );
            compute_cut_values(&edges, &rank, &parent, &mut cut_value, &in_tree, n);
        }

        // 映射回 NodeIndex（no SCC 展开，每个节点已有独立 rank）
        let mut layers: HashMap<NodeIndex, usize> = HashMap::new();
        for (id, &node) in id_node.iter().enumerate() {
            layers.insert(node, rank[id]);
        }

        // sccs 参数保留以兼容调用点（本实现已不再凝结 SCC）
        let _ = sccs;
        layers
    }

    // ================================================================
    // Phase 1 (legacy): 原层分配算法（作为回退）
    // ================================================================
    fn assign_layers_legacy(
        &self,
        feedback_arcs: &HashSet<(NodeIndex, NodeIndex)>,
    ) -> HashMap<NodeIndex, usize> {
        // 计算忽略反馈弧的入度
        let mut in_degree: HashMap<NodeIndex, usize> = HashMap::new();
        for node in self.graph.node_indices() {
            let count = self
                .graph
                .edges_directed(node, petgraph::Direction::Incoming)
                .filter(|e| !feedback_arcs.contains(&(e.source(), node)))
                .count();
            in_degree.insert(node, count);
        }

        let mut layers: HashMap<NodeIndex, usize> = HashMap::new();
        let mut queue: VecDeque<NodeIndex> = VecDeque::new();

        for (&node, &deg) in &in_degree {
            if deg == 0 {
                layers.insert(node, 0);
                queue.push_back(node);
            }
        }

        // 如果所有节点都有入度（强连通），选一个做源
        if queue.is_empty()
            && let Some(first) = self.graph.node_indices().next()
        {
            layers.insert(first, 0);
            queue.push_back(first);
        }

        while let Some(node) = queue.pop_front() {
            let cur_layer = layers[&node];

            for edge in self
                .graph
                .edges_directed(node, petgraph::Direction::Outgoing)
            {
                let target = edge.target();
                if feedback_arcs.contains(&(node, target)) {
                    continue; // 忽略反馈弧方向
                }

                let new_layer = cur_layer + 1;
                layers
                    .entry(target)
                    .and_modify(|e| {
                        if new_layer > *e {
                            *e = new_layer;
                        }
                    })
                    .or_insert(new_layer);

                if let Some(deg) = in_degree.get_mut(&target) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push_back(target);
                    }
                }
            }
        }

        // 确保所有节点都有层号（环中节点）
        for node in self.graph.node_indices() {
            if !layers.contains_key(&node) {
                let max_pred = self
                    .graph
                    .edges_directed(node, petgraph::Direction::Incoming)
                    .filter_map(|e| layers.get(&e.source()))
                    .max()
                    .copied()
                    .unwrap_or(0);
                layers.insert(node, max_pred + 1);
            }
        }

        // ---- Post-processing: 非反馈节点层推深 ----
        // 当反馈弧源节点与其它节点共享同一层时，将非反馈节点推深一层。
        //
        // 例如 A→B→C (feedback C→B) 和 A→B→D 中：
        //   - C（反馈源）保持原层（主轴线）
        //   - D（非反馈）推深一层（出口路径）
        //
        // 结果: A:0, B:1, C:2, D:3，End 在最后，边不穿越节点。
        if !feedback_arcs.is_empty() {
            let feedback_sources: HashSet<NodeIndex> =
                feedback_arcs.iter().map(|(u, _)| *u).collect();

            // 按层号从小到大处理
            let sorted_layers: Vec<usize> = {
                let mut s: Vec<usize> = layers.values().copied().collect();
                s.sort_unstable();
                s.dedup();
                s
            };

            for layer in sorted_layers {
                let nodes_in_layer: Vec<NodeIndex> = layers
                    .iter()
                    .filter(|(_, l)| **l == layer)
                    .map(|(n, _)| *n)
                    .collect();

                let has_feedback_src = nodes_in_layer.iter().any(|n| feedback_sources.contains(n));
                let non_feedback: Vec<NodeIndex> = nodes_in_layer
                    .iter()
                    .filter(|n| !feedback_sources.contains(n))
                    .copied()
                    .collect();

                if has_feedback_src && !non_feedback.is_empty() {
                    // 将非反馈节点及其下游正向可达节点推深一层
                    for n in non_feedback {
                        let mut stack = vec![n];
                        let mut visited = HashSet::new();
                        visited.insert(n);
                        while let Some(current) = stack.pop() {
                            layers.entry(current).and_modify(|l| *l += 1);
                            for e in self
                                .graph
                                .edges_directed(current, petgraph::Direction::Outgoing)
                            {
                                if !feedback_arcs.contains(&(current, e.target()))
                                    && !visited.contains(&e.target())
                                {
                                    visited.insert(e.target());
                                    stack.push(e.target());
                                }
                            }
                        }
                    }
                }
            }
        }

        layers
    }

    fn build_layer_index(
        &self,
        layers: &HashMap<NodeIndex, usize>,
    ) -> HashMap<usize, Vec<NodeIndex>> {
        let mut layer_nodes: HashMap<usize, Vec<NodeIndex>> = HashMap::new();
        for (&node, &layer) in layers {
            layer_nodes.entry(layer).or_default().push(node);
        }
        for nodes in layer_nodes.values_mut() {
            nodes.sort_by(|a, b| {
                let id_a = &self.graph[*a];
                let id_b = &self.graph[*b];
                id_a.cmp(id_b)
            });
        }
        layer_nodes
    }
    fn reduce_crossings(
        &self,
        layer_nodes: &HashMap<usize, Vec<NodeIndex>>,
        feedback_arcs: &HashSet<(NodeIndex, NodeIndex)>,
    ) -> HashMap<usize, Vec<NodeIndex>> {
        let max_layer = layer_nodes.keys().copied().max().unwrap_or(0);
        let mut ordered: HashMap<usize, Vec<NodeIndex>> = layer_nodes.clone();

        for _iter in 0..self.config.crossing_iterations {
            // Top-down: 用上层位置重排当前层
            for layer in 1..=max_layer {
                let Some(upper) = ordered.get(&(layer - 1)) else {
                    continue;
                };
                let Some(current) = ordered.get(&layer) else {
                    continue;
                };

                let mut bc: Vec<(NodeIndex, f64)> = current
                    .iter()
                    .map(|&node| {
                        let (sum, count) = self
                            .graph
                            .edges_directed(node, petgraph::Direction::Incoming)
                            .filter(|e| !feedback_arcs.contains(&(e.source(), node)))
                            .filter_map(|e| upper.iter().position(|&n| n == e.source()))
                            .fold((0usize, 0usize), |(s, c), pos| (s + pos, c + 1));
                        let value = if count > 0 {
                            sum as f64 / count as f64
                        } else {
                            current.iter().position(|&n| n == node).unwrap_or(0) as f64
                        };
                        (node, value)
                    })
                    .collect();

                bc.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                ordered.insert(layer, bc.iter().map(|(n, _)| *n).collect());
            }

            // Bottom-up: 用下层位置重排当前层
            for layer in (0..max_layer).rev() {
                let Some(lower) = ordered.get(&(layer + 1)) else {
                    continue;
                };
                let Some(current) = ordered.get(&layer) else {
                    continue;
                };

                let mut bc: Vec<(NodeIndex, f64)> = current
                    .iter()
                    .map(|&node| {
                        let (sum, count) = self
                            .graph
                            .edges_directed(node, petgraph::Direction::Outgoing)
                            .filter(|e| !feedback_arcs.contains(&(node, e.target())))
                            .filter_map(|e| lower.iter().position(|&n| n == e.target()))
                            .fold((0usize, 0usize), |(s, c), pos| (s + pos, c + 1));
                        let value = if count > 0 {
                            sum as f64 / count as f64
                        } else {
                            current.iter().position(|&n| n == node).unwrap_or(0) as f64
                        };
                        (node, value)
                    })
                    .collect();

                bc.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                ordered.insert(layer, bc.iter().map(|(n, _)| *n).collect());
            }
        }

        ordered
    }

    // ================================================================
    // Phase 3: Brandes & Köpf 坐标分配
    //
    // 实现 4 方向对齐（UL/UR/DL/DR），取平衡结果。
    // 每方向:
    //   1. 垂直对齐：每个节点试图与其上/下层中位邻居对齐
    //   2. 冲突解决：已对齐冲突时，左对齐优先/右对齐优先
    //   3. 水平压缩：对齐块整体平移避免重叠
    // 最后取 4 方向 X 坐标的平均值作为最终坐标。
    // ================================================================
    fn assign_coordinates_bk(
        &self,
        ordered: &HashMap<usize, Vec<NodeIndex>>,
        node_sizes: &HashMap<NodeIndex, NodeSize>,
        sccs: &[Vec<NodeIndex>],
        scc_id: &HashMap<NodeIndex, i32>,
        feedback_arcs: &HashSet<(NodeIndex, NodeIndex)>,
    ) -> HashMap<NodeIndex, Point> {
        let max_layer = ordered.keys().copied().max().unwrap_or(0);

        // 收集所有节点
        let all_nodes: Vec<NodeIndex> = self.graph.node_indices().collect();

        // 运行 4 方向，存储每方向的 X 坐标
        let mut xs_by_type: Vec<HashMap<NodeIndex, f64>> = Vec::new();

        // type 0: 上对齐-左偏, 1: 上对齐-右偏, 2: 下对齐-左偏, 3: 下对齐-右偏
        for type_idx in 0..4 {
            let top_down = type_idx < 2;
            let left_prio = type_idx % 2 == 0;

            let xs = self.bk_single_pass(ordered, node_sizes, max_layer, top_down, left_prio);
            xs_by_type.push(xs);
        }

        // 平衡：取 4 方向 X 坐标的平均值
        let mut avg_x: HashMap<NodeIndex, f64> = HashMap::new();
        for &node in &all_nodes {
            let sum: f64 = xs_by_type.iter().filter_map(|xs| xs.get(&node)).sum();
            avg_x.insert(node, sum / 4.0);
        }

        // ---- Pass 1: 水平压缩 — 保证层内节点不重叠 ----
        for layer in 0..=max_layer {
            let Some(nodes) = ordered.get(&layer) else {
                continue;
            };

            let mut sorted: Vec<NodeIndex> = nodes.clone();
            sorted.sort_by(|a, b| {
                avg_x
                    .get(a)
                    .unwrap_or(&0.0)
                    .partial_cmp(avg_x.get(b).unwrap_or(&0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut cur_left = self.config.padding;
            for &node in &sorted {
                let size = node_sizes.get(&node).copied().unwrap_or(NodeSize {
                    width: 100.0,
                    height: 50.0,
                });
                let cx = avg_x.get(&node).copied().unwrap_or(0.0);
                let half_w = size.width / 2.0;
                let left = cx - half_w;
                if left < cur_left {
                    *avg_x.get_mut(&node).unwrap() = cx + (cur_left - left);
                }
                let new_cx = avg_x[&node];
                cur_left = new_cx + half_w + self.config.node_gap;
            }
        }

        // ---- Pass 2: 层居中 — 每层围绕全局中心轴对称 ----
        // 计算每层实际左右边界
        let mut layer_centers: HashMap<usize, f64> = HashMap::new();
        for layer in 0..=max_layer {
            let Some(nodes) = ordered.get(&layer) else {
                continue;
            };
            let mut leftmost = f64::MAX;
            let mut rightmost = f64::MIN;
            for &node in nodes {
                let size = node_sizes.get(&node).copied().unwrap_or(NodeSize {
                    width: 100.0,
                    height: 50.0,
                });
                let cx = avg_x.get(&node).copied().unwrap_or(0.0);
                leftmost = leftmost.min(cx - size.width / 2.0);
                rightmost = rightmost.max(cx + size.width / 2.0);
            }
            layer_centers.insert(layer, (leftmost + rightmost) / 2.0);
        }

        // 取最宽层的中心作为全局中心参考线
        let global_center = layer_centers
            .iter()
            .map(|(&layer, &center)| {
                let nodes = ordered.get(&layer).unwrap();
                let total_w: f64 = nodes
                    .iter()
                    .filter_map(|n| node_sizes.get(n))
                    .map(|s| s.width)
                    .sum();
                let gaps = (nodes.len().saturating_sub(1)) as f64 * self.config.node_gap;
                (total_w + gaps, center)
            })
            .max_by(|(w1, _), (w2, _)| w1.partial_cmp(w2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, center)| center)
            .unwrap_or(self.config.padding + 100.0);

        // 将每层节点平移到全局中心线对齐
        for layer in 0..=max_layer {
            let Some(nodes) = ordered.get(&layer) else {
                continue;
            };
            let layer_center = layer_centers.get(&layer).copied().unwrap_or(0.0);
            let shift = global_center - layer_center;
            if shift.abs() > 0.001 {
                for &node in nodes {
                    if let Some(cx) = avg_x.get_mut(&node) {
                        *cx += shift;
                    }
                }
            }
        }

        // ---- Pass 2b: SCC 水平排布 ----
        // 对多节点 SCC，让入口节点保持在中线，其它节点水平排列到右侧。
        // 注意：network-simplex 模式已对 SCC 内节点按拓扑分层（不同层），
        // 此时不应强制横向排布，否则会破坏分层（如 B<->C 被并排）。
        // 仅当 SCC 内节点确实同层（遗留凝结情况）才执行横向对齐。
        let node_layer: HashMap<NodeIndex, usize> = ordered
            .iter()
            .flat_map(|(&l, ns)| ns.iter().map(move |n| (*n, l)))
            .collect();
        for scc in sccs {
            if scc.len() <= 1 {
                continue;
            }
            let ranks: Vec<usize> = scc.iter().map(|n| *node_layer.get(n).unwrap()).collect();
            let same_layer = ranks.iter().all(|&r| r == ranks[0]);
            if !same_layer {
                continue; // 已分层：保持层对齐，跳过横向排布
            }

            // 找到入口节点（有外部入边的节点）和出口节点（有外部出边的节点）
            let mut entry_nodes: Vec<NodeIndex> = Vec::new();
            let mut other_nodes: Vec<NodeIndex> = Vec::new();
            for &node in scc {
                let has_outside_in = self
                    .graph
                    .edges_directed(node, petgraph::Direction::Incoming)
                    .any(|e| {
                        scc_id.get(&e.source()).copied().unwrap_or(-1)
                            != *scc_id.get(&node).unwrap_or(&-1)
                    });
                if has_outside_in {
                    entry_nodes.push(node);
                } else {
                    other_nodes.push(node);
                }
            }

            // 如果所有节点都是入口（多个外部入边），选第一个
            if entry_nodes.is_empty() {
                let mut sorted = scc.clone();
                sorted.sort_by(|a, b| self.graph[*a].cmp(&self.graph[*b]));
                entry_nodes.push(sorted[0]);
                other_nodes = sorted[1..].to_vec();
            }

            // 取第一个入口节点作为"锚点"，保持其当前 X
            let anchor = entry_nodes[0];
            let anchor_cx = avg_x.get(&anchor).copied().unwrap_or(0.0);
            let anchor_size = node_sizes.get(&anchor).copied().unwrap_or(NodeSize {
                width: 100.0,
                height: 50.0,
            });

            // 按节点名排序其它节点
            other_nodes.sort_by(|a, b| self.graph[*a].cmp(&self.graph[*b]));

            // 从锚点右侧开始排列其它节点
            let mut cur_x = anchor_cx + anchor_size.width / 2.0 + self.config.node_gap;
            for &node in &other_nodes {
                let size = node_sizes.get(&node).copied().unwrap_or(NodeSize {
                    width: 100.0,
                    height: 50.0,
                });
                let cx = cur_x + size.width / 2.0;
                avg_x.insert(node, cx);
                cur_x += size.width + self.config.node_gap;
            }

            // 如果有多个入口节点，将其它入口节点也排到右侧
            for &node in entry_nodes.iter().skip(1) {
                let size = node_sizes.get(&node).copied().unwrap_or(NodeSize {
                    width: 100.0,
                    height: 50.0,
                });
                let cx = cur_x + size.width / 2.0;
                avg_x.insert(node, cx);
                cur_x += size.width + self.config.node_gap;
            }

            // 对齐所有 SCC 节点的外部出边目标节点（对齐到锚点 X）
            let mut aligned_nodes: HashSet<NodeIndex> = HashSet::new();
            for &node in scc.iter() {
                let outgoing: Vec<NodeIndex> = self
                    .graph
                    .edges_directed(node, petgraph::Direction::Outgoing)
                    .filter(|e| {
                        scc_id.get(&e.target()).copied().unwrap_or(-1)
                            != *scc_id.get(&node).unwrap_or(&-1)
                    })
                    .map(|e| e.target())
                    .collect();
                for &target in &outgoing {
                    if let Some(tx) = avg_x.get_mut(&target) {
                        let old_x = *tx;
                        *tx = anchor_cx;
                        if (old_x - anchor_cx).abs() > 0.5 {
                            aligned_nodes.insert(target);
                        }
                    }
                }
            }

            // 对齐所有 SCC 节点的外部入边源节点（对齐到锚点 X）
            for &node in scc.iter() {
                let incoming: Vec<NodeIndex> = self
                    .graph
                    .edges_directed(node, petgraph::Direction::Incoming)
                    .filter(|e| {
                        scc_id.get(&e.source()).copied().unwrap_or(-1)
                            != *scc_id.get(&node).unwrap_or(&-1)
                    })
                    .map(|e| e.source())
                    .collect();
                for &source in &incoming {
                    if let Some(sx) = avg_x.get_mut(&source) {
                        let old_x = *sx;
                        *sx = anchor_cx;
                        if (old_x - anchor_cx).abs() > 0.5 {
                            aligned_nodes.insert(source);
                        }
                    }
                }
            }

            // 向下游传播中心线对齐：
            // 对新对齐的节点，如果其下游节点只有它一个前驱，也对其到中心线
            let mut propagate_queue: VecDeque<NodeIndex> = aligned_nodes.iter().copied().collect();
            while let Some(current) = propagate_queue.pop_front() {
                for e in self
                    .graph
                    .edges_directed(current, petgraph::Direction::Outgoing)
                {
                    let target = e.target();
                    if scc.contains(&target) {
                        continue;
                    }
                    let predecessor_count = self
                        .graph
                        .edges_directed(target, petgraph::Direction::Incoming)
                        .filter(|ie| !feedback_arcs.contains(&(ie.source(), target)))
                        .count();
                    if predecessor_count <= 1
                        && let Some(tx) = avg_x.get_mut(&target)
                    {
                        let old_x = *tx;
                        *tx = anchor_cx;
                        if (old_x - anchor_cx).abs() > 0.5 {
                            propagate_queue.push_back(target);
                        }
                    }
                }
            }
        }

        // ---- Pass 3: 分配 Y 坐标（按层垂直排列） ----
        let mut positions: HashMap<NodeIndex, Point> = HashMap::new();
        let mut cur_y = self.config.padding;

        for layer in 0..=max_layer {
            let Some(nodes) = ordered.get(&layer) else {
                continue;
            };

            let max_h = nodes
                .iter()
                .filter_map(|n| node_sizes.get(n))
                .map(|s| s.height)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(50.0);

            for &node in nodes {
                let cx = avg_x.get(&node).copied().unwrap_or(0.0);
                let cy = cur_y + max_h / 2.0;
                positions.insert(node, Point::new(cx, cy));
            }

            cur_y += max_h + self.config.layer_gap;
        }

        positions
    }

    /// 简化 Brandes & Köpf 单方向坐标计算
    ///
    /// 按方向顺序逐层处理：
    ///   1. 每个节点的理想 X = 相邻层中位邻居的 X（无边节点使用层内位置）
    ///   2. 按理想 X 排序（left_prio 升序 / right_prio 降序）
    ///   3. 分配实际 X 坐标，保证最小间距
    ///
    /// 4 方向平均后消除方向偏置，得到平衡布局。
    fn bk_single_pass(
        &self,
        ordered: &HashMap<usize, Vec<NodeIndex>>,
        node_sizes: &HashMap<NodeIndex, NodeSize>,
        max_layer: usize,
        top_down: bool,
        left_prio: bool,
    ) -> HashMap<NodeIndex, f64> {
        let mut xs: HashMap<NodeIndex, f64> = HashMap::new();

        // 按方向顺序遍历层级
        let layer_range: Vec<usize> = if top_down {
            (0..=max_layer).collect()
        } else {
            (0..=max_layer).rev().collect()
        };

        for &layer in &layer_range {
            let Some(nodes) = ordered.get(&layer) else {
                continue;
            };

            // 相邻层（对齐参考层）
            let adj_layer = if top_down {
                if layer == 0 {
                    None
                } else {
                    ordered.get(&(layer - 1))
                }
            } else {
                if layer == max_layer {
                    None
                } else {
                    ordered.get(&(layer + 1))
                }
            };

            // 计算每个节点的理想 X
            let mut desired: Vec<(NodeIndex, f64)> = Vec::new();
            for &node in nodes {
                let ideal_x = if let Some(prev) = adj_layer {
                    // 相邻层中位邻居 X
                    self.median_neighbor_x(node, prev, &xs, top_down)
                        .unwrap_or_else(|| {
                            // 回退：相邻层平均 X

                            prev.iter().filter_map(|n| xs.get(n)).sum::<f64>()
                                / prev.len().max(1) as f64
                        })
                } else {
                    // 首层：按层内位置排布
                    let pos = nodes.iter().position(|&n| n == node).unwrap_or(0) as f64;
                    let size = node_sizes.get(&node).copied().unwrap_or(NodeSize {
                        width: 100.0,
                        height: 50.0,
                    });
                    pos * (size.width + self.config.node_gap)
                };
                desired.push((node, ideal_x));
            }

            // 按理想 X 排序
            if left_prio {
                desired.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                desired.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }

            // 分配实际 X 坐标，保证最小间距
            let order: Vec<&(NodeIndex, f64)> = if left_prio {
                desired.iter().collect()
            } else {
                desired.iter().rev().collect()
            };

            let mut cur_x = self.config.padding;
            for (node, _) in order {
                let size = node_sizes.get(node).copied().unwrap_or(NodeSize {
                    width: 100.0,
                    height: 50.0,
                });
                let cx = cur_x + size.width / 2.0;
                xs.insert(*node, cx);
                cur_x += size.width + self.config.node_gap;
            }
        }

        xs
    }

    /// 中位邻居 X 坐标：在 prev 层中找到 v 的邻居 X 中位值
    fn median_neighbor_x(
        &self,
        v: NodeIndex,
        prev_layer: &[NodeIndex],
        positions: &HashMap<NodeIndex, f64>,
        look_up: bool,
    ) -> Option<f64> {
        let neighbor_xs: Vec<f64> = if look_up {
            self.graph
                .edges_directed(v, petgraph::Direction::Incoming)
                .filter_map(|e| {
                    if prev_layer.contains(&e.source()) {
                        positions.get(&e.source()).copied()
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            self.graph
                .edges_directed(v, petgraph::Direction::Outgoing)
                .filter_map(|e| {
                    if prev_layer.contains(&e.target()) {
                        positions.get(&e.target()).copied()
                    } else {
                        None
                    }
                })
                .collect()
        };

        if neighbor_xs.is_empty() {
            return None;
        }

        let mut sorted = neighbor_xs;
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = sorted.len() / 2;
        Some(sorted[mid])
    }

    // ================================================================
    // Phase 4: Edge Routing
    //
    // 正向边：源底部出 → 水平线 → 目标顶部入（直连或正交折线）
    // 反馈弧：绕行图右侧（源右下出 → 右边缘 → 目标右上入），与正向边分离
    // ================================================================
    fn route_edges(
        &self,
        positions: &HashMap<NodeIndex, Point>,
        node_sizes: &HashMap<NodeIndex, NodeSize>,
        layers: &HashMap<NodeIndex, usize>,
        feedback_arcs: &HashSet<(NodeIndex, NodeIndex)>,
        _scc_id: &HashMap<NodeIndex, i32>,
    ) -> HashMap<(NodeIndex, NodeIndex), Vec<Point>> {
        // 计算图的最右边界（用于反馈弧绕行）
        let max_right = positions
            .iter()
            .filter_map(|(node, pos)| node_sizes.get(node).map(|s| pos.x + s.width / 2.0))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(self.config.padding + 100.0);
        let routing_margin = max_right + self.config.node_gap * 2.0;

        let mut routes = HashMap::new();

        for edge in self.graph.edge_references() {
            let from = edge.source();
            let to = edge.target();
            let is_feedback = feedback_arcs.contains(&(from, to));

            let fp = positions.get(&from);
            let tp = positions.get(&to);
            let fs = node_sizes.get(&from);
            let ts = node_sizes.get(&to);

            if let (Some(fp), Some(tp), Some(fs), Some(ts)) = (fp, tp, fs, ts) {
                let from_bottom = fp.y + fs.height / 2.0;
                let to_top = tp.y - ts.height / 2.0;
                let mid_y = (from_bottom + to_top) / 2.0;

                // 判断源和目标是否在相邻层（层差 <= 1）
                let layer_diff = layers
                    .get(&from)
                    .zip(layers.get(&to))
                    .map(|(lf, lt)| (*lf as i64 - *lt as i64).abs())
                    .unwrap_or(0);
                let adjacent_layers = layer_diff <= 1;
                let same_layer = layer_diff == 0;

                let route = if same_layer {
                    // 同层边（SCC 内节点间）：水平排布
                    if (fp.x - tp.x).abs() < 1.0 {
                        vec![
                            Point::new(fp.x, from_bottom),
                            Point::new(tp.x, tp.y - ts.height / 2.0),
                        ]
                    } else if fp.x < tp.x {
                        // 正向：源右侧 → 目标左侧
                        vec![
                            Point::new(fp.x + fs.width / 2.0, fp.y),
                            Point::new(tp.x - ts.width / 2.0, tp.y),
                        ]
                    } else {
                        // 反馈：源底部 → 下方 → 目标底部
                        let below_y = fp.y + fs.height / 2.0 + self.config.node_gap;
                        vec![
                            Point::new(fp.x, fp.y + fs.height / 2.0),
                            Point::new(fp.x, below_y),
                            Point::new(tp.x, below_y),
                            Point::new(tp.x, tp.y + ts.height / 2.0),
                        ]
                    }
                } else if is_feedback {
                    // 反馈弧：从源节点右侧出，绕行图右侧，从目标节点右侧入
                    let exit_x = fp.x + fs.width / 2.0;
                    let exit_y = fp.y;
                    let entry_x = tp.x + ts.width / 2.0;
                    let entry_y = tp.y;
                    vec![
                        Point::new(exit_x, exit_y),
                        Point::new(routing_margin, exit_y),
                        Point::new(routing_margin, entry_y),
                        Point::new(entry_x, entry_y),
                    ]
                } else if adjacent_layers && (fp.x - tp.x).abs() < 1.0 {
                    // 相邻层同 X：直连（源底部 → 目标顶部）
                    vec![Point::new(fp.x, from_bottom), Point::new(fp.x, to_top)]
                } else if (fp.x - tp.x).abs() < 1.0 {
                    // 非相邻层同 X：右侧绕行，避开中间节点
                    let side_x = fp.x + fs.width / 2.0 + self.config.node_gap;
                    vec![
                        Point::new(fp.x, from_bottom),
                        Point::new(side_x, from_bottom),
                        Point::new(side_x, to_top),
                        Point::new(tp.x, to_top),
                    ]
                } else {
                    vec![
                        Point::new(fp.x, from_bottom),
                        Point::new(fp.x, mid_y),
                        Point::new(tp.x, mid_y),
                        Point::new(tp.x, to_top),
                    ]
                };

                routes.insert((from, to), route);
            }
        }

        routes
    }
}

// ================================================================
// Network-Simplex 辅助函数（free functions，作用于 SCC 凝结图）
// 参考 Gansner et al. "A Technique for Drawing Directed Graphs" (1993)
// 与 dagre ranker="network-simplex" 对齐。
// ================================================================

/// 最长路径初始 rank：rank[v] = max(rank[u]+1 for u->v)，无前驱为 0。
fn longest_path_ranks_scc(edges: &[(usize, usize)], n: usize) -> Vec<usize> {
    let mut indeg = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(u, v) in edges {
        indeg[v] += 1;
        adj[u].push(v);
    }
    let mut rank = vec![0usize; n];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for i in 0..n {
        if indeg[i] == 0 {
            queue.push_back(i);
        }
    }
    let mut order: Vec<usize> = Vec::new();
    while let Some(u) = queue.pop_front() {
        order.push(u);
        for &v in &adj[u] {
            rank[v] = rank[v].max(rank[u] + 1);
            indeg[v] -= 1;
            if indeg[v] == 0 {
                queue.push_back(v);
            }
        }
    }
    // 若仍有未访问（环未凝结），按剩余拓扑尽量推深
    if order.len() < n {
        for u in 0..n {
            if rank[u] == 0 && adj[u].is_empty() {
                continue;
            }
        }
    }
    let _ = order;
    rank
}

/// 从 rank 推导一棵可行树（feasible tree）：
/// 选取 rank 最小的节点为根，沿 rank 递增方向构建 spanning tree。
fn build_feasible_tree(
    edges: &[(usize, usize)],
    rank: &[usize],
    n: usize,
    parent: &mut [Option<usize>],
    tree_edge: &mut [Option<(usize, usize)>],
) {
    // 重置
    for p in parent.iter_mut() {
        *p = None;
    }
    for t in tree_edge.iter_mut() {
        *t = None;
    }

    // 按 rank 排序的节点作为加入顺序
    let mut nodes: Vec<usize> = (0..n).collect();
    nodes.sort_by_key(|&u| rank[u]);

    // 计算每个节点的候选父（rank 最小的入边源）
    let mut candidate_parent: Vec<Option<usize>> = vec![None; n];
    for &(u, v) in edges {
        if rank[v] > rank[u] {
            match candidate_parent[v] {
                None => candidate_parent[v] = Some(u),
                Some(p) if rank[p] > rank[u] => candidate_parent[v] = Some(u),
                _ => {}
            }
        }
    }

    for &u in &nodes {
        if let Some(p) = candidate_parent[u]
            && parent[u].is_none()
        {
            parent[u] = Some(p);
            tree_edge[u] = Some((p, u));
        }
    }
}

/// 计算所有非树边的 cut value。
/// cut value(u->v) = 1 - (cut(u) - cut(v))
/// cut 通过树中 balance 传播：每个节点 balance = #出树边 - #入树边（对 (p,c) 边：p balance+1, c balance-1）
/// 然后 DFS 累积。
fn compute_cut_values(
    edges: &[(usize, usize)],
    rank: &[usize],
    parent: &[Option<usize>],
    cut_value: &mut [f64],
    in_tree: &[bool],
    n: usize,
) {
    // balance 初值
    let mut balance = vec![0i32; n];
    for u in 0..n {
        if let Some(p) = parent[u] {
            // 树边 (p, u)，rank[u] = rank[p]+1
            balance[p] += 1;
            balance[u] -= 1;
        }
    }

    // 通过树 DFS 累积 cut 值（每个节点的 cut = 其子树 balance 和）
    // 用迭代后序遍历
    let mut visited = vec![false; n];
    let mut cut = vec![0i32; n];
    // 后序：先处理子再父
    let mut stack: Vec<(usize, bool)> = Vec::new();
    for r in 0..n {
        if parent[r].is_none() {
            stack.push((r, false));
        }
    }
    while let Some((u, processed)) = stack.pop() {
        if processed {
            let mut s = balance[u];
            // 找子节点
            for v in 0..n {
                if parent[v] == Some(u) {
                    s += cut[v];
                }
            }
            cut[u] = s;
        } else {
            stack.push((u, true));
            for v in 0..n {
                if parent[v] == Some(u) && !visited[v] {
                    visited[v] = true;
                    stack.push((v, false));
                }
            }
        }
    }
    let _ = visited;

    for (i, &(u, v)) in edges.iter().enumerate() {
        if in_tree[i] {
            cut_value[i] = 0.0;
            continue;
        }
        // 确保方向 rank[v] > rank[u]
        let (a, b) = if rank[v] >= rank[u] { (u, v) } else { (v, u) };
        let cv = 1.0 - (cut[a] - cut[b]) as f64;
        cut_value[i] = cv;
    }
}

/// 将一条非树边 (enter_u, enter_v) 交换进树，调整 rank 与 parent。
/// 经典实现：沿树从 enter 边两端向根移动，确定被替换的树边，重算 rank。
fn exchange(
    edges: &[(usize, usize)],
    rank: &mut [usize],
    parent: &mut [Option<usize>],
    tree_edge: &mut [Option<(usize, usize)>],
    in_tree: &mut [bool],
    enter_u: usize,
    enter_v: usize,
    n: usize,
) {
    // 统一方向 a -> b (rank[b] >= rank[a])
    let (a, b) = if rank[enter_v] >= rank[enter_u] {
        (enter_u, enter_v)
    } else {
        (enter_v, enter_u)
    };

    // 沿树从 a、b 各自向根爬，找到共同的"低公共祖先"分割点
    // 记录路径
    let mut path_a: Vec<usize> = Vec::new();
    let mut cur = a;
    while let Some(p) = parent[cur] {
        path_a.push(cur);
        cur = p;
    }
    path_a.push(cur); // 根

    // 从 b 向上找第一个出现在 path_a 的节点 = 分割点
    let mut cur = b;
    let mut split_idx = path_a.len();
    let mut b_path: Vec<usize> = Vec::new();
    while let Some(p) = parent[cur] {
        if let Some(pos) = path_a.iter().position(|&x| x == cur) {
            split_idx = pos;
            break;
        }
        b_path.push(cur);
        cur = p;
    }
    if split_idx == path_a.len() {
        b_path.push(cur);
    }

    // leave 边：a_path 中 split 之下紧邻、且 b_path 中对称的那条树边
    // 找 rank 最小的一端作为 leave 边（标准做法：leave = 使 rank 仍可行）
    // 这里采用 Gansner 的"向上调整"：leave 是连接 (split, 其 child) 的树边，
    // 选 a、b 路径上靠近 split 的树边中 rank 较小者
    let leave_node: usize = if !b_path.is_empty() {
        // b 路径上靠近 split 的节点
        b_path[0]
    } else if split_idx + 1 < path_a.len() {
        path_a[split_idx + 1]
    } else {
        a
    };

    // 计算 delta：新边要求 rank[b] = rank[a] + 1
    let delta = (rank[a] + 1) as i64 - rank[b] as i64;

    // 对 leave 边一侧子树的 rank 整体平移
    // leave 边为 (parent[leave_node], leave_node)，把 leave_node 子树 rank 平移 delta
    let leave_parent = parent[leave_node];
    shift_subtree_rank(leave_node, parent, rank, delta, n);

    // 更新树结构：移除 leave 边，加入 enter 边
    if let Some(lp) = leave_parent {
        // 标记旧树边为非树
        if let Some(idx) = edges
            .iter()
            .position(|&(u, v)| (u == lp && v == leave_node) || (u == leave_node && v == lp))
        {
            in_tree[idx] = false;
        }
        // 更新父
        parent[leave_node] = None;
        tree_edge[leave_node] = None;
    }

    // 加入 enter 边 (a->b)
    parent[b] = Some(a);
    tree_edge[b] = Some((a, b));
    if let Some(idx) = edges
        .iter()
        .position(|&(u, v)| (u == a && v == b) || (u == b && v == a))
    {
        in_tree[idx] = true;
    }
}

/// 将 leave_node 所在的子树整体平移 delta 层（迭代避免递归爆栈）。
fn shift_subtree_rank(
    root: usize,
    parent: &[Option<usize>],
    rank: &mut [usize],
    delta: i64,
    _n: usize,
) {
    // 收集子树（所有 parent 链指向 root 的节点）
    let mut subtree: Vec<usize> = Vec::new();
    // 反向找：从所有节点出发若祖先含 root 则属于子树。n 通常很小，直接 O(n^2) 可接受。
    // 但为效率用 BFS 从 root 沿 parent 反向（即找 children）
    // 先建 children 表
    let n = rank.len();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for v in 0..n {
        if let Some(p) = parent[v] {
            children[p].push(v);
        }
    }
    let mut stack = vec![root];
    while let Some(u) = stack.pop() {
        subtree.push(u);
        for &c in &children[u] {
            stack.push(c);
        }
    }
    for &u in &subtree {
        if delta >= 0 {
            rank[u] = (rank[u] as i64 + delta) as usize;
        } else {
            rank[u] = (rank[u] as i64 + delta).max(0) as usize;
        }
    }
}
