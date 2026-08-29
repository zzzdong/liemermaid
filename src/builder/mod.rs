pub mod extract;
pub mod ir;
pub mod layout;
pub mod materialize;
pub mod measure;
pub mod paint;
pub mod theme;
pub mod types;

use crate::{ast::Diagram, builder::types::OutputConfig, error::DiagramResult};
use lievisual::geometry::PathSeg;
use lievisual::geometry::Transform;
use lievisual::scene::{Element, SceneNode};
use lievisual::text::{TextAlign, TextBaseline};

/// 构建 Mermaid Diagram 的视觉元素管线
///
/// 统一管线：AST → `extract`（UG）→ `measure`（UG'）→ `layout::engine`（GG）→
/// `materialize`（SceneGraph）→ `paint`（lievisual::Scene）→ `fit_to_canvas` 适配画布。
pub fn build_diagram(diagram: &Diagram) -> DiagramResult<lievisual::Scene> {
    build_diagram_with_config(diagram, &OutputConfig::default())
}

pub fn build_diagram_with_config(
    diagram: &Diagram,
    config: &OutputConfig,
) -> DiagramResult<lievisual::Scene> {
    // 新四阶段管线：extract → measure → layout → materialize → paint。
    // 全部图类型均已接入 extract，无旧管线降级分支。
    let ug = extract::run(diagram)?;
    let ug = measure::measure_all(ug);
    let (gg, style) = layout::engine::run(&ug)
        .map_err(|e| crate::error::DiagramError::RenderError(format!("layout failed: {e}")))?;
    let sg = materialize::run(&gg, &style);
    let scene = paint::run(&sg);
    // 画布贴合内容（官方 mermaid 语义：`config` 是上限而非固定画布），
    // 具体尺寸由 fit_to_canvas 据内容包围盒算出。
    let (nodes, w, h) = fit_to_canvas(scene.nodes, config);
    let mut out = lievisual::Scene::new(w, h);
    out.background = config.background;
    out.nodes.extend(nodes);
    Ok(out)
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
            Element::Ellipse { center, radii, .. } => {
                expand!(center.x - radii.x, center.y - radii.y);
                expand!(center.x + radii.x, center.y + radii.y);
            }
            Element::Pie { center, radius, .. } => {
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

/// 画布边距（内容外留白，对应官方 mermaid 的 `diagramPadding` 量级）。
const CANVAS_MARGIN: f64 = 8.0;

/// 将内容适配到画布，**画布尺寸贴合内容**（对齐官方 mermaid）：
/// 官方输出 `width="100%"` + `viewBox=内容包围盒`，不留大片空白；
/// 因此这里把 `config.width/height` 当作**上限**（内容超出才等比缩小，绝不放大），
/// 画布最终尺寸 = 缩放后内容 + 2×`CANVAS_MARGIN`。
///
/// 返回（适配后的节点, 画布宽, 画布高）。
fn fit_to_canvas(
    elements: Vec<SceneNode>,
    config: &OutputConfig,
) -> (Vec<SceneNode>, f64, f64) {
    let Some((x0, y0, x1, y1)) = compute_bbox(&elements) else {
        return (elements, config.width, config.height);
    };
    let content_w = (x1 - x0).max(1.0);
    let content_h = (y1 - y0).max(1.0);
    let avail_w = config.width - 2.0 * CANVAS_MARGIN;
    let avail_h = config.height - 2.0 * CANVAS_MARGIN;

    // 计算缩放比例（绝不放大）：配置尺寸是**上限**而非固定画布。
    let scale = (avail_w / content_w).min(avail_h / content_h).min(1.0);

    // 画布贴合内容（仅留边距），不再按 config 撑满。
    let canvas_w = content_w * scale + 2.0 * CANVAS_MARGIN;
    let canvas_h = content_h * scale + 2.0 * CANVAS_MARGIN;

    // 把内容外包左上角平移到 (margin, margin)
    let offset_x = CANVAS_MARGIN - x0 * scale;
    let offset_y = CANVAS_MARGIN - y0 * scale;

    if offset_x.abs() < 0.5 && offset_y.abs() < 0.5 && (scale - 1.0).abs() < 0.001 {
        // 内容已在原点附近：仅平移到边距即可，无需 Group 包裹。
        return (elements, canvas_w, canvas_h);
    }

    let transform = Transform::translate(offset_x, offset_y).then(&Transform::scale(scale));

    (
        vec![
            SceneNode::new(Element::Group { children: elements })
                .with_z(0)
                .with_transform(transform),
        ],
        canvas_w,
        canvas_h,
    )
}
