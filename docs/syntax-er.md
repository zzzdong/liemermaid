# Mermaid ER 图（Entity Relationship Diagram）语法参考

> 来源：Mermaid 官方文档 — <https://mermaid.js.org/syntax/entityRelationshipDiagram.html>
> 覆盖至 v11.17.0+（可选属性类型 v11.16.0+、子图 v11.17.0+）。本文档为 liemermaid 的 `erDiagram` 解析提供语法对照。

## 1. 基本语法结构

```
<first-entity> [<relationship> <second-entity> : <relationship-label>]
```

| 组成部分 | 说明 |
| -------- | ---- |
| `first-entity` | 第一个实体名称，支持 Unicode，含空格需用双引号 |
| `relationship` | 描述两个实体之间的关系（基数 + 标识性） |
| `second-entity` | 第二个实体名称 |
| `relationship-label` | 从第一个实体视角描述关系的标签 |

示例：

```
erDiagram
    PROPERTY ||--|{ ROOM : contains
```

注意：只有 `first-entity` 是必填项（允许显示无关系的实体）。若指定任何其他部分，则所有部分都必须完整。

## 2. 关系语法（基数 + 标识）

### 2.1 基数标记（Crow's Foot 表示法）

每个基数标记包含**两个字符**：最外层字符 → 最大值，最内层字符 → 最小值。

| 左侧值 | 右侧值 | 含义 |
| ------ | ------ | ---- |
| `\|o` | `o\|` | 零或一个 |
| `\|\|` | `\|\|` | 恰好一个 |
| `}o` | `o{` | 零或多个（无上限） |
| `}\|` | `\|{` | 一个或多个（无上限） |

### 2.2 基数别名

| 左侧 | 右侧 | 等同于 |
| ---- | ---- | ------ |
| one or zero / zero or one | 同上 | 零或一个 |
| one or more / one or many / many(1) / 1+ | 同上 | 一个或多个 |
| zero or more / zero or many / many(0) / 0+ | 同上 | 零或多个 |
| only one / 1 | 同上 | 恰好一个 |

### 2.3 标识性（Identifying / Non-identifying）

| 符号 | 含义 | 渲染效果 |
| ---- | ---- | -------- |
| `--` | 标识性关系（identifying） | 实线 |
| `..` | 非标识性关系（non-identifying） | 虚线 |

别名：`to`（标识性）、`optionally to`（非标识性）。

```
erDiagram
    PERSON }|..|{ CAR : "driver"   // 虚线：非标识性
    PERSON ||--o{ NAMED-DRIVER : "is"
```

## 3. 属性（Attributes）定义

### 3.1 基本属性语法

```
erDiagram
    CUSTOMER {
        string name
        string email
        int age
    }
```

- `type` 必须以字母开头，可含数字、连字符、下划线、括号、方括号。
- `name` 格式类似 `type`，可加括号、以 `*` 开头表示主键。

### 3.2 可选属性类型（v11.16.0+）

`type` 可以 `?` 结尾表示可选/可空类型：

```
CUSTOMER {
    string name?
    string email?
}
```

### 3.3 属性键（Key）和注释（Comment）

```
type name "comment"              // 基本格式
type name PK, FK "comment"       // 带键约束和注释
```

| 键类型 | 含义 |
| ------ | ---- |
| `PK` | 主键（Primary Key） |
| `FK` | 外键（Foreign Key） |
| `UK` | 唯一键（Unique Key） |

规则：
- 多个键用逗号分隔（如 `PK, FK`）。
- 注释用双引号包裹在末尾；注释中不能包含双引号字符。
- 键不支持 Markdown 格式和 Unicode。

完整示例：

```
erDiagram
    CUSTOMER {
        string id PK "客户ID"
        string name "客户姓名"
        string email UK "电子邮箱"
    }
    ORDER {
        int orderId PK
        string customerId FK "关联客户"
    }
```

## 4. 实体名称别名（Entity Name Aliases）

使用方括号 `[ ]` 为实体添加别名：

```
p[Person] {
    string firstName
    string lastName
}
p ||--o{ a : "lives at"
```

别名遵循与实体名称相同的规则。

## 5. 方向（Direction）

```
direction TB   // 从上到下（默认）
direction BT   // 从下到上
direction LR   // 从左到右
direction RL   // 从右到左
```

## 6. 子图（Subgraphs，v11.17.0+）

```
subgraph title
    graph definition
end
```

- 单词语：`subgraph 名称`（同时作为 id 和标题）。
- 多词语：`subgraph "多个词"`（引号内值同时作为 id 和标题）。
- 显式 ID：`subgraph 显式ID[标题]`。

⚠️ 子图始终通过 **id** 引用，而非标题。id 含空格时引用需用引号。子图间可定义关系，可设置子图内方向（如 `subgraph LR[左右布局]`）。

## 7. 样式与类（Styling & Classes）

```
style CUSTOMER fill:#f9f,stroke:#333,stroke-width:4px
classDef className fill:#f9f,stroke:#333,stroke-width:4px
class nodeId1 className
nodeId:::className
CUSTOMER:::customerClass ||--o{ ORDER:::orderClass : places
classDef default fill:#bbf
```

- 命名为 `default` 的类应用于所有未指定类的节点。
- 优先级：`style` 或其他类中的自定义样式优先于 `default` 类。

## 8. 配置（Configuration）

- 布局引擎：默认 `dagre`，可切换 `elk`（需要 mermaid 9.4+）：

```yaml
---
config:
  layout: elk
---
```

## 9. Markdown 格式支持

实体名称、关系、属性均支持 Unicode 文本和 Markdown 格式：

```
CUSTOMER {
    string name "**客户姓名**"
}
```

## 10. 语法要点总结

1. 最小语法：`实体名` 即可（无关系实体）。
2. 基数：双字符系统（外层=最大，内层=最小），支持多种别名。
3. 标识性：`--`（实线/标识性）、`..`（虚线/非标识性）。
4. 属性块：`实体 { type name PK, FK "注释" }`。
5. 可选类型：`type?`（v11.16.0+）。
6. 子图：`subgraph id[标题]`，通过 id 引用（v11.17.0+）。
7. 键类型：`PK`、`FK`、`UK`，可组合（`PK, FK`）。
