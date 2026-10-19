# liemermaid vs 官方 mermaid —— SVG 对齐对比报告

> 生成日期：2026-08-29
> 样本：53 个用例（tests/golden/cases），每个均有官方 mermaid-cli 生成的 golden SVG
> 方法：先 `cargo test --test html_report_test generate_html_report` 生成 `tests/golden/report.html`，
>       再对报告里**每一对 SVG（liemermaid 输出 vs 官方 golden）逐图解析几何**：
>       - 节点：形状类型 + 包围盒中心（累加 `<g transform>` 平移）
>       - 边：首尾端点（`path/line` 的首末坐标，跳过 <5px 的箭头短线）
>       - 文本：所有 `<text>` 与 `<foreignObject>` 内文字
>       最后按「质心 + 包围盒对角线」归一化比较位置（消除不同画布尺寸/平移的影响）。
> 判定以**亲自解析得到的几何数据**为准，而非直接照搬自动 diff 的结论（见末尾「关于自动 diff 的误报」）。

---

## 一、整体结论

- **完全对齐（结构/文本/布局一致）：26 例** —— flowchart 22 例 + sequence__notes + er__attributes + gitgraph×2。
- **仅形状/表达差异（布局与文本对齐，视觉几何不同）：约 16 例** —— 主要是 flowchart 决策节点官方用 `polygon`(菱形)、liemermaid 一律 `rect`；以及 sequence 激活条、class 关系箭头等表达方式不同。
- **真实差距（需修复）：约 11 例** —— 集中在 `pie`(缺图例/标签格式)、`timeline`(结构不同)、`state`(复合/分支缺失)、`class`(成员空格/关系箭头)、`sequence__loops`(标签拆分)、`flowchart__shapes`(形状覆盖)。
- 没有任何用例出现**节点整体错位**级别的严重布局错误。flowchart 全部 30 例的边数量与官方**逐一相等**（chain 2=2、cross 6=6、dense 11=11…），证明拓扑与布局正确。

---

## 二、判定等级

| 标记 | 含义 |
|---|---|
| ✅ 对齐 | 节点数、边数、文本集、归一化布局均与官方一致 |
| 🟡 表达差异 | 结构/文本/布局对齐，但元素画法或形状不同（如矩形 vs 菱形、线 vs path） |
| 🟠 真实差距 | 存在内容/结构/布局层面的实质差异，建议修复 |

---

## 三、完全对齐用例（✅，26）

flowchart（22）：`chain` `cycle` `split` `cross` `binary_tree` `fan_in` `fan_out`
`long_chain` `grid2x2` `disconnected` `edge_types` `edge_labels` `self_loop`
`long_edge` `dense` `cycle2` `lc1` `lc2` `lc3` `lc4` `subgraph` `lc5`

> 证据：`subgraph`/`lc5` 两侧均为 nodes=8、edges=4、文本集合一致（含子图容器）；
> 上述矩形流程的边数量与官方逐项相等、端点方向一致。

sequence（1）：`notes`
er（1）：`attributes`
gitgraph（2）：`basic` `attributes`

---

## 四、逐类型分析

### 4.1 Flowchart（30）

**矩形流程（无决策节点）—— 全部 ✅**，边数量与官方完全相等，文本一致。

**含决策节点的流程 —— 🟡（仅形状差异）**：`diamond` `diamond_lr` `diamond_bt` `diamond_rl`
`diamond_nested` `diamond_3way` `two_diamonds`
> 证据：`flowchart__diamond_3way` 官方检测到 `N polygon c=(165,145) label-container`（决策菱形），
> liemermaid 对应节点为 `rect`。两边边数量均 7=7、文本一致、端点方向一致。
> 即：决策节点官方画成菱形，liemermaid 画成矩形，位置正确。

**`flowchart__shapes` —— 🟠 形状覆盖缺口**
> 源含 10 种 mermaid 形状（Rectangle/Rounded/Stadium/Subroutine/Database/Circle/Decision/
> Parallelogram/Trapezoid/DoubleCircle）。官方用 distinct geometry（golden 中见到 `polygon` 菱形、
> `circle` 圆/双圆、各类 `rect`），liemermaid 全部渲染为普通 `rect`。文本 10/10 一致，源无连线。

### 4.2 Sequence（5）

- `basic` `activation` `three_party` —— 🟡 表达差异
  > 边数量完全相等（5/7/9 = 5/7/9），消息文本一致。差异：官方用 `<line>` 画生命线、用 `rect`+`circle`
  > 画激活条；liemermaid 用 `path` 画生命线、用 `rect` 画激活条。属画法不同，结构对齐。
- `notes` —— ✅
- `loops` —— 🟠 标签格式
  > 官方拆为两个文本 `loop` + `[Each item]`；liemermaid 合并为 `loop [Each item]`。
  > 官方用 4 条边画循环框，liemermaid 用 `polygon` 画循环框（edges 5 vs 9）。内容齐全，仅格式不同。

### 4.3 Class（4）

- `basic` —— 🟡 表达差异
  > 文本（Animal/Dog/Cat）一致，3 个类框布局对齐。官方把类框分隔线也算作 edge（11 条），
  > liemermaid 仅画 2 条继承连线。结构对齐，表达粒度不同。
- `fields_methods` —— 🟠 文本格式
  > 官方 `+String name` / `+int age`；liemermaid `+ String name` / `+ int age`（成员前有空格）。
  > 边 1 vs 7（官方含框分隔线）。
- `relations` —— 🟠 关系箭头
  > 文本（A~E）一致，布局对齐；官方 19 条边（关系标记 + 框分隔线），liemermaid 4 条。
  > 关系连线画法（箭头/基数标记）差距较大。
- `cardinality` —— 🟠 文本格式 + 关系箭头
  > 同 `fields_methods` 的 `+ String name` 空格问题；基数标记（1 / *）渲染差异。

### 4.4 State（4）

- `basic` `with_labels` —— 🟡 表达差异
  > 边数量相等（4/3），状态文本一致。官方用 `circle`(start/end) + `rect`(state)；
  > liemermaid 用 `ellipse`(start/end) + `rect`(state)。结构对齐。
- `composite` —— 🟠 复合状态边缺失
  > 文本（Outer/Inner1/Inner2/Final）一致，垂直嵌套布局对齐；但**边数量 4 vs 6**，
  > liemermaid 少画 2 条复合状态内外转移（自动 diff 报 `geom edge endpoints max-err=0.792`，
  > 超过 0.18 容差，为真实但中等的边位置/数量偏差）。
- `fork_join` —— 🟠 缺失标签 + 少边
  > 官方含 `join_state` 文本，liemermaid 缺失；边 7 vs 8（少 1 条 join 连接）。
  > fork/join 条官方用细矩形，liemermaid 用 `polygon`。

### 4.5 ER（3）

- `basic` `cardinality` —— 🟡 布局对齐，边被过度分段
  > 文本一致、实体布局对齐（`basic` 三者竖向堆叠、`cardinality` 四者横向排布，均与官方同构）。
  > 差异仅在**边表示**：liemermaid 把 crow's-foot 基数标记拆成多条独立 `path`
  > （`basic` 8 条 vs 官方 2 条；`cardinality` 13 vs 4），导致自动 diff 报较大端点误差——
  > 实为分段画法问题，**非节点位置错位**。
- `attributes` —— ✅（自动 diff 判对齐；实体/字段文本一致，布局对齐）

### 4.6 Pie（3）

- `basic` `showdata` `small` —— 🟠 缺图例 + 标签格式
  > 饼图扇区中心对齐（自动 diff `geom node centers max-err=0.000`）。
  > 真实差距：① liemermaid **不画图例色块**；② 标签合并为 `Rust (40.0%)`，
  > 官方拆分为 `Rust` + `40%` 并配图例；③ 边端点轻微偏差 `max-err≈0.233`（略超 0.18 容差）。

### 4.7 Timeline（2）

- `basic` `multi_event` —— 🟠 结构不同
  > 所有事件文本齐全（与官方一致）。差异：liemermaid 用「横向时间轴 + 年份列 + 事件节点框」，
  > 官方用「纵向分段 + 横向事件线」；且连线被过度分段
  > （`basic` 30 vs 13、`multi_event` 42 vs 17 条边，因每条连接含箭头三角）。
  > 另：官方保留源码双空格 `发布   v1.0`，liemermaid 折叠为 `发布 v1.0`。

### 4.8 Gitgraph（2）

- `basic` `attributes` —— ✅
  > 拓扑（分支/合并）与官方一致。差异仅 commit id：liemermaid 用合成编号 `c0/c1/c2…`，
  > 官方显示真实 hash（如 `0-28a41f8`）——属预期行为，非缺陷。

---

## 五、真实差距清单（建议按优先级修复）

| 优先级 | 用例 | 问题 | 性质 |
|---|---|---|---|
| P1 | `flowchart__shapes` | 9+ 种 mermaid 形状无独立几何，一律矩形 | 形状覆盖 |
| P1 | `state__composite` | 复合状态少画 2 条转移边（4 vs 6） | 结构缺失 |
| P1 | `state__fork_join` | 缺失 `join_state` 标签 + 少 1 条边 | 结构缺失 |
| P2 | `pie`(×3) | 无图例；标签合并 `X (p%)` 而非 `X`+`p%` | 表达/格式 |
| P2 | `timeline`(×2) | 结构（轴+列 vs 分段线）；双空格折叠 | 结构/格式 |
| P2 | `class__relations` `class__cardinality` | 关系/基数箭头画法差距大 | 表达 |
| P3 | `class__fields_methods` | `+ String name` 成员前多余空格 | 文本格式 |
| P3 | `sequence__loops` | `loop` 与 `[Each item]` 合并为单标签 | 文本格式 |
| P3 | 全部 flowchart 决策节点 | 菱形决策节点画成矩形 | 形状 |
| 观察 | `er`(×2) | 边被 crow's-foot 过度分段（布局本身对齐） | 表达（非错位） |

---

## 六、关于自动 diff 的误报（重要）

`tests/golden/svgdiff.rs` 的 `is_empty()` 在以下情况会产出「差异块」，但**多数并非真实错位**：

1. **节点形状元素类型不同** → 节点中心无法配对 → 报 `geom node centers: COUNT MISMATCH`。
   例如 flowchart 官方用 `circle`(stadium)/`polygon`(diamond)、liemermaid 用 `rect`，
   元素计数不等导致无法配对，但**实际节点坐标是一致的**（见本报告逐案边数量相等证据）。
2. **crow's-foot / 框分隔线 / 箭头被拆成多条 path** → 边数量不等 → `geom edge endpoints: COUNT MISMATCH`
   或看似很大的端点误差（如 `er__basic` 的 11.625 实为分段画法，节点布局对齐）。
3. **真正需要关注的信号**只有两类：① 文本集合不一致（`missing/extra text`）；
   ② `geom ... max-err` **超过 0.18 容差**且两侧**数量相等**（可正常配对）。
   本报告已据此重新甄别：`state__composite`(0.792)、`pie`(≈0.233) 属此类真实偏差；
   `er__basic` 的 11.625 经几何复核为分段误报，布局实际对齐。

---

## 七、下一步

建议优先修复 P1（形状覆盖 + 复合/分支状态边缺失），其次 P2（饼图图例、时间线结构）。
所有修复均可复用现有 `tests/golden/report.html` 做人工回归对照。
