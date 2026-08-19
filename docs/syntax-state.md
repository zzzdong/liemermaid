# Mermaid 状态图（State Diagram）语法参考

> 来源：Mermaid 官方文档 — <https://mermaid.js.org/syntax/stateDiagram.html>
> 覆盖至 v11.x。本文档为 liemermaid 的 `stateDiagram` / `stateDiagram-v2` 解析提供语法对照。

## 1. 状态（States）的定义

三种定义方式：

**方式一：仅定义 ID**

```
stateId
```

**方式二：`state` 关键字加描述**

```
state "描述文本" as stateId
```

**方式三：冒号语法（ID + 描述）**

```
stateId : 描述文本
```

## 2. 转换（Transitions）

转换表示状态之间的路径/边，使用箭头 `-->`：

```
state1 --> state2
```

带文本标签：

```
state1 --> state2 : 转换描述
```

注意：若转换引用了未定义的状态，该状态会自动以该 ID 创建。

## 3. 开始与结束状态

使用 `[*]` 语法，箭头方向决定其类型：

```
[*] --> State1    // 开始状态
State2 --> [*]    // 结束状态
```

## 4. 复合状态（Composite States）

```
state 复合状态ID {
    内部状态1 --> 内部状态2
}
```

- **多层嵌套**：`state 外层 { state 内层 { ... } }`
- **复合状态之间的转换**：可定义 `复合状态1 --> 复合状态2`。
- ⚠️ **限制**：不能定义属于不同复合状态的内部状态之间的转换。

## 5. 选择（Choice）

使用 `<<choice>>` 表示分支选择：

```
state 分支点 <<choice>>
状态1 --> 分支点
分支点 --> 路径A
分支点 --> 路径B
```

## 6. 分叉与汇合（Fork / Join）

使用 `<<fork>>` 和 `<<join>>` 表示并发：

```
state 分叉点 <<fork>>
状态 --> 分叉点
分叉点 --> 并发状态1
分叉点 --> 并发状态2

state 汇合点 <<join>>
并发状态1 --> 汇合点
并发状态2 --> 汇合点
汇合点 --> 后续状态
```

## 7. 备注（Notes）

```
note right of 状态ID : 备注内容
note left of 状态ID : 备注内容
```

## 8. 并发（Concurrency）

使用 `--` 符号表示并发区域：

```
state 复合状态 {
    并发区域1 --> 状态A
    --
    并发区域2 --> 状态B
}
```

## 9. 方向控制

```
direction LR   // 从左到右
direction TB   // 从上到下（默认）
```

## 10. 注释（Comments）

以 `%%` 开头，直到行尾结束：

```
%% 这是注释
state1 --> state2  %% 行尾注释
```

## 11. 样式与类定义（classDef）

```
classDef 样式名 属性:值,属性:值,...
classDef badBadEvent fill:#f00,color:white,font-weight:bold,stroke-width:2px,stroke:yellow
```

应用方式：

```
class 状态1,状态2 样式名
状态ID:::样式名
```

### ⚠️ 当前限制

1. 不能应用于开始/结束状态。
2. 不能应用于复合状态内部或其本身。

## 12. 状态名中的空格

先定义带 ID 的状态，后续通过 ID 引用：

```
yswsii : Your state with spaces in it
[*] --> yswsii
yswsii --> YetAnotherState
```

## 13. 完整示例

```
stateDiagram-v2
    direction LR
    [*] --> Still
    Still --> [*]
    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]
```

## 语法元素总结

| 语法元素 | 关键语法 |
| -------- | -------- |
| 状态定义 | `stateId` / `state "描述" as id` / `id : 描述` |
| 转换 | `-->` 或 `--> 描述文本` |
| 开始/结束 | `[*]` |
| 复合状态 | `state id { ... }` |
| 选择 | `<<choice>>` |
| 分叉/汇合 | `<<fork>>` / `<<join>>` |
| 备注 | `note right/left of 状态 : 内容` |
| 并发 | `--` |
| 注释 | `%%` |
| 样式 | `classDef` + `class` / `:::` |
