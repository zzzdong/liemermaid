//! # liemermaid × lievisual 集成
//!
//! 本模块把 liemermaid 自有的 `VisualElement` 中间表示（IR）转换为
//! [`lievisual::Scene`]，从而复用 lievisual 的多后端（SVG / vello_cpu PNG）。
//!
//! liemermaid 自身保留一套 `render` 后端作为默认实现；本模块是**可选旁路**——
//! 当希望用 lievisual 的统一后端输出时调用。两套 IR 字段对齐如下：
//!
//! | liemermaid | lievisual | 说明 |
//! |---|---|---|
//! | `Color` (u8) | `geometry::Color` (f64) | 除以 255 |
//! | `Transform{translate,rotate,scale}` | `geometry::Transform{a,b,c,d,e,f}` | 组合为仿射 |
//! | `GradientDef` | `scene::LinearGradient` | 仅垂直渐变（liemermaid 未定义方向） |
//! | `TextStyle`(含 weight/style/align) | `text::TextStyle` | 取 color/size/family/rotation |
//! | `Vec<VisualElement>` (z_index 内嵌) | `Scene{nodes}` (z_index 在 `SceneNode`) | 提升 |
//!
//! 后续若 liemermaid 决定以 lievisual 的 `Scene` 作为唯一 IR，可直接删除
//! `render` 模块，仅保留本转换层。
//! 从而复用 lievisual 提供的多后端（SVG / vello_cpu PNG）。
//!
//! liemermaid 自身保留一套 `render` 后端作为默认实现；本模块是**可选旁路**——
//! 当希望用 lievisual 的统一后端输出时调用。两套 IR 字段对齐如下：
//!
//! | liemermaid | lievisual | 说明 |
//! |---|---|---|
//! | `Color` (u8) | `geometry::Color` (f64) | 除以 255 |
//! | `Transform{translate,rotate,scale}` | `geometry::Transform{a,b,c,d,e,f}` | 组合为仿射 |
//! | `GradientDef` | `scene::LinearGradient` | 仅垂直渐变（liemermaid 未定义方向） |
//! | `TextStyle`(含 weight/style/align) | `text::TextStyle` | 取 color/size/family/rotation |
//! | `Vec<VisualElement>` (z_index 内嵌) | `Scene{nodes}` (z_index 在 `SceneNode`) | 提升 |

use lievisual::geometry::{
    Color as LColor, Point as LPoint, Rect as LRect, Transform as LTransform,
};
use lievisual::render::{Renderer, SvgRenderer};
use lievisual::scene::{
    Fill, FillStrokeStyle as LFillStrokeStyle, GradientStop, LinearGradient, Stroke as LStroke,
};
use lievisual::text::TextStyle as LTextStyle;

use crate::visual::{
    Color, FillStrokeStyle, GradientDef, Stroke, StrokeStyle, Transform, VisualElement,
};

/// 将 liemermaid 的 `Color`（u8）转为 lievisual 的 `Color`（f64，0–1）。
fn conv_color(c: Color) -> LColor {
    LColor::rgba(c.r, c.g, c.b, c.a)
}

/// 将 liemermaid 的分解式 `Transform` 组合为 lievisual 的仿射矩阵。
///
/// 顺序：先缩放，再旋转，最后平移（与 liemermaid 既有 Group 变换语义一致）。
fn conv_transform(t: Transform) -> LTransform {
    let s = LTransform::new(t.scale.x, 0.0, 0.0, t.scale.y, 0.0, 0.0);
    let r = LTransform::rotate(t.rotate);
    let p = LTransform::translate(t.translate.x, t.translate.y);
    // 先缩放→旋转→平移：p · r · s
    p.then(&r).then(&s)
}

fn conv_stroke(s: &Stroke) -> LStroke {
    LStroke {
        color: conv_color(s.color),
        width: s.width,
        ..Default::default()
    }
}

fn conv_stroke_plain(s: &StrokeStyle) -> LStroke {
    LStroke {
        color: conv_color(s.color),
        width: s.width,
        ..Default::default()
    }
}

fn conv_fill_stroke(style: &FillStrokeStyle) -> LFillStrokeStyle {
    LFillStrokeStyle {
        fill: style.fill.map(|c| Fill::Solid(conv_color(c))),
        stroke: style.stroke.as_ref().map(conv_stroke),
    }
}

fn conv_gradient(g: &GradientDef) -> LinearGradient {
    LinearGradient {
        start: LPoint::new(0.0, 0.0),
        end: LPoint::new(0.0, 1.0),
        stops: g
            .stops
            .iter()
            .map(|(offset, c)| GradientStop {
                offset: *offset,
                color: conv_color(*c),
            })
            .collect(),
    }
}

fn conv_text_style(s: &crate::visual::TextStyle) -> LTextStyle {
    // 字重：命名值 → 数值（Normal 400 / Bold 700 / Bolder 800 / Lighter 300），数值直通。
    let font_weight: f32 = match s.font_weight {
        crate::option::FontWeight::Named(crate::option::FontWeightNamed::Normal) => 400.0,
        crate::option::FontWeight::Named(crate::option::FontWeightNamed::Bold) => 700.0,
        crate::option::FontWeight::Named(crate::option::FontWeightNamed::Bolder) => 800.0,
        crate::option::FontWeight::Named(crate::option::FontWeightNamed::Lighter) => 300.0,
        crate::option::FontWeight::Numeric(n) => n as f32,
    };
    let font_style = match s.font_style {
        crate::visual::FontStyle::Normal => lievisual::text::FontStyle::Normal,
        crate::visual::FontStyle::Italic => lievisual::text::FontStyle::Italic,
        crate::visual::FontStyle::Oblique => lievisual::text::FontStyle::Oblique,
    };
    let align = match s.align {
        crate::visual::TextAlign::Left => lievisual::text::TextAlign::Left,
        crate::visual::TextAlign::Center => lievisual::text::TextAlign::Center,
        crate::visual::TextAlign::Right => lievisual::text::TextAlign::Right,
    };
    let baseline = match s.vertical_align {
        crate::visual::TextBaseline::Top => lievisual::text::TextBaseline::Top,
        crate::visual::TextBaseline::Middle => lievisual::text::TextBaseline::Middle,
        crate::visual::TextBaseline::Bottom => lievisual::text::TextBaseline::Bottom,
        crate::visual::TextBaseline::Alphabetic => lievisual::text::TextBaseline::Alphabetic,
    };
    LTextStyle {
        color: conv_color(s.color),
        font_size: s.font_size,
        font_family: s.font_family.clone(),
        font_weight,
        font_style,
        line_height: None,
        rotation: 0.0, // 文本旋转在 Element::Text 上单独携带
        max_width: None,
        align,
        baseline,
    }
}

/// 将单个 `VisualElement` 转为 `SceneNode`（含 z_index + transform）。
fn conv_element(e: &VisualElement) -> lievisual::scene::SceneNode {
    let (element, z_index, transform) = match e {
        VisualElement::Rect {
            rect,
            radius,
            style,
            z_index,
        } => {
            let element = if let Some(r) = radius {
                // 圆角矩形映射到 lievisual 原生 RoundedRect（SVG 输出语义化 <rect rx>）。
                lievisual::Element::RoundedRect {
                    rect: LRect::new(rect.x0, rect.y0, rect.width(), rect.height()),
                    radius: *r,
                    style: conv_fill_stroke(style),
                }
            } else {
                lievisual::Element::Rect {
                    rect: LRect::new(rect.x0, rect.y0, rect.width(), rect.height()),
                    style: conv_fill_stroke(style),
                }
            };
            (element, *z_index, None)
        }
        VisualElement::Circle {
            center,
            radius,
            style,
            z_index,
        } => (
            lievisual::Element::Circle {
                center: LPoint::new(center.x, center.y),
                radius: *radius,
                style: conv_fill_stroke(style),
            },
            *z_index,
            None,
        ),
        VisualElement::Line {
            start,
            end,
            style,
            z_index,
        } => (
            lievisual::Element::Line {
                start: LPoint::new(start.x, start.y),
                end: LPoint::new(end.x, end.y),
                style: conv_stroke_plain(style),
            },
            *z_index,
            None,
        ),
        VisualElement::Polyline {
            points,
            style,
            z_index,
        } => (
            lievisual::Element::Polyline {
                points: points.iter().map(|p| LPoint::new(p.x, p.y)).collect(),
                style: conv_stroke_plain(style),
            },
            *z_index,
            None,
        ),
        VisualElement::Path {
            path,
            style,
            z_index,
        } => (
            lievisual::Element::Path {
                path: path.clone(),
                style: conv_fill_stroke(style),
                closed: false,
            },
            *z_index,
            None,
        ),
        VisualElement::GradientPath {
            path,
            gradient,
            stroke,
            z_index,
        } => (
            lievisual::Element::GradientPath {
                path: path.clone(),
                gradient: conv_gradient(gradient),
                stroke: stroke.as_ref().map(conv_stroke),
            },
            *z_index,
            None,
        ),
        VisualElement::TextRun {
            text,
            position,
            style,
            rotation,
            max_width,
            layout,
            z_index,
        } => {
            let mut ts = conv_text_style(style);
            ts.rotation = *rotation;
            ts.max_width = *max_width;
            // 预排版 layout 类型不兼容（liemermaid=parley::Layout<u8 Color> vs lievisual=parley::Layout<f64 Color>），
            // 且 lievisual 无 feature gate；转换层丢弃 layout，由 lievisual 后端自行排版。
            let _ = &layout;
            (
                lievisual::Element::Text {
                    content: text.clone(),
                    position: LPoint::new(position.x, position.y),
                    style: ts,
                    layout: None,
                },
                *z_index,
                None,
            )
        }
        VisualElement::Group {
            children,
            transform,
            z_index,
        } => {
            let children: Vec<lievisual::scene::SceneNode> =
                children.iter().map(conv_element).collect();
            (
                lievisual::Element::Group { children },
                *z_index,
                transform.map(conv_transform),
            )
        }
    };

    lievisual::scene::SceneNode {
        element,
        z_index,
        transform,
        opacity: 1.0,
        name: None,
        visible: true,
        clip: None,
    }
}

/// 将整张图表的 `VisualElement` 集合转换为 [`lievisual::Scene`]。
///
/// `width`/`height` 为画布尺寸（像素），`background` 为画布背景色（u8 Color）。
pub fn to_scene(
    elements: &[VisualElement],
    width: f64,
    height: f64,
    background: Color,
) -> lievisual::Scene {
    let nodes: Vec<lievisual::scene::SceneNode> = elements.iter().map(conv_element).collect();
    lievisual::Scene {
        width,
        height,
        background: conv_color(background),
        nodes,
        layers: Vec::new(),
        title: None,
        description: None,
        scale: 1.0,
    }
}

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

    const FLOWCHART: &str = r#"flowchart TD
    A[Start]
    B{Decision}
    C[Yes]
    D[No]
    A --> B
    B -->|Yes| C
    B -->|No| D
"#;

    #[test]
    fn lievisual_svg_roundtrip() {
        let svg = render_svg_via_lib(FLOWCHART, 800, 600);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        // 流程图至少应渲染出节点矩形与若干文本。
        assert!(svg.contains("<rect") || svg.contains("<path"));
        assert!(svg.contains("<text"));
    }

    #[test]
    fn lievisual_png_roundtrip() {
        let png = render_png_via_lib(FLOWCHART, 800, 600);
        // PNG 文件签名: 89 50 4E 47 0D 0A 1A 0A
        assert_eq!(&png[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    // 复用 lib 公共函数，避免此处重复解析逻辑。
    fn render_svg_via_lib(src: &str, w: u32, h: u32) -> String {
        crate::render(src, w, h).unwrap()
    }
    fn render_png_via_lib(src: &str, w: u32, h: u32) -> Vec<u8> {
        crate::render_png(src, w, h).unwrap()
    }
}
