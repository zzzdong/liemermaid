# Mermaid 流程图（Flowchart）语法参考

> 来源：Mermaid 官方文档 — <https://mermaid.js.org/syntax/flowchart.html>
> 覆盖至 v11.17.0+。本文档为 liemermaid 的 `flowchart` / `graph` 解析提供语法对照。

## 1. 基本结构

```
flowchart LR
    A --> B
```

### 声明与方向

以 `flowchart` 或 `graph` 开头，可紧跟方向声明：

| 方向 | 含义 |
| ---- | ---- |
| `TB` / `TD` | 从上到下 |
| `BT` | 从下到上 |
| `RL` | 从右到左 |
| `LR` | 从左到右 |

### 节点定义

- 默认节点：`id`（节点 ID 即显示文本）。
- 带文本节点：`id1["这是文本"]`，多次定义以最后一次为准。
- 支持 Unicode（用双引号）和 Markdown 格式（双引号 + 反引号）。

## 2. 节点形状（传统语法）

| 形状 | 语法 |
| ---- | ---- |
| 圆角矩形 | `id1(文本)` |
| 体育场形 | `id1([文本])` |
| 子程序形 | `id1[[文本]]` |
| 圆柱形（数据库） | `id1[(数据库)]` |
| 圆形 | `id1((文本))` |
| 非对称形 | `id1>文本]` |
| 菱形（决策） | `id1{文本}` |
| 六边形 | `id1{{文本}}` |
| 平行四边形 | `id1[/文本/]` |
| 平行四边形（变体） | `id1[\文本\]` |
| 梯形 | `id1[/文本\]` |
| 梯形（变体） | `id1[\文本/]` |
| 双圆 | `id1(((文本)))` |

## 3. 扩展节点形状（v11.3.0+）

新语法格式：`A@{ shape: rect }`，等价于 `A["A"]` 或 `A`。

常用形状名：`rect`（矩形）、`rounded`（圆角）、`stadium`（体育场）、`fr-rect`（带边框矩形）、`cyl`（数据库圆柱）、`circle`（圆）、`sm-circ`（小圆）、`diam`（菱形）、`hex`（六边形）、`lean-r` / `lean-l`（输入输出）、`datastore`（数据存储）、`trap-b` / `trap-t`（梯形）、`dbl-circ`（双圆）、`fr-circ`（圆框）、`text`（文本块）、`notch-rect`（卡片）、`doc`（文档）、`cloud`（云）、`person`（人）、`folder`（文件夹）、`brace`（注释）、`docs`（多文档）、`st-rect`（堆叠矩形）、`fork`（合并/分叉）、`f-circ`（连接点）、`sl-rect`（手动输入）。

特殊形状：

```
A@{ icon: "icon-name", form: "square", label: "标签", pos: "b", h: 48 }
A@{ img: "https://example.com/image.png", label: "标签", pos: "t", w: 60, h: 60, constraint: "on" }
```

- `form` 可选：`square`、`circle`、`rounded`。
- `pos` 可选：`t`（顶部）、`b`（底部，默认）。

## 4. 边（链接）定义

### 基本边类型

| 类型 | 语法 | 示例 |
| ---- | ---- | ---- |
| 带箭头 | `-->` | `A --> B` |
| 无箭头 | `---` | `A --- B` |
| 带文本 | `--文本---` 或 `---|文本|` | `A-- 文本 ---B` |
| 箭头 + 文本 | `-->|文本|` 或 `--文本-->` | `A-- 文本 -->B` |
| 虚线 | `-.->` | `A -.-> B` |
| 虚线 + 文本 | `-.文本.->` | `A-. 文本 .-> B` |
| 粗线 | `==>` | `A ==> B` |
| 粗线 + 文本 | `==文本==>` | `A== 文本 ==>B` |
| 不可见边 | `~~~` | `A ~~~ B` |

### 新箭头类型

- 圆形边：`A --o B`
- 交叉边：`A --x B`
- 多方向箭头：`A <--> B`、`A o--o B`、`A x--x B`

### 链式链接

```
A --> B --> C --> D
A --> B & C --> D
```

### 边的最小长度

通过额外破折号加长：`---` → `----`；`-->` → `--->`；`===` → `====`；`-.-` → `-..-`。

## 5. 子图（Subgraph）

```
subgraph 标题
    图定义
end
```

- 带 ID：`subgraph id [标题]`
- 子图方向：`subgraph 子图名` 内可用 `direction LR`（TB/BT/RL/LR）。
  - ⚠️ 若子图内节点与外部有链接，子图方向将被忽略，继承父图方向。
- 可折叠子图（v11.17.0+）：`subgraph id@{ view: collapsed } [标题]`

## 6. 特殊字符与转义

- 引号包裹特殊字符：`A["需要特殊字符的文本: #35;"]`
- 实体编码：`#35;`（十进制），也支持 HTML 字符名。

## 7. Markdown 字符串

- 使用双引号包裹 markdown 文本：`**加粗**`、`*斜体*`。
- 自动换行（可配置 `markdownAutoWrap: false` 关闭）。

## 8. 交互（点击事件）

```
click nodeId callback
click nodeId "https://www.github.com" "提示文本"
click nodeId call callback() "提示文本"
click nodeId href "https://..." "提示文本"
```

目标：`_self`、`_blank`、`_parent`、`_top`。⚠️ `securityLevel='strict'` 时禁用。

## 9. 注释

以 `%%` 开头，独占一行。

## 10. 样式与类

```
linkStyle 3 stroke:#ff3,stroke-width:4px,color:red;
style nodeId fill:#f9f,stroke:#333,stroke-width:4px;
classDef className fill:#f9f,stroke:#333,stroke-width:4px;
class nodeId1 className;
A:::someclass --> B        %% 类名简写
classDef default fill:#f9f  %% 默认类
```

曲线样式（`curve`）：`basis`、`bumpX`、`bumpY`、`cardinal`、`catmullRom`、`linear`、`monotoneX`、`monotoneY`、`natural`、`step`、`stepAfter`、`stepBefore`。

## 11. FontAwesome 图标

```
flowchart TD
    B[fa:fa-twitter]
```

前缀：`fa`、`fab`、`fas`、`far`、`fal`、`fad`、`fak`。

## 12. 配置

- 渲染器：`defaultRenderer: "elk"`（默认 `dagre`，elk 更适合大型图，v9.4+ 实验性）。
- 宽度：`mermaid.flowchartConfig = { width: "100%" };`

## 13. 注意事项

- **"end" 限制**：节点名用全小写 `end` 会破坏图，须大写（`End`/`END`）或用括号包裹。
- **"o"/"x" 开头**：连接节点首字母为 `o` 或 `x` 时，需加空格或大写（`dev--- ops`），否则被解析为圆形/交叉边。
