//! # 统一布局原则 (Unified Layout Principles)
//!
//! 所有图表类型的布局算法须遵循以下统一原则，确保排布自然、符合人类直观。
//!
//! ## 1. 拓扑分层 (Topological Layering)
//!
//! ```text
//! 原则: 有向图的源节点在上，目标节点在下（TD 方向）。
//!       所有关系边均参与拓扑排序，不仅限于特定类型。
//! ```
//!
//! - 所有关系边（继承、组合、聚合、关联、依赖等）都加入有向图
//! - 入度为 0 的节点为根（Root），分配第 0 层
//! - 强连通分量（SCC/环路）内的节点共享同一层
//! - 无任何边的孤立节点默认分配第 0 层
//! - 层号越小的节点在视觉上越靠上（或靠左）
//!
//! ## 2. 层内对齐 (Layer Alignment)
//!
//! ```text
//! 原则: 同层所有节点的垂直中心对齐，基于该层的最大高度。
//!       主流程节点对齐到同一条垂直中心线。
//! ```
//!
//! - `center_y = layer_top + layer_max_h / 2` (不使用单个节点的 `size.height`)
//! - SCC 入口节点位于中心线上，其它 SCC 内节点水平排列于右侧
//! - 连接到 SCC 入口节点的外部节点（入边源/出边目标）对齐到中心线
//! - 单前驱链式节点（predecessor_count ≤ 1）沿下游传播中心线对齐
//!
//! ## 3. 节点定位 (Node Positioning)
//!
//! ```text
//! 原则: 节点间有最小间距，同层节点均匀分布，不重叠。
//! ```
//!
//! - 水平间距: `config.node_gap` (默认 60px)
//! - 垂直间距: `config.layer_gap` (默认 60px)
//! - 同层节点按名称或位置排序后等距排布
//!
//! ## 4. 边路由 (Edge Routing)
//!
//! ```text
//! 原则: 边不得穿越任何节点。
//!       同层边优先水平连接，跨层边用正交折线。
//! ```
//!
//! ### 同层边 (Same-Layer)
//! - 源在左、目标在右，且无中间节点 → 直接水平线
//! - 源在左、目标在右，但有中间节点 → 三段正交绕行：
//!   `(源右侧, 中心_y) → (源右侧, 行上方) → (目标左侧, 行上方) → (目标左侧, 中心_y)`
//! - 反馈边（SCC 内回指）→ 从目标底部绕行：
//!   `(源底部, 源_y) → 下降到 行下方 → 水平到目标下方 → 上行到目标底部`
//!
//! ### 跨层边 (Cross-Layer)
//! - 源在目标正上方 → 直接垂直线
//! - 源在目标上方但偏左/右 → 正交折线（2 个弯）
//! - 层差 ≥ 2 且同 X → 向右偏移 30px 绕行中间层节点
//!
//! ### 箭头与装饰
//! - 每个边段的起终点方向向量均需归一化（unit vector）
//! - 菱形头（组合/聚合）在起点端，箭头（继承/关联/依赖）在终点端
//!
//! ## 5. SCC 环路布局 (Loop/Cycle Layout)
//!
//! ```text
//! 原则: 环路节点水平排列在同一层，入口节点在中心线。
//! ```
//!
//! 1. 用 Tarjan 算法检测强连通分量
//! 2. 构建 SCC 凝结图（Condensation DAG）
//! 3. 在凝结图上分配层号，SCC 内节点共享同一层
//! 4. 入口节点（有外部入边的节点）居中心线
//! 5. 其它 SCC 节点从入口节点右侧水平排列
//! 6. 所有外部入边源对齐到入口节点 X
//! 7. 所有外部出边目标对齐到入口节点 X，并向下游传播
//!
//! ## 6. 布局验证 (Layout Verification)
//!
//! 每次布局后应自动验证：
//! - 无节点重叠（bounding box 无相交）
//! - 无边穿越节点（边路径与节点 box 无相交）
//! - 同层节点中心 Y 偏差 < 1px
//! - 主流程节点 X 偏差 < 1px
//! - 箭头/菱形头向量已归一化

use std::collections::HashMap;

use vello_cpu::kurbo::{Point, Rect};

use crate::ast::{Edge, NodeShape, Direction};
use crate::builder::types::OutputConfig;
use crate::error::DiagramResult;
use crate::visual::{Color, VisualElement};

use super::coord::NodeAnchors;

/// 节点 ID
pub type NodeId = String;
/// Group ID
pub type GroupId = usize;

// ===== Pass 1: 结构识别 =====

/// 逻辑子结构树
#[derive(Debug, Clone)]
pub enum LogicalGroup {
    Chain {
        items: Vec<ChainItem>,
    },
    Branch {
        source: NodeId,
        arms: Vec<BranchArm>,
        sink: Option<NodeId>,
    },
    Cycle {
        condition: NodeId,
        body: Box<LogicalGroup>,
        exit: Option<NodeId>,
    },
    Leaf {
        node_id: NodeId,
    },
}

#[derive(Debug, Clone)]
pub struct ChainItem {
    pub node_id: Option<NodeId>,
    pub sub_group: Option<Box<LogicalGroup>>,
    pub label: Option<String>,
}

impl ChainItem {
    pub fn leaf(node_id: NodeId) -> Self {
        Self {
            node_id: Some(node_id),
            sub_group: None,
            label: None,
        }
    }

    pub fn group(group: LogicalGroup) -> Self {
        Self {
            node_id: None,
            sub_group: Some(Box::new(group)),
            label: None,
        }
    }

    pub fn is_group(&self) -> bool {
        self.sub_group.is_some()
    }

    pub fn is_leaf(&self) -> bool {
        self.node_id.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct BranchArm {
    pub label: Option<String>,
    pub body: LogicalGroup,
}

#[derive(Debug, Clone)]
pub struct GroupEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub edge: Edge,
    pub from_group: GroupId,
    pub to_group: GroupId,
}

/// Pass 1 输出：识别后的结构树
#[derive(Debug, Clone)]
pub struct LayoutTree {
    pub root: LogicalGroup,
    pub orphan_edges: Vec<GroupEdge>,
}

// ===== Pass 2: 尺寸测量 =====

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

/// 逻辑组的内部布局参数
#[derive(Debug, Clone)]
pub enum InternalLayout {
    Chain {
        item_sizes: Vec<Size>,
        total_main: f64,
        max_cross: f64,
    },
    Branch {
        source_size: Size,
        branch_sizes: Vec<Size>,
        sink_size: Option<Size>,
    },
    Cycle {
        condition_size: Size,
        body_size: Size,
        exit_size: Option<Size>,
    },
}

/// 组的尺寸度量
#[derive(Debug, Clone)]
pub struct GroupMetrics {
    pub size: Size,
    pub internal: InternalLayout,
}

/// Pass 2 输出
#[derive(Debug, Clone)]
pub struct LayoutMetrics {
    pub node_metrics: HashMap<NodeId, NodeMetrics>,
    pub group_metrics: HashMap<GroupId, GroupMetrics>,
}

// ===== Pass 5: 几何定位 =====

#[derive(Debug, Clone)]
pub struct NodePosition {
    pub center: Point,
    pub anchors: NodeAnchors,
}

// ===== Pass 7: 边路由 =====

#[derive(Debug, Clone)]
pub struct RoutedEdge {
    pub edge: Edge,
    pub route: Vec<Point>,
    pub label_position: Option<(Point, f64)>,
}

// ===== 全局布局结果 =====

#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub node_positions: HashMap<NodeId, NodePosition>,
    pub group_bounds: HashMap<GroupId, Rect>,
    pub routed_edges: Vec<RoutedEdge>,
    pub canvas_size: Size,
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
}

/// 布局元数据
#[derive(Debug, Clone)]
pub struct LayoutMetadata {
    pub direction: Direction,
}

/// 布局配置参数（与 OutputConfig 分离，纯布局算法参数）
#[derive(Debug, Clone, Copy)]
pub struct LayoutConfig {
    pub node_gap: f64,
    pub layer_gap: f64,
    pub font_size: f64,
    pub padding: f64,
    pub arrow_size: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            node_gap: 60.0,
            layer_gap: 60.0,
            font_size: 13.0,
            padding: 14.0,
            arrow_size: 8.0,
        }
    }
}

/// 统一布局中间表示（Layout IR），与画布/渲染器无关
#[derive(Debug, Clone)]
pub struct Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub size: Size,
    pub metadata: LayoutMetadata,
}

/// 布局引擎 trait：每种图表类型实现自己的布局逻辑
///
/// 通过此 trait 将"布局算法"与"具体图表类型"解耦，
/// 每种图表内部的布局管线各不相同，但对外暴露统一的入口。
pub trait LayoutEngine {
    /// 执行布局管线，输出视觉元素
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<VisualElement>>;
}
