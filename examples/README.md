# Examples

示例按「图表类型 + 功能」命名，统一输出到 `examples/out/`（`*.svg` / `*.png` 已被
`.gitignore` 忽略）。先按类型浏览，再按功能查看。

```bash
cargo run --example flowchart_basic
cargo run --example output_formats
```

## 按图表类型

| 图表 | 示例 | 内容 |
|---|---|---|
| flowchart | `flowchart_basic` | 形状、带空格标签、判定分支、方向 |
| flowchart | `flowchart_shapes` | 全部节点形状 + 全部箭头端点类型 |
| flowchart | `flowchart_subgraph` | 嵌套子图、子图内 `direction` |
| sequence | `sequence_basic` | 参与者、别名、备注、`autonumber` |
| sequence | `sequence_blocks` | 激活条、`alt` / `loop` / `par` 分组块 |
| class | `class_basic` | 成员块、类外成员声明、注解、泛型、关系 |
| state | `state_basic` | 起止节点、转移标签、复合状态 |
| er | `er_basic` | 实体属性、关系基数、关系标签 |
| pie | `pie_basic` | `showData`、标题、图例、百分比 |
| timeline | `timeline_basic` | 标题、section、同一时间点多事件 |
| gitgraph | `gitgraph_basic` | 分支、标签、合并、cherry-pick |

## 按功能

| 功能 | 示例 | 内容 |
|---|---|---|
| 输出格式 | `output_formats` | SVG / PNG / 透明背景 |
| 手绘风格 | `output_sketch` | `render_sketch*`、固定种子、只手绘轮廓 |
| 解析 API | `api_parse_ast` | `MermaidParser::parse_mermaid` → `ast::Diagram`、错误处理 |

公共输出辅助位于 [`support/mod.rs`](support/mod.rs)（`svg` / `png` / `both` /
`svg_transparent`）。
