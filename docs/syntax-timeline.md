# Mermaid 时间线图（Timeline Diagram）语法参考

> 来源：Mermaid 官方文档 — <https://mermaid.js.org/syntax/timeline.html>
> 覆盖至 v11.14.0+。⚠️ 该图为**实验性功能**，语法和属性可能在后续版本中更改。本文档为 liemermaid 的 `timeline` 解析提供语法对照。

## 1. 基本语法结构

所有时间线图必须以 `timeline` 关键字开头：

```
timeline
```

### 标题（Title）

在 `timeline` 关键字之后，使用 `title` 关键字添加标题：

```
timeline
title 我的时间线标题
```

### 时间段与事件

时间段的定义格式为：`{时间段} : {事件}`

| 写法 | 说明 |
| ---- | ---- |
| `{时间段} : {事件}` | 单个事件 |
| `{时间段} : {事件} : {事件}` | 同一行多个事件 |
| `{时间段} : {事件}` + 换行 `: {事件}` | 多行多个事件 |

示例：

```
timeline
title 历史时间线
1940 : 事件A
1950 : 事件B : 事件C
1960 : 事件D
      : 事件E
```

> 注意：时间段和事件都是纯文本，不仅限于数字。

## 2. 分组（Sections/时代）

使用 `section` 关键字对时间段进行分组：

```
timeline
title 时间线标题
section 时代一
1940 : 事件A
1950 : 事件B
section 时代二
1960 : 事件C
1970 : 事件D
```

规则：
- 后续所有时间段将归入当前 section，直到定义新的 section。
- 未定义 section 时，所有时间段归入默认 section。
- 每个 section 下的时间段和事件使用**相同的配色方案**，便于区分。

## 3. 文本换行

- 默认情况下，过长的文本自动换行。
- 也可以使用 `<br>` 强制换行。

## 4. 方向（Direction）（v11.14.0+）

在 `timeline` 关键字后指定方向：

| 方向 | 说明 |
| ---- | ---- |
| `LR` | 从左到右（默认） |
| `TD` | 从上到下 |

```
timeline TD
title 垂直时间线
```

## 5. 样式定制

### 两种着色模式

- **模式一：多色（默认）**：未定义 section 时，每个时间段及其事件使用独立的配色方案。
- **模式二：禁用多色（disableMultiColor）**：通过配置：

```javascript
mermaid.initialize({
    theme: 'base',
    timeline: {
      disableMulticolor: false,
    }
});
```

### 自定义配色方案

使用主题变量 `cScale0` 到 `cScale11` 自定义背景颜色：

- `cScale0` ~ `cScale11`：控制第 1 至第 12 个 section 或时间段的**背景色**。
- `cScaleLabel0` ~ `cScaleLabel11`：控制对应 section 的**前景色（文字颜色）**。
- 超过 12 个 section 时，配色方案循环重复。

## 6. 主题支持

| 主题 | 说明 |
| ---- | ---- |
| `base` | 基础主题 |
| `forest` | 森林主题 |
| `dark` | 深色主题 |
| `default` | 默认主题 |
| `neutral` | 中性主题 |

## 7. 完整语法总结

```
timeline [方向]          ← 必填，方向可选（LR/TD）
title 标题               ← 可选
section 时代名称          ← 可选，可多个
时间段 : 事件            ← 必填，可多个
```

## 8. 注意事项

⚠️ 该图为**实验性功能**，语法和属性可能在后续版本中更改。

- 时间线使用实验性的懒加载和异步渲染特性。
- 时间段和事件的排列顺序很重要：第一个时间段在左侧/顶部，最后一个在右侧/底部；每个时间段下第一个事件在顶部，最后一个在底部。
