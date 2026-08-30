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

/// 解析配置尺寸：`None`（不限）或非法值（NaN/Inf/负数）退回默认，用于空内容的兜底画布。
fn resolve_cfg(limit: Option<f64>, default: f64) -> f64 {
    match limit {
        Some(v) if v.is_finite() && v > 0.0 => v,
        _ => default,
    }
}

/// 解析可用区域（配置尺寸减 2×边距）：`None` 表示不限该维度（无穷大），
/// 从而支持「只按宽度 / 只按高度」约束输出尺寸。
fn resolve_avail(limit: Option<f64>, default: f64) -> f64 {
    match limit {
        Some(v) if v.is_finite() && v > 0.0 => (v - 2.0 * CANVAS_MARGIN).max(1.0),
        Some(_) => (default - 2.0 * CANVAS_MARGIN).max(1.0),
        None => f64::INFINITY,
    }
}

/// 将内容适配到画布，**画布尺寸贴合内容**（对齐官方 mermaid）：
/// 官方输出 `width="100%"` + `viewBox=内容包围盒`，不留大片空白；
/// 因此这里把 `config.width/height` 当作**上限**（内容超出才等比缩小，绝不放大），
/// `None` 表示不限该维度；画布最终尺寸 = 缩放后内容 + 2×`CANVAS_MARGIN`。
///
/// 返回（适配后的节点, 画布宽, 画布高）。
fn fit_to_canvas(elements: Vec<SceneNode>, config: &OutputConfig) -> (Vec<SceneNode>, f64, f64) {
    // 配置尺寸为 `None`（不限）或非法（NaN/Inf/负数）时退回默认，用于空内容的兜底画布。
    let cfg_w = resolve_cfg(config.width, types::DEFAULT_WIDTH);
    let cfg_h = resolve_cfg(config.height, types::DEFAULT_HEIGHT);

    let Some((x0, y0, x1, y1)) = compute_bbox(&elements) else {
        return (elements, cfg_w, cfg_h);
    };
    if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
        // 内容含非法坐标（NaN/Inf）时不做适配，避免污染整个画布变换。
        return (elements, cfg_w, cfg_h);
    }
    let content_w = (x1 - x0).max(1.0);
    let content_h = (y1 - y0).max(1.0);

    // 可用区域至少 1pt：配置尺寸小于 2×边距时不能让 scale 变成负数/零，
    // 否则画布尺寸会下溢为负值（曾出现 `viewBox="0 0 0 -20.41"`）。
    // `None` 表示不限该维度（无穷大），从而支持「只按宽度 / 只按高度」约束。
    let avail_w = resolve_avail(config.width, types::DEFAULT_WIDTH);
    let avail_h = resolve_avail(config.height, types::DEFAULT_HEIGHT);

    // 计算缩放比例：默认「绝不放大」（配置尺寸是**上限**，对齐官方 mermaid）。
    // `config.upscale` 时允许放大到目标尺寸（PNG 位图需要足够像素）。
    // 下限取最小正数，保证 scale 恒为正（画布尺寸不会退化）。
    let natural = (avail_w / content_w).min(avail_h / content_h);
    // 无任何约束（宽高均为 None）时 natural 为无穷，退化为不缩放（放大目标不明确）。
    let natural = if natural.is_finite() { natural } else { 1.0 };
    let max_scale = if config.upscale { f64::MAX } else { 1.0 };
    let scale = natural.clamp(f64::MIN_POSITIVE, max_scale);

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

#[cfg(test)]
mod tests {
    use super::*;
    use lievisual::geometry::Rect;
    use lievisual::scene::FillStrokeStyle;

    fn rect_node(x0: f64, y0: f64, x1: f64, y1: f64) -> SceneNode {
        SceneNode::new(Element::rect(
            Rect::new(x0, y0, x1, y1),
            FillStrokeStyle::default(),
        ))
    }

    #[test]
    fn fit_to_canvas_upscale_scales_up() {
        // 内容 100×100，目标 800×600：upscale 时应放大（受 600 高限制，scale=5.84）。
        let elements = vec![rect_node(0.0, 0.0, 100.0, 100.0)];
        let config = OutputConfig {
            upscale: true,
            ..OutputConfig::default()
        };
        let (_, w, h) = fit_to_canvas(elements, &config);
        assert!(
            w > 500.0 && h > 500.0,
            "upscale 应放大内容到目标尺寸，got {w}x{h}"
        );
    }

    #[test]
    fn fit_to_canvas_no_upscale_keeps_natural_size() {
        // 内容 100×100，目标 800×600：不 upscale 时保持自然尺寸（仅加边距）。
        let elements = vec![rect_node(0.0, 0.0, 100.0, 100.0)];
        let config = OutputConfig::default();
        let (_, w, h) = fit_to_canvas(elements, &config);
        assert!(
            w < 200.0 && h < 200.0,
            "不 upscale 应保持自然尺寸，got {w}x{h}"
        );
    }

    #[test]
    fn fit_to_canvas_width_only_constraint() {
        // 内容 100×100，只限宽 500（height=None），upscale 时按宽度放大、高度同比例。
        let elements = vec![rect_node(0.0, 0.0, 100.0, 100.0)];
        let config = OutputConfig {
            width: Some(500.0),
            height: None,
            upscale: true,
            ..OutputConfig::default()
        };
        let (_, w, h) = fit_to_canvas(elements, &config);
        assert!((w - 500.0).abs() < 1.0, "只限宽时应放大到目标宽度，got {w}");
        assert!((h - 500.0).abs() < 1.0, "高度应按比例同步放大，got {h}");
    }

    #[test]
    fn fit_to_canvas_height_only_constraint() {
        // 内容 100×100，只限高 500（width=None），upscale 时按高度放大、宽度同比例。
        let elements = vec![rect_node(0.0, 0.0, 100.0, 100.0)];
        let config = OutputConfig {
            width: None,
            height: Some(500.0),
            upscale: true,
            ..OutputConfig::default()
        };
        let (_, w, h) = fit_to_canvas(elements, &config);
        assert!((h - 500.0).abs() < 1.0, "只限高时应放大到目标高度，got {h}");
        assert!((w - 500.0).abs() < 1.0, "宽度应按比例同步放大，got {w}");
    }
}
