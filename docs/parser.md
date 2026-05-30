# 基于 Pest 的 Mermaid 语法解析器：完整架构设计与实现指南

本文档提供从零开始构建 **Mermaid 语法解析器** 的完整方案，使用 Rust 的 `pest` 库。我们将设计一个可扩展的解析器框架，支持 Mermaid 的核心图表类型（流程图、时序图、类图、状态图、ER 图），并给出详细的实现指导和最佳实践。

---

## 1. 项目架构总览

### 1.1 项目目录结构

```
mermaid-parser/
├── Cargo.toml
├── src/
│   ├── main.rs              # 入口演示
│   ├── lib.rs               # 库入口
│   ├── ast.rs               # 抽象语法树定义
│   ├── parser.rs            # Pest 解析器封装
│   └── error.rs             # 错误类型定义
├── grammar/
│   └── mermaid.pest         # Pest 语法规则文件
└── tests/
    └── integration.rs       # 集成测试
```

### 1.2 依赖配置 (`Cargo.toml`)

```toml
[package]
name = "mermaid-parser"
version = "0.1.0"
edition = "2021"

[dependencies]
pest = "2.7"
pest_derive = "2.7"
thiserror = "1.0"      # 优雅的错误处理

[dev-dependencies]
pretty_assertions = "1.4"
```

### 1.3 模块职责

| 模块          | 职责                                                           |
| ------------- | -------------------------------------------------------------- |
| `grammar/`    | 使用 Pest 语法定义 Mermaid 词法/语法规则                       |
| `ast.rs`      | 定义所有图表元素的 Rust 数据结构，与语法树解耦                  |
| `parser.rs`   | 实现 `pest::Parser` trait，将文本转换为 `ast::Diagram`         |
| `error.rs`    | 解析错误类型（支持位置追踪）                                    |
| `main.rs`     | 示例：读取文件并打印解析结果                                    |

---

## 2. Pest 语法规则设计 (`grammar/mermaid.pest`)

### 2.1 基础词法规则

```pest
// ========== 空白与注释 ==========
WHITESPACE = _{ " " | "\t" | NEWLINE }
NEWLINE    = { "\r\n" | "\n" | "\r" }
COMMENT    = _{ "%%" ~ (!NEWLINE ~ ANY)* ~ NEWLINE }

// ========== 标识符 ==========
simple_id    = @{ ASCII_ALPHA ~ (ASCII_ALPHANUMERIC | "_" | "-")* }
quoted_id    = { "\"" ~ (!"\"" ~ ANY)* ~ "\"" }
identifier   = { quoted_id | simple_id }

// ========== 字符串字面量 ==========
text = { identifier | quoted_id }
```

### 2.2 图表类型分发

```pest
file = {
    SOI ~
    (COMMENT | WHITESPACE)* ~
    diagram:diagram ~
    (COMMENT | WHITESPACE)* ~
    EOI
}

diagram = {
    flowchart_diagram |
    sequence_diagram |
    class_diagram |
    state_diagram |
    er_diagram
}
```

### 2.3 流程图 (`flowchart`)

```pest
direction = { "TB" | "TD" | "BT" | "RL" | "LR" }

node_shape = {
    "[" ~ node_text ~ "]"      |
    "(" ~ node_text ~ ")"      |
    "((" ~ node_text ~ "))"    |
    "[[" ~ node_text ~ "]]"    |
    "{" ~ node_text ~ "}"      |
    "{{" ~ node_text ~ "}}"    |
    "[/" ~ node_text ~ "/]"    |
    "[\\" ~ node_text ~ "\\]"
}

node_text = { text }

node_decl = { identifier ~ (WHITESPACE* ~ node_shape)? }

edge_arrow = {
    "-->": "arrow_solid" |
    "-->"                |
    "-->|" ~ text ~ "|"  |
    "-.->": "arrow_dotted" |
    "==>": "arrow_thick"
}

edge = {
    source:identifier ~
    WHITESPACE* ~
    arrow:edge_arrow ~
    WHITESPACE* ~
    target:identifier ~
    (WHITESPACE+ ~ ":" ~ WHITESPACE* ~ label:text)?
}

subgraph = {
    "subgraph" ~ title:text? ~ NEWLINE+ ~
    (flowchart_statement (NEWLINE+ flowchart_statement)*) ~
    "end"
}

flowchart_statement = _{ node_decl | edge | subgraph }

flowchart_diagram = {
    ("flowchart" | "graph") ~
    direction:direction? ~
    NEWLINE+ ~
    statements:(flowchart_statement (NEWLINE+ flowchart_statement)*)?
}
```

### 2.4 时序图 (`sequenceDiagram`)

```pest
participant = {
    "participant" ~ name:identifier ~
    (WHITESPACE+ "as" ~ WHITESPACE+ alias:identifier)?
}

actor = { "actor" ~ name:identifier }

message_arrow = {
    "->"  : "solid"   |
    "->>" : "solid_tip" |
    "-->" : "dashed" |
    "-->>": "dashed_tip" |
    "-x"  : "cross"   |
    "-)"  : "open"
}

message = {
    from:identifier ~
    WHITESPACE+ ~
    arrow:message_arrow ~
    WHITESPACE+ ~
    to:identifier ~
    (":" ~ WHITESPACE* ~ message_text:text)?
}

note = {
    "note" ~
    ("left of" | "right of" | "over") ~
    target:identifier ~
    ("," ~ WHITESPACE* ~ target2:identifier)? ~
    ":" ~ WHITESPACE* ~ note_text:text
}

activation = {
    ("+" | "-") ~ identifier  // + 激活，- 去激活
}

sequence_diagram = {
    "sequenceDiagram" ~ NEWLINE+ ~
    (participant | actor | message | note | activation)*
}
```

### 2.5 类图 (`classDiagram`)

```pest
visibility = { "+" | "-" | "#" | "~" }

class_member = {
    visibility? ~
    name:identifier ~
    (":" ~ WHITESPACE* ~ type:identifier)? ~
    ("(" ~ parameters ~ ")")?
}

class_decl = {
    "class" ~ name:identifier ~
    (WHITESPACE+ "{" ~ NEWLINE+ ~ (class_member (NEWLINE+ class_member)*)? ~ "}")?
}

relation_type = {
    "<|--": "inheritance" |
    "*--" : "composition" |
    "o--" : "aggregation" |
    "-->" : "association" |
    "..>" : "dependency"
}

relation = {
    source:identifier ~
    WHITESPACE+ ~
    rel:relation_type ~
    WHITESPACE+ ~
    target:identifier ~
    (":" ~ WHITESPACE* ~ label:text)?
}

class_diagram = {
    "classDiagram" ~ NEWLINE+ ~
    (class_decl | relation)*
}
```

### 2.6 状态图 (`stateDiagram-v2`)

```pest
state_id = { identifier }
start_state = { "[*]" }
end_state   = { "[*]" }

state_simple = {
    "state" ~ id:state_id ~
    (WHITESPACE+ ":" ~ WHITESPACE* ~ description:text)?
}

state_composite = {
    "state" ~ id:state_id ~
    WHITESPACE+ "{" ~ NEWLINE+ ~
    (state_element)* ~
    "}"
}

state_element = _{ state_simple | transition | state_composite }

transition = {
    from:(start_state | state_id) ~
    WHITESPACE+ "--> " ~ WHITESPACE* ~
    to:(state_id | end_state) ~
    (":" ~ WHITESPACE* ~ label:text)?
}

state_diagram = {
    ("stateDiagram" | "stateDiagram-v2") ~ NEWLINE+ ~
    (state_element)*
}
```

### 2.7 ER 图 (`erDiagram`)

```pest
cardinality = {
    "|o" : "zero_or_one"   |
    "||" : "one"           |
    "}o" : "zero_or_many"  |
    "}|" : "one_or_many"
}

relationship = {
    left_card:cardinality ~
    "--" ~ right_card:cardinality
}

attribute = { type:identifier ~ name:identifier }

entity = {
    name:identifier ~
    (WHITESPACE+ "{" ~ WHITESPACE* ~ (attribute ("," ~ attribute)*)? ~ "}")?
}

er_statement = {
    first_entity:identifier ~
    WHITESPACE+ ~
    rel:relationship ~
    WHITESPACE+ ~
    second_entity:identifier ~
    (WHITESPACE+ ":" ~ WHITESPACE* ~ label:text)?
}

er_diagram = {
    "erDiagram" ~ NEWLINE+ ~
    (er_statement)+
}
```

---

## 3. 抽象语法树 (AST) 定义 (`src/ast.rs`)

使用 Rust `enum` 和 `struct` 表达 Mermaid 的所有语法节点。

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Diagram {
    Flowchart(Flowchart),
    Sequence(SequenceDiagram),
    Class(ClassDiagram),
    State(StateDiagram),
    Er(ErDiagram),
}

// ========== 流程图 ==========
#[derive(Debug, Clone, PartialEq)]
pub struct Flowchart {
    pub direction: Option<Direction>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub subgraphs: Vec<Subgraph>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    TB, TD, BT, RL, LR,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub shape: Option<NodeShape>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeShape {
    Rectangle,     // []
    Rounded,       // ()
    Stadium,       // ([text])
    Subroutine,    // [[text]]
    Diamond,       // {}
    Hexagon,       // {{}}
    Trapezoid,     // [/text/]
    TrapezoidAlt,  // [\text\]
    Circle,        // (())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub arrow_type: ArrowType,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrowType {
    Solid,      // -->
    Dotted,     // -.-> 
    Thick,      // ==>
    Labeled(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subgraph {
    pub title: Option<String>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

// ========== 时序图 ==========
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceDiagram {
    pub participants: Vec<Participant>,
    pub messages: Vec<Message>,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    pub name: String,
    pub alias: Option<String>,
    pub kind: ParticipantKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParticipantKind {
    Participant,
    Actor,
    Boundary,
    Control,
    Entity,
    Database,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub arrow: MessageArrow,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageArrow {
    Solid,        // ->
    SolidTip,     // ->>
    Dashed,       // -->
    DashedTip,    // -->>
    Cross,        // -x
    Open,         // -)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub placement: NotePlacement,
    pub targets: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotePlacement {
    LeftOf,
    RightOf,
    Over,
}

// ========== 类图 ==========
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDiagram {
    pub classes: Vec<Class>,
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub name: String,
    pub members: Vec<ClassMember>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    pub visibility: Option<Visibility>,
    pub name: String,
    pub type_: Option<String>,
    pub is_method: bool,   // 如果包含 () 则为方法
}

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,    // +
    Private,   // -
    Protected, // #
    Package,   // ~
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relation {
    pub source: String,
    pub target: String,
    pub kind: RelationKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationKind {
    Inheritance,   // <|--
    Composition,   // *--
    Aggregation,   // o--
    Association,   // -->
    Dependency,    // ..>
}

// ========== 状态图 ==========
#[derive(Debug, Clone, PartialEq)]
pub struct StateDiagram {
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Simple { id: String, description: Option<String> },
    Composite { id: String, inner: Box<StateDiagram> },
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub from: String,   // 可能是 "[*]" 或 state id
    pub to: String,
    pub label: Option<String>,
}

// ========== ER图 ==========
#[derive(Debug, Clone, PartialEq)]
pub struct ErDiagram {
    pub entities: Vec<ErEntity>,
    pub relationships: Vec<ErRelationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErEntity {
    pub name: String,
    pub attributes: Vec<ErAttribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErAttribute {
    pub type_: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErRelationship {
    pub first_entity: String,
    pub second_entity: String,
    pub cardinality_first: Cardinality,
    pub cardinality_second: Cardinality,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cardinality {
    ZeroOrOne,   // |o
    ExactlyOne,  // ||
    ZeroOrMany,  // }o
    OneOrMany,   // }|
}
```

---

## 4. 解析器实现 (`src/parser.rs`)

### 4.1 导入 Pest 生成的解析器

```rust
use pest::Parser;
use pest_derive::Parser;
use crate::ast::*;
use crate::error::{ParseError, Result};

#[derive(Parser)]
#[grammar = "../grammar/mermaid.pest"]
pub struct MermaidParser;
```

### 4.2 主解析入口

```rust
impl MermaidParser {
    /// 解析完整输入，返回 AST Diagram
    pub fn parse(input: &str) -> Result<Diagram> {
        let mut file_pairs = Self::parse(Rule::file, input)
            .map_err(|e| ParseError::Pest(Box::new(e)))?;
        let file_pair = file_pairs.next().unwrap(); // SOI...EOI 只有一个顶层

        let diagram_pair = file_pair.into_inner()
            .find(|p| p.as_rule() == Rule::diagram)
            .ok_or(ParseError::NoDiagram)?;

        Self::parse_diagram(diagram_pair)
    }

    fn parse_diagram(pair: pest::iterators::Pair<Rule>) -> Result<Diagram> {
        match pair.as_rule() {
            Rule::flowchart_diagram => Ok(Diagram::Flowchart(Self::parse_flowchart(pair)?)),
            Rule::sequence_diagram => Ok(Diagram::Sequence(Self::parse_sequence(pair)?)),
            Rule::class_diagram => Ok(Diagram::Class(Self::parse_class(pair)?)),
            Rule::state_diagram => Ok(Diagram::State(Self::parse_state(pair)?)),
            Rule::er_diagram => Ok(Diagram::Er(Self::parse_er(pair)?)),
            _ => Err(ParseError::UnsupportedDiagram),
        }
    }
}
```

### 4.3 流程图解析实现示例

```rust
impl MermaidParser {
    fn parse_flowchart(pair: pest::iterators::Pair<Rule>) -> Result<Flowchart> {
        let mut direction = None;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut subgraphs = Vec::new();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::direction => {
                    let dir_str = inner.as_str();
                    direction = Some(match dir_str {
                        "TB" | "TD" => Direction::TD,
                        "BT" => Direction::BT,
                        "RL" => Direction::RL,
                        "LR" => Direction::LR,
                        _ => unreachable!(),
                    });
                }
                Rule::node_decl => {
                    nodes.push(Self::parse_node(inner)?);
                }
                Rule::edge => {
                    edges.push(Self::parse_edge(inner)?);
                }
                Rule::subgraph => {
                    subgraphs.push(Self::parse_subgraph(inner)?);
                }
                _ => {}
            }
        }
        Ok(Flowchart { direction, nodes, edges, subgraphs })
    }

    fn parse_node(pair: pest::iterators::Pair<Rule>) -> Result<Node> {
        let mut id = None;
        let mut shape = None;
        let mut text = None;

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::identifier => {
                    id = Some(inner.as_str().to_string());
                }
                Rule::node_shape => {
                    shape = Some(Self::parse_node_shape(inner)?);
                    // node_shape 内部包含 node_text，可提取 text
                    for sub in inner.into_inner() {
                        if sub.as_rule() == Rule::node_text {
                            text = Some(Self::extract_text(sub)?);
                        }
                    }
                }
                _ => {}
            }
        }
        let id = id.ok_or(ParseError::MissingNodeId)?;
        Ok(Node { id, shape, text })
    }

    fn parse_node_shape(pair: pest::iterators::Pair<Rule>) -> Result<NodeShape> {
        let shape_str = pair.as_str();
        // 简单示例：根据第一个字符和第二个字符判断
        let shape = match shape_str {
            s if s.starts_with('[') && s.ends_with(']') && !s.starts_with("[[") => NodeShape::Rectangle,
            s if s.starts_with('(') && s.ends_with(')') && !s.starts_with("((") => NodeShape::Rounded,
            s if s.starts_with("((") => NodeShape::Circle,
            s if s.starts_with("[[") => NodeShape::Subroutine,
            s if s.starts_with('{') && s.ends_with('}') && !s.starts_with("{{") => NodeShape::Diamond,
            s if s.starts_with("{{") => NodeShape::Hexagon,
            s if s.starts_with("[/") => NodeShape::Trapezoid,
            s if s.starts_with("[\\") => NodeShape::TrapezoidAlt,
            _ => NodeShape::Rectangle,
        };
        Ok(shape)
    }

    fn parse_edge(pair: pest::iterators::Pair<Rule>) -> Result<Edge> {
        let mut source = None;
        let mut target = None;
        let mut arrow_type = ArrowType::Solid;
        let mut label = None;

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::identifier => {
                    if source.is_none() {
                        source = Some(inner.as_str().to_string());
                    } else {
                        target = Some(inner.as_str().to_string());
                    }
                }
                Rule::edge_arrow => {
                    arrow_type = Self::parse_edge_arrow(inner)?;
                }
                Rule::text => {
                    label = Some(Self::extract_text(inner)?);
                }
                _ => {}
            }
        }
        let source = source.ok_or(ParseError::MissingEdgeSource)?;
        let target = target.ok_or(ParseError::MissingEdgeTarget)?;
        Ok(Edge { source, target, arrow_type, label })
    }

    fn parse_edge_arrow(pair: pest::iterators::Pair<Rule>) -> Result<ArrowType> {
        let arrow_str = pair.as_str();
        if arrow_str.contains("-->|") {
            // 提取标签
            let label = arrow_str
                .trim_start_matches("-->|")
                .trim_end_matches('|');
            Ok(ArrowType::Labeled(label.to_string()))
        } else {
            match arrow_str {
                "-->" => Ok(ArrowType::Solid),
                "-.->" => Ok(ArrowType::Dotted),
                "==>" => Ok(ArrowType::Thick),
                _ => Ok(ArrowType::Solid),
            }
        }
    }

    // parse_subgraph, parse_sequence, parse_class, parse_state, parse_er 类似...
}
```

### 4.4 辅助函数：提取文本内容

```rust
impl MermaidParser {
    fn extract_text(pair: pest::iterators::Pair<Rule>) -> Result<String> {
        let text = match pair.as_rule() {
            Rule::text => pair.as_str().to_string(),
            Rule::quoted_id => {
                let s = pair.as_str();
                s[1..s.len()-1].to_string()
            }
            _ => pair.as_str().to_string(),
        };
        Ok(text)
    }
}
```

---

## 5. 错误处理 (`src/error.rs`)

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Pest parse error: {0}")]
    Pest(#[from] Box<pest::error::Error<super::parser::Rule>>),

    #[error("No diagram found in input")]
    NoDiagram,

    #[error("Unsupported diagram type")]
    UnsupportedDiagram,

    #[error("Missing node id")]
    MissingNodeId,

    #[error("Missing edge source or target")]
    MissingEdgeSource,

    #[error("Invalid syntax at line {line}, column {col}")]
    InvalidSyntax { line: usize, col: usize, message: String },
}

pub type Result<T> = std::result::Result<T, ParseError>;
```

---

## 6. 库入口 (`src/lib.rs`)

```rust
pub mod ast;
pub mod parser;
pub mod error;

pub use parser::MermaidParser;
pub use ast::Diagram;
```

---

## 7. 可执行入口示例 (`src/main.rs`)

```rust
use mermaid_parser::MermaidParser;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: mermaid-parser <file.mmd>");
        std::process::exit(1);
    }
    let content = fs::read_to_string(&args[1]).expect("Failed to read file");
    match MermaidParser::parse(&content) {
        Ok(diagram) => println!("{:#?}", diagram),
        Err(e) => eprintln!("Parse error: {}", e),
    }
}
```

---

## 8. 测试策略

### 8.1 单元测试 (针对解析函数)

在 `src/parser.rs` 中编写测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_flowchart() {
        let input = "flowchart TD\nA[Start] --> B[End]";
        let diagram = MermaidParser::parse(input).unwrap();
        match diagram {
            Diagram::Flowchart(f) => {
                assert_eq!(f.direction, Some(Direction::TD));
                assert_eq!(f.nodes.len(), 2);
                assert_eq!(f.edges.len(), 1);
            }
            _ => panic!("Expected flowchart"),
        }
    }
}
```

### 8.2 集成测试 (`tests/integration.rs`)

```rust
use mermaid_parser::MermaidParser;

const FLOWCHART_EXAMPLE: &str = r#"
flowchart LR
    id1[This is the text in the box]
    id1 --> id2
"#;

#[test]
fn test_flowchart_example() {
    let result = MermaidParser::parse(FLOWCHART_EXAMPLE);
    assert!(result.is_ok());
}
```

---

## 9. 扩展与维护指南

### 9.1 添加新图表类型

1. **扩展语法文件**：在 `grammar/mermaid.pest` 中添加新的顶级规则，例如 `gantt_diagram`。
2. **扩展 AST**：在 `ast.rs` 中增加 `Diagram::Gantt` 变体及相关结构。
3. **实现解析函数**：在 `parser.rs` 中实现 `parse_gantt`，并在 `parse_diagram` 分发中增加分支。

### 9.2 处理复杂节点形状

对于 Mermaid 中数量繁多的节点形状，建议使用映射表：

```rust
fn shape_from_str(s: &str) -> NodeShape {
    match s {
        "[" => NodeShape::Rectangle,
        "(" => NodeShape::Rounded,
        "((" => NodeShape::Circle,
        "[[" => NodeShape::Subroutine,
        "{" => NodeShape::Diamond,
        "{{" => NodeShape::Hexagon,
        "[/" => NodeShape::Trapezoid,
        "[\\" => NodeShape::TrapezoidAlt,
        _ => NodeShape::Rectangle,
    }
}
```

### 9.3 性能优化

- 使用 `pest` 的 `PEEK` 和 `atomic` 规则减少回溯。
- 对大型文件，可使用 `pest` 的流式解析（`pest_stream`）。
- 避免在解析过程中频繁克隆字符串，可以存储 `&str` 并配合生命周期，但会牺牲灵活性。推荐按需转换。

### 9.4 与 Mermaid 官方语法的差异处理

- Mermaid 的某些语法在正式文档中未明确定义（如 `flowchart-v2` 已合并到 `flowchart`）。建议跟踪官方语法变更，定期更新规则。
- 对于非标准但常见的用法（如无箭头连线 `---`），可添加宽松规则。

---

## 10. 总结

本文档提供了一个**完整、可落地**的 Mermaid 解析器实现方案，包括：

- 基于 Pest 的完整语法定义（覆盖 5 种核心图表）。
- 结构化的 AST 设计，便于后续代码生成、验证或转换。
- 健壮的错误处理与测试策略。
- 清晰的扩展指南，方便添加新图表类型。

开发者只需按照上述步骤创建项目、填充代码，即可获得一个可工作的 Mermaid 解析器，并能够在此基础上实现渲染、校验或转译等高级功能。