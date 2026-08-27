# 边感知布局（Edge-Aware Layout）设计

> 状态：草案 / 待评审
> 关联文档：`layout-system-design.md`（当前布局管线）、`layout-refactor.md`、`refactor-layout.md`
> 提出背景：现有布局算法在求解阶段**只建模节点、不建模连接线**，导致连线交叉 / 重叠 / 贴边问题无法在布局层被系统性消除。

---

## 1. 问题陈述

用户的判断：*“布局时现在只是考虑了 node，没有考虑连接线。一般 mermaid 都会避免连线交叉重叠之类的。”*

这一判断与代码事实一致。当前布局管线的核心 IR 是：

```text
AST ──convert──► LayoutGraph(纯拓扑) ──solver──► PlacedGraph(纯几何)
```

其中 `LayoutGraph` 把"边"简化为 `LEdge { source, target, ports, line_kind }`，
而 `PlacedGraph` 把"边"简化为一串几何折线 `edge_routes: Vec<Vec<Point>>`。

**关键事实**：在整条管线里，边的几何是**节点位置算好之后才被贴上去的**，
它从未作为"布局求解的输入约束"参与节点排布。具体表现在三层：

### 1.1 求解阶段：节点排布无视边

- `GridSolver`（class / er）：仅用 BFS 入度分层 + 按源码顺序水平排布，
  节点的同层相对顺序**完全由 AST 出现顺序决定**，与边怎么连无关。
  class 图的多重继承线、er 图的多对多关系线极易交叉重叠。
- `LinearSolver` / `SimpleSolver`：按出现顺序线性排布，边只是事后折线。
- `DirectedSolver`（flowchart / state）：虽然用 `heuristic_order` 做了拓扑序，
  但本质是"让边尽量自上而下"，并没有以"减少边交叉"为目标去优化同层顺序。

### 1.2 有向图虽有交叉减少，但只做了一半

`sugiyama.rs` 实现了 `reduce_crossings`（barycenter 重心启发式），
配置 `crossing_iterations: 12`。**但这只解决了"同层节点顺序导致的最小化相邻层边交叉"，
存在三个局限**：

1. **只优化 DAG 分层**：强连通分量（环）被退回成同层或忽略，环内边交叉不处理。
2. **barycenter 是启发式、非最优**：迭代 12 次得到的只是局部优，复杂图仍有明显交叉。
3. **只减"节点顺序交叉"，不减"边的几何重叠"**：
   重心排序让同层节点的连线"不交叉"，但边路由阶段（正交折线 + 端口分配）
   **完全没把"其他边可能穿过同一 x/y 通道"纳入代价**，
   所以两条边走同一条垂直通道、在中间节点旁叠线的问题无法消除。

### 1.3 路由阶段：边与边互不排斥

`coord.rs` / `sugiyama.rs` 的边路由是"端点裁剪到节点边框 + 正交绕行中间节点"。
路由只用到了"源节点、目标节点、中间节点包围盒"三者的几何，
**没有边-边排斥（edge-edge repulsion）和边-节点排斥（edge-node overlap avoidance）**。
这是 mermaid 里"两条线贴在一起 / 线穿过不相关节点"的直接根因。

---

## 2. 根因：IR 没有把"边"当作一等布局对象

当前 `LayoutGraph` / `PlacedGraph` 的设计隐含了一个错误的前提：

> **边 = 节点之间的附属连线，几何由节点位置推导。**

但 mermaid 的视觉质量恰恰取决于边。要让布局避免交叉 / 重叠，
边必须在**求解阶段**就是一等公民，携带着约束参与排布。当前 IR 缺失的边语义：

| 缺失的约束 | 说明 | 后果 |
|---|---|---|
| 边权重 / 重要性 | 主线 vs 次要线（如 state 的转移 vs 注释） | 次要线不该主导排布 |
| 边中间标签的占位空间 | class 关联线上的角色/基数、sequence 消息 | 标签被线/节点压住 |
| 边-边排斥代价 | 两条边不应走同一正交通道 | 线重叠 |
| 端口绑定对交叉的影响 | 固定 T/B/L/R 端口会强制走向、增加交叉 | 强制交叉 |
| 自环 / 双向的几何预算 | 需要节点一侧的额外空间 | 自环压到相邻节点 |

> 注：这一点与"Scene IR"提案互补。Scene IR 解决**渲染层**回查 AST 的问题（视觉语义外置），
> 本设计解决**布局层**边感知缺失的问题。两者都指向同一个结论：
> **IR 必须承载布局/渲染所需的"完整事实"，而不是把边当附属。**

---

## 3. 目标设计：边作为一等公民的布局管线

### 3.1 扩展 LayoutGraph：给边建模约束

```rust
pub struct LEdge {
    pub source: usize,
    pub target: usize,
    pub source_port: PortHint,
    pub target_port: PortHint,
    pub line_kind: LineKind,
    // —— 新增：边感知布局所需的约束 ——
    pub priority: EdgePriority,      // 主线 / 次要线，影响交叉优化的权重
    pub label_space: Size,          // 边标签占用的包围盒（路由时预留）
    pub repulsion: f64,             // 该边与其他边/节点的排斥强度（默认 1.0）
    pub routing_hint: RoutingHint,  // Orthogonal / Spline / Curved（影响路由器选型）
}

pub enum EdgePriority { Primary, Secondary, Annotation }
pub enum RoutingHint { Orthogonal, Spline, Curved, Inherit }
```

`label_space` 由 `Measure` 阶段测量（边标签文本宽高 + 内边距），与节点尺寸一起进入 IR。
这样路由阶段能为标签预留空间，避免"标签压到节点/另一条线"。

### 3.2 求解阶段：边驱动的同层排序

对 `GridSolver` / `DirectedSolver` 补强：

1. **重心交叉减少下沉为通用原语**：把 `sugiyama.rs::reduce_crossings` 抽成
   `layout/crossing.rs` 的 `minimize_crossings(layers, edges) -> layers`，
   供所有分层求解器（Grid / Directed / Grouped）复用，不再各写各的 BFS 顺序。
2. **非 DAG 也参与交叉减少**：对强连通分量先收缩成超节点、层内做局部 barycenter，
   展开后再做一轮组内排序，避免"环内边一律不优化"。
3. **迭代次数与稳定性**：`crossing_iterations` 保留，但增加"上下双向扫描"
   （sugiyama 已有 top_down / left_prio 双 pass，Grid 需对齐），并缓存上一次顺序、
   迭代收敛即停，避免无效 12 次空转。

### 3.3 路由阶段：边-边排斥 + 边-节点回避

新增独立的 `EdgeRouter` 阶段（在节点坐标确定后、产出 `edge_routes` 前）：

```rust
/// 给定节点几何 + 边拓扑 + 端口，求出无重叠、少交叉的折线。
fn route_edges(
    nodes: &[Rect],
    edges: &[LEdge],
    ports: &[(PortHint, PortHint)],
) -> Vec<Vec<Point>>;
```

实现要点：

- **正交通道分配（channel assignment）**：把所有纵向/横向候选通道按"占用边数"排序，
  给每条边分配尽量不冲突的通道（类比 mermaid 的 `aStar` / `grid-free` 路由思想）。
- **边-边排斥代价**：若两条边被迫共用通道，引入轻微垂直偏移（offset routing），
  使它们在视觉上平行而非重叠。
- **边-节点回避**：路由代价函数包含"线段穿过非端点节点包围盒"的惩罚，
  迫使线绕行，解决"线穿过不相关节点"。
- **标签占位**：路由时若 `label_space != 0`，在中点附近预留空白并放置标签锚点。

> 这是布局层最值得投入的部分。当前 `clip_to_border` 式的"端点贴边 + 直线/简单折线"
> 没有任何全局视野。引入通道分配后，交叉/重叠能从几何层面被消除，而非仅减少节点顺序交叉。

### 3.4 统一出口：PlacedGraph 不变，但内容更优

`PlacedGraph` 的结构（positions / edge_routes / edge_kinds / group_bounds / size）
**无需改动**——它仍是纯几何输出。改变的是：

- `positions` 由"边感知的求解"产出（同层顺序已最小化交叉）；
- `edge_routes` 由"边-边排斥路由"产出（无重叠、标签有空间）。

即：**IR 接口稳定，求解器与路由器内部升级**。这对现有 golden 测试是友好的——
输出格式不变，只是视觉质量提升。

---

## 4. 与 Scene IR 的关系

两份提案解决不同层的问题，但共享同一个纪律：

| 提案 | 解决层 | 核心动作 | IR 变化 |
|---|---|---|---|
| 边感知布局（本文） | 布局求解 / 路由 | 边作为一等约束参与排布 | 扩展 `LayoutGraph::LEdge` 的约束字段 |
| Scene IR（上一轮） | 渲染 | 几何 + 视觉打包，渲染不再回查 AST | 新增 `SceneDocument` 作为 `PlacedGraph` → `SceneNode` 之间的中枢 |

两者可以独立落地、互不阻塞：

- 先做 Scene IR（渲染解耦），不影响布局质量；
- 先做边感知布局（布局质量），不影响渲染解耦；
- 最终管线：

```text
AST ──convert(+边约束)──► LayoutGraph ──solver(边感知)──► PlacedGraph
                                                              │
                                                    merge(几何+视觉) → SceneDocument
                                                              │
                                                         Painter → Scene
```

---

## 5. 分阶段落地路线

### Phase 0 — 可观测性先行（必做，先量化问题）
- 在 `tests/` 现有 golden 用例里加 **"边交叉计数 / 边-边重叠计数 / 线穿节点计数"** 的度量函数
  （纯几何统计，不改动渲染）。先 baseline 出当前各图的"坏度"数字，后续改造用同一指标验证下降。
- 目的：避免"感觉好了"式的主观评审，用数据驱动。

### Phase 1 — Grid/Linear/Simple 引入交叉减少（低成本高收益）
- 抽 `layout/crossing.rs::minimize_crossings`，让 `GridSolver` 在 BFS 分层后、
  同层按 barycenter 排序（用边的 source/target 重心），取代"纯源码顺序"。
- 这一步不动路由，仅改同层顺序，就能消除 class/er 图大部分顺序性交叉。

### Phase 2 — 通用 EdgeRouter（核心投入）
- 新增 `layout/route.rs`，实现正交通道分配 + 边-边偏移 + 边-节点回避 + 标签占位。
- 先只接 `DirectedSolver` 的 `edge_routes` 产出（替换现有 `clip_to_border` 式路由），
  用 Phase 0 指标验证重叠下降。

### Phase 3 — 边约束入 IR
- `LEdge` 增加 `priority` / `label_space` / `repulsion` / `routing_hint`，
  `Measure` 阶段测量边标签尺寸；solver/router 消费这些字段。
- 非 DAG 的 SCC 收缩 + 组内 barycenter。

### Phase 4 — 收敛
- `crossing_iterations` 收敛即停；所有分层求解器统一复用 `minimize_crossings`；
- 删除各 solver 内重复的 BFS/排序代码。

---

## 6. 风险与权衡

- **性能**：边-边排斥路由是 `O(E^2)` 量级，复杂图需做通道分配的空间索引 / 早期剪枝。
  默认 `crossing_iterations` 与路由迭代应设上限，避免大图卡死。
- **确定性**：当前布局靠"源码顺序锚定"保证确定性。引入 barycenter 后，同重心节点
  的相对顺序需回退到"源码顺序"作为 tie-breaker，否则同一图在不同运行下会抖动
  （破坏现有"代码不动布局永不变"的纪律）。`minimize_crossings` 必须显式保留此 tie-breaker。
- **与 mermaid 差异**：mermaid 用 `aStar` 路由 + `krawing` 库，我们不照搬，
  而是在自有 IR 上做"通道分配 + 排斥代价"的轻量实现，保持 Rust 纯几何、无外部依赖。

---

## 7. 验收标准（建议）

1. 现有 golden 测试**全部不回归**（IR 输出格式稳定）。
2. Phase 0 度量显示：引入 Phase 1/2 后，典型 flowchart/class/er 样例的
   "边交叉数 / 边重叠数 / 线穿节点数" 较 baseline **下降 ≥ 50%**。
3. 同图多次运行布局结果一致（确定性纪律保持）。
