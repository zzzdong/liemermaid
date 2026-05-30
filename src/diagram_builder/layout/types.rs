use std::collections::HashMap;

use vello_cpu::kurbo::{Point, Rect};

use crate::ast::Edge;

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
