# 归档文档

本目录保存 2026-05 ~ 2026-09 期间的历史设计稿与评估报告，**已过时**，仅用于追溯
设计演进。当前实现请阅读 [`../architecture.md`](../architecture.md) 与
[`../README.md`](../README.md)（语法参考索引）。

| 文档 | 说明 | 被取代原因 |
|---|---|---|
| `parser.md` | 基于 Pest 的解析器设计 | 已改为 winnow 手写组合子 |
| `layout.md` | 启发式多遍布局草案 | 已落地为 extract/measure/layout 三阶段 |
| `layout-edge-aware-design.md` | 边感知布局草案 | 已由路由层与容器避让实现取代 |
| `layout-refactor.md` | 布局分层解耦笔记 | 方案已实施 |
| `layout-system-design.md` | 统一布局系统设计稿 | 已实施并演化 |
| `refactor-layout.md` | 多图表布局 IR 设计 | 已由 UG/GG/SG 三层取代 |
| `redesign-from-scratch.md` | 布局/渲染层重写方案 | 已实施 |
| `redesign-task-plan.md` | 上述方案的任务计划 | 已完成 |
| `post-refactor-review.md` | 2026-08-27 重构后评估 | 阶段性报告 |
| `svg_alignment_report.md` | 2026-08-29 SVG 对齐报告 | 一次性数据快照 |
| `svg_alignment_report-qwen.md` | SVG 差异分析 | 一次性数据快照 |
| `新管线迁移计划.md` | 全部图类型迁移进度 | 迁移已完成 |
| `路线A-视觉还原-可行性分析.md` | 视觉还原路线评估 | 阶段性决策文档 |
