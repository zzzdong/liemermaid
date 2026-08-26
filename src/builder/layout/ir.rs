//! 布局中间表示（Layout IR）。
//!
//! 定义布局管线的**输入**（[`LayoutGraph`]：纯拓扑 + 尺寸 + 少量线型/标题语义）
//! 与**输出**（[`PlacedGraph`]：纯几何坐标）。
//!
//! IR 原则：承载「布局需要的一切拓扑事实 + 极少几何语义」，**不含颜色 / 标签 / 箭头**。
//! 颜色、标签、箭头等渲染语义由渲染层回查 AST（[`crate::ast`]）的 `NodeShape` / `ArrowType` 决定。
//!
//! 概念 → IR 映射（开放概念，实现可增删字段）：
//! - `title` → [`LayoutGraph::title`]（图标题）+ [`LGroup::title`]（子图标题）
//! - `node`  → [`LNode`]（id + 尺寸 + 形状类别）
//! - `edge`  → [`LEdge`]（拓扑连接 + 端口 + 线型类别）
//! - `line`  → [`LineKind`]（线型类别）+ [`PlacedGraph::edge_routes`]（几何）
//! - `group` → [`LGroup`]（递归嵌套子图树）

use lievisual::geometry::{Point, Rect, Size};

/// 布局输入：纯拓扑 + 尺寸约束 + 少量线型/标题语义。
///
/// `nodes` / `edges` / `groups` 的顺序**严格等于 AST 源码出现顺序**，
/// 这是「确定性锚定」的基础——只要代码不动，布局就永不抖动。
#[derive(Debug, Clone, Default)]
pub struct LayoutGraph {
    /// 图标题（如 flowchart 的标题、pie / timeline 的 title）。
    pub title: Option<String>,
    /// 节点（与 AST 源码顺序一致）。
    pub nodes: Vec<LNode>,
    /// 边（与 AST 源码顺序一致）。
    pub edges: Vec<LEdge>,
    /// 组树（索引即源码顺序），子图 / 复合状态容器。
    pub groups: Vec<LGroup>,
    /// 真正连接两个组的跨组边（转换时收集，供 `GroupedDirected` 使用）。
    pub cross_group_edges: Vec<LEdge>,
}

/// 布局节点：只含尺寸 + 形状类别，不含颜色。
#[derive(Debug, Clone)]
pub struct LNode {
    /// 原始节点 ID（映射回 AST / 渲染层）。
    pub id: String,
    /// 节点包围盒尺寸（由 `Measure` 测量，含内边距）。
    pub size: Size,
    /// 形状类别（仅影响锚点 / 裁剪，不影响渲染颜色）。
    pub shape_hint: ShapeHint,
}

/// 节点形状类别（几何语义，用于锚点 / 裁剪计算）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeHint {
    Rect,
    Rounded,
    Diamond,
    Circle,
    /// fork / join 横线。
    Bar,
}

/// 布局边：拓扑连接 + 端口提示 + 线型类别。
#[derive(Debug, Clone)]
pub struct LEdge {
    /// 源节点索引（`LayoutGraph.nodes` 下标）。
    pub source: usize,
    /// 目标节点索引（`LayoutGraph.nodes` 下标）。
    pub target: usize,
    /// 源端口提示。
    pub source_port: PortHint,
    /// 目标端口提示。
    pub target_port: PortHint,
    /// 线型类别（几何拓扑语义，不是颜色 / 箭头）。
    pub line_kind: LineKind,
}

/// 连线类别：告诉求解器 / 渲染层「这条边怎么画、路由上有什么特殊约束」。
///
/// 它是拓扑语义，不承载颜色；具体颜色 / 箭头样式仍由渲染层查 AST 决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// 普通实线（正交折线）。
    Solid,
    /// 虚线（渲染层采样成 dash）。
    Dashed,
    /// 无箭头实线（`---`，渲染层不画箭头）。
    NoArrow,
    /// 粗实线（`==>`，渲染层使用更粗的 stroke）。
    Thick,
    /// 自环（求解器在节点一侧生成小环）。
    SelfLoop,
    /// 双向（路由时错开两条线）。
    Bidirectional,
    /// 贝塞尔曲线（无箭头 / 圆点等特殊终点）。
    Curved,
    /// 不可见（占位，仅参与拓扑不绘制）——如 flowchart `~~~`。
    Invisible,
}

/// 端口提示（几何拓扑语义，不是箭头）。`Auto` 交给求解器按最短边决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortHint {
    Auto,
    Top,
    Bottom,
    Left,
    Right,
}

/// 布局组（子图 / 复合状态容器），递归嵌套。
#[derive(Debug, Clone)]
pub struct LGroup {
    /// 子图标题（参与容器尺寸计算，渲染层绘制）。
    pub title: Option<String>,
    /// 成员（节点 / 子组），顺序即源码顺序。
    pub children: Vec<GroupChild>,
}

/// 组成员。
#[derive(Debug, Clone, Copy)]
pub enum GroupChild {
    Node(usize),
    Group(usize),
}

// ---------------------------------------------------------------------------

/// 布局输出：仅几何数据。数组顺序与 `LayoutGraph` 一一对应。
#[derive(Debug, Clone)]
pub struct PlacedGraph {
    /// 节点中心坐标，与 `LayoutGraph.nodes` 同序。
    pub positions: Vec<Point>,
    /// 边路径（折线 / 贝塞尔采样点），与 `LayoutGraph.edges` 同序。
    pub edge_routes: Vec<Vec<Point>>,
    /// 每条边的线型类别，与 `edge_routes` 同序（供渲染层区分虚线 / 粗线等）。
    pub edge_kinds: Vec<LineKind>,
    /// 组包围盒，与 `LayoutGraph.groups` 同序。
    pub group_bounds: Vec<Rect>,
    /// 整体画布尺寸（内容实际占据的 bbox）。
    pub size: Size,
}

impl PlacedGraph {
    /// 平移所有几何使 (0,0) 为左上（渲染前归一化）。
    ///
    /// 对 `positions` 与 `edge_routes` 求全局最小，整体平移使 min = 0。
    pub fn normalize(&mut self) {
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        for p in &self.positions {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
        }
        for route in &self.edge_routes {
            for p in route {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
            }
        }
        for b in &self.group_bounds {
            min_x = min_x.min(b.min_x());
            min_y = min_y.min(b.min_y());
        }
        if !min_x.is_finite() {
            return;
        }
        for p in self.positions.iter_mut() {
            p.x -= min_x;
            p.y -= min_y;
        }
        for route in self.edge_routes.iter_mut() {
            for p in route.iter_mut() {
                p.x -= min_x;
                p.y -= min_y;
            }
        }
        for b in self.group_bounds.iter_mut() {
            *b = Rect::new(
                b.min_x() - min_x,
                b.min_y() - min_y,
                b.max_x() - min_x,
                b.max_y() - min_y,
            );
        }
    }

    /// 内容几何中心（positions 的 bbox 中心）。
    pub fn center(&self) -> Point {
        if self.positions.is_empty() {
            return Point::ZERO;
        }
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in &self.positions {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        Point::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
    }
}
