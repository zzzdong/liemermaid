# 布局系统重构设计（Layout System Design）

> 目标：将现有「多套并行布局路径」重构为一套**完整、分层清晰、可编译、覆盖全部图表类型**的统一布局系统。
> 允许破坏性改造：推翻 `src/builder/layout/` 的历史包袱，但不丢弃已验证的成熟算法资产。

---

## 0. 现状盘点（重构的动机）

通过对当前代码的核查，现状存在明显的「历史包袱叠加」，这是重构的根本原因：

| 图表 | 当前布局路径 | 问题 |
| :--- | :--- | :--- |
| `Flowchart` | **dagre**(`dagre_layout.rs`) → 无子图时再转成 `SugiyamaResult` → 复用旧渲染；有子图时走 `Layout` IR | **同引擎内部三条路径**，靠 `has_subgraphs` 分叉，难以维护 |
| `State` | 自研 `SugiyamaLayout`(petgraph) → `state.rs` 手写全部绘制 | 布局结果(`HashMap<NodeIndex,Point>`)与绘制(画圆/箭头/标签)**强耦合**，700 行难复用 |
| `Pie/Sequence/ER/Timeline/GitGraph/Class` | 各自 `layout()` 直接产出 `SceneNode` | 完全绕开布局层，无统一管线 |

**关键结论：**
1. `SugiyamaLayout`（`src/builder/layout/sugiyama.rs`，约 1775 行）是**经过 5 个对拍测试验证、行为对齐 dagre** 的成熟算法资产——network-simplex / longest-path 双 ranker、虚拟节点拆长边、barycenter、Brandes-Köpf、SCC、反馈弧路由。**必须保留并吸收，而不是重写。**
2. 现有的 `Layout` IR（`types.rs` 的 `LayoutNode/LayoutEdge/LayoutSubgraph`）被 flowchart 部分使用，但**未被所有图统一消费**，名存实亡。
3. 真正要重构的不是「算法」，而是**「管线与边界」**：布局计算、分组、路由、渲染四条职责当前彼此纠缠。

---

## 1. 总体架构：一条管线，四类求解器

新系统**不搞「所有图都塞进 Sugiyama」**。根据图的拓扑本质，把 8 种图表归为四类，共享**同一套 `LayoutGraph → PlacedGraph` 数据契约**，但各自选用不同的 `LayoutSolver`：

```
                  ┌─────────────────────────────────────────────┐
   AST ──────────►│              layout_diagram()               │
  Diagram         │  1. ToLayoutGraph::to_layout_graph()        │
                  │     （按图表类型 dispatch 到转换器）           │
                  │        ↓  LayoutGraph（纯拓扑 + 尺寸）        │
                  │  2. solver = LayoutSolver::for_diagram()    │
                  │     ├─ DirectedSolver（flowchart/state/…）  │
                  │     ├─ GridSolver（class/er）               │
                  │     ├─ LinearSolver（sequence/timeline）    │
                  │     └─ SimpleSolver（pie/gitgraph）          │
                  │        ↓  PlacedGraph（纯几何）              │
                  │  3. 后处理：fit_to_canvas / 方向统一          │
                  │        ↓                                     │
                  │  4. 渲染：RenderLayer 按 PlacedGraph + AST   │
                  │     取形状/标签/箭头组合绘制                   │
                  └─────────────────────────────────────────────┘
                        ↓  Vec<SceneNode> → lievisual::Scene
```

**三条边界（这次是真的成立，因为 IR 是纯几何的）：**
- **输入边界**：`LayoutGraph` 只含节点 `Size`、边 `(src,tgt)`、组嵌套、`PortHint`，**不含颜色/标签/箭头**。转换器负责剥离渲染语义。
- **求解边界**：`LayoutSolver` 只读 `LayoutGraph`，产出 `PlacedGraph`（`positions` + `edge_routes` + `group_bounds`）。求解器与 AST 完全解耦。
- **渲染边界**：`RenderLayer` 拿 `PlacedGraph` 后**重新映射回 AST** 取形状/文本/箭头类型来绘制。渲染不改坐标。

> 与 `docs/layout-refactor.md` 的关键分歧：方案文档要自研一套「分组折叠 + 仿射回贴」的 `group.rs`。本设计**明确不采用**——子图/复合状态用**递归求解 + 容器占位**（见 §4），这是当前 `state.rs` 已验证可行的方式，无需引入高风险的仿射变换。

---

## 2. 数据契约（`src/builder/layout/ir.rs`）

> **放 `src/builder/layout/` 下，不新建 crate 根级 `src/layout/`**——避免两个并行布局模块。这是与方案文档的关键修正。
>
> **IR 建模原则**：`title / node / edge / line` 等是**开放概念，不是死定的字段清单**。IR 的目标是「承载布局需要的一切拓扑事实 + 极少的几何语义」，让转换器把渲染无关的信息剥离，让求解器只读拓扑。下表是概念 → IR 表达的映射（实现时可增删字段）：

| 概念 | IR 表达 | 说明 |
| :--- | :--- | :--- |
| `title`（图/子图标题） | `LGroup.title: Option<String>` + `LayoutGraph.title: Option<String>` | 子图标题参与容器尺寸计算；图标题独立存，渲染层绘制 |
| `node`（节点） | `LNode { id, size, shape_hint }` | 只含尺寸 + 形状类别（影响锚点/裁剪），不含颜色 |
| `edge`（边） | `LEdge { source, target, source_port, target_port, line_kind }` | 拓扑连接 + 端口提示 + 线型类别 |
| `line`（连线） | `LEdge.line_kind: LineKind` + `PlacedGraph.edge_routes` | 线型类别（实线/虚线/自环/双向/贝塞尔）进 IR，几何进 `edge_routes` |
| `group`（组/子图） | `LGroup { title, children }` | 递归嵌套树 |

```rust
use lievisual::geometry::{Point, Rect, Size};

/// 布局输入：纯拓扑 + 尺寸约束 + 少量线型/标题语义，不含颜色/标签/箭头。
#[derive(Debug, Clone, Default)]
pub struct LayoutGraph {
    /// 图标题（如 flowchart 的 `---` 标题、pie/timeline 的 title）
    pub title: Option<String>,
    /// 与 AST 源码顺序严格一致（确定性锚定，布局永不抖动）
    pub nodes: Vec<LNode>,
    pub edges: Vec<LEdge>,
    /// 组树，索引即源码顺序
    pub groups: Vec<LGroup>,
    /// 真正连接两个组的跨组边（转换时收集，供 GroupedDirected 使用）
    pub cross_group_edges: Vec<LEdge>,
}

#[derive(Debug, Clone)]
pub struct LNode {
    /// 原始节点 ID（映射回 AST / 渲染层）
    pub id: String,
    pub size: Size,
    /// 节点参与布局时的形状类别（仅影响锚点/裁剪，不影响渲染颜色）
    pub shape_hint: ShapeHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeHint {
    Rect,
    Rounded,
    Diamond,
    Circle,
    /// fork/join 横线
    Bar,
}

#[derive(Debug, Clone)]
pub struct LEdge {
    pub source: usize,          // 节点索引（LayoutGraph.nodes）
    pub target: usize,
    pub source_port: PortHint,
    pub target_port: PortHint,
    /// 线型类别（几何拓扑语义，不是颜色/箭头）。渲染层据此选绘制方式。
    pub line_kind: LineKind,
}

/// 连线类别：告诉求解器/渲染层「这条边怎么画、路由上有什么特殊约束」。
/// 它是拓扑语义，不承载颜色；具体颜色/箭头样式仍由渲染层查 AST 决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// 普通实线（正交折线）
    Solid,
    /// 虚线（渲染层采样成 dash）
    Dashed,
    /// 自环（求解器在节点一侧生成小环）
    SelfLoop,
    /// 双向（路由时错开两条线）
    Bidirectional,
    /// 贝塞尔曲线（无箭头/圆点等特殊终点）
    Curved,
    /// 不可见（占位，仅参与拓扑不绘制）——如 flowchart `~~~`
    Invisible,
}

/// 几何拓扑语义（不是箭头）。Auto 交给求解器按最短边决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortHint {
    Auto,
    Top, Bottom, Left, Right,
}

#[derive(Debug, Clone)]
pub struct LGroup {
    /// 子图标题（参与容器尺寸计算，渲染层绘制）
    pub title: Option<String>,
    pub children: Vec<GroupChild>,
}

#[derive(Debug, Clone, Copy)]
pub enum GroupChild {
    Node(usize),
    Group(usize),
}

// ---------------------------------------------------------------------------

/// 布局输出：仅几何数据。数组顺序与 LayoutGraph 一一对应。
#[derive(Debug, Clone)]
pub struct PlacedGraph {
    /// 节点中心坐标，与 LayoutGraph.nodes 同序
    pub positions: Vec<Point>,
    /// 边路径（折线/贝塞尔采样点），与 LayoutGraph.edges 同序
    pub edge_routes: Vec<Vec<Point>>,
    /// 组包围盒，与 LayoutGraph.groups 同序
    pub group_bounds: Vec<Rect>,
    /// 整体画布尺寸（内容实际占据的 bbox）
    pub size: Size,
}

impl PlacedGraph {
    /// 平移所有几何使 (0,0) 为左上（渲染前归一化）
    pub fn normalize(&mut self) {
        // 求 positions + edge_routes 的 min，整体平移
    }
    pub fn center(&self) -> Point { /* 内容中心 */ }
}
```

**设计要点：**
- `Rect` 用 `lievisual::geometry::Rect`（项目已从 `vello_cpu::kurbo` 收拢到 `lievisual::geometry`，见 `Cargo.toml` 的 patch，保持一致性）。
- `ShapeHint` / `LineKind` 让渲染层知道「这是菱形/圆/横线、这条线是虚线/自环」，用于裁剪与绘制，但**不承载颜色**——颜色属于渲染层，回查 AST 的 `NodeShape`/`ArrowType`。
- `title` 进 IR：图标题 `LayoutGraph.title`、子图标题 `LGroup.title`，让求解器能把标题计入容器/画布尺寸，渲染层直接读。
- 全部 `f64` 几何，避免 `#![deny(float_cmp)]` 之类的问题（沿用现有风格）。

---

## 3. 转换层（`src/builder/layout/convert.rs`）

替代方案文档的 `graph.rs`（其 `impl ToLayoutGraph for Flowchart` 有编译错误：关联函数里 `match diagram` 逻辑不通）。正确写法：

```rust
use crate::ast::{Diagram, Flowchart, State, StateDiagram};

pub trait ToLayoutGraph {
    /// 注意：`&self` 持有 diagram 引用，而非关联函数。
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph;
}

impl ToLayoutGraph for Flowchart {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        let mut lg = LayoutGraph::default();
        // 1. 顶层节点（含 subgraph 内部节点，顺序=源码顺序，去重合并空壳）
        for n in recognize::all_flowchart_nodes(self) {
            lg.nodes.push(LNode {
                id: n.id.clone(),
                size: measure.measure(&n.id, n.shape.as_ref()),
                shape_hint: shape_hint_of(&n.shape),
            });
        }
        // 2. 组树：subgraph 作为 LGroup，children 指向组内节点索引
        for sg in &self.subgraphs {
            let children = sg.nodes.iter()
                .filter_map(|n| lg.nodes.iter().position(|x| x.id == n.id))
                .map(GroupChild::Node)
                .collect();
            lg.groups.push(LGroup { id: format!("sg{}", lg.groups.len()), children });
        }
        // 3. 边：source/target 映射到节点索引；跨组边单独收集
        for e in &self.edges {
            // ... find idx of e.source / e.target
        }
        lg
    }
}

impl ToLayoutGraph for Diagram {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        match self {
            Diagram::Flowchart(fc) => fc.to_layout_graph(measure),
            Diagram::State(sd) => sd.to_layout_graph(measure),
            Diagram::Class(c) => c.to_layout_graph(measure),
            Diagram::Er(er) => er.to_layout_graph(measure),
            // 线性/简单图：也产出 LayoutGraph（空边，靠顺序），solver 走 Linear/Simple
            Diagram::Sequence(seq) => seq.to_layout_graph(measure),
            Diagram::Timeline(t) => t.to_layout_graph(measure),
            Diagram::Pie(p) => p.to_layout_graph(measure),
            Diagram::GitGraph(g) => g.to_layout_graph(measure),
        }
    }
}
```

**Measure 注入**：转换需要节点尺寸，`Measure` 封装现有的 `layout_text`/`theme` 测量逻辑（`src/builder/layout/measure.rs` 已存在，保留复用），避免转换器直接依赖 theme 常量。

---

## 4. 求解层（`src/builder/layout/solver/`）

> **决策（已确认）：彻底弃用 dagre。** 删除 `dagre = "0.1"` 依赖、`dagre_layout.rs`，以及 flowchart 中 `run_dagre_layout` / `sugiyama_result_from_dagre` / `layout_from_dagre` 三条 dagre 适配路径。全部有向图用自研 `SugiyamaLayout`。

四个 solver 共享 `LayoutSolver` trait：

```rust
pub trait LayoutSolver {
    fn solve(&self, graph: &LayoutGraph, config: &LayoutConfig) -> PlacedGraph;
}

pub enum SolverKind {
    Directed,  // flowchart / state
    Grid,      // class / er
    Linear,    // sequence / timeline
    Simple,    // pie / gitgraph
}

pub fn solver_for(diagram: &Diagram) -> SolverKind {
    match diagram {
        Diagram::Flowchart(_) | Diagram::State(_) => SolverKind::Directed,
        Diagram::Class(_) | Diagram::Er(_) => SolverKind::Grid,
        Diagram::Sequence(_) | Diagram::Timeline(_) => SolverKind::Linear,
        Diagram::Pie(_) | Diagram::GitGraph(_) => SolverKind::Simple,
    }
}
```

### 4.0 分析阶段（`src/builder/layout/analyze.rs`）——petgraph 分组信息作为一等公民

**这是补强你第 3、4 点想法的关键**：把 petgraph 的数据流分析**显式产出**，作为 `DirectedSolver` 的**输入事实**，而不是藏在 Sugiyama 内部。

```
pub struct GraphAnalysis {
    /// petgraph 有向图（节点索引 = LayoutGraph.nodes 下标）
    pub graph: petgraph::graph::DiGraph<usize, ()>,
    /// SCC 分组：每个强连通分量是环/紧密耦合的一组节点
    pub sccs: Vec<Vec<usize>>,
    /// 拓扑序（有环时先破环）
    pub topological_order: Vec<usize>,
    /// 反馈弧（构成环的边），供路由使用
    pub feedback_arcs: Vec<(usize, usize)>,
    /// 连通分量（弱连通），用于启发式按块排布
    pub connected_components: Vec<Vec<usize>>,
}

pub fn analyze(lg: &LayoutGraph) -> GraphAnalysis {
    // 构建 DiGraph（忽略自环，保留普通边）
    // tarjan_scc → 强连通分量
    // toposort（破环后）→ 拓扑序
    // DFS 回边检测 → 反馈弧
    // 弱连通 → 连通分量
}
```

**分析结果是启发式的输入：**
- **SCC**：环内节点语义上紧密耦合 → 启发式把它们作为「一个块」参与层级分配（同层或相邻层），并让反馈弧路由集中绕行。
- **连通分量**：互相独立的子图块 → 启发式把各块紧凑排布，块间留白。
- **拓扑序**：为层内初始顺序提供确定性锚定，保证「代码不动、布局不变」。

### 4.1 `DirectedSolver`（核心，纯管线编排，不侵入 Sugiyama）

**边界原则（本次调整的重点）：** `SugiyamaLayout` 是**纯求解器（黑盒）**——`DiGraph + sizes → SugiyamaResult`，**完全不动**。所有「分组 / 顺序编排 / 启发式调整」逻辑全部写在新的 `DirectedSolver` 层，由**你**实现，这是「重写管线架构」的真正内容。`SugiyamaLayout` 只是被调用的标准算法，不是被改的旧代码。

```
DirectedSolver::solve(graph, config):
  1. 若 graph 有组（subgraph / composite）→ 走 GroupedDirected（见 §4.2）
  2. 否则：
     a. 构建 petgraph DiGraph（节点顺序=LayoutGraph.nodes 顺序）
     b. analyze(graph) → GraphAnalysis                    ← petgraph 分组信息显式化
     c. 【启发式编排】基于 GraphAnalysis 做分组预处理：
        - 用 SCC 初始化层内初始顺序（同 SCC 相邻，减少环内交叉）
        - 用连通分量决定布局块聚拢（分量间留白）
        - 用拓扑序做确定性锚定
        → 产出初始层序 seed（不传给 Sugiyama 内部，而是作为其入图前的排序输入）
     d. 调 SugiyamaLayout::layout(&sizes)                  ← 纯黑盒，零侵入
     e. 把 SugiyamaResult(positions/edge_routes/layers) 映射回 PlacedGraph
  3. 方向处理：现有 sugiyama 恒为 TB；LR/BT/RL 复用 flowchart.rs 的
     transform_sugiyama_direction（把它从 #[cfg(test)] 提升为正式 API）
  4. 填充 edge_routes（SugiyamaLayout 已含虚拟节点拆边 + 反馈弧路由）
```

**启发式编排的落点（全在 `DirectedSolver` 层，不侵入 `SugiyamaLayout`）：**
- **SCC 感知的初始顺序**：`GraphAnalysis.sccs` → 把同 SCC 的节点在入图时排到一起。`SugiyamaLayout::layout` 的 `build_layer_index` 已按节点加入顺序排序，因此 `DirectedSolver` 只需**控制入图顺序**即可影响层内初始排列——**不需要改 `reduce_crossings`**。
- **连通分量聚拢**：`connected_components` → 决定节点入图顺序（同分量连续），让孤立块在最终布局中聚拢，分量间自然留白。
- **确定性锚定**：入图顺序 = `topological_order`（有环时优先）+ 源码下标，保证「代码不动、布局不变」。
- 若某个启发式引入回归，在 `DirectedSolver` 层用 `LayoutConfig` 开关切换入图顺序策略即可，**不触碰算法**。

> 这一版与上一版的关键修正：不再「对 `SugiyamaLayout` 做可选注入（改 `reduce_crossings` 内部）」，而是**把启发式全部外置到 `DirectedSolver` 的入图顺序编排**。`SugiyamaLayout` 是纯黑盒，管线重构带来的收益（分组/编排/统一）由新写的 `DirectedSolver` 承担，标准算法零改动。

**弃用 dagre 的验证依据：**
- `SugiyamaLayout` 已通过 `tests/sugiyama_consistency_test.rs` 的 5 个对拍测试（rank 单调性、反馈环分层对齐 dagre、长边拆 dummy、NS 比 LP 紧凑、同层 Y 对齐）。
- flowchart 无子图场景本就用 `SugiyamaResult` 渲染（之前只是位置被 dagre 覆盖），渲染层可无缝复用。
- flowchart **有子图**场景由新的 `GroupedDirected`（§4.2）接管，不再依赖 dagre 的 compound 模式。

### 4.2 分组折叠（`GroupedDirected`，拒绝仿射回贴）

方案文档的「子图折叠成超节点 + 仿射回贴」是已知高风险点。本设计采用 **递归求解 + 容器占位**，这是 `state.rs` 已验证的可行方式：

```
GroupedDirected::solve(graph, config):
  1. 对每个 LGroup（由内向外）：
     a. 抽出该组子图（成员节点 + 组内边）
     b. 递归 DirectedSolver::solve 得子 PlacedGraph
     c. 子图 bbox + padding + 标题高度 → 容器尺寸 rect
  2. 构建"外部图"：每个组折叠为一个 super-node（尺寸=容器 rect），
     加上组外独立节点，加上跨组边
  3. 对外部图做 DirectedSolver（子组不再递归）
  4. 把各子 PlacedGraph 通过**坐标平移**贴回到对应 super-node 位置
     （平移量 = super-node 中心 - 子图自身中心，纯平移，无旋转/缩放）
  5. 收集 group_bounds
```

**为什么不用仿射：**
- 纯平移（无缩放）保持子图内部相对位置不变，无需处理嵌套容器的 `transform` 矩阵。
- 仿射（含缩放）会引入「子图内部坐标缩放后箭头方向失真」的复杂度，当前没有任何图需要缩放内部。
- 递归天然处理任意深度嵌套（flowchart subgraph、state composite 同理）。

### 4.3 `GridSolver`（class / er）

ER/类图天然是网格/行列排布，**不经过 Sugiyama**：

```
GridSolver::solve(graph, config):
  - 按节点顺序逐行填充，每行按节点宽 + gap，行间按最大高 + gap
  - 边 route = 直线或简单的三点正交折线（从 src 右/下 → tgt 左/上）
```

### 4.4 `LinearSolver`（sequence / timeline）

```
LinearSolver::solve(graph, config):
  - sequence：按参与者列、消息行时间轴线性排布
  - timeline：按 section 列、事件行
  - 边 route = 消息的水平/垂直线段（保留现有 sequence.rs 的语义）
```

### 4.5 `SimpleSolver`（pie / gitgraph）

```
SimpleSolver::solve(graph, config):
  - pie：无节点排布，仅画布中心
  - gitgraph：按提交顺序 / 分支列线性排布
```

---

## 5. 端口路由（`src/builder/layout/router.rs`）

方案文档的 `route.rs` 只返回 `Vec<Point>` 折线，忽略了 `PlacedGraph.edge_routes` 的顺序约定。本设计把「求解器给的初值」与「精细路由」分层：

```rust
/// 用端口提示把边端点从节点中心平移到节点边框上的裁剪点。
pub fn compute_anchor(pos: &Point, size: &Size, port: PortHint) -> Point {
    match port {
        PortHint::Top    => Point::new(pos.x, pos.y - size.height / 2.0),
        PortHint::Bottom => Point::new(pos.x, pos.y + size.height / 2.0),
        PortHint::Left   => Point::new(pos.x - size.width / 2.0, pos.y),
        PortHint::Right  => Point::new(pos.x + size.width / 2.0, pos.y),
        PortHint::Auto   => *pos,
    }
}

/// 对求解器输出的原始路径做正交折线精化（可选，DirectedSolver 已含路由时可跳过）
pub fn orthogonalize(route: &[Point], config: &LayoutConfig) -> Vec<Point> {
    // 把斜线按 90° 拆成水平+垂直段，或保留直线（短边）
}
```

**职责边界：** `DirectedSolver` 输出的 `edge_routes` 已是完整正交折线（SugiyamaLayout 内置）。`router.rs` 只做三件事：(1) `Auto` 端口选择（计算节点到邻居的最短边）；(2) 对 `Grid/Linear` 等简单边做三点正交化；(3) 处理 `LineKind` 特殊边——`SelfLoop`（节点一侧生成小环，现 `sugiyama.rs` 的 `rebuild_routes_from_dummies` 已有）、`Bidirectional`（两条线错开，避免重叠）、`Invisible`（跳过路由，仅占位）。

---

## 6. 渲染层（`src/builder/render/`）

渲染层是本次重构的**第二大收益点**：把 flowchart/state 里散落的绘制代码（画节点、画箭头、画标签）收敛为按 `PlacedGraph` + AST 绘制的通用模块，各图只提供「本图特有的节点/边视觉」。

```
trait Renderer {
    /// 拿到几何 + 原始 AST，产出 SceneNode。改坐标的职责在求解层，这里只读。
    fn render(&self, placed: &PlacedGraph, ast: &Diagram, theme: &Theme) -> Vec<SceneNode>;
}

pub struct DirectedRenderer;   // flowchart/state 共用：矩形/菱形/圆/箭头/标签/子图框
pub struct GridRenderer;       // class/er：类框 + 关系线
pub struct LinearRenderer;     // sequence：泳道 + 消息线
pub struct SimpleRenderer;     // pie/gitgraph
```

**关键点：**
- `DirectedRenderer` 吸收现有 `flowchart.rs::render_sugiyama_flowchart` 与 `state.rs` 的绘制逻辑，去掉其中「手写坐标计算」部分（那已由求解层完成），只保留「给定 center+shape → 画矩形/圆/箭头」。
- 边标签、子图标题、背景框等由渲染层统一处理，不再各写一份。
- **保留已有的绘制细节能力**（虚线采样 `sample_polyline`、特殊箭头 `draw_arrow_circle/cross`、标签背景等），从 `flowchart.rs` 原样迁移。

---

## 7. 配置与入口（`src/builder/layout/config.rs` + 改造 `builder/mod.rs`）

```rust
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub direction: Direction,           // TB/TD/BT/RL/LR
    pub node_gap: f64,
    pub layer_gap: f64,
    pub group_padding: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            direction: Direction::TD,
            node_gap: 50.0,
            layer_gap: 60.0,
            group_padding: 16.0,
        }
    }
}
```

> 已弃用 dagre，故 `LayoutConfig` 不再有 `engine` 字段。

`builder/mod.rs` 的 `build_diagram_with_config` 改为**统一管线**（不再按图分叉到 8 个引擎各自 `layout()`）：

```rust
pub fn build_diagram_with_config(diagram, config) -> DiagramResult<Scene> {
    let measure = Measure::new(&theme);
    let graph = diagram.to_layout_graph(&measure);       // 转换
    let solver = solver_for(diagram);                     // 选求解器
    let mut placed = solver.solve(&graph, &config);      // 求解
    placed.normalize();
    let renderer = renderer_for(diagram);                 // 选渲染器
    let nodes = renderer.render(&placed, diagram, &theme);
    let scene = Scene::new(config.width, config.height);
    scene.background = config.background;
    scene.nodes.extend(fit_to_canvas(nodes, config));     // 保留现有 fit_to_canvas
    Ok(scene)
}
```

**保留 `fit_to_canvas` / `compute_bbox`**（`builder/mod.rs` 已有），仅需适配 `PlacedGraph`。

---

## 8. 迁移与删除清单（破坏性改造）

**保留（标准算法资产，作为纯求解器复用）**
- `src/builder/layout/sugiyama.rs`：`SugiyamaLayout` 作为**纯黑盒求解器**（`DiGraph + sizes → SugiyamaResult`），由 `DirectedSolver` 调用，**零改动、零侵入**。所有启发式编排都在 `DirectedSolver` 层实现。
- `src/builder/layout/measure.rs`、`recognize.rs`、`coord.rs`：测量/识别/方向坐标工具，保留复用。
- `src/builder/layout/analyze.rs`（**新增**）：`GraphAnalysis`——petgraph 的 SCC/拓扑序/反馈弧/连通分量显式产出，作为 `DirectedSolver` 启发式编排的输入。
- `builder/mod.rs` 的 `fit_to_canvas` / `compute_bbox`。
- 各类图的绘制细节函数（虚线采样、特殊箭头、标签背景）迁移到对应 Renderer。
- `src/builder/er.rs`、`class.rs`、`sequence.rs`、`timeline.rs`、`pie.rs`、`gitgraph.rs` 的**布局逻辑**（网格/线性/简单排布 + 绘制）迁移到对应 `Grid/Linear/Simple` Solver 与 Renderer，去掉其 `LayoutEngine` 分叉。

**删除（历史包袱 + 弃用 dagre）**
- `src/builder/layout/types.rs` 的 `Layout`/`LayoutNode`/`LayoutEdge`/`LayoutSubgraph`/`LayoutMetadata` 旧 IR → 被新的 `ir.rs` 替代。
- `LayoutEngine` trait 及 8 个 `XxxEngine::new()` 的 `layout()` 分叉 → 由 `layout_diagram` 统一管线替代。
- **`dagre = "0.1"` 依赖（`Cargo.toml`）+ `src/builder/dagre_layout.rs`**。
- flowchart 内 `has_subgraphs` 三路径分叉 → 收敛为 `DirectedSolver`。
- `flowchart.rs` 的 `run_dagre_layout` / `sugiyama_result_from_dagre` / `layout_from_dagre` dagre 适配层。
- `state.rs` 中手写坐标计算/绘制混合的部分 → 拆到 `DirectedRenderer` + `GroupedDirected`。

---

## 9. 测试策略（守护行为，防止回归）

新系统破坏性改造，必须有测试兜底：

1. **复用并保留** `tests/sugiyama_consistency_test.rs`（5 个对拍测试）——验证吸收后的 `SugiyamaLayout` 仍行为对齐 dagre。
2. **新增 `tests/layout_ir_test.rs`**：验证 `LayoutGraph` 转换的正确性（节点顺序=源码顺序、组树结构、跨组边收集、`title`/`LineKind` 映射）。
3. **新增 `tests/graph_analysis_test.rs`**：验证 `GraphAnalysis` 的 SCC/拓扑序/反馈弧/连通分量对已知图输出正确（环、多连通块、长边）。
4. **新增 `tests/placed_graph_test.rs`**：验证 `PlacedGraph` 不变量（positions/edge_routes/group_bounds 数组长度与 LayoutGraph 一一对应；`normalize` 后 min 为 0）。
5. **新增 `tests/grouped_layout_test.rs`**：嵌套 subgraph/composite 递归求解 + 平移回贴，验证容器包围盒包含所有成员节点。
6. **保留 golden / 渲染测试**：`layout_quality_test.rs`、`official_compare_test.rs`、`html_report_test.rs` 继续跑，作为端到端回归（渲染结果可能因管线统一而合理变化，但**语义不能变**——节点不重叠、边不穿模、箭头正确）。
7. **新增 `tests/determinism_test.rs`**：同一输入渲染两次，SVG 逐字节一致（验证「节点顺序=源码顺序」的确定性锚定）。

---

## 10. 实施顺序（建议）

1. **Phase A（地基）**：新增 `ir.rs`（`LayoutGraph`/`PlacedGraph`，含 `title`/`LineKind`）+ `convert.rs`（`ToLayoutGraph`）+ `Measure` 注入。写 `layout_ir_test.rs`。
2. **Phase B（分析）**：新增 `analyze.rs`（`GraphAnalysis`：SCC/拓扑序/反馈弧/连通分量）。写 `graph_analysis_test.rs`。
3. **Phase C（求解）**：新增 `solver/`（`DirectedSolver` 吸收 `SugiyamaLayout` + 启发式增强、`GroupedDirected` 递归平移、`Grid/Linear/Simple`）。写 `placed_graph_test.rs` + `grouped_layout_test.rs`。此阶段 `DirectedSolver` 先从 `state.rs` 的调用点验证。
4. **Phase D（渲染）**：新增 `render/`，先做 `DirectedRenderer`（迁移 flowchart/state 绘制），再做其余。保留 golden 回归。
5. **Phase E（收口）**：改造 `builder/mod.rs` 统一入口，删除旧 `types.rs`/`LayoutEngine`/`has_subgraphs` 分叉，**删除 `dagre` 依赖与 `dagre_layout.rs`**。全量测试 + `cargo clippy` + `cargo build --release` 验证。

---

## 附：与 `docs/layout-refactor.md` 的主要分歧

| 维度 | 方案文档 | 本设计 |
| :--- | :--- | :--- |
| 模块位置 | 新建 crate 根级 `src/layout/` | 复用 `src/builder/layout/`（避免并行模块） |
| Sugiyama | 重写，四阶段全 `todo!()` | **吸收现有 1775 行成熟实现**，零改动调用 |
| 分组 | 超节点 + 仿射回贴（高风险） | 递归求解 + 纯平移回贴（已验证） |
| flowchart | 假设统一 Sugiyama | 收敛为 `DirectedSolver`，**彻底弃用 dagre** |
| 覆盖 | — | **全部 8 种图表一次到位**（4 类 solver） |
| 渲染 | IR 只入不出，含 ARIA 语义剥离 | 明确分 `solver`(改坐标) / `render`(读坐标) 两层，统一 `Renderer` trait |
| 编译正确性 | 多处 `todo!()` 直接 panic | 每一层有对应测试守护 |
