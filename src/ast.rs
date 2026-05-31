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
    pub messages: Vec<Message>,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub name: String,
    pub alias: Option<String>,
    pub kind: ParticipantKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantKind {
    Participant,
    Actor,
    Boundary,
    Control,
    Entity,
    Database,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub arrow: MessageArrow,
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
    Simple { id: String, description: Option<String> },
    Composite { id: String, inner: Box<StateDiagram> },
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
    pub sections: Vec<TimelineSection>,
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
    Commit { tag: Option<String> },
    Branch { name: String },
    Checkout { branch: String },
    Merge { branch: String },
}