//! SVG renderer — converts visual elements to SVG markup.

use std::collections::HashMap;

use vello_cpu::kurbo::{BezPath, PathSeg, Point, Rect};

use crate::{
    error::DiagramResult as Result,
    render::Renderer,
    text::TextLayout,
    visual::{Color, FillStrokeStyle, GradientDef, Stroke, StrokeStyle, Transform, VisualElement},
};

/// SVG renderer that produces XML markup for vector output.
#[derive(Debug)]
pub struct SvgRenderer {
    svg_content: String,
    gradient_map: HashMap<*const GradientDef, usize>,
    background_color: Option<Color>,
}

impl SvgRenderer {
    pub fn new() -> Self {
        Self {
            svg_content: String::new(),
            gradient_map: HashMap::new(),
            background_color: None,
        }
    }

    /// 设置背景色（None = 透明）
    pub fn with_background(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// 递归收集所有渐变定义并建立索引映射
    fn collect_gradients(
        elements: &[VisualElement],
    ) -> (Vec<&GradientDef>, HashMap<*const GradientDef, usize>) {
        let mut gradients = Vec::new();
        let mut map = HashMap::new();
        Self::collect_gradients_recursive(elements, &mut gradients, &mut map);
        (gradients, map)
    }

    fn collect_gradients_recursive<'a>(
        elements: &'a [VisualElement],
        gradients: &mut Vec<&'a GradientDef>,
        map: &mut HashMap<*const GradientDef, usize>,
    ) {
        for element in elements {
            match element {
                VisualElement::GradientPath { gradient, .. } => {
                    let ptr: *const GradientDef = gradient as *const GradientDef;
                    map.entry(ptr).or_insert_with(|| {
                        let id = gradients.len();
                        gradients.push(gradient);
                        id
                    });
                }
                VisualElement::Group { children, .. } => {
                    Self::collect_gradients_recursive(children, gradients, map);
                }
                _ => {}
            }
        }
    }

    /// 生成 SVG 渐变定义字符串
    fn gradient_to_svg_defs(gradients: &[&GradientDef]) -> String {
        if gradients.is_empty() {
            return String::new();
        }

        let mut defs = String::from("<defs>\n");
        for (i, gradient) in gradients.iter().enumerate() {
            defs.push_str(&format!(
                r#"<linearGradient id="g{}" x1="0%" y1="0%" x2="100%" y2="0%">"#,
                i
            ));
            defs.push('\n');
            for (offset, color) in &gradient.stops {
                defs.push_str(&format!(
                    r#"<stop offset="{}%" stop-color="{}" />"#,
                    (offset * 100.0) as i32,
                    Self::color_to_css(color)
                ));
                defs.push('\n');
            }
            defs.push_str("</linearGradient>\n");
        }
        defs.push_str("</defs>\n");
        defs
    }

    /// 渲染视觉元素序列并输出 SVG 字符串
    pub fn render(mut self, elements: &[VisualElement], width: u32, height: u32) -> Result<String> {
        let mut svg = String::new();

        // SVG 头部
        svg.push_str(&format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            width, height, width, height
        ));
        svg.push('\n');

        // 背景色
        if let Some(bg) = self.background_color {
            svg.push_str(&format!(
                r#"<rect width="100%" height="100%" fill="{}" />"#,
                Self::color_to_css(&bg)
            ));
            svg.push('\n');
        }

        // 收集渐变并生成 defs
        let (gradients, map) = Self::collect_gradients(elements);
        self.gradient_map = map;
        let defs = Self::gradient_to_svg_defs(&gradients);
        svg.push_str(&defs);

        // 渲染所有元素
        self.svg_content.clear();
        self.render_elements(elements);
        svg.push_str(&self.svg_content);

        // SVG 尾部
        svg.push_str("</svg>\n");

        Ok(svg)
    }

    fn color_to_css(color: &Color) -> String {
        if color.a == 255 {
            format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
        } else {
            format!(
                "rgba({}, {}, {}, {})",
                color.r,
                color.g,
                color.b,
                color.a as f64 / 255.0
            )
        }
    }

    fn escape_xml(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    fn bezpath_to_svg_d(path: &BezPath, close: bool) -> String {
        let mut d = String::new();
        for seg in path.segments() {
            match seg {
                PathSeg::Line(line) => {
                    if d.is_empty() {
                        d.push_str(&format!("M {} {}", line.p0.x, line.p0.y));
                    }
                    d.push_str(&format!(" L {} {}", line.p1.x, line.p1.y));
                }
                PathSeg::Quad(quad) => {
                    if d.is_empty() {
                        d.push_str(&format!("M {} {}", quad.p0.x, quad.p0.y));
                    }
                    d.push_str(&format!(
                        " Q {} {} {} {}",
                        quad.p1.x, quad.p1.y, quad.p2.x, quad.p2.y
                    ));
                }
                PathSeg::Cubic(cubic) => {
                    if d.is_empty() {
                        d.push_str(&format!("M {} {}", cubic.p0.x, cubic.p0.y));
                    }
                    d.push_str(&format!(
                        " C {} {} {} {} {} {}",
                        cubic.p1.x, cubic.p1.y, cubic.p2.x, cubic.p2.y, cubic.p3.x, cubic.p3.y
                    ));
                }
            }
        }
        if close && !d.is_empty() {
            d.push_str(" Z");
        }
        d
    }
}

impl Renderer for SvgRenderer {
    fn draw_rect(&mut self, rect: Rect, radius: Option<f64>, style: &FillStrokeStyle) {
        let mut attrs = format!(
            r#"x="{}" y="{}" width="{}" height="{}""#,
            rect.x0,
            rect.y0,
            rect.width(),
            rect.height()
        );

        if let Some(r) = radius {
            attrs.push_str(&format!(r#" rx="{}" ry="{}""#, r, r));
        }

        if let Some(fill) = &style.fill {
            attrs.push_str(&format!(r#" fill="{}""#, Self::color_to_css(fill)));
        } else {
            attrs.push_str(r#" fill="none""#);
        }

        if let Some(stroke) = &style.stroke {
            attrs.push_str(&format!(
                r#" stroke="{}" stroke-width="{}""#,
                Self::color_to_css(&stroke.color),
                stroke.width
            ));
        }

        self.svg_content.push_str(&format!("<rect {} />\n", attrs));
    }

    fn draw_circle(&mut self, center: Point, radius: f64, style: &FillStrokeStyle) {
        let mut attrs = format!(r#"cx="{}" cy="{}" r="{}""#, center.x, center.y, radius);

        if let Some(fill) = &style.fill {
            attrs.push_str(&format!(r#" fill="{}""#, Self::color_to_css(fill)));
        } else {
            attrs.push_str(r#" fill="none""#);
        }

        if let Some(stroke) = &style.stroke {
            attrs.push_str(&format!(
                r#" stroke="{}" stroke-width="{}""#,
                Self::color_to_css(&stroke.color),
                stroke.width
            ));
        }

        self.svg_content
            .push_str(&format!("<circle {} />\n", attrs));
    }

    fn draw_line(&mut self, start: Point, end: Point, style: &StrokeStyle) {
        let line = format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" />"#,
            start.x,
            start.y,
            end.x,
            end.y,
            Self::color_to_css(&style.color),
            style.width
        );
        self.svg_content.push_str(&line);
        self.svg_content.push('\n');
    }

    fn draw_polyline(&mut self, points: &[Point], style: &StrokeStyle) {
        if points.len() < 2 {
            return;
        }

        let points_str: Vec<String> = points.iter().map(|p| format!("{}, {}", p.x, p.y)).collect();

        let polyline = format!(
            r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{}" />"#,
            points_str.join(" "),
            Self::color_to_css(&style.color),
            style.width
        );
        self.svg_content.push_str(&polyline);
        self.svg_content.push('\n');
    }

    fn draw_path(&mut self, path: &BezPath, style: &FillStrokeStyle) {
        let close = style.fill.is_some();
        let d = Self::bezpath_to_svg_d(path, close);
        let mut attrs = format!(r#"d="{}""#, d);

        if let Some(fill) = &style.fill {
            attrs.push_str(&format!(r#" fill="{}""#, Self::color_to_css(fill)));
        } else {
            attrs.push_str(r#" fill="none""#);
        }

        if let Some(stroke) = &style.stroke {
            attrs.push_str(&format!(
                r#" stroke="{}" stroke-width="{}""#,
                Self::color_to_css(&stroke.color),
                stroke.width
            ));
        }

        self.svg_content.push_str(&format!("<path {} />\n", attrs));
    }

    fn draw_gradient_path(
        &mut self,
        path: &BezPath,
        gradient: &GradientDef,
        stroke: Option<&Stroke>,
    ) {
        let d = Self::bezpath_to_svg_d(path, true);
        let mut attrs = format!(r#"d="{}""#, d);

        let gradient_id = self
            .gradient_map
            .get(&(gradient as *const GradientDef))
            .copied()
            .unwrap_or(0);
        attrs.push_str(&format!(r#" fill="url(#g{})""#, gradient_id));

        if let Some(stroke) = stroke {
            attrs.push_str(&format!(
                r#" stroke="{}" stroke-width="{}""#,
                Self::color_to_css(&stroke.color),
                stroke.width
            ));
        }

        self.svg_content.push_str(&format!("<path {} />\n", attrs));
    }

    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        color: Color,
        font_size: f64,
        font_family: &str,
        rotation: f64,
        layout: Option<&TextLayout>,
    ) {
        let Some(layout) = layout else {
            return;
        };

        // position 是文本块左上角
        // 不使用 text-anchor，直接计算每个 tspan 的绝对 x 坐标
        let text_attrs = format!(
            r#"fill="{}" font-size="{}" font-family="{}""#,
            Self::color_to_css(&color),
            font_size,
            Self::escape_xml(font_family),
        );

        // 添加旋转变换（围绕 position 旋转）
        let transform_attrs = if rotation != 0.0 {
            let degrees = rotation.to_degrees();
            format!(
                r#" transform="rotate({}, {}, {})""#,
                degrees, position.x, position.y
            )
        } else {
            String::new()
        };

        let mut tspans = String::new();

        // 遍历布局中的每一行
        for line in layout.lines() {
            let mut line_text = String::new();
            let mut first_glyph_pos: Option<(f64, f64)> = None;

            // 收集该行所有 glyph run 的文本和首个 glyph 位置
            for item in line.items() {
                match item {
                    parley::layout::PositionedLayoutItem::GlyphRun(glyph_run) => {
                        let run = glyph_run.run();

                        let text_range = run.text_range();
                        if text_range.end <= text_range.start {
                            continue;
                        }
                        let start = text_range.start.min(text.len());
                        let end = text_range.end.min(text.len());
                        let run_text = &text[start..end];

                        if run_text.is_empty() {
                            continue;
                        }

                        if first_glyph_pos.is_none() {
                            if let Some(first_glyph) = glyph_run.positioned_glyphs().next() {
                                let tspan_y = position.y + first_glyph.y as f64;
                                let tspan_x = position.x + first_glyph.x as f64;
                                first_glyph_pos = Some((tspan_x, tspan_y));
                            }
                        }

                        line_text.push_str(run_text);
                    }
                    parley::layout::PositionedLayoutItem::InlineBox(_) => {
                        // 内联盒子：目前暂不处理
                    }
                }
            }

            // 将该行所有文本合并为一个 tspan，让浏览器自然处理字间距
            if let Some((tspan_x, tspan_y)) = first_glyph_pos {
                if !line_text.is_empty() {
                    let escaped_text = Self::escape_xml(&line_text);
                    let tspan = format!(
                        r#"<tspan x="{}" y="{}">{}</tspan>"#,
                        tspan_x, tspan_y, escaped_text
                    );
                    tspans.push_str(&tspan);
                }
            }
        }

        if !tspans.is_empty() {
            self.svg_content.push_str(&format!(
                "<text {}{}>{}</text>\n",
                text_attrs, transform_attrs, tspans
            ));
        }
    }

    fn begin_group(&mut self, transform: Option<&Transform>) {
        let mut attrs = String::new();

        if let Some(transform) = transform {
            let mut transforms = Vec::new();

            if transform.translate.x != 0.0 || transform.translate.y != 0.0 {
                transforms.push(format!(
                    "translate({}, {})",
                    transform.translate.x, transform.translate.y
                ));
            }

            if transform.rotate != 0.0 {
                transforms.push(format!("rotate({})", transform.rotate.to_degrees()));
            }

            if transform.scale.x != 1.0 || transform.scale.y != 1.0 {
                transforms.push(format!(
                    "scale({}, {})",
                    transform.scale.x, transform.scale.y
                ));
            }

            if !transforms.is_empty() {
                attrs.push_str(&format!(r#" transform="{}""#, transforms.join(" ")));
            }
        }

        self.svg_content.push_str(&format!("<g{}>\n", attrs));
    }

    fn end_group(&mut self) {
        self.svg_content.push_str("</g>\n");
    }
}

impl Default for SvgRenderer {
    fn default() -> Self {
        Self::new()
    }
}
