# 视觉快照测试（Visual Snapshot Testing）

本目录实现"与官方 mermaid 布局对齐"的核心测试体系。

## 原理

| 组件 | 说明 |
|---|---|
| `cases/catalog.json` | 用例清单：每个用例指向一段 Mermaid 源码 + 画布尺寸 |
| `cases/*.mmd` | 各用例的 Mermaid 源码（经典 + 边界场景） |
| `golden/*.svg` | **黄金样本**：官方 mermaid-cli 渲染的"标准答案"（**本地生成，不提交**） |
| `generate_golden.js` | 调用官方 `@mermaid-js/mermaid-cli` 生成黄金样本 |
| `official_compare_test.rs` | 对拍主入口：语义/文本比对 + 结构差异报告 + 硬门槛 |
| `svgdiff.rs` / `semantics.rs` | SVG DOM 结构 / 语义抽取工具（供上述测试复用） |
| `html_report_test.rs` | 生成 `report.html` 人工目检报告 |
| `catalog_cases_render_test.rs` | 冒烟测试：每个用例源码对 liemermaid 可解析可渲染 |

对比策略（而非逐字节 diff）：

1. 解析官方 SVG 与 liemermaid SVG 的 DOM 树
2. 抽取文本集合 / 几何元素 / 节点中心 / 边端点
3. 门槛分两级：
   - **硬门槛 1：结构不丢失**——官方有节点/边几何，liemermaid 不能完全没有
   - **硬门槛 2：文本不丢失**——官方出现的每一段文本都必须出现在 liemermaid 输出中
     （空白折叠后比较，专门捕获标签/成员/语句被静默丢弃）
   - **软报告**：语义类型计数、坐标包围盒差异（两套渲染引擎天然不同，仅打印不失败）
4. 输出逐用例差异报告，可定位布局算法对齐问题

## 当前覆盖（54 个输入用例）

| 类型 | 数量 | 说明 |
|---|---|---|
| flowchart | 30 | 链/菱形/环/交叉/二叉树/扇入/扇出/长链/自环/三叉/边类型/边标签/四种方向/长边/稠密/双环/子图/形状 |
| sequence | 6 | 基础消息/激活/三方/笔记/循环/分支 |
| class | 4 | 继承/字段方法/多种关系/基数 |
| state | 4 | 基础/带标签/复合/分叉汇合 |
| er | 3 | 基础/基数/属性 |
| pie | 3 | 基础/showData/小型 |
| timeline | 2 | 基础/多事件 |
| gitgraph | 2 | 基础/属性 |

`cases/*.mmd` + `catalog.json` 是提交到仓库的**输入**；
`golden/*.svg` 是开发者本地运行 `generate_golden.js` 生成的**产物**，
不提交（不同 mermaid 版本/字体会产生差异，且体积较大）。

全部 54 个用例 liemermaid 都能解析渲染（`liemermaid: false` 已清零）。
本地没有 `golden/` 时，`official_compare_test` 会打印提示并跳过官方对比；
`catalog_cases_render_test` 仍然照常运行。

### catalog 标记语义

| 字段 | 含义 |
|---|---|
| `liemermaid: false` | liemermaid 无法解析/渲染该语法；冒烟测试跳过。当前没有用例使用该标记。 |
| `compare: false` | liemermaid 能渲染，但文本/结构对拍暂不适用（如子图容器框、note 布局、ER 属性块、git 分支）。仅做冒烟渲染；本地生成 golden 后也不参与对拍。 |
| 缺省 | 参与完整对拍（含文本硬门槛）。 |

当前 `compare: false` 的 5 个用例：`flowchart__subgraph`、`sequence__notes`、
`er__attributes`、`gitgraph__basic`、`gitgraph__attributes`。

## 黄金样本的生成源

- **mermaid-cli**：`generate_golden.js` 用官方 `@mermaid-js/mermaid-cli`
  渲染出标准 SVG（含真实文本测宽）。需要 `npm` 与网络；检测到系统 Chromium
  （如 `pacman -S chromium`）时自动通过 `PUPPETEER_EXECUTABLE_PATH` 使用它，
  避免下载 puppeteer 自带浏览器。
- 生成目录 `golden/` 已被 `.gitignore` 忽略，请勿提交。需要官方对比时在本地生成，
  CI 只跑不依赖 golden 的冒烟/单元测试。

## 运行

```bash
# 一键：生成黄金样本 + 跑测试
bash tests/golden/regenerate.sh

# 或手动
cd tests/golden
node generate_golden.js              # 官方模式（需 npm + Chromium）
cd ../..
cargo test --test official_compare_test     # 对拍（文本硬门槛 + 结构报告）
cargo test --test catalog_cases_render_test # 冒烟：用例可解析可渲染
cargo test --test html_report_test -- --nocapture  # 生成 report.html
```

## 新增用例

1. 在 `cases/` 新建 `{type}__{name}.mmd`，写入 Mermaid 源码
2. 在 `cases/catalog.json` 的 `cases` 数组追加一条记录（含 `rankdir` / 画布尺寸）
   - 若对拍暂不适用，加 `"compare": false`（仍参与冒烟渲染）
3. 生成官方参考 SVG（见上）
4. 跑 `official_compare_test` 观察差异报告

## 已知差异 / 说明

- **同层左右顺序**：自研 Sugiyama（Barycenter 初始序）与官方 mermaid 可能左右互换，
  属视觉级差异；文本门槛不受影响，坐标差异仅在报告中打印。
- **节点尺寸**：官方 mermaid 用真实文本测宽，liemermaid 用同一套 lievisual 文本测量，
  但字体/主题仍有细微差异；尺寸与坐标差异不参与硬断言。
- **语义类型计数**：官方 SVG 用丰富 `class` 标注图元，liemermaid 输出几乎无 `class`，
  因此 `semantics::compare` 的类型计数差异恒存在，仅作报告用途。
- **非 flowchart 类型**：sequence/class/state/er/pie/timeline/gitgraph 使用不同布局引擎，
  SVG 结构不同，但对拍仍会执行文本硬门槛。

## 已验证的对拍结果（官方 mermaid-cli 模式）

- 49 个 `compare != false` 的用例全部通过**文本硬门槛**与**结构不丢失门槛**。
- 语义类型计数 / 坐标包围盒差异仍会打印，属于两个渲染引擎的结构性差异。
