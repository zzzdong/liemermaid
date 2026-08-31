pub mod extract;
pub mod ir;
pub mod layout;
pub mod materialize;
pub mod measure;
pub mod paint;
pub mod theme;
pub mod types;

use crate::{ast::Diagram, builder::types::OutputConfig, error::DiagramResult};
use lievisual::fit::{FitOptions, fit_scene};
use lievisual::geometry::Size;

/// 构建 Mermaid Diagram 的视觉元素管线
///
/// 统一管线：AST → `extract`（UG）→ `measure`（UG'）→ `layout::engine`（GG）→
/// `materialize`（SceneGraph）→ `paint`（lievisual::Scene）→ `fit_scene` 适配画布。
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
    let mut scene = paint::run(&sg);
    // 画布贴合内容（官方 mermaid 语义：`config` 是上限而非固定画布），具体尺寸由
    // [`lievisual::fit::fit_scene`] 据内容包围盒算出。
    scene.background = config.background;
    fit_scene(&mut scene, fit_options(config));
    Ok(scene)
}

/// 把 [`OutputConfig`] 翻译成 [`FitOptions`]。
///
/// `config.width / height` 是**上限**（`None` 表示不限该维度），`config.upscale` 决定是否允许
/// 放大到目标尺寸（PNG 位图需要足够像素，SVG 矢量通常不需要）。
fn fit_options(config: &OutputConfig) -> FitOptions {
    let mut opts = FitOptions::new()
        .with_margin(CANVAS_MARGIN)
        .with_upscale(config.upscale);
    if let Some(w) = config.width {
        opts = opts.with_max_width(w);
    }
    if let Some(h) = config.height {
        opts = opts.with_max_height(h);
    }
    // 空内容（或坐标全为 NaN/Inf）时沿用配置尺寸兜底，与迁移前行为一致。
    opts = opts.with_empty_size(Size::new(
        resolve_cfg(config.width, types::DEFAULT_WIDTH),
        resolve_cfg(config.height, types::DEFAULT_HEIGHT),
    ));
    opts
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

// 「配置尺寸减 2×边距」的可用区域计算已移入 lievisual::fit（`FitOptions::with_max_width`
// 内部处理），这里不再需要。

#[cfg(test)]
mod tests {
    use super::*;
    use lievisual::geometry::Rect;
    use lievisual::scene::{Element, FillStrokeStyle, SceneNode};

    /// 构造一个「内容恰好是 100×100 矩形」的场景，用于验证画布适配。
    fn scene_100x100() -> lievisual::Scene {
        let mut s = lievisual::Scene::new(0.0, 0.0);
        s.push(Element::rect(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            FillStrokeStyle::default(),
        ));
        s
    }

    /// 端到端验证：走 `fit_options` + `fit_scene`，即 `build_diagram_with_config` 的实际路径。
    fn fit(config: &OutputConfig) -> (f64, f64) {
        let mut scene = scene_100x100();
        fit_scene(&mut scene, fit_options(config));
        (scene.width, scene.height)
    }

    #[test]
    fn fit_upscale_scales_up() {
        // 内容 100×100，目标 800×600：upscale 时应放大（受 600 高限制）。
        let config = OutputConfig {
            upscale: true,
            ..OutputConfig::default()
        };
        let (w, h) = fit(&config);
        assert!(
            w > 500.0 && h > 500.0,
            "upscale 应放大内容到目标尺寸，got {w}x{h}"
        );
    }

    #[test]
    fn fit_no_upscale_keeps_natural_size() {
        // 内容 100×100，目标 800×600：不 upscale 时保持自然尺寸（仅加边距）。
        let (w, h) = fit(&OutputConfig::default());
        assert!(
            w < 200.0 && h < 200.0,
            "不 upscale 应保持自然尺寸，got {w}x{h}"
        );
    }

    #[test]
    fn fit_width_only_constraint() {
        // 内容 100×100，只限宽 500（height=None），upscale 时按宽度放大、高度同比例。
        let config = OutputConfig {
            width: Some(500.0),
            height: None,
            upscale: true,
            ..OutputConfig::default()
        };
        let (w, h) = fit(&config);
        assert!((w - 500.0).abs() < 1.0, "只限宽时应放大到目标宽度，got {w}");
        assert!((h - 500.0).abs() < 1.0, "高度应按比例同步放大，got {h}");
    }

    #[test]
    fn fit_height_only_constraint() {
        // 内容 100×100，只限高 500（width=None），upscale 时按高度放大、宽度同比例。
        let config = OutputConfig {
            width: None,
            height: Some(500.0),
            upscale: true,
            ..OutputConfig::default()
        };
        let (w, h) = fit(&config);
        assert!((h - 500.0).abs() < 1.0, "只限高时应放大到目标高度，got {h}");
        assert!((w - 500.0).abs() < 1.0, "宽度应按比例同步放大，got {w}");
    }

    /// 迁移收益：包围盒现在计入**描边宽度**（此前 `compute_bbox` 完全忽略），
    /// 粗描边不再被画布裁掉半个线宽。
    #[test]
    fn fit_accounts_for_stroke_width() {
        use lievisual::geometry::Color;
        use lievisual::scene::Stroke;

        let mut s = lievisual::Scene::new(0.0, 0.0);
        s.push(Element::rect(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            FillStrokeStyle {
                fill: None,
                stroke: Some(Stroke::new(Color::BLACK, 20.0)),
            },
        ));
        let nodes = std::mem::take(&mut s.nodes);
        // 描边宽度由 fit 计入：内容应为 120×120（几何 100 + 两侧各 10）。
        s.nodes = nodes;
        fit_scene(&mut s, FitOptions::new().with_margin(0.0));
        assert!(
            (s.width - 120.0).abs() < 0.5,
            "应计入描边半宽，期望 120，got {}",
            s.width
        );
    }

    /// 迁移收益：包围盒现在应用**节点 transform**（此前 `compute_bbox` 算的是未变换坐标）。
    #[test]
    fn fit_accounts_for_node_transform() {
        use lievisual::geometry::Transform;

        let mut s = lievisual::Scene::new(0.0, 0.0);
        s.push(
            SceneNode::new(Element::rect(
                Rect::new(0.0, 0.0, 100.0, 100.0),
                FillStrokeStyle::default(),
            ))
            .with_transform(Transform::scale(2.0)),
        );
        fit_scene(&mut s, FitOptions::new().with_margin(0.0));
        assert!(
            (s.width - 200.0).abs() < 0.5,
            "应应用节点 transform，期望 200，got {}",
            s.width
        );
    }
}
