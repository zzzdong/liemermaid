# Mermaid Git 图（GitGraph）语法参考

> 来源：Mermaid 官方文档 — <https://mermaid.js.org/syntax/gitgraph.html>
> 覆盖至 v11.0.0+。本文档为 liemermaid 的 `gitGraph` 解析提供语法对照。

## 1. 基本语法结构

```
gitGraph
    [方向标识符]
    命令序列
```

**方向标识符**（可选，放在 `gitGraph` 之后）：

- `LR:` — 从左到右（**默认**）
- `TB:` — 从上到下
- `BT:` — 从下到上（v11.0.0+）

## 2. 核心命令

### 1. `commit` — 提交

```
gitGraph
    commit
    commit id: "自定义ID"
    commit type: NORMAL
    commit tag: "自定义标签"
```

| 属性 | 取值 | 说明 |
| ---- | ---- | ---- |
| `id` | 任意字符串 | 自定义提交 ID |
| `type` | `NORMAL` / `REVERSE` / `HIGHLIGHT` | 提交类型（默认 `NORMAL`） |
| `tag` | 任意字符串 | 添加标签 |

类型渲染效果：
- `NORMAL`：实心圆点
- `REVERSE`：带交叉线的实心圆点
- `HIGHLIGHT`：实心矩形（高亮）

组合使用：`commit id: "c1" type: HIGHLIGHT tag: "v1.0"`

### 2. `branch` — 创建分支

```
branch 分支名
```

- 创建新分支并将其设为**当前分支**。
- 分支名必须**唯一**。
- 若分支名与关键字冲突，需用引号括起来，如 `branch "cherry-pick"`。

### 3. `checkout` — 切换分支

```
checkout 分支名
```

- 切换到已存在的分支（与 `switch` 可互换使用）。
- 若分支不存在，会报错。

### 4. `merge` — 合并分支

```
merge 分支名 [id: "自定义ID"] [tag: "自定义标签"] [type: 类型]
```

- 将指定分支的头部提交合并到**当前分支**。
- 合并后产生一个**合并提交**（图中显示为双圆点）。
- 不能合并自身分支。

### 5. `cherry-pick` — 挑选提交

```
cherry-pick id: "要挑选的提交ID"
```

重要规则：
1. 必须提供已存在提交的 ID。
2. 目标提交必须在**另一分支**上（不能是当前分支）。
3. 当前分支必须**至少有一个提交**。
4. 若挑选的是合并提交，必须指定父提交：`cherry-pick id: "提交ID" parent: "父提交ID"`。

## 3. 配置选项（通过指令/init 设置）

| 配置项 | 类型 | 默认值 | 说明 |
| ------ | ---- | ------ | ---- |
| `showBranches` | Boolean | `true` | 是否显示分支名称和线条 |
| `showCommitLabel` | Boolean | `true` | 是否显示提交标签 |
| `rotateCommitLabel` | Boolean | `true` | 提交标签是否旋转 45° |
| `mainBranchName` | String | `main` | 默认/根分支的名称 |
| `mainBranchOrder` | Number | `0` | 主分支在分支列表中的位置 |
| `parallelCommits` | Boolean | `false` | 是否并行显示提交（v10.8.0+） |

## 4. 分支排序规则

`order` 关键字可控制分支显示顺序，优先级如下：

1. **主分支**（默认排第一，可通过 `mainBranchOrder` 修改）。
2. **未指定 order 的分支**：按出现顺序排列。
3. **指定了 order 的分支**：按 order 值从小到大排列。

## 5. 主题定制变量

- 分支颜色（最多 8 个分支，超出循环复用）：`git0` ~ `git7`（线条）、`gitBranchLabel0` ~ `gitBranchLabel7`（标签）、`gitInv0` ~ `gitInv7`（高亮提交）。
- 提交标签：`commitLabelColor`、`commitLabelBackground`、`commitLabelFontSize`。
- 标签（Tag）：`tagLabelColor`、`tagLabelBackground`、`tagLabelBorder`、`tagLabelFontSize`。

## 6. 完整示例

```
gitGraph LR:
    commit id: "c1" tag: "v1.0"
    commit id: "c2" type: HIGHLIGHT
    branch develop
    checkout develop
    commit id: "d1"
    commit id: "d2" type: REVERSE
    checkout main
    merge develop id: "m1" tag: "merge-v1" type: NORMAL
    branch feature
    checkout feature
    commit id: "f1"
    cherry-pick id: "d1"
```

## 7. 支持的预置主题

`base`、`forest`、`default`、`dark`、`neutral`。
