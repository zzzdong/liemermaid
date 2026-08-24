//! # 布局类型 (Layout Types)
//!
//! 定义所有图表共用的布局中间表示（Layout IR）与 [`LayoutEngine`] trait。
//! flowchart 的布局由 dagre（`layout::dagre_layout`）求解，
//! 其余图表（如 stateDiagram）走各自路径，最终都产出 `Vec<SceneNode>`，
//! 统一对接 lievisual 的 `Scene`。

use lievisual::geometry::{Point, Rect};

use crate::ast::{Direction, NodeShape};
use crate::builder::types::OutputConfig;
use crate::error::DiagramResult;
use lievisual::geometry::Color;
use lievisual::scene::SceneNode;

use super::coord::NodeAnchors;

/// 节点 ID
pub type NodeId = String;

// ===== 节点尺寸测量 =====

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// 单个节点的尺寸度量
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    pub size: Size,
    pub anchors: NodeAnchors,
}

// ===== 统一 Layout IR =====

/// 布局节点的样式信息
#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub fill_color: Option<Color>,
    pub stroke_color: Option<Color>,
    pub stroke_width: f64,
    pub font_size: f64,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            fill_color: None,
            stroke_color: None,
            stroke_width: 2.0,
            font_size: 13.0,
        }
    }
}

/// 布局中间表示：节点
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: NodeId,
    pub bounds: Rect,
    pub ports: Vec<Point>,
    pub label: Option<String>,
    pub shape: Option<NodeShape>,
    pub style: NodeStyle,
}

/// 布局中间表示：边
#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub path: Vec<Point>,
    pub arrow_at_end: bool,
    pub label: Option<String>,
    pub label_position: Option<Point>,
    /// 是否使用贝塞尔曲线渲染（否则正交折线）
    pub curved: bool,
}

/// 子图（subgraph）的布局容器信息
#[derive(Debug, Clone)]
pub struct LayoutSubgraph {
    /// 子图标题（无则为 None）
    pub title: Option<String>,
    /// 子图包含的成员节点 id 列表
    pub member_ids: Vec<NodeId>,
    /// 子图容器包围盒（由成员节点包围盒外扩 padding 得到）
    pub bounds: Rect,
}

/// 布局元数据
#[derive(Debug, Clone)]
pub struct LayoutMetadata {
    pub direction: Direction,
}

/// 统一布局中间表示（Layout IR），与画布/渲染器无关
#[derive(Debug, Clone)]
pub struct Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub size: Size,
    pub metadata: LayoutMetadata,
    /// 子图容器列表（渲染时作为背景框 + 标题）
    pub subgraphs: Vec<LayoutSubgraph>,
}

/// 布局引擎 trait：每种图表类型实现自己的布局逻辑
///
/// 通过此 trait 将"布局算法"与"具体图表类型"解耦，
/// 每种图表内部的布局管线各不相同，但对外暴露统一的入口。
pub trait LayoutEngine {
    /// 执行布局管线，输出视觉元素
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<SceneNode>>;
}
