# 架构说明（0.3）

本文描述当前实现的数据流与各层职责。历史方案见 [`archive/`](archive/)。

## 总览

```text
Mermaid 文本
  → parser::WinnowParser          AST（ast::Diagram）
  → builder::extract              Unigraph（UG，拓扑 + 语义）
  → builder::measure              UG'（文本测量、尺寸回填）
  → builder::layout::engine       Geograph（GG，几何坐标 + 路由）
  → builder::materialize          SceneGraph（SG，颜色 / 线型 / 字体）
  → builder::paint                lievisual::Scene
  → lib.rs::scene_to_svg/png      SVG / vello_cpu PNG
```

各层单向依赖：后一层不读取 AST，`paint` 不做图类型分支；颜色与线型只在
`materialize` 查 `theme`。

## 解析层

- 入口：`parser::WinnowParser::parse_mermaid`，按首行关键字分派 8 种图表。
- 手写 winnow 组合子，按行解析；未知语句通过 `skip_line` 跳过（保持宽容）。
- 标签：节点形状内、`|...|`、裸标签 `-- text -->` 均按闭合定界符读取，
  允许空格与标点；引号只在标签开头作为包裹引号，因此 `A[Don't stop]` 正常。
- 重复声明：同一节点多次声明以**最后一次**为准（`extract` 阶段覆盖）。
- 子图：AST 保留嵌套层级（`Subgraph.subgraphs`）与 `direction`；
  子图内边、嵌套子图边都会递归收集。
- 入口处会剥离 UTF-8 BOM。

## IR 分层

| IR | 位置 | 内容 |
|---|---|---|
| AST | `ast` | 语法树，8 种图各自的节点/边/语句结构 |
| UG | `builder::ir::unigraph` | 统一拓扑：节点、边、端口、容器成员、语义分类 |
| GG | `builder::ir::geograph` | 几何：节点中心/尺寸、边路由段、容器包围盒 |
| SG | `builder::ir::scenegraph` | 视觉自足：形状几何、填充/描边、文本布局 |

`StyleIntent` 为后续可配置主题预留；当前 `materialize` 仍直接读取 `theme` 常量。

## 布局

- **Directed**（flowchart / state）：Sugiyama 分层 + barycenter 降交叉 +
  按方向分配主轴/同层轴坐标，再做正交/样条路由与容器避让。
- **Grid**（class / er）：BFS 网格分层，层内保持源码序。
- **Linear**（timeline）：固定列距沿主轴排布。
- **Radial**（pie）：节点不参与定位，扇区由数据计算。
- **Sequence**：参与者列 + 语句行序（消息/备注/分组块/激活条）。
- **Hierarchy**（gitgraph）：按分支/提交层级布局。

### 子图方向

`subgraph X` 内的 `direction` 会在全局布局后做一次**局部重排**：

1. 按成员数升序处理，内层先布局；
2. 每个容器把「直接节点 + 直接子容器」当作布局单元，用请求方向重新分层；
3. 子容器作为整体块平移，因此嵌套子图各自的方向都保留；
4. 重排结果平移回原子图包围盒左上角，最后重新计算容器框并路由边。

## 画布与输出

- `render`/`render_png` 的 `width`/`height` 是**上限/目标**：SVG 只缩小不放大，
  PNG（`upscale = true`）放大到目标尺寸提升分辨率。
- SVG 根节点只写 `viewBox`，视口交给 CSS（`max-width` 封顶）。
- 背景色由 `OutputConfig::background` 写入 `Scene`，两种后端语义一致；
  默认不透明白，设为 `Color::TRANSPARENT` 得到透明底。
- PNG 像素数上限 8192×8192，超出返回错误。

## 扩展指南

- 新增图类型：`ast` → `parser` → `extract` → `materialize` 分派 →
  在 `official_compare_test`/golden 增加用例。
- 新增节点形状：`ast::NodeShape` → `extract::flowchart::map_shape` →
  `ShapeKind` → `measure::shape_padding` → `materialize::shape_to_geometry`。
- 新增语法语句：优先在对应 parser 的循环中加分支，未知语句保持跳过；
  若会改变语义，务必补 `tests/syntax_coverage_test.rs` 回归用例。
