//! Multiple rendering backends (bitmap, SVG, etc.).

mod pixmap;
mod svg;

pub use pixmap::PixmapRenderer;
pub use svg::SvgRenderer;
use vello_cpu::kurbo::{BezPath, Point, Rect};

use crate::{
    text::TextLayout,
    visual::{FillStrokeStyle, GradientDef, Stroke, StrokeStyle, Transform, VisualElement},
};

/// Abstract renderer interface for drawing visual elements to an output format.
pub trait Renderer {
    /// Draws a filled or stroked rectangle (radius = Some(r) for rounded corners).
    fn draw_rect(&mut self, rect: Rect, radius: Option<f64>, style: &FillStrokeStyle);

    /// Draws a filled or stroked circle.
    fn draw_circle(&mut self, center: Point, radius: f64, style: &FillStrokeStyle);

    /// Draws a single line segment.
    fn draw_line(&mut self, start: Point, end: Point, style: &StrokeStyle);

    /// Draws a connected polyline.
    fn draw_polyline(&mut self, points: &[Point], style: &StrokeStyle);

    /// Draws a filled or stroked bezier path.
    fn draw_path(&mut self, path: &BezPath, style: &FillStrokeStyle);

    /// Draws a gradient-filled bezier path with an optional stroke.
    fn draw_gradient_path(
        &mut self,
        path: &BezPath,
        gradient: &GradientDef,
        stroke: Option<&Stroke>,
    );

    /// Draws text at the given position.
    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        color: crate::visual::Color,
        font_size: f64,
        font_family: &str,
        rotation: f64,
        layout: Option<&TextLayout>,
    );

    /// Begins a transformed group.
    fn begin_group(&mut self, transform: Option<&Transform>);

    /// Ends the current transformed group.
    fn end_group(&mut self);

    /// Default implementation that iterates over visual elements and dispatches to atomic methods.
    fn render_elements(&mut self, elements: &[VisualElement])
    where
        Self: Sized,
    {
        let mut sorted: Vec<&VisualElement> = elements.iter().collect();
        sorted.sort_by_key(|e| e.z_index());
        for element in sorted {
            self.render_element(element);
        }
    }

    /// 渲染单个视觉元素
    fn render_element(&mut self, element: &VisualElement)
    where
        Self: Sized,
    {
        match element {
            VisualElement::Rect { rect, radius, style, .. } => {
                self.draw_rect(*rect, *radius, style);
            }
            VisualElement::Circle {
                center,
                radius,
                style,
                ..
            } => {
                self.draw_circle(*center, *radius, style);
            }
            VisualElement::Line { start, end, style, .. } => {
                self.draw_line(*start, *end, style);
            }
            VisualElement::Polyline { points, style, .. } => {
                self.draw_polyline(points, style);
            }
            VisualElement::Path { path, style, .. } => {
                self.draw_path(path, style);
            }
            VisualElement::GradientPath {
                path,
                gradient,
                stroke,
                ..
            } => {
                self.draw_gradient_path(path, gradient, stroke.as_ref());
            }
            VisualElement::TextRun {
                text,
                position,
                style,
                rotation,
                layout,
                ..
            } => {
                self.draw_text(
                    text,
                    *position,
                    style.color,
                    style.font_size,
                    &style.font_family,
                    *rotation,
                    layout.as_ref(),
                );
            }
            VisualElement::Group {
                children,
                transform,
                ..
            } => {
                self.begin_group(transform.as_ref());
                self.render_elements(children);
                self.end_group();
            }
        }
    }
}
