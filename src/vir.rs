//! 视觉 IR 便捷层。
//!
//! 本模块**不引入独立的 IR**：builder 直接使用 lievisual 的真实类型
//! （[`Element`] / [`SceneNode`] / [`Color`] / [`TextStyle`] / [`Stroke`] / [`FillStrokeStyle`]），
//! 这里只提供与历史 `VisualElement` 写法等价的构造器（把 `z_index` 提升到 [`SceneNode::z_index`]），
//! 减少 builder 的样板代码。
//!
//! 几何坐标统一使用 [`lievisual::geometry`]（即 kurbo 类型，由 lievisual 直接 re-export，
//! 与 builder 布局算法零转换互通）；仅矢量路径 [`BezPath`] 保留 `vello_cpu::kurbo`。
//!
//! 历史 `src/visual.rs`（`VisualElement` 及其私有样式类型）已删除，统一改用 lievisual IR。

use vello_cpu::kurbo::BezPath;
use lievisual::geometry::{Point, Rect};
use lievisual::text::TextStyle as LieTextStyle;

pub use lievisual::geometry::{Color, Point as GeoPoint, Rect as GeoRect, Transform};
pub use lievisual::scene::{Element, Fill, FillStrokeStyle, GradientStop, LinearGradient, SceneNode, Stroke};
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
    LieTextStyle::new(color, font_size, font_family)
        .with_align(align)
        .with_baseline(baseline)
}

/// 便捷构造描边。
pub fn stroke(color: Color, width: f64) -> Stroke {
    Stroke::new(color, width)
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
    SceneNode::from(Element::Path { path, style, closed: false }).with_z(z)
}

pub fn gradient_path_node(path: BezPath, gradient: LinearGradient, stroke: Option<Stroke>, z: i32) -> SceneNode {
    SceneNode::from(Element::GradientPath { path, gradient, stroke }).with_z(z)
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

/// 边终点箭头（填充三角形），与历史 `visual::draw_arrow_head` 等价。
///
/// `tip` / `dir` 为 lievisual 坐标（即 kurbo 类型）；内部构造 kurbo [`BezPath`] 供 `Element::Path` 使用。
pub fn draw_arrow_head(elements: &mut Vec<SceneNode>, tip: &Point, dir: &Point, style: &Stroke) {
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
    elements.push(path_node(path, fs_both(style.color, style.color, style.width), Z_AXIS));
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
