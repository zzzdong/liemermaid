# liemermaid

Mermaid 图表的 Rust 解析与渲染库，输出统一走 [lievisual](https://crates.io/crates/lievisual) 的声明式场景 IR（`Scene`）与多后端（SVG / vello_cpu PNG）。

## 支持图表

- **Flowchart**（流程图，TD / LR / BT / RL，含循环、回边绕行、子图、边标签）
- **Sequence**（时序图，含参与者、消息箭头、激活条、备注、分组块）
- **Class**（类图，含成员、关系与基数）
- **State**（状态图，含复合状态、转换、fork/join/choice）
- **ER**（实体关系图，含属性与基数）
- **Pie**（饼图，含标题、图例、showData）
- **Gitgraph**（Git 分支图，含 commit/branch/checkout/merge/tag）
- **Timeline**（时间线，含分节与多事件）

## 快速开始

```toml
[dependencies]
liemermaid = "0.1"
```

```rust
use liemermaid::{render, render_png};

// 渲染 SVG 字符串
let svg = render(r#"flowchart TD
    A[Start]
    B[End]
    A --> B
"#, 800, 600).expect("render failed");

// 渲染 PNG 位图字节
let png: Vec<u8> = render_png(r#"pie
    "A": 30
    "B": 50
    "C": 20
"#, 600, 400).expect("render failed");
```

### 画布语义（重要）

与官方 mermaid 一致：`width` / `height` 是**上限**，不是固定画布。

- 输出的 SVG 根节点为 `width="100%"` + 贴合内容包围盒的 `viewBox`
- 内容超出上限时**等比缩小**，绝不裁切
- 内容装得下时**不放大**，画布贴合内容（只留少量边距）

需要自定义背景色时改用 [`render_with_config`] / [`render_png_with_config`]。

## 命令行

```sh
cargo install liemermaid
liemermaid -i diagram.mmd -o out.svg        # 格式由扩展名推断
liemermaid -i diagram.mmd -o out.png -W 1200 -H 800
```

## 架构

```text
Mermaid 文本 → MermaidParser → ast::Diagram
  → builder::extract     (Unigraph / UG：与渲染无关的拓扑 + 样式意图)
  → builder::measure     (UG'：文本测量，尺寸回填)
  → builder::layout      (Geograph / GG：分层 + 排布 + 边路由)
  → builder::materialize (SceneGraph：几何与样式落定)
  → builder::paint       (lievisual::Scene)
  → 画布贴合内容 → lievisual::SvgRenderer / VelloPixmapRenderer
```

各阶段只依赖上一阶段的产物，`extract` 之后不再回查 AST，保证渲染与语法解耦。

| 模块 | 职责 |
| --- | --- |
| `parser` | winnow 手写组合子，解析 Mermaid DSL |
| `builder::extract` | AST → 与渲染无关的拓扑图（Unigraph） |
| `builder::measure` | 文本测量（parley）与节点尺寸推算 |
| `builder::layout` | Sugiyama 分层、排序、坐标分配、边路由与避让 |
| `builder::materialize` | 几何 + 主题 → 视觉自足的场景图 |
| `builder::paint` | 场景图 → `lievisual::Scene`（零分支纯翻译） |
| `scene_ext` | 委托 lievisual 后端输出，并把根节点改写为官方形态 |

## 测试

```sh
cargo test
```

覆盖：解析、语法兼容性回归、布局质量（边不穿越节点 / 同层对齐 / 无重叠）、
SVG 结构、PNG 编码、病态输入不 panic，以及与官方 mermaid-cli golden 输出的结构比对
（见 `tests/golden/`）。

## 已知限制

- 与官方输出的**像素级**仍有偏差（官方用 dagre + 完整主题系统），`tests/golden/` 中
  的比对为结构性 + 报告式，逐步收敛中。
- `classDiagram` 的 `note for X "..."` 尚未支持（需 AST 与渲染同时扩展），该行会被安全跳过。
- `sequenceDiagram` 的 `alt`/`par` 分支段（`else` / `and` / `option`）目前只渲染首个分支。
- flowchart 的 `style` / `classDef` / `linkStyle` / `click` 等修饰语句会被安全跳过（不报错、不影响其余内容）。
- 中日韩文本可正常渲染，但 parley 会输出 `No segmentation model for complex script` 警告
  （缺少 CJK 断行模型），长 CJK 文本暂不做自动换行。

## License

Apache-2.0
