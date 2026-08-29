# liemermaid vs 官方 mermaid — SVG 差异分析报告

---

## 一、总体概览

| 图表类型 | 用例数 | 主要差异等级 |
|---------|--------|------------|
| flowchart | 30 | 🔴 结构性差异 |
| class | 4 | 🟡 中等差异 |
| sequence | 5 | 🔴 结构性差异 |
| state | 4 | 🟡 中等差异 |
| er | 3 | 🟡 中等差异 |
| pie | 3 | 🟢 轻微差异 |
| gitgraph | 2 | 🟡 中等差异 |
| timeline | 2 | 🔴 结构性差异 |

---

## 二、逐类分析

---

### 2.1 Flowchart（30 个用例）

#### 2.1.1 画布与视口

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 画布尺寸 | 固定 `900×700` | 根据内容精确计算 `viewBox` |
| 背景 | 透明 `<rect fill="#00000000">` | 无背景或 `background-color: transparent` |
| 缩放 | 使用 `transform="matrix(...)"` 整体缩放 | 无额外缩放，`viewBox` 自适应 |
| 定位 | 所有元素相对于中心点偏移 | 绝对坐标，左上角为原点 |

**典型差异示例（case-0 chain）：**
- liemermaid: `viewBox="0 0 900.00 700.00"` + `transform="matrix(1,0,0,1,450,440)"`
- 官方: `viewBox="0 0 133.796875 278"` 精确包围盒

#### 2.1.2 节点渲染

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 节点形状 | `<rect>` 直接绘制 | `<rect>` + CSS class（`.node`） |
| 节点尺寸 | 固定 `120×60` | 根据文本内容动态计算 |
| 圆角 | 无（直角矩形） | 无（直角矩形），一致 |
| 填充色 | `#ececff` | `#ECECFF`（通过 CSS） |
| 边框色 | `#9370db` | `#9370DB`（通过 CSS） |
| 文本 | `<text>` 直接放置 | `<foreignObject>` 内嵌 HTML |
| 文本定位 | `text-anchor="middle"` | `foreignObject` + `text-align:center` |

**差异影响：**
- 官方节点宽度随文本变化（如 `Start` → 93.8px，`End` → 88.5px），liemermaid 固定 120px
- 官方使用 `foreignObject` 支持富文本（加粗、换行等），liemermaid 仅纯文本

#### 2.1.3 边渲染

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 路径 | `<path>` 贝塞尔曲线 | `<path>` 贝塞尔曲线（`C` 命令） |
| 曲线类型 | 单段三次贝塞尔 `C` | 多段三次贝塞尔 `C`（带中间点） |
| 箭头 | `<polygon>` 开放三角 | `<marker>` 定义的 `marker-end` |
| 箭头样式 | 空心（`fill="none"`） | 实心（`fill` + CSS class） |
| 线宽 | `1.50` | `1`（通过 CSS `.edge-thickness-normal`） |
| 颜色 | `#333333` 直接属性 | `#333333` 通过 CSS |

**差异影响：**
- 官方的 `marker` 方式使箭头自动跟随路径方向旋转
- liemermaid 的 `polygon` 箭头需要手动计算旋转角度
- 官方曲线更平滑（多控制点），liemermaid 为简单两段曲线

#### 2.1.4 边标签

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 标签背景 | `<rect>` 直接绘制 | `foreignObject` + CSS（`.edgeLabel`） |
| 标签文本 | `<text>` | `foreignObject` 内 `<span>` |
| 位置 | 路径中点 | 路径中点 |
| 样式 | 背景 `#e8e8e8cc` | `rgba(232,232,232,0.8)` |

#### 2.1.5 子图（Subgraph）

**case-23 对比：**

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 子图矩形 | `<rect>` 直接绘制 | `<rect>` + CSS class `.cluster` |
| 子图标题 | `<text>` 左上角 | `foreignObject` + CSS `.cluster-label` |
| 子图背景 | `#ffffde` | `#ffffde` |
| 子图边框 | `#aaaa33` | `#aaaa33` |

差异较小，基本一致。

#### 2.1.6 特殊形状（case-24 shapes）

| 形状 | liemermaid | 官方 mermaid | 差异 |
|------|-----------|-------------|------|
| Rectangle | `<rect>` | `<rect>` | ✅ 一致 |
| Rounded | `<rect rx="5">` | `<rect rx="5">` | ✅ 一致 |
| Stadium | `<rect rx="30">` | `<path>` 外轮廓 | 🟡 实现不同 |
| Subroutine | `<rect rx="2">` | `<polygon>` 双线 | 🔴 差异大 |
| Database | `<polygon>` 圆柱 | `<path>` 圆柱 | 🟡 实现不同 |
| Circle | `<ellipse>` | `<circle>` | ✅ 一致 |
| Decision | `<polygon>` 菱形 | `<polygon>` 菱形 | ✅ 一致 |
| Parallelogram | `<polygon>` | `<polygon>` | ✅ 一致 |
| Trapezoid | `<polygon>` | `<polygon>` | ✅ 一致 |
| DoubleCircle | `<ellipse>` | `<circle>` | ✅ 一致 |

---

### 2.2 Class Diagram（4 个用例）

#### 2.2.1 类框渲染

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 类框 | `<rect>` + 分隔线 `<rect>` | `<path>` 外轮廓（手绘风格路径） |
| 标题区 | 独立 `<rect>` + `<text>` | `<g>` 内 `foreignObject` |
| 属性区 | 逐行 `<text>` | `<g>` 内 `foreignObject` |
| 分隔线 | `<rect height="1">` | `<path>` 手绘线 |
| 文本对齐 | 属性左对齐（`text-anchor="start"`） | 左对齐 |

**差异影响：**
- 官方使用 `foreignObject` 支持 Markdown 格式
- liemermaid 属性格式为 `+ type name`，官方为 `+type name`（空格差异）

#### 2.2.2 关系渲染

| 关系 | liemermaid | 官方 mermaid | 差异 |
|------|-----------|-------------|------|
| 继承 `<\|--` | 空心三角箭头 | 空心三角箭头（marker） | ✅ 基本一致 |
| 组合 `*--` | 实心菱形 | 实心菱形（marker） | ✅ 基本一致 |
| 聚合 `o--` | 空心菱形 | 空心菱形（marker） | ✅ 基本一致 |
| 依赖 `-->` | 实心箭头 | 实心箭头（marker） | ✅ 基本一致 |

**差异影响：**
- liemermaid 关系线为直线 `<path d="M... L...">`，官方也为直线
- 箭头实现方式不同（polygon vs marker），视觉效果基本一致

---

### 2.3 Sequence Diagram（5 个用例）

#### 2.3.1 参与者

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 参与者框 | `<rect rx="5">` | `<rect rx="3">` |
| 框尺寸 | 根据文本动态 | 根据文本动态 |
| 生命线 | `<path>` 虚线 | `<line>` + CSS class `.actor-line` |
| 虚线样式 | `stroke-dasharray="6,4"` | `stroke-width:0.5px` 实线 |
| 填充色 | `#ececff` | `#eaeaea` |
| 边框色 | `#9370db` | `#666` |

**重大差异：**
- 官方生命线为细实线（`0.5px`），liemermaid 为虚线（`6,4`）
- 参与者框颜色方案不同

#### 2.3.2 消息

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 实线消息 | `<path>` 直线 | `<line>` + CSS `.messageLine0` |
| 虚线消息 | `<path>` + `dasharray="3,4"` | `<line>` + CSS `.messageLine1` + `dasharray="2,2"` |
| 箭头 | `<polygon>` | `<marker>` |
| 消息文本 | `<text>` 居中 | `<text>` + CSS `.messageText` |
| 文本位置 | 线上方 | 线上方 |

#### 2.3.3 激活条（Activation）

**case-31：**

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 激活条 | ❌ 未渲染 | ✅ `<rect>` + CSS `.activation0` |

**重大缺失：** liemermaid 未实现激活条（`+`/`-` 语法）。

#### 2.3.4 注释（Note）

**case-33：**

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 注释框 | `<rect>` + `<text>` | `<rect>` + `<text>` + CSS `.state-note` |
| 位置 | 参与者旁边 | 参与者旁边 |
| 背景色 | `#edf2ae` | `#fff5ad` |
| 边框色 | `#9370db` | `#aaaa33` |

差异：颜色方案不同，但结构基本一致。

#### 2.3.5 循环（Loop）

**case-34：**

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 循环框 | `<rect>` 虚线边框 | `<line>` × 4 + CSS `.loopLine` |
| 循环标签 | `<text>` 左上角 `[Each item]` | `<text>` + CSS `.labelText` |
| 标签格式 | `loop [Each item]` 合并显示 | `loop` 和 `[Each item]` 分开 |

---

### 2.4 State Diagram（4 个用例）

#### 2.4.1 状态节点

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 状态框 | `<rect rx="5">` | `<rect rx="5">` |
| 填充色 | `#ececff` | `#ECECFF` |
| 边框色 | `#9370db` | `#9370DB` |
| 文本 | `<text>` | `foreignObject` |

✅ 基本一致。

#### 2.4.2 起止节点

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 开始 `[*]` | `<ellipse>` 实心 | `<circle>` 实心 |
| 结束 `[*]` | 双 `<ellipse>`（外圈+内圈） | `<path>` 外圈 + `<circle>` 内圈 |
| 开始颜色 | `#9370db` | `#333333` |
| 结束外圈 | `stroke="#9370db"` | `stroke: #333333` |
| 结束内圈 | `fill="#9370db"` | `fill: #9370DB` |

#### 2.4.3 Fork/Join

**case-42：**

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| Fork/Join | `<rect>` 粗横线 | `<path>` 粗横线 |
| 尺寸 | `100×10` | 根据布局计算 |
| 颜色 | `#9370db` | `#333333` |

---

### 2.5 ER Diagram（3 个用例）

#### 2.5.1 实体渲染

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 实体框 | `<rect>` | `<path>` 外轮廓 |
| 实体名 | `<text>` 居中加粗 | `foreignObject` |
| 属性 | 逐行 `<text>`（类型+名称） | 多列布局（类型/名称/键/注释） |
| 分隔线 | `<rect height="1">` | `<path>` 分隔线 |
| 属性对齐 | 类型左对齐 + 名称左对齐 | 类型/名称/键/注释四列 |

**差异影响：**
- 官方属性支持四列（type、name、keys、comment），liemermaid 仅两列
- 官方行高更小（24px），属性更紧凑

#### 2.5.2 关系渲染

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 关系线 | `<path>` 直线 | `<path>` 直线 |
| 基数标记 | `<path>` + `<ellipse>` 组合 | `<marker>` 组合 |
| `\|\|--` | 两条短横线 | 两条短横线（marker） |
| `o{` | 空心圆 + 三角 | 空心圆 + 爪形（marker） |
| `}{` | 三角 + 空心圆 | 爪形 + 空心圆（marker） |

---

### 2.6 Pie Chart（3 个用例）

#### 2.6.1 扇形渲染

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 扇形 | `<path>` 闭合路径 | `<path>` 闭合路径 |
| 颜色 | `#ececff`、`#ffffde`、`#b9ff20` | `#ECECFF`、`#ffffde`、`hsl(80,100%,56.27%)` |
| 边框 | `stroke="#ffffff"` 白色 | `stroke:black` 黑色 |
| 线宽 | `2.00` | `2px` |
| 外圈 | ❌ 无 | ✅ `<circle>` 外圈 |

#### 2.6.2 标签与图例

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 百分比标签 | `<text>` 扇形外侧（含名称+百分比） | `<text>` 扇形外侧（仅百分比） |
| 图例 | ❌ 无 | ✅ `<g class="legend">` 色块+名称 |
| 标题 | `<text>` 上方 | `<text>` + CSS `.pieTitleText` |
| 百分比格式 | `Rust (40.0%)` | `40%` |
| 图例格式 | 无 | `Rust`、`Python`、`Go` |

**重大差异：**
- liemermaid 将名称和百分比合并显示在扇形外侧
- 官方将百分比放在扇形外侧，名称放在右侧图例中
- 官方支持 `showData` 模式（显示数值）

---

### 2.7 GitGraph（2 个用例）

#### 2.7.1 分支与提交

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 分支线 | `<path>` 直线/曲线 | `<line>` + CSS `.branch` |
| 分支样式 | 实线彩色 | 虚线灰色（`stroke-dasharray:2`） |
| 提交点 | `<ellipse>` 实心 | `<circle>` 实心 |
| 提交大小 | `r=10` | `r=10` |
| 合并提交 | 双圆（外+内） | 双圆（外+内） |
| 分支标签 | `<rect>` + `<text>` | `<rect>` + `<text>` |
| 提交标签 | `<text>` 下方 | `<text>` 下方旋转45° |
| 标签标签 | `<polygon>` 标签形状 | `<polygon>` 标签形状 |

**差异影响：**
- 官方分支线为虚线灰色，liemermaid 为实线彩色
- 官方提交标签旋转 45°，liemermaid 水平显示
- 官方颜色方案更丰富（每个分支不同颜色）

---

### 2.8 Timeline（2 个用例）

#### 2.8.1 时间轴

| 项目 | liemermaid | 官方 mermaid |
|------|-----------|-------------|
| 主轴 | `<path>` 水平线 | `<line>` 水平线 |
| 箭头 | `<path>` 手动箭头 | `<marker>` 箭头 |
| 节点 | `<ellipse>` 圆点 | 无（通过路径连接） |
| 连接线 | `<path>` 垂直虚线 | `<line>` 垂直虚线 |
| 事件框 | `<rect rx="6">` | `<path>` 圆角路径 |
| 标题 | `<text>` 上方 | `<text>` 上方 |

**差异影响：**
- 布局方式不同：liemermaid 垂直展开事件，官方水平展开
- 颜色方案不同

---

## 三、核心差异总结

### 3.1 架构层面

| 差异项 | liemermaid | 官方 mermaid | 影响 |
|--------|-----------|-------------|------|
| 渲染方式 | 直接生成 SVG 元素 | 生成带 CSS class 的 SVG | 样式灵活性 |
| 文本渲染 | `<text>` 元素 | `<foreignObject>` + HTML | 富文本支持 |
| 箭头实现 | `<polygon>` 手动计算 | `<marker>` 自动跟随 | 箭头方向精度 |
| 布局算法 | 固定间距 | 动态计算（dagre） | 空间利用率 |
| 画布尺寸 | 固定 900×700 | 自适应内容 | 文件大小/显示效果 |

### 3.2 视觉层面

| 差异项 | liemermaid | 官方 mermaid | 影响 |
|--------|-----------|-------------|------|
| 节点尺寸 | 固定（如 120×60） | 文本自适应 | 空间利用 |
| 颜色方案 | 基本一致 | 基本一致 | ✅ |
| 字体 | `trebuchet ms` | `trebuchet ms` | ✅ |
| 线宽 | 1.5px | 1px | 视觉粗细 |
| 箭头 | 空心 | 实心 | 视觉风格 |

### 3.3 功能缺失

| 功能 | liemermaid | 官方 | 影响 |
|------|-----------|------|------|
| 激活条（sequence） | ❌ | ✅ | 时序图表达力 |
| 图例（pie） | ❌ | ✅ | 可读性 |
| 外圈（pie） | ❌ | ✅ | 视觉完整性 |
| 富文本 | ❌ | ✅ | 文本格式 |
| CSS 主题 | ❌ | ✅ | 可定制性 |

---

## 四、改进建议优先级

### P0（必须修复）
1. **画布尺寸自适应** — 固定 900×700 导致大量空白，应改为根据内容计算
2. **节点尺寸自适应** — 根据文本长度动态计算节点宽度
3. **激活条支持** — sequence 图 `+`/`-` 语法

### P1（重要改进）
4. **箭头改用 `<marker>`** — 提高箭头方向精度
5. **曲线平滑度** — 增加贝塞尔控制点
6. **Pie 图图例** — 添加右侧图例
7. **Pie 图外圈** — 添加外圈圆环

### P2（体验优化）
8. **文本渲染** — 考虑 `foreignObject` 支持富文本
9. **分支线样式** — gitgraph 改为虚线
10. **时间轴布局** — 对齐官方水平布局
11. **ER 图四列属性** — 支持 keys 和 comment 列

### P3（锦上添花）
12. **CSS 主题系统** — 支持自定义主题
13. **动画支持** — 边动画效果
14. **可访问性** — 添加 `aria-roledescription` 等 ARIA 属性

---

## 五、一致性评分

| 维度 | 评分 | 说明 |
|------|------|------|
| 语义正确性 | 90% | 图表语义信息完整，关系表达正确 |
| 布局准确性 | 70% | 基本布局正确，但间距和尺寸有差异 |
| 视觉一致性 | 75% | 颜色一致，但箭头/线宽/尺寸有差异 |
| 功能完整性 | 80% | 大部分功能已实现，少数缺失 |
| 代码质量 | 65% | 缺少 CSS 类系统，样式硬编码 |

**总体评价：** liemermaid 在语义层面已能正确表达图表结构，核心渲染逻辑可用。主要差距在于精细度（自适应布局、视觉细节）和功能性（激活条、图例等）方面。建议优先解决画布尺寸和节点尺寸的自适应问题，这将显著改善用户体验。