# Liemermaid 启发式布局算法设计

> 版本：0.2.0  
> 日期：2026-05-30  
> 状态：设计草案

---

## 目录

1. [概述与动机](#1-概述与动机)
2. [现状分析](#2-现状分析)
3. [核心设计原则](#3-核心设计原则)
4. [多层布局管线](#4-多层布局管线)
5. [Pass 1 — 结构识别](#5-pass-1--结构识别)
6. [Pass 2 — 尺寸测量](#6-pass-2--尺寸测量)
7. [Pass 3 — 拓扑排序与层级分配](#7-pass-3--拓扑排序与层级分配)
8. [Pass 4 — 层级内排序与交叉最小化](#8-pass-4--层级内排序与交叉最小化)
9. [Pass 5 — 几何定位](#9-pass-5--几何定位)
10. [Pass 6 — 画布适配](#10-pass-6--画布适配)
11. [Pass 7 — 边路由](#11-pass-7--边路由)
12. [子图 (Subgraph) 布局](#12-子图-subgraph-布局)
13. [数据结构设计](#13-数据结构设计)
14. [实施路线图](#14-实施路线图)

---

## 1. 概述与动机

### 1.1 问题

当前 liemermaid 的流程图布局（`flowchart.rs`）采用简化的单次 BFS 层级分配 + 固定尺寸定位，存在以下根本性问题：

- **节点尺寸硬编码**：所有节点都是 140×50，不随文本长度自适应
- **结构识别缺失**：无法区分链式、分支、循环等子结构，一律按层级堆放
- **循环处理 ad-hoc**：靠硬编码 `has_loop_body / prev_y` 临时修补
- **画布尺寸固定**：用户必须猜测合适的 width/height，否则节点溢出或空白过多
- **无法扩展**：子图 (Subgraph)、边标签、BT/RL 方向均无法正确处理

### 1.2 目标

设计一个**启发式多遍布局管线**，将"逻辑分组"与"几何定位"完全分离，使布局引擎能够：

1. 自动识别图的逻辑子结构（链、分支、循环、并行）
2. 根据文本内容测量节点尺寸
3. 自动适配画布大小
4. 支持完整的 Mermaid flowchart 方向（TD、LR、BT、RL）
5. 为子图和边标签布局预留扩展点

---

## 2. 现状分析

### 2.1 当前管线

```
Mermaid Text
    → Pest Parser (grammar/mermaid.pest)
    → AST (ast.rs: Flowchart { direction, nodes, edges, subgraphs })
    → builder/flowchart.rs:
        1. build_graph()       → in_degree, adjacency
        2. assign_layers()     → layers: HashMap<NodeId, usize>
        3. compute_positions() → positions: HashMap<NodeId, Point>
        4. draw_edge()         → Polyline elements
        5. draw_node()         → Rect + TextRun elements
    → SVG Renderer
```

### 2.2 关键常量（硬编码）

| 常量 | 值 | 说明 |
|------|-----|------|
| `NODE_WIDTH` | 140.0 | 所有节点固定宽度 |
| `NODE_HEIGHT` | 50.0 | 所有节点固定高度 |
| `H_GAP` | 60.0 | 同一层节点间水平间距 |
| `V_GAP` | 80.0 | 相邻层间垂直间距 |
| `MARGIN` | 40.0 | 画布边距 |

### 2.3 当前缺陷

#### 缺陷 1：层级分配过于简单

`assign_layers()` 使用标准 BFS 拓扑排序。对于 A→B→C→D 的链式结构能正确产出 4 层，但对于复杂结构完全束手无策。

**案例：循环结构**
```
A[Start] → B{Continue} → C[Process]
             ↑____________|
                        ↓
                       D[End]
```

BFS 产出 `{A:0, B:1, C:2, D:2}`，然后靠 `compute_positions` 中硬编码的 `loop_body_nodes` 检测和 `prev_y` 偏移把 C 提到 B 的同 y 坐标——这完全是一种 workaround。

#### 缺陷 2：节点尺寸无法适应文本

`draw_node()` 总用 `NODE_WIDTH=140` 和 `NODE_HEIGHT=50`，不管文本是 "A" 还是 "A very long description text"。长文本会溢出。

#### 缺陷 3：没有交叉最小化

同一层有多个节点时（如分支结构 Branch1、Branch2），当前只是简单从左到右排列，不考虑如何减少边交叉。

#### 缺陷 4：边路由僵硬

正向边走标准直角路径，反向边走硬编码的右侧绕行。不同层级间、跨层的边没有智能路由。

---

## 3. 核心设计原则

### 原则 1：分组先于定位

先识别图的**逻辑子结构**（链、分支、循环、并行程），将其抽象为**逻辑组 (LogicalGroup)**，再对组内和组间进行几何定位。

这样做的好处是：
- 结构语义清晰，几何约束可从结构推导
- 每个子结构有独立的局部坐标系统
- 子结构可递归嵌套（为子图支持做准备）

### 原则 2：尺寸从内容推导

节点的宽度和高度应该根据其文本内容、形状类型、字体大小来计算：

```
node_width  = max(MIN_WIDTH, text_width + 2 × PADDING)
node_height = max(MIN_HEIGHT, text_height + 2 × PADDING)
```

对于菱形、六边形等形状，还需要额外的形状因子。

### 原则 3：画布从布局推导

不再要求用户指定画布尺寸。布局计算完成后，根据所有 Element 的 bounding box 自动确定画布尺寸：

```
canvas_width  = bbox.width  + 2 × MARGIN
canvas_height = bbox.height + 2 × MARGIN
```

如果用户显式指定了尺寸，则按比例缩放适配。

### 原则 4：方向抽象

不区分 TD/LR/BT/RL，统一用**主轴 (main axis)** 和**交叉轴 (cross axis)** 表达：
- TD: main=向下, cross=向右
- LR: main=向右, cross=向下
- BT: main=向上, cross=向右
- RL: main=向左, cross=向下

所有布局计算在逻辑坐标中进行，最后做坐标旋转/翻转。

---

## 4. 多层布局管线

```
┌─────────────────────────────────────────────────────────────┐
│                    Multi-Pass Layout Pipeline                │
├───────────────┬─────────────────────────────────────────────┤
│ Pass 1        │ 结构识别 (Structure Recognition)             │
│               │ 将图分解为 LogicalGroup 树                   │
├───────────────┼─────────────────────────────────────────────┤
│ Pass 2        │ 尺寸测量 (Sizing)                            │
│               │ 根据文本内容计算每个节点/子结构的尺寸         │
├───────────────┼─────────────────────────────────────────────┤
│ Pass 3        │ 拓扑排序与层级分配 (Layer Assignment)        │
│               │ 对每个 Group 内进行精化层级分配               │
├───────────────┼─────────────────────────────────────────────┤
│ Pass 4        │ 层级内排序与交叉最小化 (Ordering)            │
│               │ Barycenter 启发式重排，减少边交叉             │
├───────────────┼─────────────────────────────────────────────┤
│ Pass 5        │ 几何定位 (Positioning)                       │
│               │ 在逻辑坐标中分配每个节点和 anchor 的绝对坐标  │
├───────────────┼─────────────────────────────────────────────┤
│ Pass 6        │ 画布适配 (Canvas Fitting)                    │
│               │ 计算 bounding box → 自动确定画布尺寸         │
├───────────────┼─────────────────────────────────────────────┤
│ Pass 7        │ 边路由 (Edge Routing)                        │
│               │ 智能路由边，处理直连、绕行、自环、标签放置    │
└───────────────┴─────────────────────────────────────────────┘
```

---

## 5. Pass 1 — 结构识别

### 5.1 逻辑组类型

```rust
/// 逻辑子结构
enum LogicalGroup {
    /// 链式结构：A → B → C → D
    Chain {
        nodes: Vec<NodeId>,
        direction: ChainDir,
    },
    /// 分支结构：从一个节点分叉到多个节点，再汇合
    Branch {
        source: NodeId,
        branches: Vec<BranchArm>,
        sink: Option<NodeId>,
    },
    /// 循环结构：Body 中有边回到 Condition
    Cycle {
        condition: NodeId,       // 决策节点，如 B{Continue}
        body: Box<LogicalGroup>, // 循环体，如 C[Process]
        exit: Option<NodeId>,    // 循环出口，如 D[End]
    },
    /// 并行结构：多个独立分支同时执行
    Parallel {
        branches: Vec<LogicalGroup>,
    },
    /// 单个叶子节点
    Leaf {
        node_id: NodeId,
    },
}

struct BranchArm {
    label: Option<String>,
    body: LogicalGroup,
}
```

### 5.2 识别算法

从入口节点（入度为 0 的节点）出发，按以下优先级递归识别：

```
function recognize_structure(entry, graph) → LogicalGroup:
    1. 如果当前节点只有一个后继，走 Chain 识别
       → 沿单一后继方向收集所有连续节点
       
    2. 检测是否有回边（反向边）：
       a. 找到所有从某节点回到前面已访问节点的边
       b. 回边的源节点所在子结构 = 循环体
       c. 回边的目标节点 = 循环条件
       → 构造 Cycle { condition, body, exit }
       → 从 exit 节点继续
       
    3. 如果有多个出边（分叉）：
       a. 每个出边对应一个 BranchArm
       b. 递归识别每个 arm 的内部结构
       c. 检测是否有汇合点（多个 arm 汇聚到同一节点）
       → 构造 Branch { source, branches, sink }
       → 从 sink（如果有）继续
       
    4. 其余情况：
       → 构造 Leaf
```

### 5.3 识别示例

**示例 1：简单链式**
```
A → B → C → D
```
→ `Chain([A, B, C, D])`

**示例 2：分支结构**
```
      → Branch1 
A → B            → E
      → Branch2 
```
→ `Chain([A])` + `Branch(B, [Arm(→Chain([Branch1])), Arm(→Chain([Branch2]))], E)`

**示例 3：循环结构**
```
A → B{Continue} → C[Process] → B
             ↓
            D[End]
```
→ `Chain([A])` + `Cycle(B, Leaf(C), D)`

**示例 4：嵌套结构**
```
A → B → C → D → E
      ↓       ↑
      F → G → H
```
→ `Chain([A])` + `Branch(B, [Arm(→Chain([C,D,E])), Arm(→Chain([F,G,H]))])`  
或识别为 `Cycle(B, Branch(B, [Arm(→Chain([F,G,H]))], E), ...)`

### 5.4 Group Tree

识别完成后，整个图被表示为一棵 `LogicalGroup` 树：

```
Flowchart
  └─ Chain
      ├─ Leaf(A)
      └─ Cycle
          ├─ condition: Leaf(B)
          ├─ body: Leaf(C)
          └─ exit: Leaf(D)
```

这棵树是后续所有 pass 的基础。

---

## 6. Pass 2 — 尺寸测量

### 6.1 节点尺寸计算

每个节点的实际尺寸根据以下因素确定：

| 因素 | 来源 | 影响 |
|------|------|------|
| 文本内容 | `node.text` | 影响 width、height |
| 节点形状 | `node.shape` | 影响 padding、宽高比 |
| 字体设置 | `config.font_size`, `config.font_family` | 影响文本测量 |

```rust
struct NodeMetrics {
    /// 节点的外接矩形尺寸
    size: Size,
    /// 文本渲染区域（节点内部，居中对齐）
    text_bounds: Size,
    /// 节点的锚点偏移（顶部/底部/左/右出口位置相对于 center 的偏移）
    anchors: NodeAnchors,
}

struct NodeAnchors {
    top: Point,      // center + (0, -height/2)
    bottom: Point,   // center + (0, +height/2)
    left: Point,     // center + (-width/2, 0)
    right: Point,    // center + (+width/2, 0)
}

fn measure_node(node: &Node, config: &OutputConfig) -> NodeMetrics {
    let text = node.text.as_deref().unwrap_or(&node.id);
    
    // 1. 测量文本自然尺寸
    let text_layout = create_text_layout(text, &text_style, None);
    let text_w = text_layout.width();
    let text_h = text_layout.height();
    
    // 2. 根据形状类型确定 padding 和最小尺寸
    let (min_w, min_h, pad_x, pad_y) = shape_metrics(node.shape);
    
    // 3. 计算节点尺寸
    let width  = max(min_w, text_w + 2.0 * pad_x);
    let height = max(min_h, text_h + 2.0 * pad_y);
    
    NodeMetrics { size: Size::new(width, height), text_bounds: Size::new(text_w, text_h), ... }
}
```

### 6.2 Group 尺寸计算

对于逻辑组，递归计算其边界矩形和内部布局参数：

```rust
struct GroupMetrics {
    /// 组的总尺寸（包含所有子元素）
    size: Size,
    /// 组的主轴方向（用于全局布局）
    main_axis_direction: AxisDir,
    /// 组的内部布局参数
    internal_layout: InternalLayout,
}

enum InternalLayout {
    /// 链式：子节点沿主轴排列
    Chain {
        item_sizes: Vec<Size>,
        total_main: f64,  // 主轴上总长度
        max_cross: f64,   // 交叉轴上最大宽度
    },
    /// 分支：水平排列分支，条件在上
    Branch {
        source_size: Size,
        branch_sizes: Vec<Size>,
        sink_size: Option<Size>,
    },
    /// 循环：条件居中，循环体在侧面
    Cycle {
        condition_size: Size,
        body_size: Size,
        exit_size: Option<Size>,
        body_side: Side, // Left 或 Right
    },
}
```

---

## 7. Pass 3 — 拓扑排序与层级分配

### 7.1 分层策略

对每个 `LogicalGroup` 内部进行层级分配。不同类型的 Group 有不同的层级规则：

#### Chain 内层级

最简单的：按顺序每个节点一层。

```
Chain([A, B, C, D]) → layers: {A:0, B:1, C:2, D:3}
```

#### Branch 内层级

```
source  → layer 0
branches → layer 1 (所有分支共享同一层)
sink    → layer 2
```

#### Cycle 内层级

```
condition → layer 0
body      → layer 1 (但 y 坐标与 condition 对齐)
exit      → layer 2 (如果存在)
```

注意：`body` 虽然在 layer 1，但几何定位时会把它和 condition 放在同一 y 坐标上（交叉轴对齐），实现"侧面"效果。这是**逻辑层级**和**几何层级**分离的体现。

### 7.2 全局层级

Group 之间的层级也按拓扑关系确定。例如：

```
A → Cycle(B, C, D) → E
```

全局层级：`{A:0, B:1, C:1, D:2, E:3}`

### 7.3 长边处理（Dummy Nodes）

对于跨越多个层级的边（如从 layer 0 到 layer 3 的边），插入 dummy 节点：

```
A (layer 0) ────────────→ D (layer 3)

变为：
A (layer 0) → dummy_1 (layer 1) → dummy_2 (layer 2) → D (layer 3)
```

这样做的好处：
- 简化边路由：每条边只连接相邻层
- 为 Pass 4 的交叉最小化提供完整的层级约束

---

## 8. Pass 4 — 层级内排序与交叉最小化

### 8.1 问题

同一层有多少节点时，它们的水平排列顺序会影响边的交叉数量。例如：

```
Layer 0: A   B       Layer 0: A   B
          \ /                  X
          / \                 / \
Layer 1: C   D       Layer 1: C   D
```

左边的排列有 0 个交叉，右边有 1 个交叉。

### 8.2 Barycenter 启发式算法

对每一层的节点，按其在上下层中相邻节点的平均位置重新排序：

```
function order_layer(layer_k, layer_{k-1}, layer_{k+1}):
    for each node in layer_k:
        barycenter = average_x_of_neighbors_in(layer_{k-1})
                     + average_x_of_neighbors_in(layer_{k+1})
    sort layer_k by barycenter
```

重复 2-3 轮直到稳定（最多 10 轮）。

### 8.3 方向适配

在 LR 方向中，交叉轴是垂直的，所以比较的是 y 坐标而非 x 坐标。核心逻辑一致，只需交换主轴/交叉轴。

---

## 9. Pass 5 — 几何定位

### 9.1 坐标系统

所有位置计算在**逻辑坐标**中进行，使用主轴/交叉轴抽象：

```rust
struct LayoutCoord {
    main: f64,   // 主轴坐标（TD 时 = y，LR 时 = x）
    cross: f64,  // 交叉轴坐标
}

impl LayoutCoord {
    /// 根据方向转换为画布绝对坐标 (x, y)
    fn to_canvas(&self, direction: Direction) -> Point {
        match direction {
            Direction::TD | Direction::TB => Point::new(self.cross, self.main),
            Direction::BT              => Point::new(self.cross, canvas_height - self.main),
            Direction::LR              => Point::new(self.main, self.cross),
            Direction::RL              => Point::new(canvas_width - self.main, self.cross),
        }
    }
}
```

### 9.2 链式布局

```
function position_chain(items, start_main, center_cross):
    cur_main = start_main
    for each item:
        item.center.main = cur_main + item.height/2  (TD方向)
        item.center.cross = center_cross
        cur_main += item.height + V_GAP
```

### 9.3 分支布局

```
function position_branch(branch_metrics, start_main, center_cross):
    // source 居中
    source = center_cross
    
    // branches 在 source 下方，水平排列
    total_cross_width = sum of branch widths + gaps
    start_cross = center_cross - total_cross_width/2
    
    for each branch:
        branch.center.cross = start_cross + branch_width/2
        branch.center.main = source.bottom.main + V_GAP
        start_cross += branch_width + H_GAP
    
    // sink 在 branches 下方居中
    sink.cross = center_cross
    sink.main = branches_bottom + V_GAP
```

### 9.4 循环布局

```
function position_cycle(cycle_metrics, start_main, center_cross):
    // condition 居中
    condition.cross = center_cross
    condition.main = start_main
    
    // body 在 condition 侧面（同一 main 坐标）
    body.cross = MARGIN  // 左侧
    body.main = condition.main
    
    // exit 在 condition 正下方
    exit.cross = center_cross
    exit.main = condition.bottom + V_GAP
    
    // 如果 body 高度大于 condition，调整偏移
```

### 9.5 递归布局

从根 Group 开始，递归计算每个子 Group 的布局：

```rust
fn layout_group(
    group: &LogicalGroup,
    layout_ctx: &mut LayoutContext,
) -> HashMap<NodeId, NodePosition> {
    match group {
        LogicalGroup::Chain { nodes, .. } => layout_chain(nodes, layout_ctx),
        LogicalGroup::Branch { source, branches, sink } => layout_branch(...),
        LogicalGroup::Cycle { condition, body, exit } => layout_cycle(...),
        LogicalGroup::Leaf { node_id } => layout_leaf(node_id),
    }
}
```

---

## 10. Pass 6 — 画布适配

### 10.1 自动计算画布尺寸

遍历所有 Element，计算最小 bounding box：

```rust
fn compute_canvas_size(positions: &HashMap<NodeId, NodePosition>, 
                       metrics: &HashMap<NodeId, NodeMetrics>,
                       edges: &[RoutedEdge]) -> (f64, f64) {
    let mut bbox = BoundingBox::empty();
    
    for (node_id, pos) in positions {
        let size = metrics[node_id].size;
        bbox.expand(pos.center - size / 2.0);
        bbox.expand(pos.center + size / 2.0);
    }
    
    for edge in edges {
        for point in &edge.route_points {
            bbox.expand(*point);
        }
    }
    
    let width  = bbox.width() + 2.0 * MARGIN;
    let height = bbox.height() + 2.0 * MARGIN;
    (width, height)
}
```

### 10.2 用户指定尺寸时的适配

如果用户通过 `render(text, w, h)` 指定了尺寸：
- 计算缩放因子 `scale = min(w / natural_w, h / natural_h)`
- 对所有 Element 坐标应用缩放和平移，使其在画布中居中

```rust
fn fit_to_canvas(positions, metrics, edges, canvas_w, canvas_h) {
    let (natural_w, natural_h) = compute_natural_size(positions, metrics, edges);
    let scale = min(canvas_w / natural_w, canvas_h / natural_h);
    let offset_x = (canvas_w - natural_w * scale) / 2.0;
    let offset_y = (canvas_h - natural_h * scale) / 2.0;
    
    for pos in positions.values_mut() {
        pos.center.x = pos.center.x * scale + offset_x;
        pos.center.y = pos.center.y * scale + offset_y;
    }
}
```

---

## 11. Pass 7 — 边路由

### 11.1 路由策略

边的路径根据起点和终点的相对位置分情况处理：

| 情况 | 策略 | 示例 |
|------|------|------|
| 同一层，相邻 | 直线连接 | A → B |
| 相邻层，同一 x | 标准直角 (└┐) | A → B (下一层) |
| 相邻层，不同 x | 先垂直再水平再垂直 | A → B (侧移) |
| 跨越多层 | 沿 dummy 节点逐层路由 | A → ... → D |
| 反向边（循环） | 从侧面绕行 | C → B (loop back) |
| 自环 | 环形路径 | A → A |

### 11.2 边的 Anchor 点

每条边从源节点的某个 anchor 出发，到达目标节点的某个 anchor：

```rust
fn choose_anchors(from: NodePosition, to: NodePosition, direction: Direction) 
    -> (Anchor, Anchor) 
{
    // TD 方向：从 bottom 出，从 top 入
    // LR 方向：从 right 出，从 left 入
    // 反向边：从 bottom 出，从 bottom 入（侧面绕行）
}
```

### 11.3 边标签放置

边标签放在边路径的中点附近，沿着路径方向偏移：

```rust
fn place_edge_label(route: &[Point], label: &str) -> (Point, f64) {
    let mid_idx = route.len() / 2;
    let mid = route[mid_idx];
    let angle = calculate_segment_angle(route, mid_idx);
    (mid, angle)
}
```

### 11.4 箭头绘制

在边的末端（目标节点 anchor 处）绘制箭头：

```rust
fn draw_arrowhead(target: Point, direction: Vec2, arrow_type: ArrowType) -> VisualElement {
    // 根据箭头类型绘制三角形/菱形等
}
```

---

## 12. 子图 (Subgraph) 布局

### 12.1 概述

子图在 AST 中已解析 (`Subgraph { title, nodes, edges }`)，但当前未在布局中使用。子图需要：
- 独立的内部布局（递归调用完整布局管线）
- 外部由虚线框包围
- 标题放在框的左上角
- 与外部节点的连接需要穿过子图边界

### 12.2 处理方法

1. 将子图视为一个特殊的 `LogicalGroup`
2. 子图内的节点先独立布局
3. 子图作为一个整体参与外层布局
4. 子图与外部节点的边需要计算穿过子图边界的交叉点

```rust
struct SubgraphLayout {
    /// 子图标题
    title: Option<String>,
    /// 子图边界矩形
    bounds: Rect,
    /// 子图内部节点布局
    internal_positions: HashMap<NodeId, NodePosition>,
}

fn layout_subgraph(subgraph: &Subgraph, config: &OutputConfig) -> SubgraphLayout {
    // 1. 解析子图内部结构
    let group = recognize_structure(subgraph);
    
    // 2. 递归布局子图内部
    let internal = layout_group(&group, config);
    
    // 3. 计算子图边界（内部节点 + padding）
    let bounds = compute_subgraph_bounds(&internal, &subgraph.title);
    
    SubgraphLayout { title: subgraph.title.clone(), bounds, internal_positions: internal }
}
```

---

## 13. 数据结构设计

### 13.1 核心数据结构汇总

```rust
// ===== Pass 1 输出 =====
struct LayoutTree {
    root: LogicalGroup,
    orphan_edges: Vec<GroupEdge>,  // Group 之间的边
}

enum LogicalGroup {
    Chain   { items: Vec<ChainItem> },
    Branch  { source: NodeId, arms: Vec<BranchArm>, sink: Option<NodeId> },
    Cycle   { condition: NodeId, body: Box<LogicalGroup>, exit: Option<NodeId> },
    Leaf    { node_id: NodeId },
    Subgraph { title: Option<String>, body: Box<LogicalGroup> },
}

struct ChainItem {
    node_id: NodeId,
    label: Option<String>,  // 进入该节点的边标签
}

struct BranchArm {
    label: Option<String>,
    body: Box<LogicalGroup>,
}

struct GroupEdge {
    from: NodeId,
    to: NodeId,
    edge: Edge,       // 原始边数据
    from_group: GroupId,
    to_group: GroupId,
}

// ===== Pass 2 输出 =====
struct LayoutMetrics {
    node_metrics: HashMap<NodeId, NodeMetrics>,
    group_metrics: HashMap<GroupId, GroupMetrics>,
    natural_size: Size,    // 自然画布尺寸
}

// ===== Pass 5 输出 =====
struct LayoutResult {
    node_positions: HashMap<NodeId, NodePosition>,
    group_bounds: HashMap<GroupId, Rect>,
    subgraph_bounds: HashMap<String, Rect>,
    routed_edges: Vec<RoutedEdge>,
    canvas_size: Size,
}

struct NodePosition {
    center: Point,
    anchors: NodeAnchors,
}

struct RoutedEdge {
    edge: Edge,
    route: Vec<Point>,      // 路由路径点
    label_position: Option<(Point, f64)>,  // (位置, 旋转角度)
    arrowhead: Option<VisualElement>,
}

// ===== 全局上下文 =====
struct LayoutContext {
    direction: Direction,
    config: OutputConfig,
    text_styles: HashMap<NodeId, TextStyle>,
    node_metrics: HashMap<NodeId, NodeMetrics>,
    
    // 方向相关计算
    main_axis: fn(Point) -> f64,
    cross_axis: fn(Point) -> f64,
    set_main: fn(&mut Point, f64),
    set_cross: fn(&mut Point, f64),
}
```

### 13.2 方向抽象实现

```rust
impl LayoutContext {
    fn new(direction: Direction, config: OutputConfig) -> Self {
        let (main_axis, cross_axis, set_main, set_cross) = match direction {
            Direction::TD | Direction::TB => (
                |p: Point| p.y as fn(Point) -> f64,
                |p: Point| p.x,
                |p: &mut Point, v: f64| { p.y = v; },
                |p: &mut Point, v: f64| { p.x = v; },
            ),
            Direction::LR => (
                |p: Point| p.x as fn(Point) -> f64,
                |p: Point| p.y,
                |p: &mut Point, v: f64| { p.x = v; },
                |p: &mut Point, v: f64| { p.y = v; },
            ),
            // RL, BT 类似，带符号反转
        };
        LayoutContext { direction, config, main_axis, cross_axis, set_main, set_cross, ... }
    }
}
```

---

## 14. 实施路线图

### 阶段 0：基础设施（当前已完成）
- [x] Pest 语法解析器
- [x] AST 定义（Node, Edge, Subgraph）
- [x] SVG 渲染管线
- [x] 基本文本布局和字体测量

### 阶段 1：尺寸测量（Pass 2 独立实现）
- [ ] 实现 `measure_node()` 函数
- [ ] 根据节点形状调整最小尺寸和 padding
- [ ] 重构 `draw_node()` 使用动态尺寸而非硬编码 NODE_WIDTH/NODE_HEIGHT

### 阶段 2：结构识别（Pass 1）
- [ ] 定义 `LogicalGroup` enum 和相关类型
- [ ] 实现 `recognize_structure()` 函数
- [ ] 实现 Chain/Branch/Cycle 三种核心结构的识别
- [ ] 编写结构识别测试用例

### 阶段 3：层级分配重写（Pass 3）
- [ ] 在 `LogicalGroup` 基础上实现分组层级分配
- [ ] 支持 dummy 节点插入（处理长边）
- [ ] 方向抽象（主轴/交叉轴）

### 阶段 4：几何定位重写（Pass 5）
- [ ] 实现 `layout_chain/branch/cycle/leaf` 函数
- [ ] 递归布局 Group Tree
- [ ] 实现 `LayoutCoord` 和方向转换

### 阶段 5：边路由重写（Pass 7）
- [ ] Anchor 选择逻辑
- [ ] 反向边（循环）智能路由
- [ ] 边标签放置
- [ ] 箭头绘制

### 阶段 6：画布适配（Pass 6）
- [ ] Bounding box 计算
- [ ] 自动画布尺寸
- [ ] 用户指定尺寸时的缩放适配

### 阶段 7：交叉最小化（Pass 4）
- [ ] Barycenter 启发式算法
- [ ] 最多 10 轮迭代

### 阶段 8：子图支持
- [ ] 在 LayoutTree 中支持 Subgraph 类型
- [ ] 子图边界绘制
- [ ] 跨子图边路由

### 阶段 9：BT/RL 方向支持
- [ ] 完善方向抽象
- [ ] BT = TD 的 y 轴反转
- [ ] RL = LR 的 x 轴反转

---

## 附录 A：现有代码与新设计的映射

| 旧函数 | 新位置 | 变化 |
|--------|--------|------|
| `build_graph()` | Pass 1 前置 | 保持不变，作为图分析输入 |
| `assign_layers()` | Pass 3 | 重写为 Group 感知的层级分配 |
| `compute_positions()` | Pass 5 | 重写为 Group 驱动的递归定位 |
| `draw_node()` | 渲染层 | 修改为使用动态 NodeMetrics |
| `draw_edge()` | Pass 7 | 重写为智能路由 |
| `build_flowchart_elements()` | 顶层 orchestrator | 顺序调用 7 个 Pass |

## 附录 B：参考

- **Sugiyama 算法**: K. Sugiyama et al., "Methods for Visual Understanding of Hierarchical System Structures", IEEE SMC, 1981
- **Mermaid Flowchart 文档**: https://mermaid.js.org/syntax/flowchart.html
- **Graphviz dot 布局**: E. Gansner et al., "A Technique for Drawing Directed Graphs", IEEE TSE, 1993
- **dagre**: JavaScript 实现的 Sugiyama 布局库，Mermaid 内部使用

---

## 附录 C：实战调整复盘与算法深化

### C.1 重构过程中遇到的布局问题

在将 layout 系统从设计文档落实到代码的过程中，我们遇到了 4 类典型问题：

| # | 问题 | 症状 | 根因 |
|---|------|------|------|
| 1 | **循环体位错** | C[Process] 在 B{Continue} 下方而非侧面 | `position_cycle_vertical` 用 `body.height/2` 算 y，未与 condition 中心对齐 |
| 2 | **回边方向反直觉** | C→B 走 bottom→right→up 绕行 | 早期实现"所有回边从底部绕右侧"，不符合"顶部回环"的惯常画法 |
| 3 | **同层异高未对齐** | 菱形 B 与矩形 C 不在同一水平线 | 各自用 `start_main + height/2`，不同形状乘子导致中心 y 不同 |
| 4 | **分支节点位置丢失** | Branch2 等节点不出现在 SVG 中 | `measure_groups` 用独立计数器分配 GroupId，与 `position_group` 的计数器不同步 |

### C.2 每个问题背后的算法原理

#### 问题 1 & 3：相对定位 vs 绝对定位

**本质**：我们在做"绝对定位"（每个节点自己算坐标），而应该做"相对定位"（定义节点间的空间关系）。

当前代码：
```rust
// 每个节点独立计算中心 y
let cond_center_y = start_main + cond_size.height / 2.0;
let body_center_y = start_main + body_size.height / 2.0;  // 不同！
```

应改为约束式：
```rust
// 定义约束：body.center.y == condition.center.y
let body_center_y = cond_center_y;
```

**对应算法**：**Constraint-based Layout**（如 Cassowary 算法、Apple AutoLayout）。其核心思想是：声明变量之间的线性约束关系，让求解器计算满足所有约束的赋值。

在我们的场景中，需要的约束类型非常有限：

| 约束类型 | 表达 | 示例 |
|----------|------|------|
| 对齐 | `A.main == B.main` | C 和 B 同 y |
| 顺序 | `A.main < B.main` | A 在 B 上方 |
| 间距 | `B.main - A.main >= V_GAP` | 层间有最小间距 |
| 居中 | `A.cross == canvas_center` | 主节点水平居中 |
| 侧置 | `A.cross < B.cross` | C 在 B 左侧 |

对于流程图这种**网格化明显**的布局问题，不需要完整的 Cassowary 求解器——可以先把节点分配到"逻辑网格"（层 × 列），每个格子选一个代表性中心坐标，所有节点对齐到格线。这就是下面要讨论的 **Grid-based Layout**。

#### 问题 2：边路由的策略模式

**本质**：边的路径取决于三个因素——源位置、目标位置、边的语义（正向/反向/循环）。

当前代码用 if/else 判断：
```rust
if is_horizontal { route_horizontal() }
else if is_back { route_back_edge() }
else { route_vertical() }
```

更好的模型是一个 **Anchor 选择矩阵** + **路径策略表**：

**Anchor 选择矩阵**（TD 方向）：

| 相对位置 | 源 Anchor | 目标 Anchor | 路径策略 |
|----------|-----------|-------------|----------|
| 源在上方 | bottom | top | └┐ 型直角 |
| 源在下方 | top | top | 顶部绕行（回边） |
| 同一水平 | right/left | left/right | 水平直连 |
| 自环 | right | top | 环形路径 |

**对应算法**：**Channel-based Edge Routing**（通道式路由）。将布局空间划分为水平和垂直通道，每条边分配到一个通道，简化路径计算。

```
       ┌ Channel 0 (最左)
       │  ┌ Channel 1
       │  │  ┌ Channel 2 (主干道)
       │  │  │  ┌ Channel 3 (最右)
       ▼  ▼  ▼  ▼
     ═══╤═══╤═══╤═══  ── Row 0
       │   │   │
     ═══╪═══╪═══╪═══  ── Row 1
       │   │ C │ B │
     ═══╪═══╪═══╪═══  ── Row 2
       │   │   │ D │
```

在我们的 scale 下不需要完整的通道分配算法，但 **Anchor 矩阵** 这个思想可以直接应用——它为每种相对位置关系给出明确的锚点选择，消除 ad-hoc 判断。

#### 问题 4：ID 同步

**本质**：两个子系统（测量、定位）各自维护独立的遍历顺序，导致索引错位。

**修正**：让 `measure_groups` 和 `position_group` 共享同一个 GroupId 生成器，或改为在定位阶段不依赖 GroupId，直接根据 Group 结构推导尺寸。

### C.3 人们画流程图的惯常模式

观察手绘流程图和 Mermaid 官方文档的输出，可以总结出以下**约定俗成的布局规则**：

| 规则 | 说明 |
|------|------|
| **主线下行** | 核心流程沿主轴（TD 时向下，LR 时向右）一字排开 |
| **条件分叉** | 从条件节点出来后，一个分支继续主线，其他分支走向侧面 |
| **循环侧置** | 循环体放在条件节点的侧面（非主轴方向），与条件对齐 |
| **回边绕圈** | 循环体回到条件的边绕到上方/侧方，与进入条件的线融合 |
| **汇合归位** | 多个分支汇合后回到主轴的同一位置 |
| **中心对齐** | 不同高度的节点以中心对齐，而非顶/底对齐 |
| **等距分布** | 同层节点均匀分布，间距一致 |

这些规则可以形式化为约束：

```
Rule: MainlineDown
  → for each (a, b) in mainline_successors: b.main > a.main, b.cross == a.cross == CENTER

Rule: LoopSidePlace
  → loop_body.cross == LEFT_ZONE, loop_body.main == condition.main

Rule: BackEdgeUpperRoute
  → back_edge exits from loop_body.top, loops over, enters condition.top

Rule: CenterAlign
  → for each layer: all nodes in layer share same main coordinate
  → vertical offset = max_height / 2 + node_height / 2 (for top-left positioning)
```

### C.4 推荐的算法引入路线

综合考虑图规模（< 50 节点）、实现成本和收益，建议分三步演进：

#### 第一步：Anchor 矩阵 + 固定网格（当前 → 0.3.0）

**改动**：引入 anchor 选择矩阵，用固定网格规划节点位置。

```rust
struct LayoutGrid {
    /// 每行的 main 坐标
    row_mains: Vec<f64>,
    /// 每列的 cross 坐标  
    col_crosses: Vec<f64>,
}

impl LayoutGrid {
    fn place_node(&self, row: usize, col: usize, size: Size) -> NodePosition {
        let center = Point::new(self.col_crosses[col], self.row_mains[row]);
        NodePosition { center, anchors: NodeAnchors::new((size.width, size.height)) }
    }
}
```

**路线**：
- Chain 节点 → 每行一个，col = CENTER_COL
- Branch 源 → col = CENTER_COL，arms → 各自 col，sink → CENTER_COL
- Cycle condition → col = CENTER_COL，body → col = LEFT_COL

**收益**：消除 ID 同步问题，统一节点对齐逻辑。

#### 第二步：约束式布局（0.4.0）

**改动**：将布局规则表达为约束，用简单求解器求值。

```rust
enum Constraint {
    AlignMain(NodeId, NodeId),          // a.main == b.main
    AlignCross(NodeId, NodeId),         // a.cross == b.cross
    OrderMain(NodeId, NodeId, f64),     // b.main - a.main >= gap
    CenterCross(NodeId, f64),           // a.cross == center
    AnchorTo(NodeId, AnchorDir, NodeId, AnchorDir),  // a.anchor == b.anchor
}
```

**收益**：所有硬编码的 `position_chain_vertical`、`position_cycle_vertical` 等函数可统一为一个约束求解过程。

#### 第三步：完整 Sugiyama（0.5.0，需要时）

**时机**：当流程图规模增大（> 30 节点）或出现大量跨层长边时。

**改动**：引入 petgraph，实现完整的 Sugiyama 四阶段：
1. 层级分配（topological sort + longest path）
2. 层级内排序（barycenter 迭代，减少交叉）
3. 坐标分配（水平间距优化）
4. 边路由（spline/polyline）

### C.5 推荐立即实施：Anchor 矩阵

这是投资回报率最高的改进。修改 `edges.rs`，将 ad-hoc 的 if/else 替换为：

```rust
fn choose_route(from: &NodePosition, to: &NodePosition, is_back: bool, is_horizontal: bool) 
    -> Vec<Point> 
{
    let (src_anchor, tgt_anchor) = match (is_horizontal, is_back) {
        (true, _)      => (AnchorDir::Right, AnchorDir::Left),
        (false, true)  => (AnchorDir::Top, AnchorDir::Top),     // 回边：顶对顶
        (false, false) => {
            if from.center.y < to.center.y {
                (AnchorDir::Bottom, AnchorDir::Top)              // 正向：底对顶
            } else {
                (AnchorDir::Right, AnchorDir::Left)              // 向上或同层：水平连接
            }
        }
    };
    // 根据 anchor 对 + 位置关系，查表选择路径形状
    route_by_anchors(from, to, src_anchor, tgt_anchor)
}
```

同时合并 `route_vertical`、`route_horizontal`、`route_same_level`、`route_back_edge` 为一个统一的 `route_between_anchors` 函数，消除重复代码。
