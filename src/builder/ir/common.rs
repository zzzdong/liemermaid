//! IR 公共基础类型：跨三层复用的端口 / 优先级 / 样式引用 / 标签等。
//!
//! 这些类型不依赖 [`crate::ast`]，是 AST 与 IR 之间的契约边界。

pub use lievisual::geometry::Size;
pub use lievisual::text::{RichSpan, TextLayout};
use std::sync::Arc;

/// 节点 ID（与 AST 节点 ID 同构，通常为字符串）。
pub type NodeId = String;
/// 边 ID。
pub type EdgeId = String;
/// 容器（子图 / 泳道 / 类框）ID。
pub type ContainerId = String;

/// 端口方位提示（布局求解前由 extract 推断的"期望出口方向"）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortHint {
    #[default]
    Auto,
    Top,
    Bottom,
    Left,
    Right,
}

/// 节点可用端口集合（布局求解前已知）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortSet {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl Default for PortSet {
    fn default() -> Self {
        PortSet { top: true, bottom: true, left: true, right: true }
    }
}

/// 端口实际坐标（布局求解后由 coord 计算）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedPorts {
    pub top: lievisual::geometry::Point,
    pub bottom: lievisual::geometry::Point,
    pub left: lievisual::geometry::Point,
    pub right: lievisual::geometry::Point,
}

/// 边优先级：影响交叉优化权重与路由代价。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgePriority {
    #[default]
    Primary,
    Secondary,
    Annotation,
}

/// 边路由提示：决定 EdgeRouter 选用的几何风格。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoutingHint {
    #[default]
    Orthogonal,
    Spline,
    Curved,
    Inherit,
}

/// 连线线型（视觉样式，materialize 据此设 dash / 宽度 / 透明度）。
/// 由 extract 从箭头语法（`-->` / `-.->` / `==>` / `~~~`）解析，贯穿到 GG。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineKind {
    #[default]
    Solid,
    Dotted,
    Thick,
    /// 不可见连线（`~~~`）：仍参与布局，但不渲染（透明）。
    Invisible,
}

/// 箭头规格（起止各自的标记类型），纯枚举，paint 阶段查表生成标记原语。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ArrowSpec {
    pub start: ArrowKind,
    pub end: ArrowKind,
}

/// 单端箭头类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArrowKind {
    #[default]
    None,
    Arrow,
    Circle,
    Cross,
}

/// 节点语义角色（供 layout family 与 materialize 区分处理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeRole {
    #[default]
    Atom,
    Container,
    Virtual,
    Subgraph,
}

/// 节点种类（布局拓扑层面）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeKind {
    #[default]
    Atom,
    Container,
    Virtual,
    Subgraph,
}

/// 节点尺寸约束。
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum NodeConstraint {
    /// 最小尺寸，可压缩到该值以下则报错。
    Min(Size),
    /// 固定尺寸，不可压缩。
    Fixed(Size),
    /// 无约束。
    #[default]
    Free,
}


/// 尺寸提示：决定布局阶段如何确定节点包围盒。
#[derive(Debug, Clone, PartialEq)]
#[derive(Default)]
pub enum SizeHint {
    /// 由文本测量 + 形状几何推算（多数节点）。
    #[default]
    ByText,
    /// 固定尺寸。
    Fixed(Size),
    /// 由子节点撑开（容器 / 子图）。
    FromChildren,
}


/// 样式引用：指向 Theme 中某条样式规则，materialize 阶段据此查具体颜色/线型。
///
/// 不内联颜色，保证 "UG 不含颜色" 的纪律。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StyleRef {
    #[default]
    NodeDefault,
    Class(String),
    EdgeDefault,
    ContainerDefault,
}

/// 标签（未测量）：extract 阶段的文本描述，measure 阶段转为 [`MeasuredLabel`]。
#[derive(Debug, Clone, Default)]
pub struct LabelSpec {
    pub text: String,
    pub spans: Vec<RichSpan>,
}

/// 已测量标签：measure 阶段产出，携带排版结果与尺寸。
#[derive(Debug, Clone)]
pub struct MeasuredLabel {
    pub text: String,
    pub spans: Vec<RichSpan>,
    pub layout: Arc<TextLayout>,
    pub size: Size,
}

/// 标签（未测或已测），measure 阶段从前者转为后者。
#[derive(Debug, Clone)]
pub enum LabelOrMeasured {
    Spec(LabelSpec),
    Measured(MeasuredLabel),
}

impl LabelOrMeasured {
    pub fn is_measured(&self) -> bool {
        matches!(self, LabelOrMeasured::Measured(_))
    }
    pub fn as_measured(&self) -> Option<&MeasuredLabel> {
        match self {
            LabelOrMeasured::Measured(m) => Some(m),
            _ => None,
        }
    }
}

/// 容器种类（供 materialize/route 区分背景与边框语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContainerKind {
    #[default]
    Subgraph,
    Lifeline,
    ClassBox,
    Slice,
}

/// 图级元信息（标题等游离文本）。
#[derive(Debug, Clone, Default)]
pub struct DiagramMeta {
    pub title: Option<String>,
}
