pub mod layout;
pub mod render;
pub mod theme;
pub mod types;

use crate::{ast::Diagram, builder::types::OutputConfig, error::DiagramResult};
use lievisual::geometry::PathSeg;
use lievisual::geometry::Transform;
use lievisual::scene::{Element, SceneNode};
use lievisual::text::{TextAlign, TextBaseline};

/// 构建 Mermaid Diagram 的视觉元素管线
///
/// # 管线流程
///
/// 1. 统一入口：所有图表类型都先经 `layout::layout_diagram` 生成 `PlacedGraph`
///    （部分图表几何语义在渲染层基于 AST 自绘，见 `render`）。
/// 2. 渲染：由 `render::render_placed` 根据图表类型分派渲染器，输出 `SceneNode`。
/// 3. 适配：所有 `SceneNode` 经 `fit_to_canvas` 居中适配到画布。
pub fn build_diagram(diagram: &Diagram) -> DiagramResult<lievisual::Scene> {
    build_diagram_with_config(diagram, &OutputConfig::default())
}

pub fn build_diagram_with_config(
    diagram: &Diagram,
    config: &OutputConfig,
) -> DiagramResult<lievisual::Scene> {
    let layout_config = layout::LayoutConfig::default();
    let placed = layout::layout_diagram(diagram, &layout_config, config);
    let nodes = render::render_placed(&placed, diagram, config);
    let mut scene = lievisual::Scene::new(config.width, config.height);
    scene.background = config.background;
    scene.nodes.extend(fit_to_canvas(nodes, config));
    Ok(scene)
}

/// 计算所有 SceneNode 的外包矩形（画布无关的近似外包）
fn compute_bbox(elements: &[SceneNode]) -> Option<(f64, f64, f64, f64)> {
    let mut min_x: Option<f64> = None;
    let mut min_y: Option<f64> = None;
    let mut max_x: Option<f64> = None;
    let mut max_y: Option<f64> = None;

    for element in elements {
        macro_rules! expand {
            ($x:expr, $y:expr) => {
                let x = $x;
                let y = $y;
                if min_x.is_none() || x < min_x.unwrap() {
                    min_x = Some(x);
                }
                if min_y.is_none() || y < min_y.unwrap() {
                    min_y = Some(y);
                }
                if max_x.is_none() || x > max_x.unwrap() {
                    max_x = Some(x);
                }
                if max_y.is_none() || y > max_y.unwrap() {
                    max_y = Some(y);
                }
            };
        }

        match &element.element {
            Element::Rect { rect, .. } | Element::RoundedRect { rect, .. } => {
                expand!(rect.min_x(), rect.min_y());
                expand!(rect.max_x(), rect.max_y());
            }
            Element::Circle { center, radius, .. } => {
                expand!(center.x - radius, center.y - radius);
                expand!(center.x + radius, center.y + radius);
            }
            Element::Line { start, end, .. } => {
                expand!(start.x, start.y);
                expand!(end.x, end.y);
            }
            Element::Polyline { points, .. } => {
                for p in points {
                    expand!(p.x, p.y);
                }
            }
            Element::Path { path, .. } | Element::GradientPath { path, .. } => {
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
            Element::Text {
                position,
                layout,
                style,
                ..
            } => {
                let tw = layout
                    .as_ref()
                    .map(|l| l.width)
                    .unwrap_or(style.font_size * 4.0);
                let th = layout.as_ref().map(|l| l.height).unwrap_or(style.font_size);
                let (tx0, tx1) = match style.align {
                    TextAlign::Left | TextAlign::Justify => (position.x, position.x + tw),
                    TextAlign::Center => (position.x - tw / 2.0, position.x + tw / 2.0),
                    TextAlign::Right => (position.x - tw, position.x),
                };
                let (ty0, ty1) = match style.baseline {
                    TextBaseline::Top => (position.y, position.y + th),
                    TextBaseline::Middle => (position.y - th / 2.0, position.y + th / 2.0),
                    TextBaseline::Bottom => (position.y - th, position.y),
                    _ => (position.y - th * 0.8, position.y + th * 0.2),
                };
                expand!(tx0, ty0);
                expand!(tx1, ty1);
            }
            Element::Group { children } => {
                if let Some((cx0, cy0, cx1, cy1)) = compute_bbox(children) {
                    expand!(cx0, cy0);
                    expand!(cx1, cy1);
                }
            }
            _ => {}
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
fn fit_to_canvas(elements: Vec<SceneNode>, config: &OutputConfig) -> Vec<SceneNode> {
    let Some((x0, y0, x1, y1)) = compute_bbox(&elements) else {
        return elements;
    };
    let content_w = x1 - x0;
    let content_h = y1 - y0;
    let margin = 40.0;
    let avail_w = config.width - 2.0 * margin;
    let avail_h = config.height - 2.0 * margin;

    // 计算缩放比例（绝不放大）
    let scale = (avail_w / content_w).min(avail_h / content_h).min(1.0);

    // 缩放后的内容尺寸
    let scaled_w = content_w * scale;
    let scaled_h = content_h * scale;

    // 居中偏移量
    let offset_x = ((config.width - scaled_w) / 2.0) - x0 * scale;
    let offset_y = ((config.height - scaled_h) / 2.0) - y0 * scale;

    if offset_x.abs() < 0.5 && offset_y.abs() < 0.5 && (scale - 1.0).abs() < 0.001 {
        return elements;
    }

    let transform = Transform::translate(offset_x, offset_y).then(&Transform::scale(scale));

    vec![
        SceneNode::new(Element::Group { children: elements })
            .with_z(0)
            .with_transform(transform),
    ]
}
