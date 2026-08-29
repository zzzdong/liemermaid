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
    to_official_svg(renderer.into_string(), scene.width, scene.height)
}

/// 把 lievisual 的 SVG 输出改写为**官方 mermaid 的根节点形态**。
///
/// lievisual 的 `SvgRenderer` 固定输出 `<svg width="{w}" height="{h}" viewBox="0 0 w h">`
/// 且总是追加一个整画布背景 `<rect>`；官方 mermaid 则是
/// `<svg width="100%" style="max-width: {w}px; background-color: transparent;" viewBox="...">`
/// 且**不画背景矩形**（底色交给 CSS 的 `background-color`）。
///
/// 由于 `SvgRenderer` 未开放 viewBox/根属性定制，这里在渲染后做一次**纯字符串后处理**：
/// 只改根节点标签、去掉整画布的透明背景矩形，不动任何内容节点。
fn to_official_svg(svg: String, width: f64, height: f64) -> String {
    let mut out = svg;

    // 1) 根节点：把 `width="{w}"` 就地替换为 `width="100%"`，并在标签末尾追加
    //    `style="max-width: {w}px; background-color: transparent;"`。
    //    保持原有属性顺序（`<svg xmlns=...` 开头），viewBox 沿用 `0 0 w h`。
    if let Some(start) = out.find("<svg")
        && let Some(end) = out[start..].find('>').map(|i| start + i)
    {
        let tag = &out[start..end];
        let new_tag = tag
            .split_whitespace()
            .map(|a| {
                if a.starts_with("width=") {
                    r##"width="100%""##
                } else {
                    a
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let rest = out[end..].to_string();
        out = format!(
            "{new_tag} style=\"max-width: {width:.3}px; background-color: transparent;\"{rest}"
        );
    }

    // 2) 去掉 lievisual 追加的整画布背景矩形（官方无此元素）。
    //    形如 `<rect x="0" y="0" width="W" height="H" fill="#00000000"/>`，位于 `<svg ...>` 之后。
    out = strip_canvas_background_rect(out, width, height);
    out
}

/// 删除覆盖整个画布的透明背景矩形（若存在）。
fn strip_canvas_background_rect(svg: String, width: f64, height: f64) -> String {
    // 只匹配紧贴画布尺寸的全幅矩形，避免误删内容矩形。
    let needle = format!(
        r##"<rect x="0" y="0" width="{:.2}" height="{:.2}" fill="#00000000"/>"##,
        width, height
    );
    match svg.find(&needle) {
        Some(i) => {
            let mut out = String::with_capacity(svg.len());
            out.push_str(&svg[..i]);
            out.push_str(&svg[i + needle.len()..]);
            // 顺带去掉因删除而残留的空行
            out.replace("\r\n\r\n", "\r\n")
        }
        None => svg,
    }
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
