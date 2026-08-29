//! 第二层 IR：[`Geograph`]（GG，几何图）。
//!
//! 由 `layout` 阶段（solver + 路由）产出：坐标已定、尺寸已测、边已路由、
//! 标签锚点已定，**不含颜色**。是"几乎能画"的状态，只差视觉细节（Stage 3 注入）。

use lievisual::geometry::{Point, Rect, Size};

use super::common::*;
use super::shape::*;
use super::unigraph::EdgeKind;

/// 路由段：直线或三次贝塞尔。
///
/// 路由阶段（`layout/route.rs`）输出连续段序列（相邻段首尾相接），
/// materialize / paint 按段类型决定画 `<line>` 或 `<path d="M..C..">`。
#[derive(Debug, Clone, Copy)]
pub enum RouteSegment {
    /// 直线段。
    Line { from: Point, to: Point },
    /// 三次贝塞尔 p0→p3（控制点 p1, p2）。
    CubicBezier { p0: Point, p1: Point, p2: Point, p3: Point },
}

impl RouteSegment {
    pub fn start(&self) -> Point {
        match self {
            RouteSegment::Line { from, .. } => *from,
            RouteSegment::CubicBezier { p0, .. } => *p0,
        }
    }
    pub fn end(&self) -> Point {
        match self {
            RouteSegment::Line { to, .. } => *to,
            RouteSegment::CubicBezier { p3, .. } => *p3,
        }
    }
}

/// 路由路径 = 连续的路由段序列（相邻段首尾相接）。
#[derive(Debug, Clone, Default)]
pub struct RoutePath(pub Vec<RouteSegment>);

impl RoutePath {
    pub fn new() -> Self {
        RoutePath(Vec::new())
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn first(&self) -> Option<&RouteSegment> {
        self.0.first()
    }
    pub fn last(&self) -> Option<&RouteSegment> {
        self.0.last()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, RouteSegment> {
        self.0.iter()
    }
    pub fn push(&mut self, seg: RouteSegment) {
        self.0.push(seg);
    }
    pub fn extend(&mut self, segs: impl IntoIterator<Item = RouteSegment>) {
        self.0.extend(segs);
    }
    pub fn start(&self) -> Point {
        self.first().map(RouteSegment::start).unwrap_or(Point::new(0.0, 0.0))
    }
    pub fn end(&self) -> Point {
        self.last().map(RouteSegment::end).unwrap_or(Point::new(0.0, 0.0))
    }
    /// 首段方向单位向量（源端口出方向）。对贝塞尔段取起点切线 `p1 - p0`。
    pub fn first_direction(&self) -> Point {
        let Some(seg) = self.first() else { return Point::new(0.0, 0.0) };
        match seg {
            RouteSegment::Line { from, to } => norm(from, to),
            RouteSegment::CubicBezier { p0, p1, .. } => norm(p0, p1),
        }
    }
    /// 末段方向单位向量（指向目标端口，即入口方向）。对贝塞尔段取终点切线 `p3 - p2`。
    pub fn last_direction(&self) -> Point {
        let Some(seg) = self.last() else { return Point::new(0.0, 0.0) };
        match seg {
            RouteSegment::Line { from, to } => norm(from, to),
            RouteSegment::CubicBezier { p2, p3, .. } => norm(p2, p3),
        }
    }
    /// 中段中点（边标签锚点）。
    pub fn midpoint(&self) -> Point {
        if self.is_empty() {
            return Point::new(0.0, 0.0);
        }
        if self.len() == 1 {
            let s = self.0[0].start();
            let e = self.0[0].end();
            return Point::new((s.x + e.x) / 2.0, (s.y + e.y) / 2.0);
        }
        let mid = self.len() / 2;
        let a = self.0[mid - 1].end();
        let b = self.0[mid].start();
        Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    }
    /// 遍历所有锚点（线段端点 + 贝塞尔端点 p0/p3），用于碰撞/回避计算。
    pub fn anchors(&self) -> Vec<Point> {
        let mut out = Vec::new();
        for seg in &self.0 {
            out.push(seg.start());
        }
        if let Some(last) = self.last() {
            out.push(last.end());
        }
        out
    }
}

/// 归一化方向向量（a→b）。
fn norm(a: &Point, b: &Point) -> Point {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        Point::new(0.0, 0.0)
    } else {
        Point::new(dx / len, dy / len)
    }
}

/// 便捷构造：从折线点序列直接构建（纯线段）。
pub fn line_route(points: &[Point]) -> RoutePath {
    let mut r = RoutePath::new();
    for i in 0..points.len().saturating_sub(1) {
        r.push(RouteSegment::Line { from: points[i], to: points[i + 1] });
    }
    r
}

/// 几何图（纯几何 + 已测量尺寸）。
#[derive(Debug, Clone, Default)]
pub struct Geograph {
    pub size: Size,
    pub background: lievisual::geometry::Color,
    pub nodes: Vec<GGNode>,
    pub edges: Vec<GGEdge>,
    pub containers: Vec<GGContainer>,
    /// 图级标题（timeline / pie 的标题等游离文本）。
    pub title: Option<String>,
    /// pie 图是否显示数据值（`showData`）。
    pub show_data: bool,
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
    /// 结构化节点详情（类框 / 实体框等），None 为普通单栏节点。
    pub detail: NodeDetail,
}

/// GG 边（已路由折线）。
#[derive(Debug, Clone)]
pub struct GGEdge {
    pub id: EdgeId,
    /// 端点节点 id（路由阶段用于查找节点几何；materialize/paint 不使用）。
    pub source: NodeId,
    pub target: NodeId,
    /// 路由路径（直线/贝塞尔段序列）。
    pub route: RoutePath,
    /// 边标签文本（供 materialize 绘制边标签；已在 measure 阶段测量为 `label`）。
    pub label_text: Option<String>,
    /// 边标签放置点（路由阶段已为标签预留空间）。
    pub label_anchor: Option<Point>,
    pub kind: EdgeKind,
    pub arrow: ArrowSpec,
    pub routing_hint: RoutingHint,
    /// 线型（实线 / 虚线 / 粗线 / 不可见），materialize 据此设样式。
    pub line_kind: LineKind,
    /// ER 关系基数（source 端, target 端），非 ER 边为 (None, None)。
    pub cardinality: (Option<ErCardinality>, Option<ErCardinality>),
    /// class 关系基数文本（`"1"` / `"*"` / `"many"` 等），非 class 边为 (None, None)。
    pub cardinality_text: (Option<String>, Option<String>),
}

/// GG 容器（仅几何包围盒 + 标题候选）。
#[derive(Debug, Clone)]
pub struct GGContainer {
    pub id: ContainerId,
    pub bounds: Rect,
    pub title: Option<String>,
    pub kind: ContainerKind,
    /// 容器成员节点 id（供边路由避让判定：边两端均非成员时才避让该容器）。
    pub member_ids: Vec<NodeId>,
}
