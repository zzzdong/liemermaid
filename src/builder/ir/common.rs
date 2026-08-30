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
        PortSet {
            top: true,
            bottom: true,
            left: true,
            right: true,
        }
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
    /// 时序图生命线（participant）。
    Lifeline,
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SizeHint {
    /// 由文本测量 + 形状几何推算（多数节点）。
    ///
    /// 各形状的 padding 取自 `measure::shape_padding`（实测自官方 golden）。
    #[default]
    ByText,
    /// 固定尺寸。
    Fixed(Size),
    /// 由子节点撑开（容器 / 子图）。
    FromChildren,
    /// 由文本测量 + **显式** padding 推算。
    ///
    /// 用于「同形状但 padding 不同」的图表：state 节点与 flowchart 圆角节点
    /// 同为 [`crate::builder::ir::shape::ShapeKind::Rounded`]，但官方 state 节点
    /// padding=8（如 `Idle` → 41.8×40），flowchart 为 30/15（如 `Start` → 93.8×54）。
    Padded { pad_x: f64, pad_y: f64 },
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
    /// state 复合状态容器（白底 + 标题在顶部，区别于 flowchart 淡黄 subgraph）。
    StateComposite,
    /// 时序图分组块（loop / alt / opt / par）。
    SequenceBlock,
    /// git 分支（容器：成员 = 该分支的 commit 节点）。
    GitBranch,
}

/// 图级元信息（标题等游离文本）。
#[derive(Debug, Clone, Default)]
pub struct DiagramMeta {
    pub title: Option<String>,
    /// pie 图是否显示数据值（`showData`）。
    pub show_data: bool,
}

/// ER 关系基数（端点装饰，materialize 据此绘制 `||` / `|o` / `}|` / `}o`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErCardinality {
    ZeroOrOne,
    ExactlyOne,
    ZeroOrMany,
    OneOrMany,
}

/// ER 实体属性（类型 + 名，分两列绘制）。
#[derive(Debug, Clone, PartialEq)]
pub struct EntityAttr {
    pub type_: String,
    pub name: String,
}

/// 结构化节点详情（类框 / 实体框等多栏节点）。
///
/// extract 阶段从 AST 填充（成员行已格式化为字符串），measure 阶段据此
/// 计算多栏总尺寸，materialize 阶段据此绘制分栏 + 分隔线。
/// 节点标题（类名 / 实体名，含泛型）由 [`LabelSpec`] / [`MeasuredLabel`] 承载。
#[derive(Debug, Clone, PartialEq, Default)]
pub enum NodeDetail {
    /// 默认：普通单栏节点（flowchart / state 等）。
    #[default]
    None,
    /// UML 类框：header（类名 + 注解）+ attrs 栏 + methods 栏（三栏）。
    Class {
        /// 注解 / 构造型（如 `Interface`，绘制为 `«Interface»`）。
        annotation: Option<String>,
        /// 属性行（已格式化，如 `+ id: int`）。
        attrs: Vec<String>,
        /// 方法行（已格式化，如 `+ foo(): void`）。
        methods: Vec<String>,
    },
    /// ER 实体框：header（实体名）+ 属性栏（属性分 type / name 两列）。
    Entity {
        /// 属性列表（类型列 + 名称列）。
        attrs: Vec<EntityAttr>,
    },
    /// 时间轴 section 列：节点标签为 section 名，`events` 携带该列的事件文本。
    /// Linear 家族：布局按列排布，materialize 画 section 块 / 时间点 / 事件块 / 连线。
    TimelineSection {
        /// 该 section 下的事件文本列表（按源码序）。
        events: Vec<String>,
    },
    /// 时序图备注（Note）：不属于参与者节点，作为特殊节点由 materialize 绘制备注框。
    SequenceNote {
        /// 备注文本。
        text: String,
        /// 覆盖的参与者名列表（对应 Lifeline 节点 id）。
        targets: Vec<String>,
        /// 备注放置位置（决定框相对目标列的横向偏移）。
        placement: SequenceNotePlacement,
    },
    /// pie 图扇区：节点不占据位置（所有扇区叠于原点），materialize 据数据算角度绘制。
    PieSlice {
        /// 数据项标签。
        label: String,
        /// 数值（extract 已解析）。
        value: f64,
    },
    /// git 提交点：节点中心即提交位置（engine Hierarchy 布局），materialize 据信息绘制。
    GitCommit {
        /// 所属分支名。
        branch: String,
        /// 显式提交 id（`commit id: "abc"`），可空；为空时 materialize 用节点 id 作显示标签。
        id: Option<String>,
        /// 提交标签（`commit tag: "v1"`），可空。
        tag: Option<String>,
        /// 提交类型（`commit type: "HIGHLIGHT"`），可空；materialize 据此选择绘制样式。
        commit_type: Option<String>,
        /// 是否为合并提交（分支色外圆 + 白芯）。
        is_merge: bool,
    },
}

/// 时序图备注的放置位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SequenceNotePlacement {
    /// `Note over A,B`：横跨目标列。
    #[default]
    Over,
    /// `Note left of A`：在目标左侧。
    LeftOf,
    /// `Note right of A`：在目标右侧。
    RightOf,
}
