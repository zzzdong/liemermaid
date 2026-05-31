//! Pixmap renderer — renders visual elements to bitmap via vello_cpu.

use vello_cpu::{
    Pixmap, RenderContext, Resources,
    kurbo::{BezPath, Circle, Point, Rect, Shape as KurboShape, Stroke as KurboStroke},
    peniko::color::AlphaColor,
};

use crate::{
    error::DiagramResult as Result,
    render::Renderer,
    text::TextLayout,
    visual::{Color, FillStrokeStyle, GradientDef, Stroke, StrokeStyle, Transform, VisualElement},
};

/// Bitmap renderer backed by Vello CPU pipeline.
///
/// Produces PNG/JPEG images via the `image` crate.
#[derive(Debug)]
pub struct PixmapRenderer {
    ctx: RenderContext,
    resources: Resources,
    width: u32,
    height: u32,
    background_color: Option<Color>,
}

impl PixmapRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            ctx: RenderContext::new(width as u16, height as u16),
            resources: Resources::new(),
            width,
            height,
            background_color: None,
        }
    }

    /// 设置背景色（None = 透明）
    pub fn with_background(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// 渲染视觉元素序列并输出 Pixmap
    pub fn render(mut self, elements: &[VisualElement]) -> Result<Pixmap> {
        // 先填充背景色
        if let Some(bg) = self.background_color {
            let rect = Rect::new(0.0, 0.0, self.width as f64, self.height as f64);
            let fill = FillStrokeStyle::new().with_fill(bg);
            self.draw_rect(rect, None, &fill);
        }

        self.render_elements(elements);

        let mut pixmap = Pixmap::new(self.width as u16, self.height as u16);
        self.ctx.render_to_pixmap(&mut self.resources, &mut pixmap);

        Ok(pixmap)
    }

    fn color_to_vello(color: &Color) -> AlphaColor<vello_cpu::color::Srgb> {
        AlphaColor::from_rgba8(color.r, color.g, color.b, color.a)
    }

    /// 将 Stroke 转换为 KurboStroke 并设置到渲染上下文
    fn set_stroke_style(&mut self, stroke: &Stroke) {
        let kurbo_stroke = KurboStroke::new(stroke.width);
        self.ctx.set_stroke(kurbo_stroke);
    }
}

impl Renderer for PixmapRenderer {
    fn draw_rect(&mut self, rect: Rect, radius: Option<f64>, style: &FillStrokeStyle) {
        if let Some(r) = radius {
            // Vello has no native rounded rect, approximate with BezPath
            let path = rounded_rect_path(rect, r);
            if let Some(fill) = &style.fill {
                let color = Self::color_to_vello(fill);
                self.ctx.set_paint(color);
                self.ctx.fill_path(&path);
            }
            if let Some(stroke) = &style.stroke {
                let color = Self::color_to_vello(&stroke.color);
                self.ctx.set_paint(color);
                self.set_stroke_style(stroke);
                self.ctx.stroke_path(&path);
            }
            return;
        }
        if let Some(fill) = &style.fill {
            let color = Self::color_to_vello(fill);
            self.ctx.set_paint(color);
            self.ctx.fill_rect(&rect);
        }

        if let Some(stroke) = &style.stroke {
            let color = Self::color_to_vello(&stroke.color);
            self.ctx.set_paint(color);
            self.set_stroke_style(stroke);
            self.ctx.stroke_rect(&rect);
        }
    }

    fn draw_circle(&mut self, center: Point, radius: f64, style: &FillStrokeStyle) {
        let circle = Circle::new(center, radius);
        let path = circle.to_path(0.1);

        if let Some(fill) = &style.fill {
            let color = Self::color_to_vello(fill);
            self.ctx.set_paint(color);
            self.ctx.fill_path(&path);
        }

        if let Some(stroke) = &style.stroke {
            let color = Self::color_to_vello(&stroke.color);
            self.ctx.set_paint(color);
            self.set_stroke_style(stroke);
            self.ctx.stroke_path(&path);
        }
    }

    fn draw_line(&mut self, start: Point, end: Point, style: &StrokeStyle) {
        let color = Self::color_to_vello(&style.color);
        self.ctx.set_paint(color);
        self.ctx.set_stroke(KurboStroke::new(style.width));

        let mut path = BezPath::new();
        path.move_to(start);
        path.line_to(end);
        self.ctx.stroke_path(&path);
    }

    fn draw_polyline(&mut self, points: &[Point], style: &StrokeStyle) {
        if points.len() < 2 {
            return;
        }

        let mut path = BezPath::new();
        path.move_to(points[0]);
        for point in &points[1..] {
            path.line_to(*point);
        }

        let color = Self::color_to_vello(&style.color);
        self.ctx.set_paint(color);
        self.ctx.set_stroke(KurboStroke::new(style.width));
        self.ctx.stroke_path(&path);
    }

    fn draw_path(&mut self, path: &BezPath, style: &FillStrokeStyle) {
        if let Some(fill) = &style.fill {
            let color = Self::color_to_vello(fill);
            self.ctx.set_paint(color);
            self.ctx.fill_path(path);
        }

        if let Some(stroke) = &style.stroke {
            let color = Self::color_to_vello(&stroke.color);
            self.ctx.set_paint(color);
            self.set_stroke_style(stroke);
            self.ctx.stroke_path(path);
        }
    }

    fn draw_gradient_path(
        &mut self,
        path: &BezPath,
        gradient: &GradientDef,
        stroke: Option<&Stroke>,
    ) {
        use vello_cpu::{
            kurbo::Point as KurboPoint,
            peniko::{
                ColorStop, ColorStops, Extend, Gradient, GradientKind, InterpolationAlphaSpace,
                LinearGradientPosition,
                color::{ColorSpaceTag, DynamicColor, HueDirection},
            },
        };

        let stops: Vec<ColorStop> = gradient
            .stops
            .iter()
            .map(|(offset, color)| ColorStop {
                offset: *offset as f32,
                color: DynamicColor::from_alpha_color(AlphaColor::from_rgba8(
                    color.r, color.g, color.b, color.a,
                )),
            })
            .collect();

        // 使用路径的包围盒来确定渐变坐标，使渐变覆盖整个路径区域
        let bounds = path.bounding_box();
        let peniko_gradient = Gradient {
            kind: GradientKind::Linear(LinearGradientPosition {
                start: KurboPoint::new(bounds.x0, bounds.y0),
                end: KurboPoint::new(bounds.x1, bounds.y0),
            }),
            extend: Extend::Pad,
            interpolation_cs: ColorSpaceTag::Srgb,
            hue_direction: HueDirection::default(),
            interpolation_alpha_space: InterpolationAlphaSpace::Premultiplied,
            stops: ColorStops::from(stops.as_slice()),
        };

        self.ctx.set_paint(peniko_gradient);
        self.ctx.fill_path(path);

        if let Some(stroke) = stroke {
            let color = Self::color_to_vello(&stroke.color);
            self.ctx.set_paint(color);
            self.set_stroke_style(stroke);
            self.ctx.stroke_path(path);
        }
    }

    fn draw_text(
        &mut self,
        _text: &str,
        position: Point,
        _color: Color,
        _font_size: f64,
        _font_family: &str,
        rotation: f64,
        layout: Option<&TextLayout>,
    ) {
        use vello_cpu::kurbo::Affine;

        let Some(layout) = layout else {
            return;
        };

        // position 是组件已计算好的文本块左上角
        // glyph 坐标是 layout 内绝对坐标，直接平移+旋转即可
        let transform = Affine::translate((position.x, position.y)) * Affine::rotate(rotation);

        // 遍历布局中的每一行和每个 glyph run
        for line in layout.lines() {
            for item in line.items() {
                match item {
                    parley::layout::PositionedLayoutItem::GlyphRun(glyph_run) => {
                        let run = glyph_run.run();

                        // 获取字体数据
                        let font_data = run.font();
                        let run_font_size = run.font_size();

                        // 将 parley 的 glyph 转换为 vello_cpu 的 glyph
                        let glyphs: Vec<vello_cpu::Glyph> = glyph_run
                            .positioned_glyphs()
                            .map(|g| vello_cpu::Glyph {
                                id: g.id,
                                x: g.x,
                                y: g.y,
                            })
                            .collect();

                        if glyphs.is_empty() {
                            continue;
                        }

                        // 获取颜色
                        let brush = glyph_run.style().brush;
                        self.ctx.set_paint(brush.as_vello_color());

                        // 使用 vello_cpu 渲染 glyph run
                        self.ctx
                            .glyph_run(&mut self.resources, font_data)
                            .font_size(run_font_size)
                            .glyph_transform(transform)
                            .fill_glyphs(glyphs.into_iter());
                    }
                    parley::layout::PositionedLayoutItem::InlineBox(_inline_box) => {
                        // 内联盒子：目前暂不处理
                    }
                }
            }
        }
    }

    fn begin_group(&mut self, _transform: Option<&Transform>) {
        // vello_cpu暂不支持变换组，直接渲染子元素
        // 未来可以在这里保存/恢复渲染状态
    }

    fn end_group(&mut self) {
    }
}

/// Build a Bezier approximation of a rounded rectangle.
fn rounded_rect_path(rect: Rect, radius: f64) -> BezPath {
    let w = rect.width() / 2.0;
    let h = rect.height() / 2.0;
    let cx = rect.x0 + w;
    let cy = rect.y0 + h;
    let r = radius.min(w).min(h);
    let segments = 10;
    let mut path = BezPath::new();
    // 顶边
    path.move_to(Point::new(cx - w + r, cy - h));
    path.line_to(Point::new(cx + w - r, cy - h));
    // 右上角
    for i in 0..=segments {
        let a = std::f64::consts::FRAC_PI_2 * i as f64 / segments as f64
            - std::f64::consts::FRAC_PI_2;
        path.line_to(Point::new(cx + w - r + r * a.cos(), cy - h + r + r * a.sin()));
    }
    // 右边
    path.line_to(Point::new(cx + w, cy + h - r));
    // 右下角
    for i in 0..=segments {
        let a = std::f64::consts::FRAC_PI_2 * i as f64 / segments as f64;
        path.line_to(Point::new(cx + w - r + r * a.cos(), cy + h - r + r * a.sin()));
    }
    // 底边
    path.line_to(Point::new(cx - w + r, cy + h));
    // 左下角
    for i in 0..=segments {
        let a = std::f64::consts::FRAC_PI_2 * i as f64 / segments as f64
            + std::f64::consts::FRAC_PI_2;
        path.line_to(Point::new(cx - w + r + r * a.cos(), cy + h - r + r * a.sin()));
    }
    // 左边
    path.line_to(Point::new(cx - w, cy - h + r));
    // 左上角
    for i in 0..=segments {
        let a = std::f64::consts::FRAC_PI_2 * i as f64 / segments as f64
            + std::f64::consts::PI;
        path.line_to(Point::new(cx - w + r + r * a.cos(), cy - h + r + r * a.sin()));
    }
    path.close_path();
    path
}
