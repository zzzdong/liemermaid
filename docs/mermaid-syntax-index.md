# Mermaid 语法参考资料索引

本目录收录 **Mermaid 官方语法定义** 的完整参考，供 liemermaid（基于 winnow 手写组合子的 Mermaid 解析器/渲染器）实现与校验时对照使用。

资料来源：Mermaid 官方文档 <https://mermaid.js.org/syntax/>，覆盖至 v11.x。

## 图表类型与文档对照

本项目 `src/parser/`（winnow 手写组合子）当前支持以下 8 种图表类型，各语法文档如下：

| 图表类型 | 语法关键字 | 参考文档 |
| -------- | ---------- | -------- |
| 流程图 | `flowchart` / `graph` | [syntax-flowchart.md](syntax-flowchart.md) |
| 时序图 | `sequenceDiagram` | [syntax-sequence.md](syntax-sequence.md) |
| 类图 | `classDiagram` | [syntax-class.md](syntax-class.md) |
| 状态图 | `stateDiagram` / `stateDiagram-v2` | [syntax-state.md](syntax-state.md) |
| ER 图 | `erDiagram` | [syntax-er.md](syntax-er.md) |
| 饼图 | `pie` | [syntax-pie.md](syntax-pie.md) |
| 时间线图 | `timeline` | [syntax-timeline.md](syntax-timeline.md) |
| Git 图 | `gitGraph` | [syntax-gitgraph.md](syntax-gitgraph.md) |

## 通用约定

所有图表类型共同遵循以下约定：

- **声明开头**：每个图表必须以对应的语法关键字开头。
- **注释**：以 `%%` 开头，直到行尾；解析器忽略。
- **空白**：空格、Tab、换行均为空白。
- **标识符**：字母/下划线开头，可含字母、数字、下划线、连字符；支持 Unicode（需用双引号包裹时按各图规则）。
  **实现注意**：连字符需前瞻——`-` 后紧跟 `-` / `.` / `>` 时属于箭头（`-->`、`-.->`、`---`…），
  不能计入标识符，否则 `A-->B` 会被切成 `A--` + `>B` 而解析失败。见 `src/parser/common.rs::identifier`。

## 注意事项

- 官方文档更新较快，本文档标注了各语法特性引入的最低版本（如 `v11.16.0+`）。
- 部分语法为实验性功能（如 timeline），后续版本可能变更。
- 若与 `src/parser/` 现有实现不一致，以官方文档为准，再决定是否扩展语法规则。

## 相关文档

- [parser.md](parser.md) — Mermaid 解析器架构设计
- [layout.md](layout.md) — 布局引擎相关说明
- [refactor-layout.md](refactor-layout.md) — 布局重构记录
