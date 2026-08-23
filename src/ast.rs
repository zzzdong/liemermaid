use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Diagram {
    Flowchart(Flowchart),
    Sequence(SequenceDiagram),
    Class(ClassDiagram),
    State(StateDiagram),
    Er(ErDiagram),
    Pie(PieDiagram),
    Timeline(TimelineDiagram),
    GitGraph(GitGraphDiagram),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flowchart {
    pub direction: Option<Direction>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub subgraphs: Vec<Subgraph>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    TB,
    TD,
    BT,
    RL,
    LR,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub shape: Option<NodeShape>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeShape {
    Rectangle,
    Rounded,
    Stadium,
    Subroutine,
    Diamond,
    Hexagon,
    Circle,
    DoubleCircle,
    Cylinder,
    Asymmetric,
    Parallelogram,
    ParallelogramAlt,
    Trapezoid,
    TrapezoidAlt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub arrow_type: ArrowType,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrowType {
    Solid,
    Dotted,
    Thick,
    /// 无箭头（`---`）
    NoArrow,
    /// 双向箭头（`<-->`）
    Both,
    /// 终点圆点（`--o`）
    Circle,
    /// 终点叉号（`--x`）
    Cross,
    /// 不可见边（`~~~`）
    Invisible,
    /// 双向圆点箭头（`o--o`）
    MultiCircle,
    /// 双向叉号箭头（`x--x`）
    MultiCross,
    Labeled(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subgraph {
    pub title: Option<String>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceDiagram {
    pub participants: Vec<Participant>,
    /// 顶层语句（消息、备注、分组块）按输入顺序排列
    pub statements: Vec<SequenceStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceStatement {
    Message(Message),
    Note(Note),
    Block(SequenceBlock),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceBlockKind {
    Loop,
    Alt,
    Opt,
    Par,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceBlock {
    pub kind: SequenceBlockKind,
    pub label: Option<String>,
    /// 块内的语句（消息、备注、嵌套块）
    pub items: Vec<SequenceItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceItem {
    Message(Message),
    Note(Note),
    Block(SequenceBlock),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub name: String,
    pub alias: Option<String>,
    pub kind: ParticipantKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantKind {
    Participant,
    Actor,
    Boundary,
    Control,
    Entity,
    Database,
    Collections,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageActivation {
    Activate,
    Deactivate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub arrow: MessageArrow,
    /// 箭头后的激活/取消激活快捷符号（`->>+B` 激活，`-->>-A` 取消）
    pub activation: Option<MessageActivation>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageArrow {
    Solid,
    SolidTip,
    Dashed,
    DashedTip,
    Cross,
    Open,
    /// 双向箭头（`<<->>` / `<<-->>`）
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub placement: NotePlacement,
    pub targets: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotePlacement {
    LeftOf,
    RightOf,
    Over,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassDiagram {
    pub classes: Vec<Class>,
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Class {
    pub name: String,
    /// 泛型参数（mermaid `~T~` 语法），如 `T`
    pub generic: Option<String>,
    /// 注解/构造型，如 `<<Interface>>`
    pub annotation: Option<String>,
    pub members: Vec<ClassMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassMember {
    pub visibility: Option<Visibility>,
    pub name: String,
    pub type_: Option<String>,
    pub is_method: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub source: String,
    pub target: String,
    pub kind: RelationKind,
    /// 源端基数（`"1"`），可选
    pub cardinality_first: Option<String>,
    /// 目标端基数（`"many"`），可选
    pub cardinality_second: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    Inheritance,
    Composition,
    Aggregation,
    Association,
    Dependency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDiagram {
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Simple {
        id: String,
        description: Option<String>,
    },
    Composite {
        id: String,
        inner: Box<StateDiagram>,
    },
    Fork {
        id: String,
    },
    Join {
        id: String,
    },
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErDiagram {
    pub entities: Vec<ErEntity>,
    pub relationships: Vec<ErRelationship>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErEntity {
    pub name: String,
    pub attributes: Vec<ErAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErAttribute {
    pub type_: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErRelationship {
    pub first_entity: String,
    pub second_entity: String,
    pub cardinality_first: Cardinality,
    pub cardinality_second: Cardinality,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cardinality {
    ZeroOrOne,
    ExactlyOne,
    ZeroOrMany,
    OneOrMany,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieDiagram {
    pub title: Option<String>,
    pub show_data: bool,
    pub data: Vec<PieData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieData {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineDiagram {
    pub title: Option<String>,
    /// 方向：`LR`（默认）或 `TD`
    pub direction: Option<TimelineDirection>,
    pub sections: Vec<TimelineSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineDirection {
    LR,
    TD,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineSection {
    pub name: String,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitGraphDiagram {
    pub statements: Vec<GitGraphStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitGraphStatement {
    Commit {
        id: Option<String>,
        commit_type: Option<String>,
        tag: Option<String>,
    },
    Branch {
        name: String,
    },
    Checkout {
        branch: String,
    },
    Merge {
        branch: String,
        id: Option<String>,
        tag: Option<String>,
        commit_type: Option<String>,
    },
    CherryPick {
        id: Option<String>,
        parent: Option<String>,
    },
}
