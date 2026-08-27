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
    pub meta: DiagramMeta,
}

impl Default for Unigraph {
    fn default() -> Self {
        Unigraph {
            family: GraphFamily::default(),
            direction: Direction::TB,
            nodes: Vec::new(),
            edges: Vec::new(),
            meta: DiagramMeta::default(),
        }
    }
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
    /// 边标签（Stage 1.5 测量后填充）。
    pub label: Option<MeasuredLabel>,
    pub priority: EdgePriority,
    pub routing_hint: RoutingHint,
    pub arrow: ArrowSpec,
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
