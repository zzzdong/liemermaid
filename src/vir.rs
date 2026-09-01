//! 视觉 IR 便捷层。
//!
//! 本模块**不引入独立的 IR**：builder 直接使用 lievisual 的真实类型
//! （[`Element`] / [`SceneNode`] / [`Color`] / [`TextStyle`] / [`Stroke`] / [`FillStrokeStyle`]），
//! 这里只提供与历史 `VisualElement` 写法等价的构造器（把 `z_index` 提升到 [`SceneNode::z_index`]），
//! 减少 builder 的样板代码。
//!
//! 几何坐标统一使用 [`lievisual::geometry`]（即 kurbo 类型，由 lievisual 直接 re-export，
//! 与 builder 布局算法零转换互通）
//!
//! 历史 `src/visual.rs`（`VisualElement` 及其私有样式类型）已删除，统一改用 lievisual IR。

use lievisual::geometry::BezPath;
use lievisual::geometry::{Point, Rect};
pub use lievisual::geometry::{Color, Point as GeoPoint, Rect as GeoRect, Transform};
pub use lievisual::scene::{
    Element, Fill, FillStrokeStyle, GradientStop, LinearGradient, SceneNode, Stroke,
};
pub use lievisual::text::{FontStyle, TextAlign, TextBaseline, TextStyle};

// ---------------------------------------------------------------------------
// z 层级常量（与历史 visual.rs 一致）
// ---------------------------------------------------------------------------
pub const Z_BACKGROUND: i32 = 0;
pub const Z_GRID: i32 = 10;
pub const Z_SERIES: i32 = 20;
pub const Z_SERIES_FILL: i32 = 20;
pub const Z_SERIES_LINE: i32 = 21;
pub const Z_SERIES_POINT: i32 = 22;
pub const Z_AXIS: i32 = 30;
pub const Z_LABEL: i32 = 40;
pub const Z_TITLE: i32 = 50;
/// 子图容器框（位于节点与边之下）
pub const Z_SUBGRAPH: i32 = 5;
/// 子图标题文字
pub const Z_SUBGRAPH_LABEL: i32 = 6;

// ---------------------------------------------------------------------------
// 图元样式构造器
// ---------------------------------------------------------------------------

/// 构造 [`TextStyle`]。字重固定常规（400）、样式固定正体，
/// 与 lievisual 迁移后的 builder 实际用法一致（不再经过旧 `FontWeight` 枚举）。
pub fn text_style(
    color: Color,
    font_size: f64,
    font_family: impl Into<String>,
    align: TextAlign,
    baseline: TextBaseline,
) -> TextStyle {
    TextStyle::new(color, font_size, font_family)
        .with_align(align)
        .with_baseline(baseline)
}

/// 便捷构造描边。
pub fn stroke(color: Color, width: f64) -> Stroke {
    Stroke::new(color, width)
}

/// 便捷构造虚线描边。
pub fn dashed_stroke(color: Color, width: f64, dash_array: Vec<f64>) -> Stroke {
    Stroke::dashed(color, width, dash_array)
}

/// 仅填充样式。
pub fn fs_fill(color: Color) -> FillStrokeStyle {
    FillStrokeStyle::fill(color)
}

/// 仅描边样式。
pub fn fs_stroke(color: Color, width: f64) -> FillStrokeStyle {
    FillStrokeStyle::stroke(color, width)
}

/// 填充 + 描边样式。
pub fn fs_both(fill: Color, stroke: Color, width: f64) -> FillStrokeStyle {
    FillStrokeStyle {
        fill: Some(fill.into()),
        stroke: Some(Stroke::new(stroke, width)),
    }
}

// ---------------------------------------------------------------------------
// 图元构造（z 提升到 SceneNode.z_index）
// ---------------------------------------------------------------------------

/// 构造矩形 / 圆角矩形节点。
///
/// 坐标统一使用 lievisual `Point` / `Rect`（不再经 kurbo → lievisual 转换）。
/// 仅矢量路径（[`BezPath`]）保留 kurbo（lievisual 的 `Element::Path` 原生使用它）。
pub fn rect_node(rect: Rect, radius: Option<f64>, style: FillStrokeStyle, z: i32) -> SceneNode {
    let el = match radius {
        Some(r) => Element::rounded_rect(rect, r, style),
        None => Element::rect(rect, style),
    };
    SceneNode::from(el).with_z(z)
}

pub fn circle_node(center: Point, radius: f64, style: FillStrokeStyle, z: i32) -> SceneNode {
    SceneNode::from(Element::circle(center, radius, style)).with_z(z)
}

pub fn line_node(start: Point, end: Point, style: Stroke, z: i32) -> SceneNode {
    SceneNode::from(Element::line(start, end, style)).with_z(z)
}

pub fn polyline_node(points: Vec<Point>, style: Stroke, z: i32) -> SceneNode {
    SceneNode::from(Element::poly(points, style)).with_z(z)
}

pub fn path_node(path: BezPath, style: FillStrokeStyle, z: i32) -> SceneNode {
    SceneNode::from(Element::Path {
        path,
        style,
        closed: false,
    })
    .with_z(z)
}

/// Catmull-Rom 转贝塞尔的平滑曲线：把折线点序列平滑为开放路径。
fn smooth_curve(pts: &[Point]) -> BezPath {
    let mut p = BezPath::new();
    if pts.len() < 2 {
        return p;
    }
    p.move_to((pts[0].x, pts[0].y));
    if pts.len() == 2 {
        // 两点边：三次贝塞尔柔和弧（控制点沿轴向 1/3、2/3）
        let a = pts[0];
        let b = pts[1];
        let c1 = Point::new(a.x + (b.x - a.x) * 0.33, a.y + (b.y - a.y) * 0.5);
        let c2 = Point::new(a.x + (b.x - a.x) * 0.67, a.y + (b.y - a.y) * 0.5);
        p.curve_to((c1.x, c1.y), (c2.x, c2.y), (b.x, b.y));
        return p;
    }
    // 正交折线（sugiyama 路由，每段水平/垂直）→ 拐角圆角平滑
    if pts.windows(2).all(|w| is_axis_aligned(w[0], w[1])) {
        smooth_orthogonal(&mut p, pts);
        return p;
    }
    // 自由折线 → Catmull-Rom
    for i in 0..pts.len() - 1 {
        let p0 = if i == 0 { pts[0] } else { pts[i - 1] };
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = if i + 2 < pts.len() {
            pts[i + 2]
        } else {
            pts[i + 1]
        };
        let c1 = Point::new(p1.x + (p2.x - p0.x) / 6.0, p1.y + (p2.y - p0.y) / 6.0);
        let c2 = Point::new(p2.x - (p3.x - p1.x) / 6.0, p2.y - (p3.y - p1.y) / 6.0);
        p.curve_to((c1.x, c1.y), (c2.x, c2.y), (p2.x, p2.y));
    }
    p
}

/// 判断两点连线是否水平/垂直（sugiyama 路由含微小浮点误差，用宽松阈值）。
fn is_axis_aligned(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 0.5 || (a.y - b.y).abs() < 0.5
}

/// 正交折线拐角圆角平滑：每个 90° 拐点用贝塞尔圆弧过渡。
/// 使用标准圆角贝塞尔系数 K=0.5523，避免 Catmull-Rom 在直角处的横向摆动。
fn smooth_orthogonal(p: &mut BezPath, pts: &[Point]) {
    const K: f64 = 0.5523; // 90° 圆弧的贝塞尔系数
    const RADIUS: f64 = 10.0;
    let n = pts.len();
    // 逐点推进：从起点沿直线到每个拐角的圆角起点，贝塞尔过拐角，最后到终点
    let mut cur = pts[0];
    for i in 1..n - 1 {
        let a = pts[i - 1];
        let b = pts[i];
        let c = pts[i + 1];
        // 前段方向（a→b），后段方向（b→c）
        let d_in = unit_dir(b, a);
        let d_out = unit_dir(c, b);
        // 圆角半径（不超过前后段长度，避免过冲）
        let r = RADIUS.min((b.distance(a) * 0.5).min(b.distance(c) * 0.5));
        let start = Point::new(b.x - d_in.x * r, b.y - d_in.y * r);
        let end = Point::new(b.x + d_out.x * r, b.y + d_out.y * r);
        p.line_to((start.x, start.y));
        let c1 = Point::new(start.x - d_in.x * r * K, start.y - d_in.y * r * K);
        let c2 = Point::new(end.x + d_out.x * r * K, end.y + d_out.y * r * K);
        p.curve_to((c1.x, c1.y), (c2.x, c2.y), (end.x, end.y));
        cur = end;
    }
    let last = pts[n - 1];
    if cur.distance(last) > 1e-6 {
        p.line_to((last.x, last.y));
    }
}

/// 从 `from` 指向 `to` 的单位方向向量。
fn unit_dir(to: Point, from: Point) -> Point {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    Point::new(dx / len, dy / len)
}

/// 平滑曲线边：把路由点平滑为贝塞尔开放路径（仅描边）。
/// 末端箭头由调用方用 [`draw_arrow_head`] 等自绘，IR 本身不含箭头概念。
pub fn curved_edge_node(points: Vec<Point>, style: Stroke, z: i32) -> SceneNode {
    let path = smooth_curve(&points);
    let fs = FillStrokeStyle {
        fill: None,
        stroke: Some(style),
    };
    SceneNode::from(Element::Path {
        path,
        style: fs,
        closed: false,
    })
    .with_z(z)
}

/// 三次贝塞尔边：从 `start` 经控制点 `c1`/`c2` 到 `end` 的单条贝塞尔曲线。
///
/// 用于回边等需要明确 S 形绕行的边，避免 Catmull-Rom 对折点的摆动。
pub fn cubic_bezier_edge(
    start: Point,
    c1: Point,
    c2: Point,
    end: Point,
    style: Stroke,
    z: i32,
) -> SceneNode {
    let mut path = BezPath::new();
    path.move_to((start.x, start.y));
    path.curve_to((c1.x, c1.y), (c2.x, c2.y), (end.x, end.y));
    let fs = FillStrokeStyle {
        fill: None,
        stroke: Some(style),
    };
    SceneNode::from(Element::Path {
        path,
        style: fs,
        closed: false,
    })
    .with_z(z)
}

pub fn gradient_path_node(
    path: BezPath,
    gradient: LinearGradient,
    stroke: Option<Stroke>,
    z: i32,
) -> SceneNode {
    SceneNode::from(Element::GradientPath {
        path,
        gradient,
        stroke,
    })
    .with_z(z)
}

pub fn text_node(
    content: impl Into<String>,
    position: Point,
    style: TextStyle,
    rotation: f64,
    max_width: Option<f64>,
    z: i32,
) -> SceneNode {
    let mut s = style;
    if rotation != 0.0 {
        s.rotation = rotation;
    }
    if max_width.is_some() {
        s.max_width = max_width;
    }
    SceneNode::from(Element::text(content, position, s)).with_z(z)
}

pub fn group_node(children: Vec<SceneNode>, transform: Option<Transform>, z: i32) -> SceneNode {
    let mut n = SceneNode::new(Element::Group { children });
    if let Some(t) = transform {
        n = n.with_transform(t);
    }
    n.with_z(z)
}

/// 边终点箭头。
///
/// `filled = true` → 实心三角形（sequence 默认箭头，对齐官方）；
/// `filled = false` → 空心三角形（flowchart 普通连线，对齐官方 fill=none）。
///
/// `tip` / `dir` 为 lievisual 坐标（即 kurbo 类型）；内部构造 kurbo [`BezPath`] 供 `Element::Path` 使用。
pub fn draw_arrow_head(
    elements: &mut Vec<SceneNode>,
    tip: &Point,
    dir: &Point,
    style: &Stroke,
    filled: bool,
) {
    let sz = 10.0;
    let perp_x = -dir.y;
    let perp_y = dir.x;
    let base = Point::new(tip.x - dir.x * sz, tip.y - dir.y * sz);
    let p1 = Point::new(base.x + perp_x * sz * 0.5, base.y + perp_y * sz * 0.5);
    let p2 = Point::new(base.x - perp_x * sz * 0.5, base.y - perp_y * sz * 0.5);
    let mut path = BezPath::new();
    path.move_to(Point::new(tip.x, tip.y));
    path.line_to(Point::new(p1.x, p1.y));
    path.line_to(Point::new(p2.x, p2.y));
    path.close_path();
    let node_style = if filled {
        fs_both(style.color, style.color, style.width)
    } else {
        fs_stroke(style.color, style.width)
    };
    elements.push(path_node(path, node_style, Z_AXIS));
}

/// 边终点圆点标记（`--o`），实心圆。
pub fn draw_arrow_circle(elements: &mut Vec<SceneNode>, tip: &Point, style: &Stroke) {
    let r = 5.0;
    let mut path = BezPath::new();
    // 用贝塞尔近似圆
    let k = 0.5522847498;
    path.move_to(Point::new(tip.x + r, tip.y));
    path.curve_to(
        Point::new(tip.x + r, tip.y + r * k),
        Point::new(tip.x + r * k, tip.y + r),
        Point::new(tip.x, tip.y + r),
    );
    path.curve_to(
        Point::new(tip.x - r * k, tip.y + r),
        Point::new(tip.x - r, tip.y + r * k),
        Point::new(tip.x - r, tip.y),
    );
    path.curve_to(
        Point::new(tip.x - r, tip.y - r * k),
        Point::new(tip.x - r * k, tip.y - r),
        Point::new(tip.x, tip.y - r),
    );
    path.curve_to(
        Point::new(tip.x + r * k, tip.y - r),
        Point::new(tip.x + r, tip.y - r * k),
        Point::new(tip.x + r, tip.y),
    );
    path.close_path();
    elements.push(path_node(
        path,
        fs_both(style.color, style.color, style.width),
        Z_AXIS,
    ));
}

/// 边终点叉号标记（`--x`），两条交叉线。
pub fn draw_arrow_cross(elements: &mut Vec<SceneNode>, tip: &Point, style: &Stroke) {
    let sz = 5.0;
    let mut path = BezPath::new();
    path.move_to(Point::new(tip.x - sz, tip.y - sz));
    path.line_to(Point::new(tip.x + sz, tip.y + sz));
    path.move_to(Point::new(tip.x - sz, tip.y + sz));
    path.line_to(Point::new(tip.x + sz, tip.y - sz));
    elements.push(path_node(path, fs_stroke(style.color, style.width), Z_AXIS));
}

/// 历史 `GradientDef { stops, angle }` 到 lievisual `LinearGradient` 的兼容构造。
pub fn gradient_def(angle_deg: f64, stops: Vec<(f64, Color)>) -> LinearGradient {
    let a = angle_deg.to_radians();
    let dx = a.cos();
    let dy = a.sin();
    let half = 0.5;
    let start = lievisual::geometry::Point::new(0.5 - dx * half, 0.5 - dy * half);
    let end = lievisual::geometry::Point::new(0.5 + dx * half, 0.5 + dy * half);
    LinearGradient {
        start,
        end,
        stops: stops
            .into_iter()
            .map(|(offset, color)| GradientStop { offset, color })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// 主题色（原 visual::theme）已下沉为 `builder::theme`，此处重导出以保持
// 各图表 builder 的 `use crate::vir::theme` 引用不变。
pub use crate::builder::theme;
