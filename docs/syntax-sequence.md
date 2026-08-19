# Mermaid 时序图（Sequence Diagram）语法参考

> 来源：Mermaid 官方文档 — <https://mermaid.js.org/syntax/sequenceDiagram.html>
> 覆盖至 v11.15.0+。本文档为 liemermaid 的 `sequenceDiagram` 解析提供语法对照。

## 1. 基本结构

```
sequenceDiagram
    participant A
    participant B
    A->>B: 消息文本
```

## 2. 参与者定义

### 隐式定义

参与者按首次出现在消息中的顺序自动渲染。

### 显式定义与别名

```
participant A as 别名文本
actor B as 别名文本
```

### 参与者类型

| 关键字 | 图标类型 |
| ------ | -------- |
| `participant` | 矩形框 |
| `actor` | 人形图标 |
| `boundary` | 边界符号 |
| `control` | 控制符号 |
| `entity` | 实体符号 |
| `database` | 数据库符号 |
| `collections` | 集合符号 |
| `queue` | 队列符号 |

### 别名优先级

外部别名（`as` 关键字）优先于配置中的内联 `"alias"` 字段。

### 创建与销毁（v10.3.0+）

```
create participant B
A->>B: 创建消息
destroy B
```

- 只能创建消息接收方；发送方和接收方均可被销毁。
- 报错时建议升级至 v10.7.0+。

### 分组 / 盒子（Box）

```
box Aqua 分组描述
    participant A
    participant B
end
box rgb(33,66,99)   # 支持 rgb/rgba/hsl/hsla 颜色
    participant C
end
```

⚠️ 十六进制颜色（`#ff0000`）**不支持**（`#` 被当作注释符号）。

## 3. 消息与箭头类型

基本语法：`[参与者][箭头][参与者]: 消息文本`

### 标准箭头

| 箭头 | 说明 |
| ---- | ---- |
| `->` | 实线，无箭头 |
| `-->` | 虚线，无箭头 |
| `->>` | 实线，带箭头 |
| `-->>` | 虚线，带箭头 |
| `<<->>` | 实线，双向箭头（v11.0.0+） |
| `<<-->>` | 虚线，双向箭头（v11.0.0+） |
| `-x` | 实线，末端带叉号 |
| `--x` | 虚线，末端带叉号 |
| `-)` | 实线，末端开放箭头（异步） |
| `--)` | 虚线，末端开放箭头（异步） |

### 半箭头（v11.12.3+）

`-|\`（上半）、`--|\`、`-|/`（下半）、`--|/`、`/|-`（反向）、`/|--` 等，共 16 种变体。

### 中央连接（v11.12.3+）

```
A->>() : 中央连接消息
```

## 4. 激活（Activation）

- 独立声明：`activate A` / `deactivate A`。
- 快捷符号：消息箭头后追加 `+`（激活）或 `-`（取消激活）：

```
A->>+B: 消息（激活 B）
B-->>-A: 回复（取消激活 B）
```

- 同一参与者可叠加多层激活（嵌套）。

## 5. 注释（Notes）

```
Note [right of | left of | over] [参与者]: 注释文本
```

- `Note right of A`、`Note left of A`、`Note over A`、`Note over A, B`。
- 换行：使用 `<br/>`（也适用于消息文本）。

## 6. 流程控制结构

### 循环（Loop）

```
loop 循环描述文本
    ...语句...
end
```

### 条件分支（Alt / Opt）

```
alt 条件描述
    ...语句...
else
    ...语句...
end

opt 可选描述
    ...语句...
end
```

### 并行（Parallel）

```
par [动作1]
    ...语句...
and [动作2]
    ...语句...
end
```

支持嵌套并行块。

### 临界区（Critical）

```
critical [必须执行的动作]
    ...语句...
option [情况A]
    ...语句...
end
```

- 可省略所有 option；支持嵌套。

### 中断（Break）

```
break [异常描述]
    ...语句...
end
```

### 背景高亮（Rect）

```
rect rgb(0, 255, 0)
    ...内容...
end
```

仅支持 `rgb()` 和 `rgba()` 格式。

## 7. 注释（Comments）

以 `%%` 开头，单独一行，行内之后内容视为注释。

## 8. 特殊字符转义

`#数字;` 方式（十进制 ASCII），也支持 HTML 字符名：

| 转义 | 含义 |
| ---- | ---- |
| `#35;` | `#` |
| `#59;` | `;`（分号可代替换行符，故需转义） |
| `#9829;` | ♥ 等 |

## 9. 自动编号（sequenceNumbers）

- 全局：`mermaid.initialize({ sequence: { showSequenceNumbers: true } })`
- 图内：`autonumber`
- 自定义起始值与步长（v11.15.0+）：`autonumber <起始值> <增量>`（最多两位小数）。

## 10. 参与者菜单

```
link <参与者>: <链接标签> @ <链接URL>
links <参与者>: {"链接标签": "链接URL"}
```

## 11. 样式（Styling）

主要 CSS 类：`.actor`、`.actor-line`、`.messageLine0`（实线）、`.messageLine1`（虚线）、`.messageText`、`.labelBox` / `.labelText`、`.loopLine`、`.note` / `.noteText`。

## 12. 配置参数

| 参数 | 说明 | 默认值 |
| ---- | ---- | ------ |
| `mirrorActors` | 是否在下方镜像渲染参与者 | `false` |
| `bottomMarginAdj` | 底部间距 | `1` |
| `actorFontSize` | 参与者字体大小 | `14` |
| `noteFontSize` | 注释字体大小 | `14` |
| `noteAlign` | 注释文本对齐 | `center` |
| `messageFontSize` | 消息字体大小 | `16` |

## 13. 注意事项

1. **"end" 关键字**：节点名使用 `end` 可能导致解析失败，需用括号/引号/花括号包裹。
2. **十六进制颜色**不适用于 box 和 rect。
3. **分号**（`;`）可代替换行符分隔语句，需转义时使用 `#59;`。
