# liemermaid

Mermaid 图表的 Rust 解析与渲染库，输出统一走 [lievisual](https://crates.io/crates/lievisual) 的声明式场景 IR（`Scene`）与多后端（SVG / vello_cpu PNG）。

## 支持图表

- **Flowchart**（流程图，TD / LR / BT / RL，含循环、分支、分组）
- **Sequence**（时序图，含参与者、消息箭头、备注）
- **Class**（类图，含成员、关系）
- **State**（状态图，含嵌套、转换、描述）
- **ER**（实体关系图，含基数）
- **Pie**（饼图，含标题、showData）
- **Gitgraph**（Git 分支图，含标签）
- **Timeline**（时间线，含分节、事件）

## 快速开始

```toml
[dependencies]
liemermaid = "0.1.0-alpha.1"
```

```rust
use liemermaid::{render, render_png};

// 渲染 SVG 字符串
let svg = render(r#"flowchart TD
    A[Start]
    B[End]
    A --> B
"#, 800, 600).expect("render failed");

// 渲染 PNG 位图字节
let png: Vec<u8> = render_png(r#"pie
    "A": 30
    "B": 50
    "C": 20
"#, 600, 400).expect("render failed");
```

## 架构

```
Mermaid 文本 → MermaidParser → builder::build_diagram_with_config → VisualElement (IR)
                                                                    ↓ scene_ext::to_scene
                                                         lievisual::Scene
                                                                    ↓
                                            lievisual::SvgRenderer / VelloPixmapRenderer
```

- `parser`：pest 语法解析 Mermaid DSL
- `builder`：布局引擎（Sugiyama 分层、坐标计算、边路由）产出 `VisualElement`
- `scene_ext`：将 `VisualElement` 转换为 `lievisual::Scene` 的适配层
- 渲染统一委托 lievisual（SVG / vello_cpu PNG），本 crate 不维护自有渲染后端

## 测试

```sh
cargo test
```

包含解析、布局质量（边不穿越节点 / 同层对齐 / 无重叠）、SVG 结构、PNG 编码与 lievisual 集成往返测试。

## License

Apache-2.0
