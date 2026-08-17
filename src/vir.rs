//! 视觉 IR 便捷层。
//!
//! 本模块**不引入独立的 IR**：builder 直接使用 lievisual 的真实类型
//! （[`Element`] / [`SceneNode`] / [`Color`] / [`TextStyle`] / [`Stroke`] / [`FillStrokeStyle`]），
//! 这里只提供与历史 `VisualElement` 写法等价的构造器（把 `z_index` 提升到 [`SceneNode::z_index`]），
//! 以及从 liemermaid 旧枚举（`FontWeight` / `FontStyle` / `TextAlign` / `TextBaseline`）到 lievisual
//! 类型的转换，减少 builder 的样板代码。
//!
//! 历史 `src/visual.rs`（`VisualElement` 及其私有样式类型）已删除，统一改用 lievisual IR。

use crate::option::{FontWeight, FontWeightNamed};
use vello_cpu::kurbo::{BezPath, Point, Rect};
use lievisual::text::{FontStyle as LieFontStyle, TextStyle as LieTextStyle};

pub use lievisual::geometry::{Color, Transform};
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
// 兼容构造器
// ---------------------------------------------------------------------------

/// 从 liemermaid 旧 `FontWeight` 枚举转换到 lievisual 的字重（f32，100–900）。
pub fn font_weight_to_f32(w: FontWeight) -> f32 {
    match w {
        FontWeight::Named(FontWeightNamed::Normal) => 400.0,
        FontWeight::Named(FontWeightNamed::Bold) => 700.0,
        FontWeight::Named(FontWeightNamed::Bolder) => 800.0,
        FontWeight::Named(FontWeightNamed::Lighter) => 300.0,
        FontWeight::Numeric(n) => n as f32,
    }
}

/// 兼容历史 `TextStyle { color, font_size, font_family, font_weight, font_style, align, vertical_align }`。
pub fn text_style(
    color: Color,
    font_size: f64,
    font_family: impl Into<String>,
    weight: FontWeight,
    font_style: FontStyle,
    align: TextAlign,
    baseline: TextBaseline,
) -> TextStyle {
    let fs = match font_style {
        FontStyle::Normal => LieFontStyle::Normal,
        FontStyle::Italic => LieFontStyle::Italic,
        FontStyle::Oblique => LieFontStyle::Oblique,
    };
    LieTextStyle::new(color, font_size, font_family)
        .with_weight(font_weight_to_f32(weight))
        .with_style(fs)
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

fn to_lie_point(p: Point) -> lievisual::geometry::Point {
    lievisual::geometry::Point::new(p.x, p.y)
}

fn to_lie_rect(r: Rect) -> lievisual::geometry::Rect {
    lievisual::geometry::Rect::new(r.x0, r.y0, r.width(), r.height())
}

pub fn rect_node(rect: Rect, radius: Option<f64>, style: FillStrokeStyle, z: i32) -> SceneNode {
    let lr = to_lie_rect(rect);
    let el = match radius {
        Some(r) => Element::rounded_rect(lr, r, style),
        None => Element::rect(lr, style),
    };
    SceneNode::from(el).with_z(z)
}

pub fn circle_node(center: Point, radius: f64, style: FillStrokeStyle, z: i32) -> SceneNode {
    SceneNode::from(Element::circle(to_lie_point(center), radius, style)).with_z(z)
}

pub fn line_node(start: Point, end: Point, style: Stroke, z: i32) -> SceneNode {
    SceneNode::from(Element::line(to_lie_point(start), to_lie_point(end), style)).with_z(z)
}

pub fn polyline_node(points: Vec<Point>, style: Stroke, z: i32) -> SceneNode {
    SceneNode::from(Element::poly(points.into_iter().map(to_lie_point).collect(), style)).with_z(z)
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
    SceneNode::from(Element::text(content, to_lie_point(position), s)).with_z(z)
}

pub fn group_node(children: Vec<SceneNode>, transform: Option<Transform>, z: i32) -> SceneNode {
    let mut n = SceneNode::new(Element::Group { children });
    if let Some(t) = transform {
        n = n.with_transform(t);
    }
    n.with_z(z)
}

/// 边终点箭头（填充三角形），与历史 `visual::draw_arrow_head` 等价。
pub fn draw_arrow_head(elements: &mut Vec<SceneNode>, tip: &Point, dir: &Point, style: &Stroke) {
    let sz = 10.0;
    let perp_x = -dir.y;
    let perp_y = dir.x;
    let base = Point::new(tip.x - dir.x * sz, tip.y - dir.y * sz);
    let p1 = Point::new(base.x + perp_x * sz * 0.5, base.y + perp_y * sz * 0.5);
    let p2 = Point::new(base.x - perp_x * sz * 0.5, base.y - perp_y * sz * 0.5);
    let mut path = BezPath::new();
    path.move_to(*tip);
    path.line_to(p1);
    path.line_to(p2);
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
// 主题色（原 visual::theme）
// ---------------------------------------------------------------------------
pub mod theme {
    use super::Color;

    // ---- 基础 ----
    pub const BACKGROUND: Color = Color::new(255 as f64 / 255.0, 255 as f64 / 255.0, 255 as f64 / 255.0, 1.0);
    pub const FONT_FAMILY: &str = "Segoe UI, system-ui, -apple-system, sans-serif";
    pub const FONT_SIZE: f64 = 13.0;
    pub const NODE_RADIUS: f64 = 6.0;

    // ---- 连线通用 ----
    pub const EDGE_COLOR: Color = Color::new(148 as f64 / 255.0, 163 as f64 / 255.0, 184 as f64 / 255.0, 1.0); // slate-400
    pub const EDGE_WIDTH: f64 = 2.0;
    pub const TEXT_COLOR: Color = Color::new(30 as f64 / 255.0, 41 as f64 / 255.0, 59 as f64 / 255.0, 1.0); // slate-800

    // ==================== Flowchart (蓝) ====================
    pub mod flowchart {
        use super::super::Color;
        pub const FILL: Color = Color::new(238 as f64 / 255.0, 242 as f64 / 255.0, 255 as f64 / 255.0, 1.0); // indigo-50
        pub const STROKE: Color = Color::new(99 as f64 / 255.0, 102 as f64 / 255.0, 241 as f64 / 255.0, 1.0); // indigo-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const EDGE: Color = super::EDGE_COLOR;
        pub const SUBGRAPH_STROKE: Color = Color::new(148 as f64 / 255.0, 163 as f64 / 255.0, 184 as f64 / 255.0, 1.0); // slate-400
        pub const SUBGRAPH_TITLE: Color = Color::new(71 as f64 / 255.0, 85 as f64 / 255.0, 105 as f64 / 255.0, 1.0); // slate-600
    }

    // ==================== State (绿) ====================
    pub mod state {
        use super::super::Color;
        pub const FILL: Color = Color::new(240 as f64 / 255.0, 253 as f64 / 255.0, 244 as f64 / 255.0, 1.0); // green-50
        pub const STROKE: Color = Color::new(34 as f64 / 255.0, 197 as f64 / 255.0, 94 as f64 / 255.0, 1.0); // green-500
        pub const TEXT: Color = Color::new(22 as f64 / 255.0, 101 as f64 / 255.0, 52 as f64 / 255.0, 1.0); // green-800
        pub const EDGE: Color = super::EDGE_COLOR;
        pub const START_FILL: Color = Color::new(22 as f64 / 255.0, 101 as f64 / 255.0, 52 as f64 / 255.0, 1.0); // green-800
        pub const END_STROKE: Color = Color::new(34 as f64 / 255.0, 197 as f64 / 255.0, 94 as f64 / 255.0, 1.0); // green-500
    }

    // ==================== Class (紫) ====================
    pub mod class {
        use super::super::Color;
        pub const FILL: Color = Color::new(255 as f64 / 255.0, 255 as f64 / 255.0, 255 as f64 / 255.0, 1.0); // white
        pub const HEADER_FILL: Color = Color::new(250 as f64 / 255.0, 245 as f64 / 255.0, 255 as f64 / 255.0, 1.0); // purple-50
        pub const STROKE: Color = Color::new(168 as f64 / 255.0, 85 as f64 / 255.0, 247 as f64 / 255.0, 1.0); // purple-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const EDGE: Color = super::EDGE_COLOR;
        pub const SEPARATOR: Color = Color::new(214 as f64 / 255.0, 188 as f64 / 255.0, 250 as f64 / 255.0, 1.0); // purple-200
        pub const DIAMOND_FILL: Color = Color::new(168 as f64 / 255.0, 85 as f64 / 255.0, 247 as f64 / 255.0, 1.0); // purple-500
    }

    // ==================== Sequence (天蓝) ====================
    pub mod sequence {
        use super::super::Color;
        pub const ACTOR_FILL: Color = Color::new(240 as f64 / 255.0, 249 as f64 / 255.0, 255 as f64 / 255.0, 1.0); // sky-50
        pub const ACTOR_STROKE: Color = Color::new(14 as f64 / 255.0, 165 as f64 / 255.0, 233 as f64 / 255.0, 1.0); // sky-500
        pub const FILL: Color = Color::new(240 as f64 / 255.0, 249 as f64 / 255.0, 255 as f64 / 255.0, 1.0); // sky-50
        pub const STROKE: Color = Color::new(14 as f64 / 255.0, 165 as f64 / 255.0, 233 as f64 / 255.0, 1.0); // sky-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const EDGE: Color = super::EDGE_COLOR;
        pub const LIFELINE: Color = Color::new(203 as f64 / 255.0, 213 as f64 / 255.0, 225 as f64 / 255.0, 1.0); // slate-300
        pub const NOTE_FILL: Color = Color::new(254 as f64 / 255.0, 252 as f64 / 255.0, 232 as f64 / 255.0, 1.0); // yellow-50
        pub const NOTE_STROKE: Color = Color::new(234 as f64 / 255.0, 179 as f64 / 255.0, 8 as f64 / 255.0, 1.0); // yellow-500
    }

    // ==================== ER (琥珀) ====================
    pub mod er {
        use super::super::Color;
        pub const FILL: Color = Color::new(255 as f64 / 255.0, 251 as f64 / 255.0, 235 as f64 / 255.0, 1.0); // amber-50
        pub const HEADER_FILL: Color = Color::new(254 as f64 / 255.0, 243 as f64 / 255.0, 199 as f64 / 255.0, 1.0); // amber-100
        pub const STROKE: Color = Color::new(245 as f64 / 255.0, 158 as f64 / 255.0, 11 as f64 / 255.0, 1.0); // amber-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const EDGE: Color = super::EDGE_COLOR;
    }

    // ==================== Timeline (粉) ====================
    pub mod timeline {
        use super::super::Color;
        pub const LINE: Color = Color::new(236 as f64 / 255.0, 72 as f64 / 255.0, 153 as f64 / 255.0, 1.0); // pink-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const TITLE: Color = Color::new(30 as f64 / 255.0, 41 as f64 / 255.0, 59 as f64 / 255.0, 1.0); // slate-800
    }

    // ==================== Git Graph (多分支) ====================
    pub mod gitgraph {
        use super::super::Color;
        pub const BRANCH_COLORS: [Color; 8] = [
            Color::new(99 as f64 / 255.0, 102 as f64 / 255.0, 241 as f64 / 255.0, 1.0),  // indigo-500
            Color::new(249 as f64 / 255.0, 115 as f64 / 255.0, 22 as f64 / 255.0, 1.0),  // orange-500
            Color::new(34 as f64 / 255.0, 197 as f64 / 255.0, 94 as f64 / 255.0, 1.0),   // green-500
            Color::new(234 as f64 / 255.0, 179 as f64 / 255.0, 8 as f64 / 255.0, 1.0),   // yellow-500
            Color::new(168 as f64 / 255.0, 85 as f64 / 255.0, 247 as f64 / 255.0, 1.0),  // purple-500
            Color::new(6 as f64 / 255.0, 182 as f64 / 255.0, 212 as f64 / 255.0, 1.0),   // cyan-500
            Color::new(148 as f64 / 255.0, 163 as f64 / 255.0, 184 as f64 / 255.0, 1.0), // slate-400
            Color::new(236 as f64 / 255.0, 72 as f64 / 255.0, 153 as f64 / 255.0, 1.0),  // pink-500
        ];
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const COMMIT_STROKE: Color = Color::new(255 as f64 / 255.0, 255 as f64 / 255.0, 255 as f64 / 255.0, 1.0);
    }

    // ==================== Pie (多色轮盘) ====================
    pub mod pie {
        use super::super::Color;
        pub const COLORS: [Color; 10] = [
            Color::new(99 as f64 / 255.0, 102 as f64 / 255.0, 241 as f64 / 255.0, 1.0), // indigo-500
            Color::new(14 as f64 / 255.0, 165 as f64 / 255.0, 233 as f64 / 255.0, 1.0), // sky-500
            Color::new(249 as f64 / 255.0, 115 as f64 / 255.0, 22 as f64 / 255.0, 1.0), // orange-500
            Color::new(34 as f64 / 255.0, 197 as f64 / 255.0, 94 as f64 / 255.0, 1.0),  // green-500
            Color::new(168 as f64 / 255.0, 85 as f64 / 255.0, 247 as f64 / 255.0, 1.0), // purple-500
            Color::new(234 as f64 / 255.0, 179 as f64 / 255.0, 8 as f64 / 255.0, 1.0),  // yellow-500
            Color::new(236 as f64 / 255.0, 72 as f64 / 255.0, 153 as f64 / 255.0, 1.0), // pink-500
            Color::new(6 as f64 / 255.0, 182 as f64 / 255.0, 212 as f64 / 255.0, 1.0),  // cyan-500
            Color::new(239 as f64 / 255.0, 68 as f64 / 255.0, 68 as f64 / 255.0, 1.0),  // red-500
            Color::new(20 as f64 / 255.0, 184 as f64 / 255.0, 166 as f64 / 255.0, 1.0), // teal-500
        ];
    }
}
