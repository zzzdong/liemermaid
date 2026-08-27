//! 第二层 IR：[`Geograph`]（GG，几何图）。
//!
//! 由 `layout` 阶段（solver + 路由）产出：坐标已定、尺寸已测、边已路由、
//! 标签锚点已定，**不含颜色**。是"几乎能画"的状态，只差视觉细节（Stage 3 注入）。

use lievisual::geometry::{Point, Rect, Size};

use super::common::*;
use super::shape::*;
use super::unigraph::EdgeKind;

/// 几何图（纯几何 + 已测量尺寸）。
#[derive(Debug, Clone, Default)]
pub struct Geograph {
    pub size: Size,
    pub background: lievisual::geometry::Color,
    pub nodes: Vec<GGNode>,
    pub edges: Vec<GGEdge>,
    pub containers: Vec<GGContainer>,
}

/// GG 节点（几何）。
#[derive(Debug, Clone)]
pub struct GGNode {
    pub id: NodeId,
    pub role: NodeRole,
    pub center: Point,
    pub size: Size,
    pub shape: ShapeKind,
    pub ports: ResolvedPorts,
    /// 节点标签（measure 阶段已测量），materialize 据此绘制文本。
    pub label: Option<super::common::MeasuredLabel>,
}

/// GG 边（已路由折线）。
#[derive(Debug, Clone)]
pub struct GGEdge {
    pub id: EdgeId,
    /// 端点节点 id（路由阶段用于查找节点几何；materialize/paint 不使用）。
    pub source: NodeId,
    pub target: NodeId,
    pub route: Vec<Point>,
    /// 边标签放置点（路由阶段已为标签预留空间）。
    pub label_anchor: Option<Point>,
    pub kind: EdgeKind,
    pub arrow: ArrowSpec,
    pub routing_hint: RoutingHint,
}

/// GG 容器（仅几何包围盒 + 标题候选）。
#[derive(Debug, Clone)]
pub struct GGContainer {
    pub id: ContainerId,
    pub bounds: Rect,
    pub title: Option<String>,
    pub kind: ContainerKind,
}
