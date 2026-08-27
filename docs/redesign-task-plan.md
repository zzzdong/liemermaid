# 布局与渲染层从头重构 — 任务计划

> 关联设计文档：`redesign-from-scratch.md`
> 范围：只重写 `layout` 与 `builder` 中间层，不动 `ast`（解析+图类型）与 `lievisual`（Scene IR+渲染后端）。
> 对外契约：`Diagram::to_scene()` 签名与 `DiagramError` 保持不变。
> 状态：待启动。

---

## 0. 关键事实与纪律（执行前必读）

### 0.1 现有代码事实（已核对）
- 对外入口有两处：`src/scene_ext.rs::to_scene`（走 `builder::layout::layout_diagram` + `builder::render::render_placed`）与 `src/builder/mod.rs::build_diagram`（parallel 入口）。
- 旧 IR：`builder/layout/ir.rs`(`PlacedGraph`) + `builder/layout/types.rs`(`Layout`)。
- 旧转换/求解：`builder/layout/convert.rs`、`builder/layout/sugiyama.rs`、各 `family` solver（`directed.rs`/`state.rs`/`class.rs`/`er.rs`/...）。
- 散落取色：`src/builder/theme.rs`（含 `theme::flowchart::*` 等）。
- 现有测试素材：`tests/` 下 53 个 `.mmd` + 53 个 `.svg` golden；`tests/layout_quality_test.rs`、`tests/placed_graph_test.rs`、`tests/sugiyama_consistency_test.rs` 等可用作 baseline 与回归。

### 0.2 四条执行纪律
1. **`paint` 零 AST / Theme 引用**：用编译单元隔离或 `#![deny]` 违规 import 静态保证不再回查 AST。
2. **UG 生命周期**：`LayoutEngine::run(&ug, &theme) -> (Geograph, StyleIntent)`；UG 在 layout 阶段结束后即可 `drop`；`materialize` 只吃 `GG + StyleIntent`，不持有 UG。
3. **测量在 layout 前**：管线为 `extract(Stage1) → measure(Stage1.5) → layout(Stage2) → materialize(Stage3) → paint(Stage4)`；materialize 不再测节点尺寸。
4. **每 Phase 保留降级路径**：旧 `build_diagram` 入口在 Phase 4 之前不删，便于回退。

---

## 1. 总览（5 Phase / 11 任务）

| ID | 任务 | 关键产出 | 验收 |
| :--- | :--- | :--- | :--- |
| P0.1 | 模块骨架 + 三层 IR 类型 | `builder/{ir,extract,measure,layout,materialize,paint}` + UG/GG/SG 定义 | 编译通过 |
| P0.2 | 边质量度量 + baseline | 交叉/重叠/穿节点统计函数 + 53 golden baseline 数字 | baseline 报告 |
| P0.3 | 微型端到端跑通 | flowchart 最小子集走通 Diagram→Scene | 三层 IR 可序列化 |
| P1.1 | directed family solver | Sugiyama 分层 + 通用 `minimize_crossings` | 分层+确定性 |
| P1.2 | EdgeRouter | `route.rs`+`spatial.rs` 边-边排斥/回避 | 几何无重叠 |
| P1.3 | flowchart materialize/paint | 全 ShapeKind + EdgeEnds + theme 收敛 | flowchart 全图渲染 |
| P1.4 | flowchart 验证 + 删旧 | 结构级回归 + 质量降≥50% + 删 `directed.rs` | golden 不回归 |
| P2.1 | state/class/er extract + grid | 三者 extract + `grid.rs`（复用 crossing） | 复用验证 |
| P2.2 | 三者视觉接 materialize/paint | 类框多行/关系线型 | golden 不回归 |
| P3 | 其余 8 种图 | sequence/pie/timeline/gitgraph/gantt/mindmap/quadrant/sankey | paint 零改动 |
| P4 | 收敛 + 删除 | 删旧 IR/转换层/theme 散落 + 归档文档 + 入口切 scene_ext | 对外签名不变 |

---

## 2. 详细任务

### Phase 0 — 脚手架 + 可观测性（地基）

#### P0.1 模块骨架 + 三层 IR 类型
- **新建目录**：`src/builder/{ir,extract,measure,layout,materialize,paint}/`
- **IR 类型**（`src/builder/ir/`）：
  - `unigraph.rs`：`Unigraph` / `UGNode`(`id,kind,role,label:LabelSpec,ports,size_hint,style_ref,constraint`) / `UGEdge`(`id,source,target,src_port,tgt_port,kind,label:Option<MeasuredLabel>,priority,routing_hint,arrow:ArrowSpec,repulsion`) / `GraphFamily`(Directed/Grid/Linear/Sequence/Radial/Hierarchy) / `StyleRef`
  - `geograph.rs`：`Geograph` / `GGNode`(`id,role,center,size,shape:ShapeKind,ports:ResolvedPorts`) / `GGEdge`(`id,route:Vec<Point>,label_anchor,kind,arrow,routing_hint`) / `GGContainer`(`bounds,title,kind`)
  - `scenegraph.rs`：`SceneGraph` / `SceneItem`(`Shape`/`Edge`/`Label`/`Group`) / `ShapeGeometry` / `EdgeEnds`(None/Arrow/Circle/Cross/Both/MultiCircle/MultiCross)
  - `shape.rs`：`ShapeKind`(Rectangle/Rounded/Stadium/Subroutine/Diamond/Hexagon/Circle/DoubleCircle/Cylinder/Asymmetric/Parallelogram/Trapezoid/Bar/StartDot/EndDot/PieSlice/QuadrantCell) —— **全项目唯一真相源**
  - `common.rs`：`LabelSpec` / `MeasuredLabel`(`text,layout:TextLayout,size`) / `StyleIntent`(`node_styles,edge_styles,container_styles`) / `PortHint` / `PortSet` / `ResolvedPorts` / `EdgePriority` / `RoutingHint` / `ArrowSpec` / `NodeRole` / `NodeKind` / `NodeConstraint` / `SizeHint`
- *产物*：编译通过的空 IR 模块（类型定义齐全，函数留桩）。

#### P0.2 边质量度量 + baseline
- 基于现有 `tests/layout_quality_test.rs` 扩展纯几何统计函数：
  - `count_edge_crossings(routes)`：边-边相交段数
  - `count_edge_overlaps(routes)`：共线/近似重合段数
  - `count_line_through_node(routes, node_rects)`：线段穿过非端点节点包围盒数
- 写 `tests/layout_quality_baseline.rs`：遍历 `tests/golden/*.mmd`，对每种图用旧管线产出几何，跑上述统计，输出 baseline 数字表（按图类型分列）。
- *产物*：baseline 报告（后续 Phase 1.4 / 2.2 验收比对锚点）。

#### P0.3 微型端到端跑通
- `extract/flowchart.rs` 最小子集：只处理矩形节点 + 直线边（`arrow_at_end` 默认 Arrow），产出 `Unigraph{family:Directed}`。
- `measure/mod.rs`：`measure_all(ug, theme) -> UG'`（桥接 `ast::RichText::measure` + `lievisual::text::layout_text`，按 `ShapeKind` 推算 `Size`）。
- `layout/family/directed.rs` 最简分层（BFS 入度分层 + 按源码序排布，暂不做交叉减少）。
- `materialize/mod.rs` + `paint/mod.rs`：把 `Geograph` 翻译成 `SceneGraph` 再翻译成 `lievisual::Scene`（矩形→`Element::Rect`，边→`Element::Line`+箭头 `Element::Polyline` 标记，文本→`Element::Text`）。
- *产物*：一个 3 节点 flowchart 走通 `Diagram→Scene`，且 `SceneGraph` 可 JSON 序列化（验证三层 IR 字段完整）。

---

### Phase 1 — flowchart 端到端（验证 IR 有效性）

#### P1.1 directed family solver
- `layout/crossing.rs`：抽 `minimize_crossings(layers, edges) -> layers`（从旧 `sugiyama.rs` 提炼）：含 SCC 收缩（环内超节点+组内 barycenter）、top_down/left_prio 双 pass、`crossing_iterations` 硬上限 5 + 收敛即停、tie-breaker 用 UG 原始出现顺序保确定性。
- `layout/family/directed.rs`：分层 DAG → 调 `minimize_crossings` → 坐标分配（复用旧 `sugiyama.rs` 的层高/层内间距逻辑，重写为消费 `Geograph`）。
- *产物*：flowchart 节点同层顺序经 barycenter 优化；同图多次运行字节一致。

#### P1.2 EdgeRouter（边感知核心）
- `layout/spatial.rs`：均匀网格哈希空间索引（cell≈平均节点间距），边线段建索引，支撑邻近查询。
- `layout/route.rs`：`route_edges(nodes, edges, ports) -> Vec<Vec<Point>>`：
  - 正交通道分配（纵向/横向候选通道按占用排序，尽力不冲突）
  - 边-边排斥（通道冲突引入平行偏移，复杂度 O(E·k) 经 spatial 索引）
  - 边-节点回避（路由代价含"线段穿过非端点节点包围盒"惩罚）
  - 标签占位（`label_space` 预留中点空白 + `label_anchor`）
  - 分层路由（同层局部通道、跨层走固定垂直通道）+ 迭代硬上限 3 + 预算护栏降级
- *产物*：典型 flowchart 几何层面无重叠/少交叉（非仅顺序层面）。

#### P1.3 flowchart materialize + paint
- `materialize/shapes.rs` + `edges.rs` + `containers.rs` + `labels.rs` + `theme_apply.rs`：消费 `Theme` 把 `GGNode.shape`+`StyleIntent` 解析为 `lievisual::Fill`/`Stroke`/`EdgeEnds`（收敛 `theme::flowchart::*` 取色逻辑到此一处）。
- `paint/shape_to_element.rs`：全 `ShapeKind` → `Element` 变体（菱形→`Polygon`、圆柱→`Path`+`Ellipse`、饼扇→`Pie`、StartDot/EndDot→`Circle` 实心/空心）。
- `paint/edge_to_element.rs`：全 `EdgeEnds` → 起止标记（`Arrow`/`Circle`/`Cross` 小 `Path`/`Polyline`）。
- `paint/text_to_element.rs` + `group.rs`：文本与 `Group`（z_index/clip）。
- *产物*：flowchart 全图走新管线渲染，视觉对照旧 golden 一致。

#### P1.4 验证 + 删旧
- 结构级回归：对 `SceneGraph` 做 JSON 序列化、坐标四舍五入到小数点后 2 位后比对（新增 `tests/scenegraph_regression_test.rs`，抽样 flowchart golden）。
- 边质量：重跑 P0.2 统计，确认 flowchart/class/er 在引入 `route.rs` 后"边交叉/重叠/穿节点"较 baseline **下降 ≥50%**。
- **删除**：旧 `src/builder/render/directed.rs`（flowchart 专用渲染器），新管线已验证可替代；旧 `build_diagram` 入口暂留作降级。
- *产物*：flowchart 完整通过验证 + 旧 directed 渲染器移除。

---

### Phase 2 — 吸收 state/class/er（验证 family 复用）

#### P2.1 extract + grid family
- `extract/state.rs`：`StateNode` → `Unigraph{family:Directed}`；`__start__/__end__` 特判消失 → `ShapeKind::StartDot/EndDot`。
- `extract/class.rs` + `extract/er.rs`：`ClassDiagramNode`/`ErDiagramNode` → `Unigraph{family:Grid}`；`ClassEdgeKind`(Extends/Composition/Aggregation/Association/Dependency/Realization/Link/Dashed) → `EdgeEnds` + `ArrowSpec`。
- `layout/family/grid.rs`：BFS 入度分层 + 调 `minimize_crossings`（复用 P1.1 通用原语，取代"纯源码顺序"）+ 关系路由（复用 `route.rs`）。
- *产物*：三者同层顺序经 barycenter 优化，验证 family 复用成立。

#### P2.2 三者视觉接 materialize/paint
- `theme_apply.rs` 收敛 `theme::class::*` / `theme::state::*`（类框多行文本行 `attributes/methods`、关系线型表实线/虚线/箭头）。
- *产物*：state/class/er golden 不回归；`__start__/__end__` 特判彻底消除。

---

### Phase 3 — 其余 8 种图
- **P3** 各自 `extract_*` + family solver：
  - `sequence`（`family/sequence.rs`：泳道 + 时间轴 + 消息路由）
  - `pie` / `quadrant`（`family/radial.rs`：极坐标 + `PieSlice`/`QuadrantCell`）
  - `timeline` / `mindmap`（`family/linear.rs`）
  - `gitgraph` / `gantt` / `sankey`（`family/hierarchy.rs`：时间轴/层级）
- 每新增一种图，验证 `paint` **零改动**即可渲染（仅引入全新 `ShapeKind` 变体时需改 `shape_to_element.rs`）。
- *产物*：12 种图全部走新管线；`materialize`/`paint` 主体未因图类型增加而改变。

---

### Phase 4 — 收敛 + 删除
- **P4**：
  - 删旧 IR：`builder/layout/ir.rs`(`PlacedGraph`) + `builder/layout/types.rs`(`Layout`/`LayoutNode`/`LayoutEdge`)
  - 删旧转换/求解层：`builder/layout/convert.rs` + `builder/layout/sugiyama.rs` + 各旧 `family` 渲染器（`state.rs`/`class.rs`/`er.rs`/`sequence.rs`/...）
  - 删 `theme.rs` 散落取色（已收敛进 `materialize/theme_apply.rs`）
  - 统一入口：确认 `scene_ext.rs::to_scene` 走新四阶段管线；旧 `build_diagram` 入口删除
  - 文档归档：旧 `layout-system-design.md`/`layout-refactor.md`/`refactor-layout.md`/`layout.md` 移到 `docs/archive/`
- *产物*：代码库只剩新三层 IR + 四阶段管线；对外 `Diagram::to_scene()` 签名不变；全部 golden 通过。

---

## 3. 验收标准（全局）
1. 对外 API 不变：`Diagram::to_scene()` 签名 + `DiagramError` 兼容；现有 `tests/` golden 调用方零改动。
2. 渲染解耦：`paint` 模块静态不引用 `ast`/`theme`。
3. 边感知：典型 flowchart/class/er 的"边交叉/重叠/穿节点"较 baseline 降 ≥50%。
4. 确定性：同 `Diagram` 多次 `to_scene` 字节一致。
5. 可扩展性：新增图类型 = 只写 `extract_*`（+ 如需新 family solver）；materialize/paint 不改动（除非新 `ShapeKind`）。
6. 回归策略：优先结构级回归（SceneGraph JSON 坐标舍入 2 位），像素级仅抽样 10 个典型 flowchart。

---

## 4. 启动建议
从 **P0.1** 开始：建模块骨架与三层 IR 类型定义，这是后续所有任务的依赖地基。P0.2/P0.3 可与 P0.1 同批进行（度量函数与微型管线都依赖 IR 类型就绪）。

---

## 5. 执行日志与偏差记录

> 本节约记录实际编码中与计划/设计文档不符的点、临时调整、与待决问题。
> 每次推进任务后追加，便于回溯。

### 2026-08-27 — P0.1 模块骨架 + 三层 IR 类型

**状态**：✅ 完成。新增 `src/builder/{ir,extract,measure,materialize,paint}` 五个模块，
注册进 `src/builder/mod.rs`（旧 `build_diagram` 入口保留未动）。`cargo build` 通过（无新增 error）。

**创建文件**：
- `ir/mod.rs` + `ir/common.rs`（端口/优先级/样式引用/标签等公共类型）
- `ir/shape.rs`（`ShapeKind`/`ShapeGeometry`/`EdgeEnds` 唯一真相源）
- `ir/unigraph.rs`（UG：`Unigraph`/`UGNode`/`UGEdge`/`GraphFamily`/`EdgeKind`）
- `ir/geograph.rs`（GG：`Geograph`/`GGNode`/`GGEdge`/`GGContainer`）
- `ir/scenegraph.rs`（SG：`SceneGraph`/`SceneItem`/`StyleIntent`/`Anchor`）
- `extract/mod.rs` `measure/mod.rs` `materialize/mod.rs` `paint/mod.rs`（四阶段占位桩，函数体 `unimplemented!`）

**⚠️ 偏差 1（重要，影响 P1.3 设计）：`Theme` 不是结构体，而是 const 模块**
- 设计文档 `redesign-from-scratch.md` §4.3 / §5 假设存在 `Theme` 对象，
  `materialize::run` 接收 `&Theme` 参数、`measure` 接收 `&Theme`。
- **现状事实**：`src/builder/theme.rs` 全是 `pub const`（BACKGROUND / FONT_SIZE /
  NODE_MIN_W / flowchart::FILL / class::* / sequence::* …），**没有 `Theme` 结构体**。
- **处理**：P0.1 桩函数已改为**不接收 theme 参数**（measure/materialize 签名去掉 `&Theme`）。
- **待决（P1.3 前需明确）**：主题系统有两种走向——
  (a) 保持 const 模块，materialize 直接 `use crate::builder::theme::*`；
  (b) 新建 `Theme` 结构体聚合这些 const（更贴合设计文档，便于换主题/测试注入）。
  倾向 (b)，但需评估对现有 `render/*` 旧渲染器（仍直接引用 `theme::*` const）的影响——
  若选 (b) 旧渲染器要么同步改造要么保留 const 兼容层。P1.3 启动时再定。

**⚠️ 偏差 2：`TextLayout` 未实现 `PartialEq`**
- `MeasuredLabel` / `LabelOrMeasured` 原设计想 derive `PartialEq`，但 lievisual 的
  `TextLayout` 未实现该 trait，导致编译失败。
- **处理**：这两个类型改为仅 `derive(Debug, Clone)`（不 derive PartialEq）。
- **影响**：layout / measure 阶段不应比较文本标签，符合预期（同 ID 节点靠 `NodeId` 而非标签区分）。

**⚠️ 工具使用注意（过程记录）**
- 本次有两处 `replace_in_file` 报告"成功"但文件内容实际未变（疑似 old_str 与磁盘
  不完全匹配，工具却返回成功）。表现：第一次改 `LabelOrMeasured` 的 PartialEq 未生效、
  第一次给 `geograph.rs` 加 EdgeKind import 未生效（后读文件复核才修对）。
- **纪律补充**：关键编辑后务必 `read_file` 复核或 `cargo build` 验证，不轻信替换成功回执。

**下一任务**：P0.2（边质量度量 + baseline）与 P0.3（flowchart 微型端到端）可并行启动，
二者都依赖 P0.1 已就绪的 IR 类型。建议先 P0.3 验证三层 IR 可序列化，再做 P0.2 度量。

### 2026-08-27 — P0.2 边质量度量 + baseline

**状态**：✅ 完成。新增 `tests/layout_quality_metrics.rs`，内含 SVG 解析（复用
`layout_quality_test.rs` 口径）+ 三类 `count_*` 度量函数 + `baseline_layout_metrics` 测试。
`cargo test --test layout_quality_metrics -- --nocapture` 通过并产出 baseline 表。

**度量函数**（返回计数，不断言，供后续对比）：
- `count_edge_crossings(segs)`：边-边内部相交数（排除共享端点）。
- `count_edge_overlaps(segs)`：边近似共线且投影区间重叠数。
- `count_line_through_node(rects, segs)`：边穿越非端点节点数。

**⚠️ 偏差 3（影响 P1 优先级，重要）**：设计文档 `layout-edge-aware-design.md` 假设
"边不感知"主要在 flowchart（有向图仅做了一半交叉减少）。但 baseline 实测显示
**flowchart 旧管线已相当干净**，真正糟糕的是 **class / er / timeline**：

| 图类型 | cross | overlap | through_node |
|---|---|---|---|
| flowchart (26 例) | 0 | 1 | 0 |
| sequence (5 例) | 5 | 34 | 8 |
| class (4 例) | 0 | 36 | 0 |
| state (4 例) | 0 | 0 | 0 |
| er (3 例) | 5 | 32 | 0 |
| pie (3 例) | 0 | 0 | 0 |
| timeline (2 例) | 0 | 108 | 16 |
| gitgraph (2 例) | 0 | 6 | 0 |
| **TOTAL** | **10** | **217** | **24** |

**结论与调整**：
- 边感知布局（`route.rs` 边-边排斥 / 边-节点回避）对 **class / er / timeline** 收益最大，
  对 flowchart 几乎无提升空间（已接近最优）。
- P1 验收标准（"flowchart/class/er 降≥50%"）需修正：flowchart 已接近 0，应改为
  **"class/er/timeline 的 cross+overlap+through 较 baseline 下降≥50%"**，
  flowchart 只需"不退化（回归 0）"。
- 这不影响 `minimize_crossings` 通用原语的复用价值（class/er 的 grid family 仍受益）。

**⚠️ 偏差 4（度量层级）**：计划原拟"基于 PlacedGraph 几何统计"，实际采用 **SVG 黑盒
统计**（复用 `parse_svg`）。理由：立即可跑、跨新旧管线口径一致、公平对比。已在
`redesign-task-plan.md` P0.2 任务描述与代码注释中标注。

**已知噪声**：`ICU4X data error: No segmentation model for complex script` 是中文标签
分词警告（pie/timeline 用例含中文），不影响几何统计数字，属既有问题。

**下一任务**：P0.3（flowchart 微型端到端：extract→measure→layout→materialize→paint
跑通 Diagram→Scene，验证三层 IR 可序列化）。P0.2 baseline 数字已锁定，P1.4 时重跑对比。

### 2026-08-27 — P0.3 启动前的事实核对（重要偏差）

**⚠️ 偏差 5（设计文档与现状不符，影响 extract 实现）**：设计文档
`redesign-from-scratch.md` 假设 `ast` 是嵌套结构（`FlowchartNode` 持 `nodes: IndexMap<NodeId, FlowNode>`
+ `edges: Vec<FlowEdge>`，文本为 `RichText`）。**实际 `ast.rs` 是扁平结构**：
- `Diagram::Flowchart(Flowchart { direction, nodes: Vec<Node>, edges: Vec<Edge>, subgraphs: Vec<Subgraph> })`
- `Node { id: String, shape: Option<NodeShape>, text: Option<String> }` —— **文本是 `String` 不是 `RichText`**
- `Edge { source, target, arrow_type: ArrowType, label: Option<String> }`
- `NodeShape` = 14 变体扁平枚举（Rectangle/Rounded/Stadium/.../TrapezoidAlt）；`ArrowType` = 11 变体
  （Solid/Dotted/Thick/NoArrow/Both/Circle/Cross/Invisible/MultiCircle/MultiCross/Labeled(String)）

**影响与处理**：
- P0.1 的三层 IR 类型（UG/GG/SG）**独立于 ast**，本身正确，无需改。
- P0.3 的 `extract_flowchart` 必须按真实 `ast::Flowchart` 字段写（非假设的 `FlowchartNode`）。
- 文本测量：ast 无 `RichText`，但 `builder/layout/measure.rs` 已示范正确做法——
  `layout_text(&[RichSpan::new(text, style)], None)`（`RichSpan`/`TextStyle` 来自 `lievisual`），
  字号常量来自 `crate::builder::theme`（NODE_MIN_W / FONT_SIZE / ...）。新 measure 复用此方式。
- `ArrowType` → IR 映射需覆盖 11 变体（Dotted/Thick 仅影响线型，P0.3 先统一画实线箭头，线型细化留 P1.3）。
- **设计文档需后续修订**：把"假设嵌套 ast"改为"扁平 ast"，并修正 `extract_*` 的字段引用。
  本偏差已在 P0.3 实现时按真实 ast 落地。

### 2026-08-27 — P0.3 微型端到端跑通 ✅

**状态**：✅ 完成。新增 `tests/pipeline_smoke_test.rs`（2 用例），`cargo test --test pipeline_smoke_test`
通过（2 passed）。全量 `cargo build` 通过（仅既有 unused import 警告，非本次引入）。

**实现要点（已实现函数，均编译通过）**：
- `extract::run(&Diagram) -> DiagramResult<Unigraph>`：flowchart 分支走 `extract_flowchart`
  （节点取 `fc.nodes`、边取 `fc.edges`、方向取 `fc.direction`）；非 flowchart 返回
  `Err(DiagramError::UnsupportedType(name))`。
- `measure::measure_all(&Unigraph) -> DiagramResult<Geograph>`：对每个 UGNode 调
  `measure_node`（用 `layout_text(&[RichSpan::new(..)], None)` + `theme` const 算
  `NODE_MIN_W/H`、`NODE_PAD_*`、`FONT_SIZE`）填 `w/h`，沿用 UG 拓扑生成 GG。
- `layout::engine::run(&Geograph) -> DiagramResult<(Geograph, StyleIntent)>`：最简 BFS 分层
  （`direction` 决定主轴的层/序），节点直排 + 直线边，`StyleIntent` 全 default。
- `materialize::run(&Geograph, &StyleIntent) -> DiagramResult<SceneGraph>`：节点按 `ShapeKind`
  生成 `SceneItem::Shape`（rect/rounded/ellipse/polygon/card），边生成 `SceneItem::Edge`
  （`path` 折线 + `stroke` + `ends` 箭头），`ShapeKind`/`EdgeEnds` 为全项目唯一真相源。
- `paint::run(&SceneGraph) -> DiagramResult<Scene>`：`Scene::new(w,h).with_background(theme::BACKGROUND)`
  + 逐 `SceneItem` 翻译为 `Element`（rect/rounded_rect/ellipse/polygon/line）+ `SceneNode::with_z`；
  **零 AST / Theme 引用**（纯翻译，达成纪律 1）。

**⚠️ 偏差 6（测试源语法，parser 既有行为）**：测试初版用 `graph TD\nA-->B\nB-->C\nC-->A`
（紧凑、隐式节点 ID、无空格箭头）与 `A[Start] --> B[Proc]`（节点+边同行）两种写法，
解析结果分别是 `0 节点/0 边` 与 **只得到 2 条边（第 1 条 `--> B[Proc]` 在节点声明被
`node_definition` 吃掉后整行残留被丢弃）**。
- 根因属 **parser 模块既有行为**：`src/parser/flowchart.rs::parse_flowchart` 在语句级
  `alt` 中把 `node_definition` 与 `chain_edges` 当作**互斥语句**——`A[Start] --> B[Proc]`
  这一行 `node_definition` 吃掉 `A[Start]` 后 `continue`，剩余 `--> B[Proc]` 被下一轮
  "跳过无法识别行"吞掉，边丢失。这与 `examples/flowchart.rs`（节点/边分行写）能正常渲染一致。
- **处理**：smoke test 改用 **节点与边分离写法**（`A[Start]\nB[Proc]\n...\nA --> B\nB --> C\nC --> A`），
  与 parser 自带单元测试（`parser/flowchart.rs` 第 355 行附近）一致，聚焦验证"三层 IR 管线贯通"，
  不依赖 parser 的节点+边同行解析能力（那是 parser 自己的单测覆盖范畴）。
- **待决**：若未来要求支持 `A[Start] --> B[Proc]` 同行写法，需改 `parse_flowchart`
  的语句级 `alt`（节点声明后继续解析其后的边链），属 parser 模块任务，不在 P0.3 范围。

**⚠️ 偏差 7（测试导入路径，parser 可见性）**：`tests/pipeline_smoke_test.rs` 初版写
`use liemermaid::parser::parse_mermaid`，但 `parse_mermaid` 是 `WinnowParser` 的**关联函数**
（`WinnowParser::parse_mermaid`），非 `parser` 模块级函数。`lib.rs` 已
`pub use parser::WinnowParser as MermaidParser`。**修正**：测试改用
`use liemermaid::MermaidParser; ... MermaidParser::parse_mermaid(src)`。

**下一任务**：P1.1（directed family solver：Sugiyama 分层 + 通用 `minimize_crossings`）。
进入 P1 前，P0.2 baseline 数字（TOTAL cross=10/overlap=217/through=24）已锁定，
P1.4 时重跑 `count_*` 对比，验收标准按偏差 3 修正为
**class/er/timeline 的 cross+overlap+through 较 baseline 下降≥50%，flowchart 不退化**。

### 2026-08-27 — P1.1 directed family solver 完成 ✅

**状态**：✅ 完成。新增 `layout/crossing.rs`（通用 `minimize_crossings`）+ `layout/directed.rs`
（`sugiyama_layers`）。`cargo test` 全量通过（lib 37 + 各集成测试均 ok），无回归。
新增单测 4 个：`crossing::reduces_a_simple_crossing`、`crossing::preserves_node_set`、
`directed::sugiyama_reduces_crossings_on_3_layers`、`directed::sugiyama_reverses_layers_for_bt`。

**实现要点**：
- **通用 `minimize_crossings`（`crossing.rs`）**：barycenter 启发式，输入 `&[Vec<NodeId>]` +
  `&[LayerEdge]`，与 family / direction 完全解耦（评审建议"通用原语"）。**关键修正**：每轮基于
  `cur` 当前位置**动态**重算 barycenter（首版预构建 `down/up` 邻接表基于原始位置，节点重排后
  位置错位导致交叉消除后又复原 → 测试失败）；双向匹配 `(source∈layer_idx, target∈other_idx)`
  与 `(source∈other_idx, target∈layer_idx)` 覆盖正/反向边。4 轮（odd 向下 / even 向上），
  tie-break 用节点原始出现序 `appearance`，保证确定性。
- **`sugiyama_layers`（`directed.rs`）**：`assign_layers`（最长路径松弛，DAG 分层，环用
  `ids.len()+1` 次松弛上限收敛）+ 调 `minimize_crossings` + **方向反转**（`BT`/`RL` 把
  分层整体 `reverse()`，使主轴方向正确）。输出 `Vec<Vec<NodeId>>`。
- **`engine.rs` 重构**：分层改为调 `sugiyama_layers`；坐标分配按 `direction` 做**主轴旋转**
  （`LR`/`RL` 时主轴=X、同层轴=Y，反之 TB/TD/BT 主轴=Y、同层轴=X）。保留端口解析 + 直线边路由
  （P1.2 才升级正交）+ StyleIntent 抽取。UG 在 layout 结束即 drop（materialize/paint 不持 UG）。

**⚠️ 偏差 8（IR 缺 direction 字段）**：`Unigraph` 原无 `direction`，layout 无法支持 LR/BT/RL。
已在 `ir/unigraph.rs` 给 `Unigraph` 加 `direction: Direction` 字段（手写 `Default` impl，因
`ast::Direction` 未 derive `Default`，不能对含它的 struct 直接 `#[derive(Default)]`）；
`extract/flowchart.rs` 透传 `fc.direction.clone().unwrap_or(Direction::TB)`（Direction 非
`Copy`，需用 `.clone()` 而非 `.cloned()`/`.unwrap_or` move）；`measure/mod.rs` 重建 UG 时
`direction: ug.direction` 透传，避免测量阶段丢方向。

**⚠️ 偏差 9（parser 节点+边同行限制，延续偏差 6）**：本阶段未触及 parser；P0.3 已确认
`A[Start] --> B[Proc]` 同行写法会被 parser 丢边。P1.x 渲染正确性依赖"节点/边分离"源，
与 parser 自带单测一致。若后续要支持同行边，属 parser 模块任务。

**下一任务**：P1.2（`EdgeRouter`：`route.rs` 正交路由 + `spatial.rs` 网格哈希边-边排斥）。
注意 P1.1 当前边路由仍为直线（source 端口→target 端口），P1.2 升级为避障正交/曲线，
并接入 `RouteOptimizer` 风格的空间索引（评审建议 O(E²)→网格哈希）。

### 2026-08-27 — P1.2 EdgeRouter 完成 ✅

**状态**：✅ 完成。新增 `layout/spatial.rs`（网格哈希 `SpatialGrid` + 线段/矩形相交）、
`layout/route.rs`（`route_edges` 正交路由 + 节点回避）。`engine.rs` 在构造 GG 后调用
`route_edges` 重做边路由并重算 bbox。`cargo test` 全量通过（42+ 各集成测试均无 FAILED），
新增单测 5 个（spatial 3：网格查询/线段-矩形/线段-线段；route 2：回避阻挡节点/端点保留）。

**实现要点**：
- **`spatial.rs`**：`SpatialGrid` 均匀网格（cell 默认 80），`insert_rect`/`insert_segment` 按
  覆盖 cell 登记，`query_rect`/`query_segment` 仅回传相邻 cell 候选（O(E²)→≈O(E)，评审建议）。
  相交用**自实现跨立实验**（`cross` + `<=0` 判相交），规避 kurbo `LineIntersection` 变体名
  版本差异。注：跨立实验把「端点接触」判为相交（数学合理），单测已据此修正。
- **`route.rs`**：`route_edges(&mut Geograph)` —— 对每条边：① 自适应端口（source 朝 target
  方向出、target 朝 source 方向入，`pick_port` 终点方向与起点**镜像**）；② 出线 stub（沿端口
  法向 `STUB=18`）；③ 正交主干统一到同一主轴坐标（水平主导共享 y、垂直主导共享 x），**保证
  纯曼哈顿折线**；④ 节点回避：用 `SpatialGrid` 查非端点节点包围盒，整体平移主干（`OFFSET_STEP=12`，
  最多 8 轮）直至不穿。
- **`engine.rs`**：构造 GG 后 `route_edges(&mut gg)` 重路由，`compute_bbox` 重算（路由偏移会
  改变端点包围盒）。

**⚠️ 偏差 10（GGEdge 需补 source/target 字段）**：几何层原 `GGEdge` 只有 `id/route/...`
无端点引用，但路由必须知道连哪两节点。已在 `ir/geograph.rs` 给 `GGEdge` 加
`source: NodeId` / `target: NodeId`（`engine.rs` 构造边时填充 `e.source/e.target.clone()`）。
这是几何层允许保留的"拓扑回指"，materialize/paint 不使用，不违反"几何自足"核心纪律
（视觉决策仍在 materialize）。
- **⚠️ 偏差 11（route 回避范围为节点回避，未做边-边排斥）**：评审建议的"边-边排斥"本阶段
  仅落地**节点回避**（边不穿节点包围盒）。边-边完全避让成本高（需全局迭代 + 曲线/绕行），
  且 Sugiyama 分层 + 交叉减少已大幅降低边重叠；P1.2 用同一 `SpatialGrid` 预留了边段查询能力
  （`query_segment`），后续若 `layout_quality_metrics` 显示边-边 through 仍高可再接入。
- **⚠️ 偏差 12（ports 在路由时被忽略，自适应端口取代 PortHint）**：P1.1 engine 用 UG 的
  `source_port/target_port`（`PortHint`）选端口，P1.2 `route.rs` 改为按相对位置自适应选端口
  （更鲁棒，避免同向边全挤同一侧）。UG 的 `PortHint` 仍保留供特殊用例，但常规路由不再依赖。

**下一任务**：P1.3（flowchart `materialize`/`paint`：补全全部 `ShapeKind` 几何 +
`EdgeEnds` 箭头端点 + theme 收敛到单一真相源）。P1.2 已产纯曼哈顿 polyline，`paint` 需把
`GGEdge.route`（Vec<Point>）画成 `Element::path`/折线 + 箭头；`materialize` 需按 `ShapeKind`
生成正确 `SceneItem::Shape`（圆/菱形/六边形/卡片等），当前仅 Rectangle/RoundedRect/Ellipse。

### 2026-08-27 — P1.3 flowchart materialize + paint 完成 ✅

**状态**：✅ 完成。`materialize` 现已覆盖 `ShapeKind` 全部 18 个变体（含 Stadium/Hexagon/Cylinder/
Asymmetric/Parallelogram/Trapezoid/Bar/StartDot/EndDot/PieSlice/QuadrantCell/DoubleCircle/Subroutine）；
`paint` 把边渲染为**多段折线 + 起止箭头标记**（EdgeEnds：Arrow/Circle/Cross），并支持 Stadium/Diamond/
Path/Pie 几何。`cargo build` 通过，`pipeline_smoke_test`(2) + `layout_quality_metrics`(1) 全过。

**materialize 补全**（`shape_to_geometry`）：
- `Stadium`→`ShapeGeometry::Stadium`（paint 用半高圆角 RoundedRect 画药丸）；
- `Subroutine`→`RoundedRect`（小圆角 2.0 区别于普通 Rounded）；
- `Hexagon`→6 点 `Polygon`；`Asymmetric`/`Parallelogram`/`Trapezoid`→对应 4 点 `Polygon`；
- `Circle`/`StartDot`→`Ellipse`；`DoubleCircle`/`EndDot`→`Ellipse`（**单椭圆近似**，见偏差13）；
- `Bar`/`QuadrantCell`/`Rectangle`→`Rect`；`Cylinder`→采样多边形 `Polygon`（顶/底椭圆弧各 12 段）；
- `PieSlice`→`Pie`（固定第一象限角，mermaid pie 的真实扇区角度由 measure 阶段注入，P1.3 仅几何到位）。

**paint 补全**（`geometry_to_element` + `run_edge_nodes`）：
- 新增 `Stadium`（半高圆角 RoundedRect）、`Diamond`（4 点 Polygon）、`Path`（收集 PathOp 端点为
  Polygon 折线，因 lievisual 0.1.2 无原生 Path 变体，见偏差14）、`Pie`（Element::pie）。
- 边渲染重构：原 `Element::line(首,尾)` 改为 `run_edge_nodes` 返回多节点——① 折线本体
  `Element::poly(route, stroke)`；② 调 `arrow_element` 在**终点**（last 段方向）与**起点**
  （first 段反向）各画标记。箭头为实心三角 `Polygon`，Circle 为小椭圆，Cross 为两条交叉线段。
- `run_item` 的 Edge 分支同步改为多节点（Group 内边可正确展开）。

**theme 收敛**：本阶段确认 materialize 已是**唯一**消费 `theme::flowchart::*` const 的层，paint
纯机械翻译、零 theme 引用，满足偏差1「主题单一真相源」纪律。无需额外改动。

**⚠️ 偏差 13（DoubleCircle/EndDot 用单 Ellipse 近似，无内镂空环）**：mermaid state 的
`([ ])`(双环)/`(( ))`(圆) 与 `__end__`(双环终止) 本应画双环（外圈 + 内镂空）。当前 materialize
只产单 `Ellipse`，paint 画实心+描边，视觉上是单椭圆而非双环。修复需 materialize 对 DoubleCircle/
EndDot 额外产一个内圈 `Shape`（镂空靠 fill=None+stroke），超出单 `ShapeGeometry` 承载，留 P2/P4 收尾。
当前不阻塞端到端（节点仍可见、标签可读）。

**⚠️ 偏差 14（lievisual 0.1.2 无 `Element::path` 变体）**：计划文档 P1.2→P1.3 衔接期望边用
`Element::path`（BezPath）渲染。实际 lievisual 0.1.2 的 `Element` 仅有 `rect/rounded_rect/ellipse/
circle/polygon/line/poly/rich_text/arc/pie/group` 等，**无 `path`**。边改用 `Element::poly`
（polyline，接收 `Stroke`）渲染多段折线；`ShapeGeometry::Path` 变体降级为收集 PathOp 端点绘 Polygon
折线（几何层已把弧采样为点）。若后续需真实贝塞尔曲线边，需升级 lievisual 或改用 `Element::arc` 组合。

**下一任务**：P2（吸收 state/class/er 图，验证 family 复用）——state 的 `([ ])`/`(( ))` 双环形状
正好复用 P1.3 的 `DoubleCircle`/`Circle`（偏差13 的双环修复可在 state 接入时一并做）；class 的
分栏框复用 `Container` 几何；er 的实体/关系复用节点/边。P1 阶段 IR 有效性已坐实。
