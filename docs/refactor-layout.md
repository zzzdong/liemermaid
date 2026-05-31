# 布局系统设计文档：支持多图表类型的 Rust 实现

## 1. 设计目标

构建一个与画布/渲染器无关的布局系统，能够将 Mermaid AST 转换为包含**逻辑几何信息**的中间表示（Layout IR），供后续渲染器（SVG、Canvas、文本等）消费。系统需支持以下图表类型：

- 流程图（Flowchart）
- 时序图（Sequence Diagram）
- 类图（Class Diagram）
- 状态图（State Diagram）
- ER 图（Entity Relationship Diagram）

**核心原则**：
1. **算法与几何分离**：布局算法仅依赖图论和几何计算，不涉及具体输出格式。
2. **模块化**：每种图表类型有独立的布局引擎，可插拔。
3. **可扩展**：容易添加新图表类型或替换布局策略。
4. **可测试**：布局结果可序列化，便于单元测试和可视化验证。

---

## 2. 总体架构

```
┌─────────────────┐
│   AST (crate::ast) │
└────────┬────────┘
         │
         ▼
┌────────────────────────────────────────────┐
│              Layout Engine                  │
│  ┌────────────┐ ┌────────────┐ ┌─────────┐ │
│  │ Flowchart  │ │ Sequence   │ │ Class   │ │
│  │ Layout     │ │ Layout     │ │ Layout  │ │
│  └────────────┘ └────────────┘ └─────────┘ │
│  ┌────────────┐ ┌────────────┐             │
│  │ State      │ │ ER         │ ...         │
│  │ Layout     │ │ Layout     │             │
│  └────────────┘ └────────────┘             │
└────────────────────────────────────────────┘
         │
         ▼
┌─────────────────┐
│   Layout IR     │  (与画布无关的几何数据)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Renderer      │  (SVG / Canvas / Text)
└─────────────────┘
```

**组件说明**：
- `LayoutEngine` trait：定义统一布局接口。
- 具体布局实现：每个图表类型一个结构体，实现 `LayoutEngine`。
- `Layout`：布局结果，包含节点几何、边路径、总尺寸。
- `TextMeasurer`：抽象文本测量，允许不同实现（估算、rusttype、WASM 桥接）。

---

## 3. 核心数据结构

### 3.1 几何基础类型

```rust
use euclid::{Point2D, Size2D, Rect, Vector2D};

pub type Point = Point2D<f32, LayoutUnit>;
pub type Size = Size2D<f32, LayoutUnit>;
pub type Rect = euclid::Rect<f32, LayoutUnit>;

pub enum LayoutUnit {}  // 标记单位
```

### 3.2 布局中间表示（IR）

```rust
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: String,              // 对应 AST 中的唯一标识
    pub bounds: Rect,            // 相对原点(0,0)的绝对位置和尺寸
    pub ports: Vec<Point>,       // 预定义的连接点（上、下、左、右中）
    pub label: Option<String>,
    pub shape: NodeShape,        // 枚举（矩形、圆角、菱形等）
    pub style: NodeStyle,        // 可选样式（颜色、字体）
}

#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: String,
    pub to: String,
    pub path: Vec<Point>,        // 控制点序列，至少两个点
    pub arrow_at_end: bool,
    pub label: Option<String>,
    pub label_position: Option<Point>,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub size: Size,               // 整个画布的总尺寸
    pub metadata: LayoutMetadata, // 方向、缩放因子等
}

#[derive(Debug, Clone)]
pub struct LayoutMetadata {
    pub direction: Direction,    // TB, LR, etc.
    pub unit_scale: f32,         // 1 逻辑单位 = 多少毫米（可选）
}
```

### 3.3 布局引擎 Trait

```rust
pub trait LayoutEngine {
    fn layout(&self, diagram: &Diagram, measurer: &dyn TextMeasurer) -> Result<Layout, LayoutError>;
}

pub trait TextMeasurer {
    fn measure(&self, text: &str, font_size: f32) -> Size;
}
```

### 3.4 配置参数

```rust
#[derive(Debug, Clone, Copy)]
pub struct LayoutConfig {
    pub node_gap: f32,          // 同层节点之间的水平间距
    pub layer_gap: f32,         // 层级之间的垂直间距
    pub font_size: f32,
    pub padding: f32,           // 节点内边距
    pub arrow_size: f32,        // 箭头尺寸
}
```

---

## 4. 各图表类型的布局算法

### 4.1 流程图与状态图（有向分层图）

**算法**：Sugiyama 风格的分层布局（Longest Path + Barycenter）。

**步骤**：
1. **构建图**：使用 `petgraph::DiGraph`，节点 = 流程图节点或状态，边 = 箭头。
2. **节点尺寸测量**：调用 `TextMeasurer` 获取每个节点（含形状内边距）的宽高。
3. **层级分配**：
   - 采用最长路径法（Longest Path）为每个节点分配层级（layer）。
   - 对于分支节点，强制各分支上的对应节点同层（通过插入虚拟节点或调整边长）。
4. **同级排序**：
   - 每层内使用重心法（Barycenter）排序，最小化边交叉。
   - 保留用户定义的顺序（如边出现顺序）作为初始权重。
5. **坐标分配**：
   - 若方向为 TB：`y = layer * (avg_node_height + layer_gap)`；`x` 根据排序索引和节点宽度线性分配。
   - 若方向为 LR：交换 x 与 y 角色。
6. **边路由**：
   - 默认使用直线连接源节点与目标节点的最近端口。
   - 可选正交路由：使用 A* 在网格上搜索路径（用于复杂图）。
7. **子图处理**：
   - 递归处理 `subgraph`：内部先布局，得到内部尺寸；外部图中子图作为单个节点参与布局；最终平移子图内部坐标。

**关键代码片段**：

```rust
impl LayoutEngine for FlowchartLayout {
    fn layout(&self, diagram: &Diagram, measurer: &dyn TextMeasurer) -> Result<Layout, LayoutError> {
        let flowchart = match diagram { Diagram::Flowchart(f) => f, _ => return Err(...) };
        // 构建 graph...
        let layers = assign_layers(&graph);
        let ordered_layers = order_layers(&graph, &layers);
        let positions = assign_coordinates(&ordered_layers, &sizes, self.config);
        // 构建 Layout...
    }
}
```

### 4.2 时序图（线性时轴布局）

**算法**：基于时间线的垂直布局。

**步骤**：
1. **收集参与者**：遍历所有 `participant` 和 `actor`，分配列索引，计算每列宽度（取最大文本宽度+内边距）。
2. **计算总高度**：按顺序遍历消息、激活条、注释，累加高度。
3. **逐元素定位**：
   - 消息：Y 坐标递增固定高度；水平坐标根据 from/to 参与者列计算（水平线）。
   - 激活条：维护每个参与者的激活栈，绘制矩形（起点消息 Y，终点消息 Y）。
   - 注释：根据 placement 计算跨列区域，文本居中。
4. **边处理**：消息本身即为边，直线连接两列。

### 4.3 类图（继承层次 + 力导向）

**策略**：继承关系用分层布局，其他关系（关联、聚合）用力导向微调。

**步骤**：
1. 提取继承边（`RelationKind::Inheritance`），构建有向图（父→子）。
2. 使用分层布局得到初步位置（父在上，子在下）。
3. 对于非继承关系，构建无向图，应用力导向算法（如 spring 模型）在水平方向调整节点位置，保持垂直层次大致不变。
4. 类内部成员布局：将成员划分为属性区和方法区，计算类矩形尺寸。

### 4.4 ER 图（实体关系布局）

**算法**：分层布局 + 手动调整边标签位置。

- 将实体视为节点，关系视为边（方向可忽略）。
- 使用与流程图相同的分层布局算法。
- 特殊处理：关系上的基数标记（`||--o{`）绘制在边中央，需要计算路径中点。

### 4.5 状态图（复用流程图布局）

状态图与流程图几乎一致，可直接复用 `FlowchartLayout`，只需调整节点形状（状态通常是圆角矩形，起始/终止为圆形）。

---

## 5. 文本测量抽象

由于 Rust 原生无文本渲染，提供多种 `TextMeasurer` 实现：

### 5.1 估算实现（用于快速原型）

```rust
pub struct EstimatorTextMeasurer {
    char_width: f32,
    line_height: f32,
}

impl TextMeasurer for EstimatorTextMeasurer {
    fn measure(&self, text: &str, font_size: f32) -> Size {
        let width = text.chars().count() as f32 * self.char_width * font_size;
        let height = self.line_height * font_size;
        Size::new(width, height)
    }
}
```

### 5.2 精确实现（使用 rusttype 加载字体）

```rust
pub struct FontTextMeasurer {
    font: rusttype::Font<'static>,
    scale: rusttype::Scale,
}

impl TextMeasurer for FontTextMeasurer {
    fn measure(&self, text: &str, font_size: f32) -> Size {
        // 使用 font.glyph 计算精确 advance
    }
}
```

### 5.3 桥接实现（WASM 环境调用浏览器）

```rust
#[cfg(target_arch = "wasm32")]
pub struct JsTextMeasurer;

impl TextMeasurer for JsTextMeasurer {
    fn measure(&self, text: &str, font_size: f32) -> Size {
        // 调用 JavaScript 函数返回宽度、高度
    }
}
```

布局引擎应接受 `&dyn TextMeasurer` 参数，便于注入不同实现。

---

## 6. 扩展性设计

### 6.1 添加新图表类型

1. 在 AST 中添加新变体（例如 `Diagram::Gantt(Gantt)`）。
2. 实现 `LayoutEngine` for `GanttLayout`。
3. 在布局分发函数中增加分支。

### 6.2 替换布局算法

- 对同一图表类型，可提供多种算法（例如流程图支持 Dagre / ELK）。通过配置选择。
- 使用策略模式：`FlowchartLayout` 内部包含一个 `Box<dyn HierarchicalLayout>`。

### 6.3 可配置参数

所有布局参数（间距、字体大小、是否启用正交路由等）应集中在 `LayoutConfig` 中，并支持从外部文件或命令行加载。

---

## 7. 错误处理与调试

- 定义 `LayoutError` 枚举：`GraphCycle`（图存在环）、`TextMeasureFailed`、`UnsupportedShape` 等。
- 对于有环图，可以破环（例如反转边或删除）并发出警告。
- 提供调试模式：输出中间步骤的层级和排序信息，可绘制为 SVG 方便人工检查。

---

## 8. 与渲染器集成

布局完成后，渲染器只需读取 `Layout` 数据即可绘制，无需任何计算。例如 SVG 渲染器：

```rust
fn render_to_svg(layout: &Layout) -> String {
    let mut svg = String::new();
    svg.push_str(&format!("<svg width='{}' height='{}'>", layout.size.width, layout.size.height));
    for node in &layout.nodes {
        // 根据 shape 绘制 rect 或路径
    }
    for edge in &layout.edges {
        // 根据 path 绘制 polyline，添加箭头
    }
    svg.push_str("</svg>");
    svg
}
```

---

## 9. 测试策略

### 9.1 单元测试

- 测试文本测量：给定字符串，断言尺寸正确。
- 测试层级分配：构造小图，验证分支节点同层。
- 测试坐标分配：断言节点位置符合期望的网格。

### 9.2 集成测试

- 输入完整的 Mermaid 文本，输出布局，人工检查或与参考实现对比。
- 使用 `insta` 进行快照测试，序列化 `Layout` 为 JSON，确保变化被跟踪。

### 9.3 可视化测试

- 在测试模块中生成 SVG 文件并保存到 `target/layouts/`，开发者可打开查看。

---

## 10. 性能考量

- 对于节点数 < 500 的图，分层布局 O(V^2) 可接受。
- 力导向布局需要迭代 100-300 次，大规模图（>200 节点）需启用快速收敛策略。
- 文本测量结果可缓存（`HashMap<String, Size>`），避免重复测量同一文本。

---

## 11. 示例：流程图布局的完整流程

```rust
use mermaid_parser::MermaidParser;
use layout::{FlowchartLayout, LayoutConfig, EstimatorTextMeasurer};

fn main() {
    let input = "flowchart TD; A[Start] --> B{Decision}; B -->|Yes| C[OK]; B -->|No| D[Cancel];";
    let ast = MermaidParser::parse(input).unwrap();
    let config = LayoutConfig::default();
    let measurer = EstimatorTextMeasurer::new(8.0, 16.0);
    let engine = FlowchartLayout::new(config, Direction::TD);
    let layout = engine.layout(&ast, &measurer).unwrap();
    // 输出 SVG...
}
```

---

## 12. 总结

本文档定义了一个**完整、可实施**的布局系统设计方案，核心包括：

- 与画布无关的中间表示 `Layout`。
- 基于 `petgraph` 的分层布局算法（流程图、状态图、类图继承层次）。
- 针对时序图的线性布局。
- 可插拔的文本测量抽象。
- 错误处理、测试和调试策略。

开发者可以按照此文档逐步实现，最终得到一个健壮、可扩展的 Mermaid 图表布局引擎。