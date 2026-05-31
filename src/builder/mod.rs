pub mod class;
pub mod er;
pub mod flowchart;
pub mod gitgraph;
pub mod layout;
pub mod pie;
pub mod sequence;
pub mod state;
pub mod timeline;
pub mod types;

use crate::{
    ast::Diagram,
    builder::{
        layout::types::LayoutEngine,
        types::OutputConfig,
    },
    error::DiagramResult,
    visual::{TextAlign, TextBaseline, Transform, VisualElement},
};
use vello_cpu::kurbo::Vec2;

use vello_cpu::kurbo::PathSeg;

/// 构建 Mermaid Diagram 的视觉元素管线
///
/// # 管线流程
///
/// 1. 分发：根据 Diagram 枚举类型创建对应的 LayoutEngine
/// 2. 布局：引擎内部执行布局管线，输出视觉元素
/// 3. 渲染：返回 Vec<VisualElement> 供渲染器消费
pub fn build_diagram(diagram: &Diagram) -> DiagramResult<Vec<VisualElement>> {
    build_diagram_with_config(diagram, &OutputConfig::default())
}

pub fn build_diagram_with_config(
    diagram: &Diagram,
    config: &OutputConfig,
) -> DiagramResult<Vec<VisualElement>> {
    let engine: Box<dyn LayoutEngine + '_> = match diagram {
        Diagram::Pie(pie) => Box::new(pie::PieEngine::new(pie)),
        Diagram::Flowchart(fc) => Box::new(flowchart::FlowchartEngine::new(fc)),
        Diagram::Sequence(seq) => Box::new(sequence::SequenceEngine::new(seq)),
        Diagram::Class(class) => Box::new(class::ClassEngine::new(class)),
        Diagram::State(state) => Box::new(state::StateEngine::new(state)),
        Diagram::Er(er) => Box::new(er::ErEngine::new(er)),
        Diagram::Timeline(timeline) => Box::new(timeline::TimelineEngine::new(timeline)),
        Diagram::GitGraph(gg) => Box::new(gitgraph::GitGraphEngine::new(gg)),
    };
    let elements = engine.layout(config)?;
    Ok(fit_to_canvas(elements, config))
}

/// 计算所有 VisualElement 的外包矩形（画布无关的近似外包）
fn compute_bbox(elements: &[VisualElement]) -> Option<(f64, f64, f64, f64)> {
    let mut min_x: Option<f64> = None;
    let mut min_y: Option<f64> = None;
    let mut max_x: Option<f64> = None;
    let mut max_y: Option<f64> = None;

    for element in elements {
        macro_rules! expand {
            ($x:expr, $y:expr) => {
                let x = $x;
                let y = $y;
                if min_x.is_none() || x < min_x.unwrap() { min_x = Some(x); }
                if min_y.is_none() || y < min_y.unwrap() { min_y = Some(y); }
                if max_x.is_none() || x > max_x.unwrap() { max_x = Some(x); }
                if max_y.is_none() || y > max_y.unwrap() { max_y = Some(y); }
            };
        }

        match element {
            VisualElement::Rect { rect, .. } => {
                expand!(rect.x0, rect.y0);
                expand!(rect.x1, rect.y1);
            }
            VisualElement::Circle { center, radius, .. } => {
                expand!(center.x - radius, center.y - radius);
                expand!(center.x + radius, center.y + radius);
            }
            VisualElement::Line { start, end, .. } => {
                expand!(start.x, start.y);
                expand!(end.x, end.y);
            }
            VisualElement::Polyline { points, .. } => {
                for p in points {
                    expand!(p.x, p.y);
                }
            }
            VisualElement::Path { path, .. } | VisualElement::GradientPath { path, .. } => {
                for seg in path.segments() {
                    match seg {
                        PathSeg::Line(line) => {
                            expand!(line.p0.x, line.p0.y);
                            expand!(line.p1.x, line.p1.y);
                        }
                        PathSeg::Quad(quad) => {
                            expand!(quad.p0.x, quad.p0.y);
                            expand!(quad.p1.x, quad.p1.y);
                            expand!(quad.p2.x, quad.p2.y);
                        }
                        PathSeg::Cubic(cubic) => {
                            expand!(cubic.p0.x, cubic.p0.y);
                            expand!(cubic.p1.x, cubic.p1.y);
                            expand!(cubic.p2.x, cubic.p2.y);
                            expand!(cubic.p3.x, cubic.p3.y);
                        }
                    }
                }
            }
            VisualElement::TextRun { position, layout, style, .. } => {
                let tw = layout.as_ref().map(|l| l.width() as f64).unwrap_or(style.font_size * 4.0);
                let th = layout.as_ref().map(|l| l.height() as f64).unwrap_or(style.font_size);
                let (tx0, tx1) = match style.align {
                    TextAlign::Left => (position.x, position.x + tw),
                    TextAlign::Center => (position.x - tw / 2.0, position.x + tw / 2.0),
                    TextAlign::Right => (position.x - tw, position.x),
                };
                let (ty0, ty1) = match style.vertical_align {
                    TextBaseline::Top => (position.y, position.y + th),
                    TextBaseline::Middle => (position.y - th / 2.0, position.y + th / 2.0),
                    TextBaseline::Bottom => (position.y - th, position.y),
                    TextBaseline::Alphabetic => (position.y - th * 0.8, position.y + th * 0.2),
                };
                expand!(tx0, ty0);
                expand!(tx1, ty1);
            }
            VisualElement::Group { children, .. } => {
                if let Some((cx0, cy0, cx1, cy1)) = compute_bbox(children) {
                    expand!(cx0, cy0);
                    expand!(cx1, cy1);
                }
            }
        }
    }

    match (min_x, min_y, max_x, max_y) {
        (Some(x0), Some(y0), Some(x1), Some(y1)) => Some((x0, y0, x1, y1)),
        _ => None,
    }
}

/// 将内容居中适配到画布：
/// 1. 计算所有元素的外包矩形
/// 2. 若内容超出可用空间（画布 - 2×margin），按比例缩小
/// 3. 用 Group + transform（translate + scale）居中
fn fit_to_canvas(elements: Vec<VisualElement>, config: &OutputConfig) -> Vec<VisualElement> {
    let Some((x0, y0, x1, y1)) = compute_bbox(&elements) else {
        return elements;
    };
    let content_w = x1 - x0;
    let content_h = y1 - y0;
    let margin = 40.0;
    let avail_w = config.width as f64 - 2.0 * margin;
    let avail_h = config.height as f64 - 2.0 * margin;

    // 计算缩放比例（绝不放大）
    let scale = (avail_w / content_w)
        .min(avail_h / content_h)
        .min(1.0);

    // 缩放后的内容尺寸
    let scaled_w = content_w * scale;
    let scaled_h = content_h * scale;

    // 居中偏移量
    let offset_x = ((config.width as f64 - scaled_w) / 2.0) - x0 * scale;
    let offset_y = ((config.height as f64 - scaled_h) / 2.0) - y0 * scale;

    if offset_x.abs() < 0.5 && offset_y.abs() < 0.5 && (scale - 1.0).abs() < 0.001 {
        return elements;
    }

    let transform = Transform {
        translate: Vec2::new(offset_x, offset_y),
        scale: Vec2::new(scale, scale),
        ..Default::default()
    };

    vec![VisualElement::Group {
        children: elements,
        transform: Some(transform),
        z_index: 0,
    }]
}

