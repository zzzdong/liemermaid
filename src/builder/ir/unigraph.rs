//! 第一层 IR：[`Unigraph`]（UG，统一拓扑图）。
//!
//! 由 `extract` 阶段从 [`crate::ast::Diagram`] 产出，是 AST 的唯一出口。
//! 含节点 / 边 / 端口 / 约束 / `GraphFamily`（决定 solver 策略），**不含颜色**。
//! 文本在 Stage 1 产出时为 [`LabelSpec`]（未测量），经 Stage 1.5 measure 后写回
//! [`MeasuredLabel`] 得到 UG'。

use crate::ast::Direction;

use super::{common::*, shape::*};

/// 图家族：决定 LayoutEngine 选用哪套 solver / 路由策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphFamily {
    /// flowchart / state：分层 + barycenter + 正交路由。
    #[default]
    Directed,
    /// class / er：网格 + 交叉减少 + 关系路由。
    Grid,
    /// mindmap / timeline：线性排布。
    Linear,
    /// sequence：泳道 + 消息时序路由。
    Sequence,
    /// pie / quadrant：极坐标。
    Radial,
    /// gitgraph / gantt / sankey：层级 / 时间轴。
    Hierarchy,
}

/// 统一拓扑图。
#[derive(Debug, Clone)]
pub struct Unigraph {
    pub family: GraphFamily,
    /// 主布局方向（TB/TD/BT/LR/RL），决定层轴与同层轴。
    /// extract 从 ast 透传；layout 据其旋转坐标（不依赖具体 family）。
    pub direction: Direction,
    pub nodes: Vec<UGNode>,
    pub edges: Vec<UGEdge>,
    /// 子图（subgraph / 泳道 / 类框）成员关系：layout 据此计算容器包围盒。
    pub subgraphs: Vec<UGSubgraph>,
    pub meta: DiagramMeta,
}

impl Default for Unigraph {
    fn default() -> Self {
        Unigraph {
            family: GraphFamily::default(),
            direction: Direction::TB,
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            meta: DiagramMeta::default(),
        }
    }
}

/// 子图（subgraph）规格：容器 id / 标题 / 成员节点 id 列表。
///
/// 仅描述"哪些节点属于哪个容器"，几何包围盒由 layout 阶段据成员节点坐标计算。
#[derive(Debug, Clone)]
pub struct UGSubgraph {
    pub id: String,
    pub title: Option<String>,
    pub member_ids: Vec<NodeId>,
}

/// UG 节点（语义拓扑，未含颜色）。
#[derive(Debug, Clone)]
pub struct UGNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub role: NodeRole,
    /// 几何形状类别（拓扑属性，非颜色）：layout 据其算端口/尺寸。
    pub shape: ShapeKind,
    /// Stage 1 为 [`LabelSpec`]；Stage 1.5 measure 后替换为 [`MeasuredLabel`]。
    pub label: LabelOrMeasured,
    pub ports: PortSet,
    pub size_hint: SizeHint,
    pub style_ref: StyleRef,
    pub constraint: NodeConstraint,
}

/// UG 边（语义连接，未含颜色）。
#[derive(Debug, Clone)]
pub struct UGEdge {
    pub id: EdgeId,
    pub source: NodeId,
    pub target: NodeId,
    pub source_port: PortHint,
    pub target_port: PortHint,
    pub kind: EdgeKind,
    /// 边标签文本（Stage 1 extract 填原文，Stage 1.5 measure 据此测量出 [`MeasuredLabel`]）。
    pub label_text: Option<String>,
    /// 边标签（Stage 1.5 测量后填充）。
    pub label: Option<MeasuredLabel>,
    pub priority: EdgePriority,
    pub routing_hint: RoutingHint,
    pub arrow: ArrowSpec,
    /// 线型（实线 / 虚线 / 粗线 / 不可见），来自箭头语法，materialize 据此设样式。
    pub line_kind: LineKind,
    /// 与其他边 / 节点的排斥强度（默认 1.0）。
    pub repulsion: f64,
}

/// 边语义类别（驱动 materialize 选线型 / 箭头表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeKind {
    #[default]
    Flow,
    StateTransition,
    ClassExtends,
    ClassComposition,
    ClassAggregation,
    ClassAssociation,
    ClassDependency,
    ClassRealization,
    ClassLink,
    ClassDashed,
    SequenceMessage,
    PieConnection,
    Generic,
}
