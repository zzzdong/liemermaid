//! 第三层 IR：[`SceneGraph`]（SG，视觉自足 IR）。
//!
//! 由 `materialize` 阶段（消费 GG + StyleIntent + Theme）产出：颜色 / 线型 / 字体已解析，
//! 与 [`crate::ast`] 完全解耦。是 `paint` 的唯一输入；`paint` 将其纯机械翻译成
//! `lievisual::Scene` 原语，不再有任何图类型判断或 theme 硬编码。

use lievisual::geometry::{Color, Point, Size};
use lievisual::scene::{Fill, Stroke};
use lievisual::text::{RichSpan, TextStyle};

use super::common::*;
use super::shape::*;
use super::unigraph::EdgeKind;

/// 视觉自足的场景图。
#[derive(Debug, Clone, Default)]
pub struct SceneGraph {
    pub size: Size,
    pub background: Color,
    /// 按 z_index 升序，painter 直接遍历。
    pub items: Vec<SceneItem>,
}

/// 场景绘制项（视觉自足）。
#[derive(Debug, Clone)]
pub enum SceneItem {
    /// 形状（含容器 / 节点 / 扇区）。
    Shape {
        geometry: ShapeGeometry,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        z: i32,
    },
    /// 连线（已含箭头标记作为 ends 描述）。
    Edge {
        path: Vec<Point>,
        stroke: Stroke,
        ends: EdgeEnds,
        z: i32,
    },
    /// 文本（已测量，含布局）。
    Label {
        text: Vec<RichSpan>,
        position: Point,
        style: TextStyle,
        anchor: Anchor,
        z: i32,
    },
    /// 分组（仅用于 z / clip 管理，子项已展平到 items）。
    Group {
        children: Vec<SceneItem>,
        z: i32,
    },
}

/// 文本对齐锚点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

/// 视觉意图：LayoutEngine 在 layout 结束时从 UG 抽取的轻量结构，
/// 供 materialize 查 Theme 解析具体颜色 / 线型。**不含几何坐标**。
///
/// 这样 UG 可在 layout 后即可 drop，materialize / paint 不持有 UG 引用。
#[derive(Debug, Clone, Default)]
pub struct StyleIntent {
    pub node_styles: Vec<(NodeId, StyleRef)>,
    pub edge_styles: Vec<(EdgeId, EdgeKind, ArrowSpec)>,
    pub container_styles: Vec<(ContainerId, ContainerKind)>,
}
