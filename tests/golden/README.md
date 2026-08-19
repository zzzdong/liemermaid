# 视觉快照测试（Visual Snapshot Testing）

本目录实现"与官方 mermaid 布局对齐"的核心测试体系。

## 原理

| 组件 | 说明 |
|---|---|
| `cases/catalog.json` | 用例清单：每个用例指向一段 Mermaid 源码 + 画布尺寸 |
| `cases/*.mmd` | 各用例的 Mermaid 源码（经典 + 边界场景） |
| `golden/*.svg` | **黄金样本**：官方 mermaid-cli 渲染的"标准答案" |
| `generate_golden.js` | 调用官方 `@mermaid-js/mermaid-cli` 生成黄金样本 |
| `generate_golden_from_dagre.js` | 回退生成器：从 dagre 布局数据生成黄金样本（无需网络） |
| `golden_snapshot_test.rs` | 用 liemermaid 渲染同一源码，与黄金样本做**结构化对比** |
| `catalog_cases_render_test.rs` | 冒烟测试：每个用例源码对 liemermaid 可解析可渲染 |

对比策略采用**结构化对比**（而非逐字节 diff）：

1. 解析官方 SVG 与 liemermaid SVG 的 DOM 树
2. 提取每个节点的矩形（位置 + 尺寸）
3. 做三层对比：
   - **硬断言 1：节点数量一致**（错一个即失败）
   - **硬断言 2：同层节点数一致**（拓扑层序正确性）
   - **软断言：归一化坐标距离 < 容差** + **节点尺寸相对容差**
4. 输出逐用例差异报告，可定位布局算法对齐问题

## 当前覆盖（48 个用例，全部有官方参考 SVG）

| 类型 | 数量 | 说明 |
|---|---|---|
| flowchart | 25 | 链/菱形/环/交叉/二叉树/扇入/扇出/长链/自环/三叉/边类型/边标签/四种方向/长边/稠密/双环/子图/形状 |
| sequence | 5 | 基础消息/激活/三方/笔记/循环 |
| class | 4 | 继承/字段方法/多种关系/基数 |
| state | 4 | 基础/带标签/复合/分叉汇合 |
| er | 3 | 基础/基数/属性 |
| pie | 3 | 基础/showData/小型 |
| timeline | 2 | 基础/多事件 |
| gitgraph | 2 | 基础/属性 |

每个用例的**官方参考 SVG** 均已通过 mermaid-cli 生成并保存在 `golden/`，
与 `cases/*.mmd` 一一对应（48 个源码 = 48 个官方 SVG），**不依赖 liemermaid 是否支持**。

### catalog 标记语义

| 字段 | 含义 |
|---|---|
| `liemermaid: false` | liemermaid 当前尚无法解析/渲染该语法（如跨层长边触发渲染 bug、`Note`、复合状态、`<<fork>>` 等）。冒烟测试跳过，官方参考 SVG 仍保留。 |
| `compare: false` | liemermaid 能渲染，但结构化对拍解析器尚未覆盖该场景（如子图容器框、曲线形状）。快照测试跳过对拍，官方参考 SVG 仍保留。 |
| 缺省 | 两者均支持，参与完整对拍。 |

策略：**先为所有输入生成官方标准 SVG，再逐步扩展 liemermaid 实现与对拍解析器**，逐步把 `liemermaid: false` / `compare: false` 的用例纳入覆盖。

## 黄金样本的生成源

- **首选（mermaid-cli）**：`generate_golden.js` 用官方 `@mermaid-js/mermaid-cli`
  渲染出标准 SVG（含真实文本测宽）。需要 `npm` 与网络；检测到系统 Chromium
  （如 `pacman -S chromium`）时自动通过 `PUPPETEER_EXECUTABLE_PATH` 使用它，
  避免下载 puppeteer 自带浏览器。
- **回退（dagre）**：`generate_golden_from_dagre.js` 读取
  `tests/dagre_ref/layouts.json`，生成与官方 SVG 同构的节点结构。仅覆盖 flowchart。

## 运行

```bash
# 一键：生成黄金样本 + 跑测试（mermaid-cli 失败则回退 dagre）
bash tests/golden/regenerate.sh

# 或手动
cd tests/golden
node generate_golden.js              # 官方模式（需 npm + Chromium）
node generate_golden_from_dagre.js   # 回退模式（仅 flowchart，无需 npm）
cd ../..
cargo test --test golden_snapshot_test       # 结构化对拍（当前仅 flowchart）
cargo test --test catalog_cases_render_test  # 冒烟：用例可解析可渲染
```

## 新增用例

1. 在 `cases/` 新建 `{type}__{name}.mmd`，写入 Mermaid 源码
2. 在 `cases/catalog.json` 的 `cases` 数组追加一条记录（含 `rankdir` / 画布尺寸）
   - 若 liemermaid 暂不支持该语法，加 `"liemermaid": false`（官方参考 SVG 仍保留，
     冒烟测试会跳过）
3. 生成官方参考 SVG（见上）
4. 跑 Rust 测试观察对拍结果

## 已知差异 / 说明

- **同层左右顺序**：dagre（Barycenter 初始序）与 liemermaid 可能左右互换，属视觉级
  差异。测试按"同层集合"匹配（容忍顺序），以软断言如实报告坐标距离。
  例如 `cycle` 用例报告 `soft=DIFF`，是预期的顺序差异。
- **节点尺寸**：官方 mermaid 用真实文本测宽，liemermaid 用近似宽度。测试用相对
  容差（60%）对比，尺寸不参与硬断言。
- **长边/虚拟节点（已知库限制）**：含跨层长边或长回边的用例（`long_edge`、`dense`、
  `cycle2`）会触发 liemermaid 渲染路径的 index-out-of-bounds（Sugiyama 引入虚拟
  节点后 `render_sugiyama_flowchart` 无法映射回图节点）。这些用例标记
  `"liemermaid": false`，冒烟测试跳过，**官方参考 SVG 仍已生成**，待渲染路径修复后
  改为 `liemermaid: true` 并纳入对拍。
- **非 flowchart 类型的对比**：sequence/class/state/er/pie/timeline/gitgraph 走不同
  布局引擎（elkjs 等），SVG 结构不同，`golden_snapshot_test.rs` 当前只对 flowchart
  做结构化对拍，其余类型先保留官方参考 SVG + 冒烟测试，对比解析器后续逐步实现。
- **子图/形状的对拍**：`subgraph`（容器框）、`shapes`（椭圆/曲线 path）能被 liemermaid
  渲染，但快照对拍的形状解析器尚未覆盖，标记 `"compare": false`，对拍测试跳过，
  官方参考 SVG 已生成，待解析器增强后纳入。
- **liemermaid 暂不支持的语法**：`sequence` 的 `Note`、`state` 的复合状态与
  `<<fork>>`、`er` 的多行属性块、`gitGraph` 的无方向写法——标记 `"liemermaid": false`，
  冒烟测试跳过，官方参考 SVG 仍保留。

## 已验证的对拍结果（官方 mermaid-cli 模式）

- 20 个可对拍的 flowchart 用例全部通过**硬断言**（节点数 + 层数一致、同层 Y 对齐）。
- 其中 16 个坐标+尺寸对拍通过；4 个（cycle / binary_tree / disconnected /
  self_loop）因左右顺序或自环布局差异报 `soft=DIFF`，均为预期行为。
