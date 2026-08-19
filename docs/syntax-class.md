# Mermaid 类图（Class Diagram）语法参考

> 来源：Mermaid 官方文档 — <https://mermaid.js.org/syntax/classDiagram.html>
> 覆盖至 v11.15.0+。本文档为 liemermaid 的 `classDiagram` 解析提供语法对照。

## 1. 类的定义方式

| 方式 | 语法 | 示例 |
| ---- | ---- | ---- |
| 显式定义 | `class 类名` | `class Animal` |
| 通过关系定义 | `类A 关系 类B` | `Vehicle <|-- Car` |

命名规范：类名可包含字母数字（含 Unicode）、下划线（`_`）和连字符（`-`）。

## 2. 类标签（Class Labels）

- 基本语法：`class 类名["显示标签"]`
- 可用反引号转义特殊字符。

## 3. 类成员（属性与方法）

通过是否包含括号 `()` 区分：有 `()` 为方法，无 `()` 为属性。

**方式一：冒号逐一定义**

```
class Animal
Animal : +int age
Animal : +String gender
Animal : +isMammal()
```

**方式二：花括号批量定义**

```
class Duck{
  +String beakColor
  +swim()
  +quack()
}
```

## 4. 可见性（Visibility）

| 符号 | 含义 |
| ---- | ---- |
| `+` | Public（公有） |
| `-` | Private（私有） |
| `#` | Protected（受保护） |
| `~` | Package/Internal（包内/内部） |

### 附加分类器

- **方法附加**（放在 `()` 或返回类型之后）：`*`（Abstract 抽象）、`$`（Static 静态）。
- **字段附加**（放在末尾）：`$`（Static 静态）。

## 5. 返回类型与泛型

**返回类型**：方法定义末尾加空格后写返回类型。

```
class Animal {
  +int getAge() int
  +String getName() String
}
```

**泛型**：使用 `~`（波浪号）包裹泛型类型。

```
class List~T~
class List~T~ {
  +List~T~ getList()
}
```

⚠️ 支持嵌套泛型（`List<List<int>>`），不支持含逗号的泛型（`List<List<K, V>>`）。泛型类型不属于类名的一部分。

## 6. 关系类型（Relationships）

基本语法：`[类A][箭头][类B]:标签文本`

### 八种 UML 关系类型

| 类型 | 描述 |
| ---- | ---- |
| `<\|--` | 继承（Inheritance） |
| `*--` | 组合（Composition） |
| `o--` | 聚合（Aggregation） |
| `-->` | 关联（Association） |
| `--` | 链接（Link/Solid） |
| `..>` | 依赖（Dependency） |
| `..\|>` | 实现（Realization） |
| `..` | 虚线链接（Link/Dashed） |

### 双向关系

`[关系类型A][连接线][关系类型B]`

| 关系类型 | 描述 |
| -------- | ---- |
| `<\|` | 继承 |
| `*` | 组合 |
| `o` | 聚合 |
| `>` / `<` | 关联 |
| `\|>` | 实现 |

连接线：`--`（实线）或 `..`（虚线）。

### Lollipop 接口

```
bar ()-- foo
foo --() bar
```

## 7. 基数/多重性（Cardinality）

放在引号 `""` 中，位于箭头前后：`[类A] "基数1" [箭头] "基数2" [类B]:标签`

可选值：`1`、`0..1`、`1..*`、`*`、`n`（n>1）、`0..n`、`1..n`。

## 8. 命名空间（Namespace）

```
namespace 名称 {
  class 类名
}
```

- 命名空间标签（v11.15.0+）：`namespace 名称["显示标签"]`
- 嵌套：点号语法 `namespace A.B.C { }` 或语法嵌套。
- 紧凑模式：`hierarchicalNamespaces: false`。

## 9. 注解（Annotations）

三种添加方式（效果相同）：

| 方式 | 示例 |
| ---- | ---- |
| 内联 | `class Animal <<Interface>>` |
| 单独一行 | `<<Interface>> Animal` |
| 嵌套结构 | `class Animal { <<Interface>> +int age }` |

常见注解：`<<Interface>>`、`<<Abstract>>`、`<<Service>>`、`<<Enumeration>>`。

## 10. 其他语法元素

- **注释**：以 `%%` 开头，独立一行。
- **方向设置**：`direction LR`（或 `TB`/`RL`/`BT`）。
- **交互**：`action className "reference" "tooltip"`、`click className call callback()`、`click className href "url" "tooltip"`。⚠️ 需 `securityLevel='loose'`。
- **备注**：`note "line1\nline2"`、`note for 类名 "line1\nline2"`。

## 11. 样式（Styling）

```
style 类名 fill:#f9f,stroke:#333,stroke-width:4px
classDef className fill:#f9f,stroke:#333,stroke-width:4px;
cssClass "nodeId1" className;
class Animal:::className
classDef default fill:#f9f,stroke:#333,stroke-width:4px;
```

## 12. 配置参数

| 参数 | 描述 | 默认值 |
| ---- | ---- | ------ |
| `hideEmptyMembersBox` | 隐藏类节点的空成员框 | `false` |

## 13. 完整示例

```
classDiagram
direction LR

class Animal {
  +int age
  +String gender
  +isMammal() bool
  +mate()
}
class Duck {
  +String beakColor
  +swim()
  +quack()
}

Animal <|-- Duck : 继承
Animal : +int weight
Animal : +String name
```
