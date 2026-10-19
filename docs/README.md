# 文档索引

本目录只保留**与当前实现一致**的文档。历史设计稿、已被取代的方案与阶段性评估
放在 [`archive/`](archive/)，仅供追溯，不代表当前行为。

## 当前文档

| 文档 | 内容 |
|---|---|
| [architecture.md](architecture.md) | 当前解析 / 布局 / 渲染管线与 IR 分层（先读这篇） |
| [mermaid-syntax-index.md](mermaid-syntax-index.md) | Mermaid 官方语法资料索引 |
| [winnow_api.md](winnow_api.md) | winnow 1.0.4 常用 API 速查（解析器实现参考） |

### 各图表语法参考

| 图表 | 文档 |
|---|---|
| flowchart / graph | [syntax-flowchart.md](syntax-flowchart.md) |
| sequenceDiagram | [syntax-sequence.md](syntax-sequence.md) |
| classDiagram | [syntax-class.md](syntax-class.md) |
| stateDiagram | [syntax-state.md](syntax-state.md) |
| erDiagram | [syntax-er.md](syntax-er.md) |
| pie | [syntax-pie.md](syntax-pie.md) |
| timeline | [syntax-timeline.md](syntax-timeline.md) |
| gitGraph | [syntax-gitgraph.md](syntax-gitgraph.md) |

## 归档文档

[`archive/`](archive/) 收录 2026-05 ~ 2026-09 期间的布局/解析重构设计稿与评估报告，
包括 Pest 时代的 `parser.md`、多版 `layout-*` / `redesign-*` 方案、SVG 对齐报告与
迁移计划。这些文档描述的设计**已被当前实现取代**，请勿据此修改代码。
