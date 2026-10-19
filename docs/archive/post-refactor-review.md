# 重构后视觉与设计评估（2026-08-27）

> 状态：评估稿（重构阶段性总结 + 决策建议）
> 适用范围：liemermaid 新四阶段管线（extract→measure→layout→materialize→paint）
> 关联文档：`redesign-from-scratch.md`、`redesign-task-plan.md`、`路线A-视觉还原-可行性分析.md`

---

## 1. 背景与目标

新管线（`extract → measure → layout → materialize → paint`）按 `redesign-from-scratch.md` 的设计原则落地：
- 三层 IR 分离（UG → GG → SG），单向依赖
- 新增 `ir/{common,shape,unigraph,geograph,scenegraph}`，配套 `measure/materialize/paint`
- `flowchart` 与 `state` 切到新管线（其余图类型降级到旧管线）
- 测试体系重构（pipeline_smoke / shape_kind_coverage / layout_quality / official_compare）

**本评估的目的**：在用户多次反馈边视觉、节点布局、端口分散等问题后，回顾"打补丁式迭代"累积的系统性问题，决定是**继续打补丁**还是**重新设计**。

---

## 2. 已完成的关键改动（累计）

| 阶段 | 改动 | 行数（diff） |
|---|---|---|
| 数据结构 | `ir/common.rs` 新增 `LineKind`；`UGEdge/GGEdge` 加 `line_kind`；`SceneItem::Edge` ends 改二元组 `(start, end)` | +29 |
| extract | flowchart 新增 `map_line` + 合并 subgraph 节点 shape/text；state 新增完整 extract | +101 |
| layout/directed | 分层 `assign_layers` 加重构图反向边（back edge）检测 + 层号截断 | +40 |
| layout/engine | 新增 `EDGE_LABEL_GAP`、层间标签预留；`GGContainer` bounds 计算（修 kurbo Rect 参数顺序 bug） | +97 |
| layout/route | 新增 `Slot` + `port_point_at`（节点边端口按槽位分散）；`orthogonal_route` 接收槽位；stub 偏移；avoid_nodes | +197 |
| materialize | 节点双环/实心圆/横条；容器渲染；边标签白底；`ends` 二元组；按 `line_kind` 设 dash/宽度；`edge_style` | +276 |
| paint | `run_edge_nodes` 用 `(start, end)` 二元组，只画对应端箭头；`bezier_from_route` 多次迭代（圆角→Catmull-Rom→单段大弧度→端口直段+短弧→最终回退到单段大弧度） | +158 |
| measure | shape 几何约束（圆方形、菱形乘数、Bar/Fixed SizeHint） | +189 |
| 测试 | shape_kind_coverage 加双环断言；pipeline_test 改 `<path>`；official_compare 加无边图豁免 | +50 |

合计：**1000+ 行** diff，覆盖 13 个核心文件。

---

## 3. 当前已知问题（用户反馈）

### 3.1 视觉层面（最直观）

| # | 问题 | 截图位置 | 根因 |
|---|---|---|---|
| 1 | **连线不够美观** | `flowchart__cycle` B↔C back edge、A→B 弧线方向 | 路由仍是**正交折线**（`orthogonal_route`），paint 把折线强行画成大弧度贝塞尔，导致：
- **节点小**时弧线方向"歪"（控制点沿端口主轴方向，但路由首段斜了导致主轴错）
- **back edge**（如 B↔C）路由是 S 形折线，画成贝塞尔后呈"之字形诡异曲线"，不是官方那种**弧线绕开中间节点**
 |
| 2 | **连线重叠** | cycle 的 B→C 和 C→B 两条边路由几乎重合 | 端口分散**只分散槽位偏移**（沿边 ±spacing），但**back edge 跨层**时两端节点的入口/出口都在同一侧（如 B 右边和 C 左边），两条边路由完全重合 + 视觉重叠 |
| 3 | **箭头不完整** | 用户报告"箭头看不完整" | paint 的 `run_edge_nodes` 在贝塞尔末端**沿首段方向**画箭头，但贝塞尔切向在末端是**端点切线方向**（`end - c2` 的反向），与最后一段方向**不一致**。箭头方向错位或被贝塞尔遮盖 |
| 4 | **节点间隔太小** | 所有图，节点挤在一起 | `LAYER_GAP = 40`、`NODE_GAP = 40` 是 2026-05 拍板的常量，未考虑 back edge 半径、边标签宽度、长跨度需求 |

### 3.2 架构层面（更深层）

| # | 问题 | 描述 |
|---|---|---|
| A | **paint 的 bezier 反复迭代**：圆角折线 → Catmull-Rom（弱）→ 单段大弧度 → 端口直段+短弧 → 回退到单段大弧度 | 每个版本的视觉问题不同，且都没完美。说明**用 paint 修补路由缺陷**是错的方向。根因是**路由骨架本身就不对**（正交折线，不适合直接画弧） |
| B | **路由端口分散是"槽位偏移"**，不是真正的"侧边选择" | 同一节点多条出边**仍可能用同一端口边**（如 Bottom），仅在 Bottom 边上分散 x 偏移。如果某条边的目标在左上方，会强制走 Bottom 出口再绕远路 |
| C | **back edge 没有专用路由** | `detect_back_edges` 只在分层时跳过，回路由普通 `orthogonal_route` 处理，画出 S 形折线。官方 back edge 用**专门曲线**（绕开中间节点的弧线） |
| D | **构图反向边检测是"成对"启发式** | 只检测 `u→v` 和 `v→u` 同时存在的情况。更长的环（A→B→C→A）检测不到，但这种图 Sugiyama 应做 **SCC 收缩** |
| E | **Materialize 里 `ends` 二元组 + paint 的"只画对应端"** 是一个**补丁**——本质是 `ArrowSpec` 信息应该在路由时已确定起止端口，不需要 paint 再判断 | 边界清晰的修复，但暴露了"边数据在多阶段流转"的耦合 |
| F | **边标签预留（`EDGE_LABEL_GAP`）只对相邻层边** | 跨多层的边标签没有专门的"中段预留"，可能被节点遮挡 |

---

## 4. 原设计的合理性评审

`redesign-from-scratch.md` 的**核心原则**仍然正确：

✅ **三层 IR 分离**：UG/GG/SG 各管一段，无回查，方向单向
✅ **测量在 Layout 前**：节点尺寸提前，路由能预留边标签空间
✅ **Materialize 零 AST 引用**：纯几何 + 样式意图 → 视觉原语
✅ **Paint 纯机械翻译**：无图类型判断、无主题硬编码
✅ **测试分层**：micro/structural/semantic 三层

但**实现策略**有两点未达设计要求：

⚠️ **"边感知布局"（`layout-edge-aware-design.md`）未落地**
   - 当前 `route.rs` 只做节点回避，**没有边-边排斥**（两条同向长边可能完全重合）
   - `crossing.rs` 的 barycenter 只对相邻层有效，**没有全局迭代**
   - back edge 没有专属路由（如官方 dagre 的 `backEdges: [{ sx, ex, sy, ey }]`）

⚠️ **"边弧度样式"（`routing_hint: Spline/Curved`）未实现**
   - 设计文档说 `UGEdge.routing_hint` 决定 Orthogonal/Spline/Curved
   - 当前所有边都走 `orthogonal_route`，paint 再"翻译"为贝塞尔——这是**逆向工程**，不是按 `routing_hint` 分流

---

## 5. 决策：继续打补丁 vs 重新设计

### 5.1 选项 A：继续打补丁（迭代式改进）

**做法**：在现有管线基础上修修补补：
- paint 的 bezier 加切向修正（让箭头方向正确）
- LAYER_GAP/NODE_GAP 增大
- 端口分散从"槽位偏移"升级为"按出边方向智能选边"
- back edge 路由加"绕行弧线"

**优点**：
- 改动可控（每处几十行）
- 不破坏现有测试
- 用户能逐步看到改进

**缺点**：
- **不解决根因**：路由产出正交折线，paint 强行画弧线，本末倒置
- **累积技术债**：每个补丁都让代码更难懂（已出现 paint 的 bezier 多次迭代）
- **架构偏离**：原设计 `routing_hint: Orthogonal/Spline/Curved` 是要在**路由阶段**分流，不是 paint 后处理
- **下一次用户反馈大概率还在**：端口分散+back edge+弧线方向是路由问题，不是 paint 问题

### 5.2 选项 B：重新设计边路由层（推荐）

**核心思路**：承认"路由决定边的形状"——路由阶段就该输出**弧线骨架**（贝塞尔控制点序列），paint 只做直线化或描边。

具体：

1. **`routing_hint` 三档路由分流**
   - `Orthogonal`（默认）：保持当前 `orthogonal_route` 输出正交折线，paint 描折线
   - `Spline`（flowchart 默认升级）：路由输出**三次贝塞尔控制点**（P0-P1-P2-P3），paint 描平滑曲线
   - `Curved`（back edge 默认）：路由输出**绕行大弧度**控制点（弧线绕开中间节点）

2. **Spline 路由算法**
   - 类似 dagre 的"立方贝塞尔"：从源节点出口直段 → 单段三次贝塞尔 → 目标节点入口直段
   - 控制点沿源/目标端口主轴方向延伸，弧度与跨层距离成比例
   - **不再依赖正交折线骨架**

3. **Back edge 路由**
   - 检测 back edge（构图反向或分层后 source.layer > target.layer）
   - 用**专门曲线**：从源右/左边出，控制点拉到中间空白区域，绕开中间节点
   - 官方 mermaid 的 back edge 就是这种"U 形大弧线"

4. **边-边排斥（routeseparation）**
   - 同向长边（共享通道）做**通道分配**（channel routing）
   - 至少做到**同一对源-目标之间的多条边平行分开**（解决 cycle 的 B↔C 重叠）

5. **路由结构调整**
   - `Route` IR 不再是 `Vec<Point>` 折线点，而是 **`Vec<BezSegment>`**（贝塞尔段序列：line/curve）
   - Materialize/Paint 消费 `Route` 段类型决定画直线/曲线

### 5.3 决策建议：选项 B

**理由**：
1. 当前架构（管线、四阶段、单向依赖）是对的，**不应推倒重来**——那会丢掉所有测试与已经稳定的 state/flowchart 实现
2. **问题集中在路由层**：paint 反复迭代 bezier 暴露了"路由应该已经决定形状"的根因
3. **修一处治一片**：把 `Route` 从折线升级为贝塞尔段，`routing_hint` 真正分流，能同时解决：弧线方向、重叠、back edge 路由、箭头方向（贝塞尔端切线 = 箭头方向，天然对齐）
4. **不破坏现有状态**：`orthogonal_route` 保留为默认分支（`routing_hint=Orthogonal`），`Spline` 是新分支，可逐步推广

---

## 6. 推荐的实施路径（选项 B 落地）

### 阶段 1：路由 IR 升级（小步）
- 改 `GGEdge.route: Vec<Point>` → `route: Vec<RouteSegment>`
- `RouteSegment::{ Line(Point, Point), CubicBezier(P0,P1,P2,P3) }`
- paint 用 match 决定画 `<line>`/`<path d="M..C..">`
- 所有现有测试断言改 `Vec<Point>` → `Vec<RouteSegment>`（机械修改）

### 阶段 2：Spline 路由实现
- `route_spline(s, t)`：单段三次贝塞尔，控制点沿端口主轴方向延伸
- `routing_hint` 分流：默认 `Spline`（flowchart），state 用 `Spline`
- 老的 `orthogonal_route` 改为 `routing_hint=Orthogonal` 的分支（备用）

### 阶段 3：Back edge 路由
- `route_back_edge(s, t)`：检测 source.layer > target.layer，输出大弧度曲线绕开中间节点
- 类似官方 dagre 的 `backEdges` 处理

### 阶段 4：边-边排斥（可选）
- 同向同源同目标的边做"槽道"分配（channel routing）
- 解决 B↔C 重叠问题

### 阶段 5：间距与视觉打磨
- LAYER_GAP/NODE_GAP 改为按内容（边标签宽度、back edge 弧度）动态
- 端口分散从"槽位"升级为"侧边智能选择"（如果目标在左上方，选 Left 出口而非 Bottom）

---

## 7. 立即可做的快速改进（在选项 B 落地前）

如果短期看不到选项 B 落地，可做的低成本补丁：

1. **修箭头方向**：paint 在贝塞尔末端用 `end - c2` 的归一化方向（端切线）而非 `last - prev`，让箭头与曲线切向对齐
2. **增大间距**：`LAYER_GAP` 60、`NODE_GAP` 50（视觉明显改善）
3. **back edge 视觉区分**：检测 source.layer > target.layer 后，渲染用**双弧线**（左右分开）+ 偏移端口槽位

---

## 8. 结论

- **架构正确**：四阶段管线原则保留并验证有效
- **问题集中在路由层**：paint 的 bezier 反复迭代是"症状"，路由才是"病因"
- **不要重写管线**：投入太大，收益不高（state/flowchart 已工作）
- **要做局部重设计**：把路由层从"折线"升级到"贝塞尔段"，让 `routing_hint` 真正分流
- **打补丁可以短期止血**，但路由层重设计才是根治