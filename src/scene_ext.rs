//! # liemermaid × lievisual 集成
//!
//! builder 已直接产出 [`lievisual::Scene`]（其节点即 lievisual 的 `SceneNode` /
//! `Element`），本模块仅作为**渲染旁路**：当希望用 lievisual 的统一后端
//! （SVG / vello_cpu PNG）输出时调用。
//!
//! 字段对齐说明（builder 侧已直接采用 lievisual IR）：
//!
//! | liemermaid builder | lievisual |
//! |---|---|
//! | 颜色 | `geometry::Color` (f64) |
//! | 节点 | `scene::Element` / `SceneNode` |
//! | `z` 层级 | `SceneNode.z_index` |
//! | 文本样式 | `lievisual::text::TextStyle` |
//!
//! liemermaid 自身保留一套 `render` 后端作为默认实现；本模块是可选旁路。

use lievisual::render::{Renderer, SvgRenderer};

/// 用 lievisual 的 SVG 后端渲染场景为字符串。
pub fn render_scene_svg(scene: &lievisual::Scene) -> String {
    let mut renderer =
        SvgRenderer::new(scene.width, scene.height).with_background(scene.background);
    renderer.render_scene(scene);
    renderer.into_string()
}

/// 用 lievisual 的 vello_cpu 后端渲染场景为 PNG 字节。
pub fn render_scene_png(scene: &lievisual::Scene) -> Vec<u8> {
    use lievisual::render::VelloPixmapRenderer;
    VelloPixmapRenderer::new(scene.width as u32, scene.height as u32)
        .with_background(scene.background)
        .render_png(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_is_nonempty() {
        let scene = lievisual::Scene::new(100.0, 100.0);
        assert!(!render_scene_svg(&scene).is_empty());
    }
}
