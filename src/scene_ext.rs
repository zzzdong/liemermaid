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

use lievisual::geometry::Color;
use lievisual::render::{Renderer, SvgRenderer};

/// 用 lievisual 的 SVG 后端渲染场景为字符串。
///
/// SVG 输出使用透明背景：mermaid 的默认 SVG 不绘制背景 `<rect>`，底色由 CSS 的
/// `background-color` 决定。透明背景让导出的 SVG 与官方结构对齐，嵌入时也可由容器/
/// 样式决定底色，避免多一层整画布矩形。PNG 路径仍保留白底（见 [`render_scene_png`]）。
pub fn render_scene_svg(scene: &lievisual::Scene) -> String {
    let mut renderer =
        SvgRenderer::new(scene.width, scene.height).with_background(Color::TRANSPARENT);
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

    #[test]
    fn svg_is_nonempty() {
        let scene = lievisual::Scene::new(100.0, 100.0);
        assert!(!render_scene_svg(&scene).is_empty());
    }
}
