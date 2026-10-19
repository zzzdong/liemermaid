# liemermaid 布局与渲染层从头重构设计

> 状态：草案 / 待评审
> 范围：**只重写 `layout` 与 `builder` 两个中间层**，不动 `ast`（解析 + 图类型）与 `lievisual`（Scene IR + 渲染后端）。
> 目标：消除现有设计里"边感知缺失"与"渲染回查 AST"两个根本缺陷，建立一条清晰、可测、可扩展的管线。
> 关联文档：`layout-system-design.md`、`layout-refactor.md`、`refactor-layout.md`、`layout-edge-aware-design.md`（边感知布局）、Scene IR 提案（渲染解耦）。

---

## 0. 不变的两块基石（约束前提）

重构只能在 `ast` 与 `lievisual` 之间做文章，因此先精确列出它们的**对外契约**，新管线必须严格适配、不得假设其内部可改。

### 0.1 `ast`（输入侧，只读）

- `Diagram`（标签枚举）持有各图类型子树：
  `Flowchart(FlowchartNode)` / `State(StateNode)` / `Sequence(SequenceNode)` /
  `Class(ClassDiagramNode)` / `Er(ErDiagramNode)` / `Gantt` / `Pie(PieNode)` /
  `GitGraph(GitGraphNode)` / `MindMap` / `Timeline(TimelineNode)` / `Quadrant` / `Sankey`。
- 关键类型（仅列举新管线需要消费的部分）：
  - **节点 id**：`NodeId = String`。
  - **FlowchartNode / StateNode**：`nodes: IndexMap<NodeId, FlowNode>`（含 `text: RichText`、可选 `shape: Option<NodeShape>`、`classes: Vec<String>`）、`edges: Vec<FlowEdge>`（`from/to: NodeId`、`label: Option<RichText>`、`line_kind`、`arrow_at_end/start`、`curved`）。
  - **ClassDiagramNode**：`nodes: IndexMap<ClassName, ClassDefinition>`（含 `attributes/methods` 文本行、可见性）、`edges: Vec<ClassEdge>`（`from/to`、`label`、`edge_kind: ClassEdgeKind {Extends/Composition/Aggregation/Association/Dependency/Realization/Link/Dashed}`、`start_arrow/end_arrow`、`line_style`）。
  - **SequenceNode**：`participants`、`messages: Vec<SequenceMessage>`（`from/to`、`content`、`kind: SeqMessageKind {Sync/Async/Reply/Found/Lost/...}`、`activate: Activation`）。
  - **PieNode**：`title` + `sections: Vec<PieSection>`（`label`、`value: f64`、`color: Option<String>`）。
  - **样式**：`NodeStyle`（CSS 颜色/边框/背景/字体/圆角）、`ClassStyle`、主题 `Theme`。
  - **测量**：`RichText::measure(&Theme) -> MeasureResult`（被 `builder::Measure` 调用）。
- `ast` **不提供**任何"平面图坐标"，只提供"语义拓扑 + 文本/样式"。坐标必须由新 `layout` 层计算。

### 0.2 `lievisual`（输出侧，只写）

- `Scene { width, height, background: Color, nodes: Vec<SceneNode>, layers: Vec<Layer>, title, description, scale }`。
- `SceneNode { element: Element, z_index, transform, opacity, name, visible, clip }`；提供 `with_z / with_transform / with_opacity / with_name / with_visible / with_clip`。
- `Element` 枚举（绘图原语，自足的几何 + 样式）：
  `Rect / Circle / Ellipse / RoundedRect / Line / Polyline / Polygon / Arc / Pie / Path / Image / GradientPath / Text / Group`。
- `FillStrokeStyle { fill: Option<Fill>, stroke: Option<Stroke> }`；
  `Fill::{Solid(Color), LinearGradient, RadialGradient}`；
  `Stroke { color, width, line_cap, line_join, dash_array, dash_offset, miter_limit }`。
- 文本：`Element::Text { spans: Vec<RichSpan>, position: Point, style: TextStyle, layout: Option<Arc<TextLayout>> }`；
  测量用 `lievisual::text::measure_text` / `layout_text`。
- **关键能力**：`Group` 可递归嵌套（子节点继承父 `transform`/`opacity`，用于容器、节点阴影、序列泳道分组）；`z_index` 稳定排序；`Clip` 可做裁剪。

> 设计纪律：**新管线产出的"最后一跳"是 `Element` 原语集合**。mermaid 任何图最终都只是这些原语的组合——方盒 + 文字 + 折线 + 箭头标记 + 容器 Group。

### 0.3 对用户的兼容契约（必须保留）

`scene_ext.rs` 当前暴露：

```rust
pub trait ToScene { fn to_scene(&self) -> Result<Scene, DiagramError>; }
impl ToScene for Diagram { fn to_scene(&self) -> Result<Scene, DiagramError>; }
```

重构后该 trait 与签名**保持不变**，只是内部实现换成新管线。`DiagramError` 错误枚举也保留（仅按需补充 variant）。

---

## 1. 现有设计的核心缺陷（为什么要彻底重写）

1. **IR 不统一、互相打架**：`layout/types.rs` 的 `LayoutNode` 已是"厚 IR"（含 shape/style/label），但 `layout/ir.rs` 的 `PlacedGraph` 又是纯几何，`render/mod.rs` 注释明说只有 Directed 走 `PlacedGraph`、其余家族"基于 AST 自绘不经过通用 PlacedGraph"。两条渲染路径、两套 IR。
2. **渲染回查 AST**：`directed.rs` 渲染时 `node.shape.clone()`、`fc.edges.get(ei)`、`theme::flowchart::*` 硬编码取色——IR 没带上视觉，渲染器被迫持有 `IR index → AST 节点 → 该画什么` 的映射表。
3. **边不感知**：详见 `layout-edge-aware-design.md` —— 求解阶段只排节点、不排边；路由无边-边排斥；交叉减少只在有向图做了一半。
4. **主题/样式散落**：`theme::*` 散落在各渲染器，换主题要改渲染层。

新架构用**三层 IR + 单一渲染器**一次性解决。

---

## 2. 全新管线总览

```
                  ┌─────────────────────────────────────────────────────────┐
   AST (只读) ───►│  Stage 1: Extract   语义拓扑 → 统一拓扑图 (UG)            │
                  └─────────────────────────────────────────────────────────┘
                                          │  UG (节点 id / 边 / 端口 / 约束 / 未测文本)
                                          ▼
                  ┌─────────────────────────────────────────────────────────┐
                  │  Stage 1.5: Measure  测量所有文本 → 节点/边尺寸写回 UG    │
                  │   - 节点标签：RichText → 尺寸（决定节点包围盒）            │
                  │   - 边标签：RichText → 尺寸（决定边 label_space）         │
                  │   - 图标题 / 子图标题：测量为游离标签                      │
                  │   测量必须在 Layout 之前完成（solver 需要尺寸做分层/网格） │
                  └─────────────────────────────────────────────────────────┘
                                          │  UG' (含已测尺寸，仍不含颜色)
                                          ▼
                  ┌─────────────────────────────────────────────────────────┐
                  │  Stage 2: Layout     边感知布局求解 → 几何图 (GG)         │
                  │   - 测量 (Measure)                                       │
                  │   - 分层 / 网格 / 线性 / 时序 等 family solver           │
                  │   - 边交叉最小化 (barycenter 通用原语)                    │
                  │   - 边路由 (正交通道分配 + 边-边排斥 + 边-节点回避)        │
                  └─────────────────────────────────────────────────────────┘
                                          │  GG (坐标 + 折线 + 已测量尺寸)
                                          ▼
                  ┌─────────────────────────────────────────────────────────┐
                  │  Stage 3: Materialize  几何 → 场景图 (SG = 视觉自足 IR)   │
                  │   - 消费 Theme，把"几何 + 样式意图"解析成具体颜色/线型     │
                  │   - 形状/箭头/文本 → 具体绘制原语描述                      │
                  │   - 不再引用 AST，只引用 GG + Theme + 形状枚举            │
                  └─────────────────────────────────────────────────────────┘
                                          │  SG (与图类型/ AST 完全解耦)
                                          ▼
                  ┌─────────────────────────────────────────────────────────┐
                  │  Stage 4: Paint    SG → Vec<SceneNode> (lievisual 原语)   │
                  │   - 纯机械翻译，零分支、无图类型判断、无 theme 硬编码      │
                  └─────────────────────────────────────────────────────────┘
                                          │
                                          ▼
                                    lievisual::Scene
```

四条 stage 都在 `builder` crate（或 `layout` + `builder` 两个模块协作），但**严格单向依赖**：

```
ast ──► builder::extract ──► builder::layout ──► builder::materialize ──► builder::paint ──► lievisual
```

每一层只认自己上一层的 IR，**`paint` 完全不碰 `ast`**。

---

## 3. 三层 IR 定义

所有 IR 定义在 `src/builder/ir/` 下，与 `ast`/`lievisual` 正交。

### 3.1 Stage 1 输出：`Unigraph`（UG，统一拓扑图）

把 12 种图的语义拓扑**归一化**成一个统一的"节点 + 边 + 端口 + 约束"图。
目的：让后续 layout/materialize/paint 对所有图类型走同一套代码，差异只在
`extract` 阶段（把具体 AST 翻译成 UG）和少量 family 专属求解策略。

> **测量时序（关键）**：UG 在 Stage 1 产出时**只含未测文本**（`label: LabelSpec`）。
> 尺寸测量发生在 **Stage 1.5（Measure）**，在 Layout 之前——因为 solver（分层/网格/泳道）
> 必须知道节点包围盒才能排布。详见 §1.5。Measure 把 `LabelSpec` 换成 `MeasuredLabel`
> （带 `Size` + 预排版 `TextLayout`），写回 UG 得到 `UG'`。
> 因此 **UG'（非原始 UG）才是 Layout / Materialize 的输入**；原始 UG 仅在 extract→measure
> 之间短暂存在。

#### 1.5 Stage 1.5：测量（Measure）

```
src/builder/measure/
  mod.rs            // measure_all(ug, theme) -> UG'：遍历 UG 所有标签，量成尺寸写回
  text.rs           // 桥接 lievisual::text::{measure_text, layout_text}
  shape_size.rs     // 据 ShapeKind + 文本尺寸推算节点包围盒（菱形/圆/圆柱等几何约束）
```

要点：
- **节点标签**：`RichText → lievisual::text::layout_text` 得到 `TextLayout{width,height}`，
  再按 `ShapeKind` 推算最终 `Size`（参考现有 `measure.rs::measure_node` 的圆/跑道/圆柱几何约束）。
- **边标签**：量出尺寸后写入 `UGEdge.label.label_space`，供 `route.rs` 预留中点空白。
- **图标题 / 子图标题**：量成 `GGLabel` 候选，进入 `UG.meta`。
- **为什么在 Layout 前**：`family/directed.rs` 的分层、`family/grid.rs` 的网格、
  `family/sequence.rs` 的泳道宽度都依赖节点尺寸；延迟到 materialize 再测会让 solver 无尺寸可用。
- **Materialize 不再测节点尺寸**：Stage 3 只重测边标签本身的绘制布局（若需精确换行）
  与图标题，节点尺寸已随 `GGNode.size` 从 Layout 一路带下，无需二次测量。
- **确定性**：测量是纯函数（同输入同输出），不破坏"代码不动布局永不变"的纪律。

```rust
// builder/ir/unigraph.rs
pub struct Unigraph {
    pub nodes: Vec<UGNode>,
    pub edges: Vec<UGEdge>,
    pub families: GraphFamily,        // 决定用哪套 solver / 路由策略
    pub meta: DiagramMeta,            // 图标题等游离信息
}

pub struct UGNode {
    pub id: NodeId,
    pub kind: NodeKind,               // Atom / Container / Virtual(stub) / Subgraph
    pub role: NodeRole,               // 语义角色：参与者的语义（普通节点/泳道/类框/扇区…）
    pub label: MeasuredLabel,         // 已测量的文本 + 富文本片段（见 §1.5 测量阶段）
    pub ports: PortSet,               // 可用端口（T/B/L/R + 任意角）
    pub size_hint: SizeHint,          // 由 family 决定：固定 / 按文本测量 / 由子节点撑开
    pub style_ref: StyleRef,          // 指向 Theme 中某条样式（class / 节点类型）
    pub constraint: NodeConstraint,   // 最小尺寸、是否可压缩、是否参与交叉优化
}

pub struct UGEdge {
    pub id: EdgeId,
    pub source: NodeId,
    pub target: NodeId,
    pub source_port: PortHint,
    pub target_port: PortHint,
    pub kind: EdgeKind,               // 语义：流程 / 状态转移 / 类关系 / 消息 / 扇区连接…
    pub label: Option<MeasuredLabel>, // 边标签（已在 §1.5 测量，含占位空间需求）
    pub priority: EdgePriority,       // Primary / Secondary / Annotation（影响交叉权重）
    pub routing_hint: RoutingHint,    // Orthogonal / Spline / Curved / Inherit
    pub arrow: ArrowSpec,             // 起止箭头类型（枚举，非字符串）
    pub repulsion: f64,               // 与其他边/节点的排斥强度
}

pub enum GraphFamily {
    Directed,        // flowchart / state → 分层 + barycenter + 正交路由
    Grid,            // class / er → 网格 + 交叉减少 + 关系路由
    Linear,          // mindmap / timeline → 线性排布
    Sequence,        // sequence → 泳道 + 消息时序路由
    Radial,          // pie / quadrant → 极坐标
    Hierarchy,       // gitgraph / gantt → 层级 / 时间轴
}
```

> `extract` 是"唯一接触 AST 的地方"。每个图类型一个 `extract_xxx(ast_node) -> Unigraph`。
> 好处：新增图类型只需写一个 `extract`，layout/materialize/paint 完全不动。

### 3.2 Stage 2 输出：`Geograph`（GG，几何图）

布局求解产物。纯几何 + 已测量的尺寸，**不含颜色**（颜色在 Stage 3 注入）。

```rust
// builder/ir/geograph.rs
pub struct Geograph {
    pub size: Size,
    pub background: Color,                 // 仍来自 Theme（背景是"画布属性"，非节点样式）
    pub nodes: Vec<GGNode>,
    pub edges: Vec<GGEdge>,
    pub containers: Vec<GGContainer>,      // 子图/泳道/类框分组（仅几何包围盒）
    pub labels: Vec<GGLabel>,             // 游离文本（图标题/子图标题）
}

pub struct GGNode {
    pub id: NodeId,
    pub role: NodeRole,
    pub center: Point,
    pub size: Size,                        // 已测量
    pub shape: ShapeKind,                  // 已解析的几何形状（见 §3.4）
    pub ports: ResolvedPorts,              // 各端口的实际坐标
}

pub struct GGEdge {
    pub id: EdgeId,
    pub route: Vec<Point>,                 // 已路由折线（含边-边排斥偏移）
    pub label_anchor: Option<Point>,       // 边标签放置点（已为标签预留空间）
    pub kind: EdgeKind,
    pub arrow: ArrowSpec,
    pub routing_hint: RoutingHint,
}

pub struct GGContainer {
    pub bounds: Rect,
    pub title: Option<String>,             // 文本待 Stage3 测尺寸
    pub kind: ContainerKind,               // Subgraph / Lifeline / ClassBox / Slice
}
```

> `Geograph` 已经是"几乎能画"的状态，只差颜色/线型/字体等视觉细节——这正是 Stage 3 要补的。

### 3.3 Stage 3 输出：`SceneGraph`（SG，视觉自足 IR）

这是上一轮"Scene IR"提案的落地形态：**几何 + 视觉，完全解耦 AST**。
它描述"要画什么、什么颜色、什么线型、什么文字样式"，但仍用**抽象绘制项**而非 `lievisual::Element`——
以便 Stage 3 与具体渲染后端解耦，且便于单元测试（不依赖 lievisual 文本布局）。

```rust
// builder/ir/scenegraph.rs
pub struct SceneGraph {
    pub size: Size,
    pub background: Color,
    pub items: Vec<SceneItem>,             // 按 z_index 升序，painter 直接遍历
}

pub enum SceneItem {
    /// 形状（含容器/节点/扇区）
    Shape {
        geometry: ShapeGeometry,           // Rect/RoundedRect/Circle/Diamond/... + 坐标
        fill: Option<Fill>,                // 已解析成 lievisual::Fill
        stroke: Option<Stroke>,            // 已解析成 lievisual::Stroke
        z: i32,
    },
    /// 连线（已含箭头标记作为子项或独立项）
    Edge {
        path: Vec<Point>,
        stroke: Stroke,
        ends: EdgeEnds,                    // 起止箭头（枚举 → painter 查表生成标记）
        z: i32,
    },
    /// 文本（已测量，含布局）
    Label {
        text: Vec<RichSpan>,
        position: Point,
        style: TextStyle,
        layout: Option<Arc<TextLayout>>,
        anchor: Anchor,                    // 对齐锚点（中心/左/右/上/下）
        z: i32,
    },
    /// 分组（容器背景 + 边框 + 子项已展平到 items，Group 仅用于 z/clip 管理）
    Group { children: Vec<SceneItem>, z: i32 },
}

pub enum ShapeKind {                       // 与 §3.4 一致，几何已解析
    Rectangle, Rounded, Stadium, Subroutine, Diamond, Hexagon, Circle,
    DoubleCircle, Cylinder, Asymmetric, Parallelogram, Trapezoid, Bar,
    StartDot, EndDot, PieSlice, QuadrantCell,
}

pub enum EdgeEnds {
    None, Arrow, Circle, Cross, Both, MultiCircle, MultiCross,
}
```

> **Stage 3 是唯一的"视觉决策点"**：消费 `Theme`，把 `GGNode.shape` + `UGNode.style_ref` +
> `UGEdge.kind` 解析成具体 `Fill`/`Stroke`/`EdgeEnds`。`theme::*` 的全部散落逻辑在此收敛。
> 之后 `paint` 不再有任何 theme 依赖、不再有任何图类型判断。

### 3.4 形状枚举的唯一真相源

`ShapeKind` 全项目只定义一份（在 `builder/ir/shape.rs`），三处共用：
- `extract` 把 `ast::NodeShape` / class 框 / pie 扇区 → `ShapeKind`；
- `layout` 据 `ShapeKind` 计算端口与尺寸；
- `paint` 据 `ShapeKind` 选 `Element` 变体（菱形 → `Polygon`、圆柱 → `Path` + `Ellipse`、饼扇 → `Pie`）。

AST 的 `NodeShape`（rectangle/round/ Stadium/subroutine/diamond/hexagon/circle/doublecircle/
cylinder/asymmetric/parallelogram/trapezoid/bar）与 state 的 start/end 全部映射到 `ShapeKind`，
`__start__/__end__` 的特判彻底消失（变成 `StartDot/EndDot`）。

---

## 4. 四阶段模块设计

### 4.1 `builder::extract`（唯一碰 AST）

```
src/builder/extract/
  mod.rs            // dispatch: match &diagram { Flowchart(f) => extract_flowchart(f), ... }
  flowchart.rs      // FlowchartNode -> Unigraph (family=Directed)
  state.rs          // StateNode -> Unigraph
  class.rs          // ClassDiagramNode -> Unigraph (family=Grid)
  er.rs             // ErDiagramNode -> Unigraph (family=Grid)
  sequence.rs       // SequenceNode -> Unigraph (family=Sequence)
  pie.rs            // PieNode -> Unigraph (family=Radial)
  timeline.rs       // TimelineNode -> Unigraph (family=Linear)
  gitgraph.rs       // GitGraphNode -> Unigraph (family=Hierarchy)
  gantt.rs          // Gantt -> Unigraph (family=Hierarchy)
  mindmap.rs        // MindMap -> Unigraph (family=Linear/Hierarchy)
  quadrant.rs       // Quadrant -> Unigraph (family=Radial)
  sankey.rs         // Sankey -> Unigraph (family=Hierarchy)
  common.rs         // 共享：把 RichText -> LabelSpec、class 串 -> 多行 LabelSpec、端口推导
```

每个 `extract_*` 只负责语义 → `Unigraph` 的翻译，不碰任何坐标。
**这是新增图类型的唯一入口。**

### 4.2 `builder::layout`（边感知布局，核心）

```
src/builder/layout/
  mod.rs            // LayoutEngine::run(ug, theme) -> Geograph
  measure.rs        // 测量所有 LabelSpec / 容器子项 -> 尺寸（桥接 ast::RichText::measure + lievisual 文本度量）
  crossing.rs       // ★ 通用交叉减少原语 minimize_crossings(layers, edges)
  family/
    directed.rs     // 分层 DAG：Sugiyama 风格（分层 + barycenter 双 pass + SCC 收缩）
    grid.rs         // 网格排布 + 调用 minimize_crossings
    linear.rs       // 线性/径向支架
    sequence.rs     // 泳道 + 时间轴 + 消息路由
    radial.rs       // 饼/象限极坐标
    hierarchy.rs    // gitgraph/gantt 时间轴
  route.rs          // ★ EdgeRouter：正交通道分配 + 边-边偏移 + 边-节点回避 + 标签占位
  coord.rs          // 端口 → 实际坐标、裁剪到边框
  spatial.rs        // ★ 边-边排斥用的空间索引（网格哈希），把 O(E²) 降到 O(E·邻近)
```

**边感知布局要点（呼应 `layout-edge-aware-design.md`）：**

1. **`minimize_crossings` 通用化**：从 `sugiyama.rs` 抽出，所有分层 family 复用；
   含 SCC 收缩（环内也优化）、双向扫描、`crossing_iterations` 收敛即停（硬上限 5 轮，见 §4.2 性能）。
2. **tie-breaker 保确定性**：同重心节点按 UG 中原始出现顺序（来自 AST 稳定遍历）排序，
   保持"代码不动布局永不变"的纪律。
3. **`EdgeRouter`（独立阶段）**：节点坐标定后，给每条边分配**正交通道**，
   通道冲突则引入平行偏移（边-边排斥），路由代价含"线段穿过非端点节点"惩罚（边-节点回避），
   边标签 `label_space` 预留中点空白。这从几何层面消除重叠/交叉，而非仅顺序层面。
4. **`UGEdge` 约束全消费**：`priority` 影响交叉权重、`routing_hint` 选路由器、
   `arrow` 决定 `ArrowSpec`、`repulsion` 进路由代价。
5. **性能方案（评审 3.1 采纳）**：
   - **空间索引降复杂度**：`spatial.rs` 用均匀网格哈希（cell ≈ 平均节点间距）对所有边线段建索引，
     边-边排斥只在与自己 bounding-box 重叠的少量邻近边间计算，从朴素 `O(E²)` 降至
     `O(E · k)`（k 为邻近边数，通常 ≪ E）。大图（>500 节点）启用。
   - **分层路由**：同层内边先做通道分配（局部搜索），跨层边走固定垂直通道，
     避免全局两两比较。
   - **迭代硬上限**：`crossing_iterations` 默认 5（旧实现 12，多为无效空转）、路由偏移迭代上限 3；
     连续两轮代价无改善即提前收敛。
   - **预算护栏**：对超大图设时间/迭代预算，超限则降级为"无排斥的简单正交路由"，保证不卡死。

### 4.3 `builder::materialize`（视觉决策点，唯一碰 Theme）

```
src/builder/materialize/
  mod.rs            // Geograph + StyleIntent + Theme -> SceneGraph
  shapes.rs         // GGNode.shape + style_ref -> FillStrokeStyle（查 Theme）
  edges.rs          // StyleIntent 中边样式 -> Stroke + EdgeEnds（查 Theme 的线型表）
  labels.rs         // GGLabel -> 重测绘制布局 -> RichSpan + TextStyle
  containers.rs     // GGContainer -> 背景 Rect + 边框 + 标题 Label
  theme_apply.rs    // 把 Theme 的 class/节点/边样式表映射成 lievisual 的 Fill/Stroke
```

`theme_apply.rs` 一次性吞下现在散落在 `theme::flowchart::*` / `theme::class::*` /
`theme::sequence::*` 的所有取色逻辑。换主题 = 换 `Theme` 入参，materialize 与 paint 不改动。

**生命周期（评审 3.2 采纳）**：`materialize` **不直接依赖 `UG`**，而是依赖
`LayoutEngine` 一并产出的 `StyleIntent`——它是 UG 中 `style_ref` / `edge.kind` / `arrow`
等"视觉意图"在 layout 结束时被抽取出的轻量结构（与几何坐标无关）。这样
**UG 在 layout 阶段结束后即可 `drop`**，只在 `extract → measure → layout` 期间存活，
`materialize`/`paint` 完全不持有 UG 引用。

```rust
pub struct StyleIntent {
    pub node_styles: Vec<(NodeId, StyleRef)>,   // 节点 id → 样式引用
    pub edge_styles: Vec<(EdgeId, EdgeKind, ArrowSpec)>, // 边 → 语义 + 箭头
    pub container_styles: Vec<(ContainerId, ContainerKind)>,
}
```

### 4.4 `builder::paint`（零分支纯翻译）

```
src/builder/paint/
  mod.rs            // SceneGraph -> Scene（遍历 items，逐个 emit Element）
  shape_to_element.rs  // ShapeKind -> Element 变体（菱形→Polygon、圆柱→Path、饼扇→Pie…）
  edge_to_element.rs   // EdgeEnds -> 起止 Marker（Arrow/Circle/Cross 的小 Path/Polyline）
  text_to_element.rs   // Label -> Element::Text（带 RichSpan + layout）
  group.rs             // Group -> SceneNode::group（处理 z_index / clip）
```

`paint` 是纯函数 `fn paint(sg: &SceneGraph) -> Scene`，**不接收 `&Diagram`、不接收 `&Theme`**，
因此结构上不可能回查 AST。这是上一轮"渲染解耦"痛点的彻底解决。

---

## 5. 入口衔接（保持对外 API 不变）

`scene_ext.rs` 重写为：

```rust
use crate::builder::{extract, layout, materialize, paint};

pub trait ToScene {
    fn to_scene(&self) -> Result<Scene, DiagramError>;
}

impl ToScene for Diagram {
    fn to_scene(&self) -> Result<Scene, DiagramError> {
        let theme = Theme::default();                 // 或来自 Diagram 携带的主题
        let ug = extract::run(self)?;                 // Stage 1：AST → UG（含未测文本）
        let ug = measure::measure_all(ug, &theme)?;   // Stage 1.5：测量 → UG'（含尺寸）
        let (gg, style) = layout::LayoutEngine::run(&ug, &theme)?; // Stage 2 + 抽取 StyleIntent
        // ug 此后可 drop；materialize 只依赖 gg + style，不再持有 ug
        let sg = materialize::run(&gg, &style, &theme)?; // Stage 3：几何 + 视觉 → SG
        let scene = paint::run(&sg);                  // Stage 4：SG → lievisual::Scene
        Ok(scene)
    }
}
```

> 注：`LayoutEngine::run` 返回 `(Geograph, StyleIntent)`，把"几何"与"视觉意图"在 layout
> 结束时分离。**UG 仅在 `extract → measure → layout` 期间存活**，layout 后即可释放；
> `materialize`/`paint` 完全不持有 UG 引用（见 §4.3）。`paint` 只吃 `sg`。
> 对外 `to_scene` 签名与错误类型完全不变，现有 `tests/` 下的 golden 调用方无需修改。

---

## 6. 与现有设计文档的关系

| 旧模块 | 去处 |
|---|---|
| `ast.rs` / `vir.rs` | **保留不动**（输入契约） |
| `scene_ext.rs` | 重写为 §5 的四阶段调用（API 不变） |
| `builder/layout/ir.rs`（`PlacedGraph`） | 被 `Geograph` 取代 |
| `builder/layout/types.rs`（`Layout`/`LayoutNode`） | 被 `Unigraph`/`Geograph`/`SceneGraph` 三层取代 |
| `builder/layout/solver/*` + `sugiyama.rs` | 拆并进 `layout/family/*` + `crossing.rs` + `route.rs` |
| `builder/render/*`（directed/state/class/... 回查 AST 的渲染器） | 删除，统一为 `paint`（零分支） |
| `builder/theme/*`（`theme::flowchart::*` 散落取色） | 收敛进 `materialize/theme_apply.rs` |
| `builder/measure.rs` | 保留并升级为 `layout/measure.rs` |

待删除的旧文档：`layout-system-design.md`、`layout-refactor.md`、`refactor-layout.md`
（可保留为 `docs/archive/` 历史参考）；新增 `layout-edge-aware-design.md`（边感知，已落地为 §4.2）+ 本文。

---

## 7. 分阶段落地路线

### Phase 0 — 脚手架 + 可观测性
- 建 `builder/{ir,extract,layout,materialize,paint}` 空模块与三层 IR 类型。
- 实现 `crossing.rs` 的 `minimize_crossings` + 几何度量（边交叉数 / 边重叠数 / 线穿节点数），
  作为后续验收基准（详见 `layout-edge-aware-design.md` §5）。
- 加 `Diagram` → `Unigraph` 的 `extract_flowchart` 最小实现（只节点 + 无标签边）。

### Phase 1 — flowchart 端到端打通（验证 IR 有效性）
- `layout/family/directed.rs` + `route.rs` 接 flowchart（含边感知路由）。
- `materialize` + `paint` 接 flowchart 全部 `ShapeKind` 与 `EdgeEnds`。
- 用现有 flowchart golden 验证：IR 输出格式不变、视觉质量提升（交叉/重叠下降）。
- **里程碑**：flowchart 全图走新管线，旧 `directed.rs` 渲染器可删。

### Phase 2 — 吸收 state / class / er（验证 family 复用）
- `extract_state/class/er` + `family/grid.rs`（复用 `minimize_crossings`）。
- 验证 state 的 `StartDot/EndDot`、class 的 `ClassEdgeKind` 箭头全部落到 `ShapeKind`/`EdgeEnds`。

### Phase 3 — sequence / pie / timeline / gitgraph / gantt / mindmap / quadrant / sankey
- 各自 `extract_*` + family solver；验证 `paint` 零改动即可渲染（仅新增 `ShapeKind` 变体时改 `shape_to_element`）。

### Phase 4 — 收敛与删除（分阶段，保留降级路径，评审 3.6 采纳）
- **Phase 1 完成后**：删除 `directed.rs`（flowchart 专用渲染器），新管线已验证可完全替代；
  其余旧渲染器保留作为回退。
- **Phase 3 完成后**：删除 `state.rs`/`class.rs`/`er.rs`/`sequence.rs`/... 所有其他旧渲染器。
- **Phase 4 最后**：删除旧 IR（`PlacedGraph` / `Layout` / `LayoutNode`）+ `theme::*` 散落逻辑
  + `builder/layout/convert.rs`/`sugiyama.rs` 等旧转换层；文档归档（见 §6）。
- 每阶段结束后保留至少一条可用渲染路径（旧或新），降低回退成本。

---

## 8. 验收标准

1. **对外 API 不变**：`Diagram::to_scene()` 签名、`DiagramError` 兼容；现有 `tests/` golden 调用方零改动。
2. **渲染解耦成立**：`paint` 模块静态保证不引用 `ast` / `theme`（可用 `grep` + 编译单元隔离验证；建议把 `paint` 放独立 crate 或 `#![deny]` 违规 import）。
3. **边感知成立**：Phase 0 度量显示，引入 `route.rs` 后典型 flowchart/class/er 的
   "边交叉数 / 边重叠数 / 线穿节点数"较旧实现 **下降 ≥ 50%**。
4. **确定性纪律**：同一 `Diagram` 多次 `to_scene` 结果字节一致（tie-breaker 生效）。
5. **可扩展性**：新增一种图类型 = 只写 `extract_*` +（如需）一个 family solver；
   `materialize`/`paint` 不改动（除非引入全新 `ShapeKind`）。
6. **回归策略（评审 3.5 采纳）**：优先**结构级回归**——对 `SceneGraph` 做 JSON 序列化、
   坐标四舍五入到小数点后 2 位后比对，避免浮点误差导致 golden 不稳定；
   **像素级**（SVG/PNG 字节）仅抽样 10 个典型 flowchart 做，不做全量，避免 CI 因浮点频繁失败。

---

## 9. 风险与权衡

- **性能**：`route.rs` 边-边排斥默认经 `spatial.rs` 空间索引降到 `O(E·k)`（见 §4.2 要点 5）；
  超大图走分层路由 + 预算护栏降级，避免卡死；`crossing_iterations` 硬上限 5。
- **文本布局耦合**：`materialize/labels.rs` 需调用 `lievisual::text` 度量/布局，
  这层依赖不可避免（文字尺寸必须真实测量），但仅限 `materialize`，`paint` 仍纯净。
- **ir 体积**：三层 IR 看起来比旧 `PlacedGraph` 重，但每层职责单一、单向流动，
  比"旧 IR 半吊子 + 渲染器回查 AST"的总复杂度低，且可独立单测。
- **向后兼容 golden**：IR 输出格式变了，但 `lievisual::Scene` 输出（SVG/PNG）应保持一致；
  结构级回归（§8 要点 6）优先，像素级仅抽样。

---

## 10. 数据流示例（一个最小 flowchart，评审 4.2 采纳）

以 mermaid 文本 `A-->B-->C`（三个矩形、两条实线箭头）为例，展示三层 IR 的真实结构切片。

### Stage 1 → UG（extract 产物，未测文本）

```rust
Unigraph {
  family: Directed,
  nodes: [
    UGNode { id:"A", role:Atom, label: LabelSpec{ text:"A", spans:[RichSpan("A")] },
             ports: PortSet{T,B,L,R}, size_hint: ByText, style_ref: StyleRef::NodeDefault, .. },
    UGNode { id:"B", .. 同构 .. },
    UGNode { id:"C", .. 同构 .. },
  ],
  edges: [
    UGEdge { id:"e0", source:"A", target:"B", src_port:PortHint::Bottom, tgt_port:PortHint::Top,
             kind:Flow, label:None, priority:Primary, routing:Orthogonal, arrow:ArrowSpec{end:Arrow}, .. },
    UGEdge { id:"e1", source:"B", target:"C", .. 同构 .. },
  ],
}
```

### Stage 1.5 → UG'（measure 写回尺寸，仍以 UG 结构存在）

```rust
UGNode { id:"A", label: MeasuredLabel{ text:"A", layout: TextLayout{width:14.0,height:16.0} },
         size_hint: Resolved(Size{width:54.0, height:36.0}) /* 由 ShapeKind::Rect + 文本推算 */, .. }
// B、C 同理
```

### Stage 2 → GG（layout 产出，纯几何，无颜色）

```rust
Geograph {
  size: Size{width:120, height:160},
  nodes: [
    GGNode { id:"A", shape:Rect, center:Point{60,28},  size:Size{54,36}, ports: ResolvedPorts{top:(60,10),bottom:(60,46),..} },
    GGNode { id:"B", shape:Rect, center:Point{60,80},  size:Size{54,36}, ports: {top:(60,62),bottom:(60,98)} },
    GGNode { id:"C", shape:Rect, center:Point{60,132}, size:Size{54,36}, ports: {top:(60,114),bottom:(60,150)} },
  ],
  edges: [
    GGEdge { id:"e0", route:[Point{60,46}, Point{60,62}], label_anchor:None, kind:Flow, arrow:ArrowSpec{end:Arrow} },
    GGEdge { id:"e1", route:[Point{60,98}, Point{60,114}], .. },
  ],
  // StyleIntent 同时产出：
  StyleIntent { node_styles:[("A",NodeDefault),("B",..),("C",..)],
                edge_styles:[("e0",Flow,ArrowSpec{end:Arrow}),("e1",..)] }
}
```

> 注意：GG 里**没有颜色**。`A/B/C` 的填充色、`e0` 的线色都还在 `StyleIntent` 里，
> 等待 Stage 3 查 `Theme` 解析。

### Stage 3 → SG（materialize 产物，视觉自足，无 AST 引用）

```rust
SceneGraph {
  size: Size{width:120, height:160},
  background: Color::WHITE,
  items: [
    SceneItem::Shape { geometry: Rect{at:Point{33,10}, w:54, h:36},
                       fill: Some(Solid(Color{r:0xEE,g:0xF7,..})),  // 来自 theme.flowchart.node_fill
                       stroke: Some(Stroke{color:BLACK, width:1.0,..}), z:0 },
    SceneItem::Shape { geometry: Rect{at:Point{33,62},..}, fill:.., stroke:.., z:0 },  // B
    SceneItem::Shape { geometry: Rect{at:Point{33,114},..}, fill:.., stroke:.., z:0 }, // C
    SceneItem::Edge  { path:[Point{60,46},Point{60,62}], stroke: Stroke{color:BLACK,width:1.5,..},
                       ends: EdgeEnds::Arrow, z:1 },
    SceneItem::Edge  { path:[Point{60,98},Point{60,114}], stroke:.., ends: Arrow, z:1 },
    SceneItem::Label { text:[RichSpan("A")], position:Point{60,28}, style:TextStyle{..center}, anchor:Center, z:2 },
    SceneItem::Label { text:[RichSpan("B")], position:Point{60,80}, .. z:2 },
    SceneItem::Label { text:[RichSpan("C")], position:Point{60,132}, .. z:2 },
  ],
}
```

### Stage 4 → Scene（paint 纯翻译，lievisual 原语）

```rust
Scene {
  width:120, height:160, background: WHITE,
  nodes: [
    SceneNode{ element: Element::Rect{position:{33,10}, size:{54,36}, style: FillStrokeStyle{fill,stroke}}, z_index:0 },
    SceneNode{ element: Element::Rect{..B..}, z_index:0 },
    SceneNode{ element: Element::Rect{..C..}, z_index:0 },
    SceneNode{ element: Element::Line{start:{60,46}, end:{60,62}, style:Stroke{..}}, z_index:1 },
    SceneNode{ element: Element::Line{..e1..}, z_index:1 },
    SceneNode{ element: Element::Text{spans:[RichSpan("A")], position:{60,28}, style:..}, z_index:2 },
    // B / C 文本同理
  ],
}
```

> 这个切片清晰展示了：颜色/线型**只在 Stage 3 注入一次**；`paint` 只是
> `SceneItem::Shape → Element::Rect`、`SceneItem::Edge → Element::Line` 的字段映射，
> 没有任何 `match diagram` 或 `theme::*` 调用。这就是"渲染不回查 AST"的落地形态。
