//! # liemermaid × lievisual 集成
//!
//! builder 直接产出 [`lievisual::Scene`]（节点即 lievisual 的 `SceneNode` /
//! `Element`），本模块是 **唯一的渲染路径**：委托 lievisual 的统一后端
//! （SVG / vello_cpu PNG）输出。liemermaid 自身不维护任何渲染后端。
//!
//! 字段对齐说明（builder 侧已直接采用 lievisual IR）：
//!
//! | liemermaid builder | lievisual |
//! |---|---|
//! | 颜色 | `geometry::Color` (f64) |
//! | 节点 | `scene::Element` / `SceneNode` |
//! | `z` 层级 | `SceneNode.z_index` |
//! | 文本样式 | `lievisual::text::TextStyle` |

use lievisual::render::{Renderer, SvgRenderer, SvgSizing};

/// 用 lievisual 的 SVG 后端渲染场景为字符串。
///
/// 背景与 PNG 路径**一致**：取 [`lievisual::Scene::background`]（由
/// [`crate::OutputConfig::background`] 写入，默认不透明白）。
///
/// 想要官方 mermaid 那样的透明底，把背景设为
/// [`lievisual::geometry::Color::TRANSPARENT`] 即可 —— 与 PNG 一样什么都不铺（SVG 里表现为一个 `fill-opacity="0"` 的全画布矩形，无可见像素）。
pub fn render_scene_svg(scene: &lievisual::Scene) -> String {
    let bg_css = if scene.background.a == 0 {
        "transparent".to_string()
    } else {
        scene.background.to_hex()
    };
    // 响应式视口：[`SvgSizing::Intrinsic`] 省略根节点的 `width` / `height`，只保留
    // `viewBox`（SVG 2 中缺省值 `auto` ≈ `100%`，按 viewBox 的宽高比定高），再用
    // `max-width` 封顶，使容器比图更宽时不被放大。
    let mut renderer = SvgRenderer::new(scene.width, scene.height)
        .with_background(scene.background)
        .with_sizing(SvgSizing::Intrinsic)
        .with_style(format!(
            "max-width: {:.3}px; background-color: {bg_css};",
            scene.width
        ));
    renderer.render_scene(scene);
    renderer.into_string()
}

/// 用 lievisual 的 vello_cpu 后端渲染场景为 PNG 字节。
pub fn render_scene_png(scene: &lievisual::Scene) -> Vec<u8> {
    use lievisual::render::VelloPixmapRenderer;
    let w = (scene.width.round() as i64).max(1) as u32;
    let h = (scene.height.round() as i64).max(1) as u32;
    let mut renderer = VelloPixmapRenderer::new(w, h).with_background(scene.background);
    renderer.render_png(scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lievisual::geometry::Color;

    #[test]
    fn svg_is_nonempty() {
        let scene = lievisual::Scene::new(100.0, 100.0);
        assert!(!render_scene_svg(&scene).is_empty());
    }

    /// 背景照 `scene.background` 绘制；全透明时矩形为零不透明度（与 PNG 不铺底色等价）。
    #[test]
    fn svg_background_follows_scene_background() {
        let scene = lievisual::Scene::new(100.0, 100.0).with_background(Color::WHITE);
        let svg = render_scene_svg(&scene);
        assert!(svg.contains("background-color: #ffffff;"), "{svg}");
        assert!(
            svg.contains(r##"<rect x="0" y="0" width="100.00" height="100.00" fill="#ffffff""##),
            "不透明背景应铺满画布: {svg}"
        );

        let scene = lievisual::Scene::new(100.0, 100.0).with_background(Color::TRANSPARENT);
        let svg = render_scene_svg(&scene);
        assert!(svg.contains("background-color: transparent;"), "{svg}");
        assert!(
            svg.contains(r##"fill-opacity="0.000""##),
            "透明背景不应产生可见像素: {svg}"
        );
    }

    /// 端到端：`OutputConfig::background` 为透明时，SVG 与 PNG 一样不铺底色。
    #[test]
    fn transparent_output_config_renders_no_visible_background() {
        let config = crate::OutputConfig {
            background: Color::TRANSPARENT,
            ..Default::default()
        };
        let svg = crate::render_with_config("flowchart TD\nA --> B", &config).expect("render");
        // 响应式视口：根节点不写死 width / height，几何只在 viewBox 里。
        assert!(!svg.contains("<svg width=\""), "{svg}");
        assert!(svg.contains("viewBox="), "{svg}");
        assert!(svg.contains("background-color: transparent;"), "{svg}");
        assert!(
            svg.contains(r##"fill-opacity="0.000""##),
            "透明底不应有可见像素: {svg}"
        );
    }
}
