# 官方布局对拍（dagre reference）

用 `@dagrejs/dagre`（mermaid 的 flowchart 布局引擎）生成"官方"布局 fixture，
与 liemermaid 的 Sugiyama 布局做端到端结构化对拍。

## 原理

mermaid flowchart 的布局由 dagre 负责。我们把同一组图分别交给：
- **dagre**（`run.js`）：输出每个节点的中心坐标 `(x,y)` + 尺寸 `(w,h)`、每条边的端口折线点
- **liemermaid**（`official_compare_test.rs`）：调用 `SugiyamaLayout::layout` 得到 `SugiyamaResult`

两边做三层对比：
1. **拓扑偏序（硬）**：沿每条非回边 `u->v`，双方都必须满足 `rank(u) < rank(v)`。
   验证层序正确性，容忍环（双向边）导致的绝对层号差异。
2. **同层 Y 对齐（硬）**：同 rank 节点中心 y 差 `< 1.0`。
3. **归一化坐标（软）**：包围盒归一到 `[0,1]`，同层集合匹配后比较节点中心距离 `< 0.06`。
   容忍同层节点左右顺序（Barycenter 初始序）差异；BT/RL 方向已做镜像对齐。

边的逐点形状**不做强对比**：dagre 输出端口交点，liemermaid 输出中心路由点，语义不同。

## 运行

```bash
# 1. 生成 fixture（需先 npm install @dagrejs/dagre）
cd tests/dagre_ref && node run.js        # 生成 layouts.json

# 2. 跑 Rust 对拍测试
cd ../.. && cargo test --test official_compare_test
```

或一键：

```bash
bash tests/dagre_ref/regenerate.sh
```

## 已知差异

- **环处理（已对齐）**：liemermaid 的 network-simplex 现已反转反馈弧后在原图节点上运行
  （不再凝结 SCC），环内节点可分到不同层（如 `B↔C` 给 B:1,C:2），与 dagre 一致。
- **同层左右顺序**：Barycenter 初始顶点序不同，导致同层节点左右互换，视觉级差异，软对比如实报告。
- **classDiagram**：走独立的 `class.rs` 布局（mermaid 用 elkjs），不在本对拍范围。
  其 2 个既有测试（`class_relations_no_edge_crosses` / `regression_edges_avoid_nodes`）为历史问题，与本次无关。

## 新增 case

编辑 `run.js` 的 `cases.push(runCase(...))`，重跑 `node run.js` 即可。
