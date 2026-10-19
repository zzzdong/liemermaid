# 路线 A：视觉高度还原官方 Mermaid —— 重构与可行性分析

> 状态：评估稿（2026-08-22）
> 适用范围：`liemermaid` 渲染管线（本仓库）+ `lievisual` SVG 后端（外部 crate `d:/code/rust/lievisual`）
> 配套验收工具：`tests/html_report_test.rs`（人工对比 HTML）、`tests/official_compare_test.rs`（语义/结构 diff）、`tests/golden/svgdiff.rs` + `semantics.rs`

---

## 1. 目标与范围

**路线 A 定义**：让 `liemermaid::render` 输出的 SVG 在**视觉/像素层面**逼近官方 `mermaid`（`mermaid-cli` 生成）的渲染结果——同一张 Mermaid 图，两者在浏览器里看起来"几乎一样"。

明确**不在**本路线范围（属于路线 B"语义等价"或后续优化）：
- 动画/`click` 交互/`mermaid.init` 运行时行为
- 主题切换 UI、配置的完整 mermaid config 兼容
- 与官方**逐字节**一致的 SVG 字符串（不可能也无意义，几何坐标天然不同）

**"高度还原"的可量化目标**（验收基线见 §7）：
- 文本内容与位置基本对齐（偏差 < 1 行高）
- 节点形状、边走向、箭头/关系标记类型一致
- 整体布局拓扑一致（不要求坐标相等，但相对位置/层级不颠倒）
- DOM 结构语义化（有 `node`/`edge`/`cluster` 分组与 style 体系），便于下游换肤

---

## 2. 范式差异事实对照（实测）

下表基于对 `tests/golden/golden/*.svg`（官方）与 `tests/golden/report.html` 内联的 liemermaid 输出（同一组 48 个 case）实测统计。

| 维度 | 官方 Mermaid | liemermaid 当前 | 差异性质 |
|---|---|---|---|
| **文本排版** | `<foreignObject><div>`（HTML 排版，支持自动换行/emoji/富文本） | SVG `<text>`（parley 预排版，仅算背景框） | 根本性（P0） |
| **边的几何元素** | 每条边 = 主 `<path>` + 标签 `<path>`/`<rect>`，箭头走 `<defs><marker>` | 整条 `<polyline>`/`<line>` + 独立 `<path>` 箭头头 | 结构性（P0/P1） |
| **节点锚点** | 每个节点附带 1 个透明 `<circle>` anchor（供边连接，计入元素数） | 无 anchor | 计数偏差（P1） |
| **节点形状** | `<rect rx>`/`<circle>`/`<polygon>`/`<path>`，圆角用 CSS | 自写 `<path>` 近似（Stadium 用 k=0.5523 贝塞尔近似圆） | 视觉细节（P1/P2） |
| **class 关系线** | `<path>` + 三角/菱形头（`#my-svg .marker`）+ polygon 继承三角 | `<line>` 直线 + 独立 `<path>` 头（class 图已渲染，但 er 缺语义种类） | 视觉不符（P1） |
| **ER 关系** | 区分 identifying/non-identifying（不同线型/箭头） | 仅"实体间一条带基数直线"，`ErRelationship` 无 `kind` 字段 | 语义缺失（P1） |
| **样式体系** | `<style>` 块 + 语义 class（`node rect`/`.edgePath .path`/`.cluster rect`/`.edgeLabel`） | 扁平输出 `class="node"/"edge"`，无 `<style>`、无分层 | 结构性（P1） |
| **布局坐标** | dagre 产出，像素网格对齐，`viewBox` 自适应 | 自研 Sugiyama（无 subgraph 时）+ 手写管线（有 subgraph 时），`fit_to_canvas` 整组缩放 | 几何偏差（P0/P1） |

**实测元素计数对照（代表性 case）**：

| Case | 官方 (path/line/poly/circle/text) | liemermaid (path/line/poly/circle/text) |
|---|---|---|
| flowchart__chain | 9 / 0 / 0 / 4 / 0(foreignObj) | 2 / 0 / 2 / 0 / 3 |
| class__cardinality | 23 / 0 / 0 / 4 / 0 | 1 / 4 / 0 / 0 / 8 |
| er__basic | 10 / 0 / 0 / 4 / 0 | 0 / 23 / 0 / 0 / 5 |
| sequence__basic | 10 / 5 / 0 / 1 / 7 | （line 化） |

> 注：官方 `text=0` 是因为文本统一放在 `<foreignObject>` 内（`<div>`），而非 `<text>` 标签，这是 liemermaid 直接数 `<text>` 的口径差异来源之一。`circle=4` 在官方各图均出现，对应 4 个节点各自的 anchor 圆。

**当前差距规模**（来自 `official_compare_test` 最近一次全量）：48 个 case 全部进入 mismatch（语义或结构 diff），但 **0 个结构性回归**（核心实体类型未完全缺失）——说明"骨架正确，细节/范式不符"。主要差距集中在：flowchart 边数量 2.5–3× 差、节点数普遍差 1、class/er 关系线样式与基数文本缺失。

---

## 3. 障碍点分级

### P0 —— 必须解决，否则无法谈"视觉还原"
1. **文本排版引擎切换**：官方用 `<foreignObject>`+HTML，liemermaid 用 `<text>`+parley。两套管线差异巨大，直接决定文字观感（换行、字体、上下标、emoji、多语言）。需决策：a) liemermaid 也改 `<foreignObject>`（与官方一致，但丧失纯 SVG 矢量性、PNG 后端需另处理）；b) 保持 `<text>` 但补齐 baseline/字间距/换行对齐（接近但非完全一致）。
2. **布局几何对齐 dagre**：有 subgraph 的 flowchart 当前**完全绕过自研 Sugiyama**，走手写 `compute_positions`+`route_edges`，结果明显偏离官方。self-loop/回边/子图约束处理为简化版。

### P1 —— 高优先，决定"像不像"
3. **边 path 化 + marker 体系**：把 `polyline`/`line` 边改为 `<path>`（正交折线 d=），箭头改 `<defs><marker>` 复用（当前每箭头独立 `<path>` 导致元素数膨胀且缩放行为不同）。
4. **节点 anchor circle**：每个节点补一个透明 anchor（或至少在 svgdiff 口径上对齐，避免"节点数差 1"误报）。
5. **class/er 关系线 path 化 + UML 标记**：class 继承/组合/聚合用 polygon/path 三角/菱形头（官方已有雏形，需与 path 边统一）；er 补充 `kind` 字段区分关系种类。
6. **SVG 分组 + `<style>` 体系**：`lievisual` 的 `SvgRenderer` 需输出 `<g class="node"/>edgePaths/edgeLabels/cluster>` 嵌套 + 内联 `<style>`，而非扁平属性。

### P2 —— 打磨，决定"还原度上限"
7. 节点形状近似修正（圆/药丸用 `<ellipse>`/`<rect rx>` 而非贝塞尔近似）。
8. 边标签白底框、圆角、内边距与官方一致。
9. 坐标网格吸附，消除亚像素偏移。
10. 主题色板对齐官方默认 theme（`#ECECFF` fill / `#9370DB` stroke 等）。

---

## 4. 重构方案（分阶段）

### 阶段 0：决策锚点（0.5 周）
- **文本策略决策**（P0-1）：选 `<foreignObject>` 路线 vs `<text>` 增强路线。建议：默认 `<text>` 增强（保矢量/PNG 一致），对富文本/emoji 再局部用 foreignObject；或提供 `render` 配置开关。
- **跨 crate 协作约定**：明确 liemermaid 的 `Scene` IR 需要新增哪些语义字段（如 `anchor`、`marker_ref`、`style_class`、`foreign_object_text`）供 `lievisual` 序列化。

### 阶段 1：SVG 后端语义化（lievisual，`svg.rs`）—— 2~3 周
- `SvgRenderer` 支持：嵌套 `<g class=...>` 分组、内联 `<style>` 模板、`<defs><marker>` 复用、可选 `<foreignObject>` 文本通道。
- 新增 `Scene`→SVG 的语义映射：`Element` 增加 `css_class` / `marker` / `anchor` 元数据。
- 配套：PNG 后端（vello）保持 `<text>` 路径，foreignObject 在 PNG 走降级（用已有 parley 排版），保证双后端一致。

### 阶段 2：边与标记统一（liemermaid builder）—— 2~3 周
- flowchart/state/sequence 的边：polyline→path，箭头→marker。
- class/er 关系线：line→path + 三角/菱形 marker，er 补 `kind` 解析与样式。
- 全图统一锚点 circle 输出。

### 阶段 3：布局对齐 dagre（liemermaid layout）—— 3~4 周（最高风险）
- 让 **有 subgraph 的 flowchart 也走统一 Sugiyama**（或引入 dagre 作为可选后端），消除两条路径分叉。
- 校准 rankdir、回边、self-loop 与官方坐标分布。
- 若自研难达精度，评估引入 `dagre` Rust 绑定（需新增依赖，权衡离线/体积）。

### 阶段 4：视觉打磨 + 验收收敛 —— 1~2 周
- 主题色板、圆角、边标签框、font-family 对齐。
- 用 §7 的验收闭环把 48 case 的 diff 逐个收敛。

**累计估算：约 8.5~12.5 人周**（单开发者；含联调与回归）。其中阶段 3 风险最高、弹性最大。

---

## 5. 难度评估矩阵

| 模块 | 改动位置 | 难度 | 风险 | 说明 |
|---|---|---|---|---|
| 文本排版 | liemermaid `vir.rs`/lievisual `svg.rs` | 高 | 高 | 影响 PNG 后端一致性，决策敏感 |
| SVG 分组/style | lievisual `svg.rs` | 中 | 低 | 纯序列化层改动，可逆 |
| 边 path 化+marker | liemermaid `builder/*` | 中 | 中 | 各图逐个改，需回归 |
| class/er 关系线 | liemermaid `class.rs`/`er.rs`/`ast.rs` | 中 | 中 | er 需扩 AST（`kind`） |
| 布局对齐 dagre | liemermaid `layout/*` | 高 | 高 | subgraph 路径分叉，可能需引入依赖 |
| 形状/样式打磨 | lievisual + builder | 低 | 低 | 机械对齐 |

**总体难度：中高。** 架构已分层（IR+后端），降低了"牵一发动全身"的风险；但文本引擎与布局对齐两块是硬骨头，且跨两个 crate 协作。

---

## 6. 风险与取舍

- **R1 跨 crate 改动**：SVG 序列化在 `lievisual`，重构需同步改该 crate 及所有调用方（含 PNG 路径）。建议先在 `Scene` IR 加可选元数据字段，保持向后兼容。
- **R2 PNG 一致性**：若 SVG 改 `<foreignObject>`，vello/PNG 后端需降级策略，否则出现 SVG/PNG 观感分裂。
- **R3 布局精度**：自研 Sugiyama 即使四阶段对标 dagre，坐标仍不会逐点相等；"视觉还原"靠相对拓扑+样式，不靠坐标相等。需管理预期。
- **R4 投入产出**：若业务只需"能看、结构对"，路线 A 的高投入可能不划算；建议先确认需求是"像素级一致"还是"可接受近似"。
- **R5 官方版本漂移**：mermaid 版本升级会改变 SVG 范式，golden 需锁定版本（现有 `generate_golden.js` 已隐含此约束）。

---

## 7. 验收闭合（复用现有基建）

现有测试基建已能支撑路线 A 的增量验收，无需新建：

1. **人工对比**：`cargo test --test html_report_test -- --nocapture` → 打开 `tests/golden/report.html`，逐 case 三栏（源码/liemermaid/官方）肉眼核对。
2. **自动 diff 门槛**：`tests/official_compare_test.rs` 的 `official_semantic_compare` 已打印每个 case 的 `SEMANTICS`/`SVGDIFF` 差异；可逐步把"报告式"差异收紧为硬断言。
3. **结构回归护栏**：已有 `STRUCT REGRESSION` 门槛（核心实体类型不得完全缺失），重构期间必须保持绿色。
4. **自比防回归**：`liemermaid_self_golden` 防止重构意外破坏现有输出。

**阶段完成判据示例**：
- 阶段 1 完成：所有 case SVG 含 `<g class="node"/"edgePaths">` 分组与 `<style>` 块，html_report 肉眼可见 DOM 结构对齐。
- 阶段 2 完成：flowchart 边元素数差距从 3× 收敛到 <1.2×，箭头形态与官方一致。
- 阶段 3 完成：有 subgraph 的 flowchart 布局拓扑与官方一致（svgdiff 节点/边计数差 <10%）。
- 路线 A 总体完成：48 case 中 ≥90% 在 html_report 中"肉眼难分"，其余为已知微小偏差（字体/亚像素）。

---

## 8. 结论

路线 A **技术可行**，架构分层使其可控，但**成本不低（约 2~3 个月单人力）且集中在"文本引擎"与"布局对齐"两块硬骨头**。

- 若目标是"结构化正确、可嵌入文档"，当前实现已达标，路线 A 优先级可后置。
- 若目标是"与官方渲染无感替换/换肤/主题兼容"，路线 A 必要，建议从阶段 0 的决策（尤其文本策略）开始，并以 §7 的 html_report 作为每日验收抓手。

**建议下一步**：先落地阶段 0 的两个决策（文本策略、跨 crate 字段约定），再用 html_report 跑一轮"当前差距分类清单"，把 48 case 的 diff 按 P0/P1/P2 标注，作为阶段 1/2/3 的工单池。
