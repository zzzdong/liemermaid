//! Pure-data visual primitives decoupled from any rendering backend.

use vello_cpu::kurbo::{BezPath, Point, Rect, Vec2};

use crate::text::TextLayout;

/// A low-level visual drawing primitive used by the rendering pipeline.
pub enum VisualElement {
    // ---- 基础图形 ----
    Rect {
        rect: Rect,
        radius: Option<f64>,
        style: FillStrokeStyle,
        z_index: i32,
    },
    Circle {
        center: Point,
        radius: f64,
        style: FillStrokeStyle,
        z_index: i32,
    },
    Line {
        start: Point,
        end: Point,
        style: StrokeStyle,
        z_index: i32,
    },
    Polyline {
        points: Vec<Point>,
        style: StrokeStyle,
        z_index: i32,
    },
    Path {
        path: BezPath,
        style: FillStrokeStyle,
        z_index: i32,
    },

    // ---- 渐变路径 ----
    GradientPath {
        path: BezPath,
        gradient: GradientDef,
        stroke: Option<Stroke>,
        z_index: i32,
    },

    // ---- 文本 ----
    TextRun {
        text: String,
        position: Point, // 锚点位置（配合 align/baseline 确定文本块左上角）
        style: crate::visual::TextStyle,
        rotation: f64, // 弧度
        max_width: Option<f64>,
        layout: Option<Box<TextLayout>>, // 预排版结果
        z_index: i32,
    },

    // ---- 变换组合 ----
    Group {
        children: Vec<VisualElement>,
        transform: Option<Transform>,
        z_index: i32,
    },
}

impl Clone for VisualElement {
    fn clone(&self) -> Self {
        match self {
            VisualElement::Rect {
                rect,
                radius,
                style,
                z_index,
            } => VisualElement::Rect {
                rect: *rect,
                radius: *radius,
                style: style.clone(),
                z_index: *z_index,
            },
            VisualElement::Circle {
                center,
                radius,
                style,
                z_index,
            } => VisualElement::Circle {
                center: *center,
                radius: *radius,
                style: style.clone(),
                z_index: *z_index,
            },
            VisualElement::Line {
                start,
                end,
                style,
                z_index,
            } => VisualElement::Line {
                start: *start,
                end: *end,
                style: style.clone(),
                z_index: *z_index,
            },
            VisualElement::Polyline {
                points,
                style,
                z_index,
            } => VisualElement::Polyline {
                points: points.clone(),
                style: style.clone(),
                z_index: *z_index,
            },
            VisualElement::Path {
                path,
                style,
                z_index,
            } => VisualElement::Path {
                path: path.clone(),
                style: style.clone(),
                z_index: *z_index,
            },
            VisualElement::GradientPath {
                path,
                gradient,
                stroke,
                z_index,
            } => VisualElement::GradientPath {
                path: path.clone(),
                gradient: gradient.clone(),
                stroke: stroke.clone(),
                z_index: *z_index,
            },
            VisualElement::TextRun {
                text,
                position,
                style,
                rotation,
                max_width,
                layout,
                z_index,
            } => VisualElement::TextRun {
                text: text.clone(),
                position: *position,
                style: style.clone(),
                rotation: *rotation,
                max_width: *max_width,
                layout: layout.clone(),
                z_index: *z_index,
            },
            VisualElement::Group {
                children,
                transform,
                z_index,
            } => VisualElement::Group {
                children: children.clone(),
                transform: *transform,
                z_index: *z_index,
            },
        }
    }
}

impl std::fmt::Debug for VisualElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VisualElement::Rect {
                rect,
                radius,
                style,
                z_index,
            } => f
                .debug_struct("Rect")
                .field("rect", rect)
                .field("radius", radius)
                .field("style", style)
                .field("z_index", z_index)
                .finish(),
            VisualElement::Circle {
                center,
                radius,
                style,
                z_index,
            } => f
                .debug_struct("Circle")
                .field("center", center)
                .field("radius", radius)
                .field("style", style)
                .field("z_index", z_index)
                .finish(),
            VisualElement::Line {
                start,
                end,
                style,
                z_index,
            } => f
                .debug_struct("Line")
                .field("start", start)
                .field("end", end)
                .field("style", style)
                .field("z_index", z_index)
                .finish(),
            VisualElement::Polyline {
                points,
                style,
                z_index,
            } => f
                .debug_struct("Polyline")
                .field("points", points)
                .field("style", style)
                .field("z_index", z_index)
                .finish(),
            VisualElement::Path {
                path: _,
                style,
                z_index,
            } => f
                .debug_struct("Path")
                .field("path", &"<BezPath>")
                .field("style", style)
                .field("z_index", z_index)
                .finish(),
            VisualElement::GradientPath {
                path: _,
                gradient,
                stroke,
                z_index,
            } => f
                .debug_struct("GradientPath")
                .field("path", &"<BezPath>")
                .field("gradient", gradient)
                .field("stroke", stroke)
                .field("z_index", z_index)
                .finish(),
            VisualElement::TextRun {
                text,
                position,
                style,
                rotation,
                max_width,
                layout,
                z_index,
            } => f
                .debug_struct("TextRun")
                .field("text", text)
                .field("position", position)
                .field("style", style)
                .field("rotation", rotation)
                .field("max_width", max_width)
                .field("layout", &layout.as_ref().map(|_| "<TextLayout>"))
                .field("z_index", z_index)
                .finish(),
            VisualElement::Group {
                children,
                transform,
                z_index,
            } => f
                .debug_struct("Group")
                .field("children", children)
                .field("transform", transform)
                .field("z_index", z_index)
                .finish(),
        }
    }
}

impl VisualElement {
    pub fn z_index(&self) -> i32 {
        match self {
            VisualElement::Rect { z_index, .. } => *z_index,
            VisualElement::Circle { z_index, .. } => *z_index,
            VisualElement::Line { z_index, .. } => *z_index,
            VisualElement::Polyline { z_index, .. } => *z_index,
            VisualElement::Path { z_index, .. } => *z_index,
            VisualElement::GradientPath { z_index, .. } => *z_index,
            VisualElement::TextRun { z_index, .. } => *z_index,
            VisualElement::Group { z_index, .. } => *z_index,
        }
    }
}

pub const Z_BACKGROUND: i32 = 0;
pub const Z_GRID: i32 = 10;
pub const Z_SERIES: i32 = 20;
pub const Z_SERIES_FILL: i32 = 20;
pub const Z_SERIES_LINE: i32 = 21;
pub const Z_SERIES_POINT: i32 = 22;
pub const Z_AXIS: i32 = 30;
pub const Z_LABEL: i32 = 40;
pub const Z_TITLE: i32 = 50;

/// 2D 变换
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub translate: Vec2,
    pub rotate: f64, // 弧度
    pub scale: Vec2,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translate: Vec2::new(0.0, 0.0),
            rotate: 0.0,
            scale: Vec2::new(1.0, 1.0),
        }
    }
}

impl Transform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_translation(x: f64, y: f64) -> Self {
        Self {
            translate: Vec2::new(x, y),
            ..Default::default()
        }
    }

    pub fn with_rotation(angle: f64) -> Self {
        Self {
            rotate: angle,
            ..Default::default()
        }
    }

    pub fn with_scale(x: f64, y: f64) -> Self {
        Self {
            scale: Vec2::new(x, y),
            ..Default::default()
        }
    }
}

/// A resolved RGBA color used throughout the rendering pipeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn set_alpha(&self, alpha: f64) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: (alpha.clamp(0.0, 1.0) * 255.0) as u8,
        }
    }

    /// 从十六进制字符串解析颜色
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::new(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::with_alpha(r, g, b, a))
            }
            _ => None,
        }
    }

    pub fn as_vello_color(&self) -> vello_cpu::color::AlphaColor<vello_cpu::color::Srgb> {
        vello_cpu::color::AlphaColor::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

impl From<Color> for vello_cpu::color::AlphaColor<vello_cpu::color::Srgb> {
    fn from(c: Color) -> Self {
        c.as_vello_color()
    }
}

/// Gradient stops and direction definition.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientDef {
    /// Gradient stop points (offset 0.0~1.0, color).
    pub stops: Vec<(f64, Color)>,
}

impl GradientDef {
    pub fn new(stops: Vec<(f64, Color)>) -> Self {
        Self { stops }
    }
}

/// Full stroke style with color, width, dash, and cap/join.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub width: f64,
}

impl Stroke {
    pub fn new(color: Color, width: f64) -> Self {
        Self { color, width }
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            color: Color::new(0, 0, 0),
            width: 1.0,
        }
    }
}

/// 描边样式（简化版，用于 Line/Polyline）
#[derive(Clone, Debug)]
pub struct StrokeStyle {
    pub color: Color,
    pub width: f64,
}

impl StrokeStyle {
    pub fn new(color: Color, width: f64) -> Self {
        Self { color, width }
    }
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            color: Color::new(0, 0, 0),
            width: 1.0,
        }
    }
}

/// Fill and stroke style used by visual elements.
#[derive(Debug, Clone, Default)]
pub struct FillStrokeStyle {
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
}

impl FillStrokeStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fill(mut self, color: Color) -> Self {
        self.fill = Some(color);
        self
    }

    pub fn with_stroke(mut self, color: Color, width: f64) -> Self {
        self.stroke = Some(Stroke::new(color, width));
        self
    }
}

/// 文本对齐方式
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// 文本基线方式
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum TextBaseline {
    Top,
    Middle,
    Bottom,
    #[default]
    Alphabetic,
}

// 文本样式已移除：改用 model::TextStyle + TextRun 的 align/baseline/rotation 字段

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

use crate::option::{FontWeight, FontWeightNamed};

#[derive(Debug, Clone)]
pub struct TextStyle {
    pub color: Color,
    pub font_size: f64,
    pub font_family: String,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub align: TextAlign,
    pub vertical_align: TextBaseline,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::new(0, 0, 0),
            font_size: 12.0,
            font_family: "sans-serif".to_string(),
            font_weight: FontWeight::Named(FontWeightNamed::Normal),
            font_style: FontStyle::Normal,
            align: TextAlign::Left,
            vertical_align: TextBaseline::Top,
        }
    }
}

/// 在边终点绘制箭头（填充三角形）
pub fn draw_arrow_head(
    elements: &mut Vec<VisualElement>,
    tip: &Point,
    dir: &Point,
    style: &StrokeStyle,
) {
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

    elements.push(VisualElement::Path {
        path,
        style: FillStrokeStyle::new()
            .with_fill(style.color)
            .with_stroke(style.color, style.width),
        z_index: Z_AXIS,
    });
}

// ===================================================================
// 统一主题系统 — 集中管理所有配色（参考 AntV G6 + Tailwind 设计）
// ===================================================================

/// 通用主题色常量
pub mod theme {
    use super::Color;

    // ---- 基础 ----
    pub const BACKGROUND: Color = Color::new(255, 255, 255);
    pub const FONT_FAMILY: &str = "Segoe UI, system-ui, -apple-system, sans-serif";
    pub const FONT_SIZE: f64 = 13.0;
    pub const NODE_RADIUS: f64 = 6.0;

    // ---- 连线通用 ----
    pub const EDGE_COLOR: Color = Color::new(148, 163, 184); // slate-400
    pub const EDGE_WIDTH: f64 = 2.0;
    pub const TEXT_COLOR: Color = Color::new(30, 41, 59); // slate-800

    // ==================== Flowchart (蓝) ====================
    pub mod flowchart {
        use super::super::Color;
        pub const FILL: Color = Color::new(238, 242, 255); // indigo-50
        pub const STROKE: Color = Color::new(99, 102, 241); // indigo-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const EDGE: Color = super::EDGE_COLOR;
    }

    // ==================== State (绿) ====================
    pub mod state {
        use super::super::Color;
        pub const FILL: Color = Color::new(240, 253, 244); // green-50
        pub const STROKE: Color = Color::new(34, 197, 94); // green-500
        pub const TEXT: Color = Color::new(22, 101, 52); // green-800
        pub const EDGE: Color = super::EDGE_COLOR;
        pub const START_FILL: Color = Color::new(22, 101, 52); // green-800
        pub const END_STROKE: Color = Color::new(34, 197, 94); // green-500
    }

    // ==================== Class (紫) ====================
    pub mod class {
        use super::super::Color;
        pub const FILL: Color = Color::new(255, 255, 255); // white
        pub const HEADER_FILL: Color = Color::new(250, 245, 255); // purple-50
        pub const STROKE: Color = Color::new(168, 85, 247); // purple-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const EDGE: Color = super::EDGE_COLOR;
        pub const SEPARATOR: Color = Color::new(214, 188, 250); // purple-200
        pub const DIAMOND_FILL: Color = Color::new(168, 85, 247); // purple-500
    }

    // ==================== Sequence (天蓝) ====================
    pub mod sequence {
        use super::super::Color;
        pub const ACTOR_FILL: Color = Color::new(240, 249, 255); // sky-50
        pub const ACTOR_STROKE: Color = Color::new(14, 165, 233); // sky-500
        pub const FILL: Color = Color::new(240, 249, 255); // sky-50
        pub const STROKE: Color = Color::new(14, 165, 233); // sky-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const EDGE: Color = super::EDGE_COLOR;
        pub const LIFELINE: Color = Color::new(203, 213, 225); // slate-300
        pub const NOTE_FILL: Color = Color::new(254, 252, 232); // yellow-50
        pub const NOTE_STROKE: Color = Color::new(234, 179, 8); // yellow-500
    }

    // ==================== ER (琥珀) ====================
    pub mod er {
        use super::super::Color;
        pub const FILL: Color = Color::new(255, 251, 235); // amber-50
        pub const HEADER_FILL: Color = Color::new(254, 243, 199); // amber-100
        pub const STROKE: Color = Color::new(245, 158, 11); // amber-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const EDGE: Color = super::EDGE_COLOR;
    }

    // ==================== Timeline (粉) ====================
    pub mod timeline {
        use super::super::Color;
        pub const LINE: Color = Color::new(236, 72, 153); // pink-500
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const TITLE: Color = Color::new(30, 41, 59); // slate-800
    }

    // ==================== Git Graph (多分支) ====================
    pub mod gitgraph {
        use super::super::Color;
        pub const BRANCH_COLORS: [Color; 8] = [
            Color::new(99, 102, 241),  // indigo-500
            Color::new(249, 115, 22),  // orange-500
            Color::new(34, 197, 94),   // green-500
            Color::new(234, 179, 8),   // yellow-500
            Color::new(168, 85, 247),  // purple-500
            Color::new(6, 182, 212),   // cyan-500
            Color::new(148, 163, 184), // slate-400
            Color::new(236, 72, 153),  // pink-500
        ];
        pub const TEXT: Color = super::TEXT_COLOR;
        pub const COMMIT_STROKE: Color = Color::new(255, 255, 255);
    }

    // ==================== Pie (多色轮盘) ====================
    pub mod pie {
        use super::super::Color;
        pub const COLORS: [Color; 10] = [
            Color::new(99, 102, 241), // indigo-500
            Color::new(14, 165, 233), // sky-500
            Color::new(249, 115, 22), // orange-500
            Color::new(34, 197, 94),  // green-500
            Color::new(168, 85, 247), // purple-500
            Color::new(234, 179, 8),  // yellow-500
            Color::new(236, 72, 153), // pink-500
            Color::new(6, 182, 212),  // cyan-500
            Color::new(239, 68, 68),  // red-500
            Color::new(20, 184, 166), // teal-500
        ];
    }
}
