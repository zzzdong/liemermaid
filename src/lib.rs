//! Mermaid 图表的 Rust 解析与渲染库。
//!
//! 支持 8 种图表：flowchart / sequence / class / state / er / pie / gitgraph / timeline。
//!
//! 输出统一走 [lievisual](https://crates.io/crates/lievisual) 的声明式场景 IR
//! （`Scene`）与多后端（SVG / vello_cpu PNG），本 crate 不维护自有渲染后端。
//!
//! # 管线
//!
//! ```text
//! Mermaid 文本 → MermaidParser → ast::Diagram
//!   → builder::extract  (Unigraph, UG)
//!   → builder::measure  (UG'，尺寸回填)
//!   → builder::layout   (Geograph, GG)
//!   → builder::materialize (SceneGraph)
//!   → builder::paint    (lievisual::Scene)
//!   → 画布贴合内容 + lievisual 渲染
//! ```
//!
//! # 画布语义
//!
//! 与官方 mermaid 一致：`width` / `height` 是**上限**而非固定画布。输出 SVG 只写贴合
//! 内容包围盒的 `viewBox`（根节点不写死 `width` / `height`，视口交给 CSS，用
//! `max-width` 封顶）；内容超出上限时等比缩小，内容装得下时**不放大**
//! （画布贴合内容，只留少量边距）。
//!
//! # 示例
//!
//! ```
//! use liemermaid::render;
//!
//! let svg = render("flowchart TD\nA[Start] --> B[End]", 800, 600).unwrap();
//! assert!(svg.starts_with("<svg"));
//! assert!(svg.contains("viewBox="), "画布几何由 viewBox 承载");
//! ```

pub mod ast;
pub mod builder;
pub mod error;
pub mod parser;
pub mod scene_ext;
pub mod vir;
pub use ast::Diagram;
/// 默认解析器入口（基于 winnow 手写组合式解析器，覆盖全部 8 种图表）。
pub use parser::WinnowParser as MermaidParser;

use builder::build_diagram_with_config;
pub use builder::types::OutputConfig;

// 字体注册 API 转发自 lievisual，供 WASM 演示站在浏览器中注册自定义字体（如 CJK / 等宽字体），
// 与 liecharts 的做法一致：宿主用 `liemermaid::register_font` 即可，无需直接依赖 lievisual。
pub use lievisual::{FontSource, parse_generic_family, register_font, register_font_generic};

/// 渲染 Mermaid 图表为 SVG 字符串。
///
/// builder 直接产出 [`lievisual::Scene`]，本函数交由 lievisual 的矢量后端
/// （`SvgRenderer`）输出。这是唯一的渲染路径。
///
/// # 参数
/// - `mermaid_text`: Mermaid 语法文本
/// - `width`: 画布宽度**上限**（内容超出时等比缩小，装得下时不放大）
/// - `height`: 画布高度**上限**
///
/// # 示例
/// ```
/// use liemermaid::render;
///
/// let svg = render(r#"flowchart TD
///     A[Start]
///     B[End]
///     A --> B
/// "#, 800, 600).expect("render failed");
/// assert!(svg.starts_with("<svg"));
/// ```
pub fn render(mermaid_text: &str, width: u32, height: u32) -> error::DiagramResult<String> {
    let diagram = MermaidParser::parse_mermaid(mermaid_text)?;

    // 使用用户指定的尺寸创建配置
    let config = OutputConfig {
        width: Some(width as f64),
        height: Some(height as f64),
        ..OutputConfig::default()
    };

    let scene = build_diagram_with_config(&diagram, &config)?;
    Ok(scene_ext::render_scene_svg(&scene))
}

/// 渲染 Mermaid 图为 PNG 位图字节（供 liepress 等宿主嵌入 PDF/PNG/SVG/DOCX）。
///
/// 形态与 liecharts 的 `render_png` 对齐：`render_png(text, w, h) -> Result<Vec<u8>>`。
/// 底层同样转换为 [`lievisual::Scene`]，交由 lievisual 的 vello_cpu 后端（`VelloPixmapRenderer`）栅格化并编码 PNG。
///
/// 与 [`render`]（SVG）不同，PNG 是位图：这里把 `width` / `height` 作为**目标尺寸**
/// （内容放大到目标，提升分辨率），避免简单图自然尺寸偏小、被宿主放大到页宽后发虚。
pub fn render_png(mermaid_text: &str, width: u32, height: u32) -> error::DiagramResult<Vec<u8>> {
    let diagram = MermaidParser::parse_mermaid(mermaid_text)?;

    let config = OutputConfig {
        width: Some(width as f64),
        height: Some(height as f64),
        upscale: true,
        ..OutputConfig::default()
    };

    let scene = build_diagram_with_config(&diagram, &config)?;
    Ok(scene_ext::render_scene_png(&scene))
}

/// 渲染 Mermaid 图表为 SVG，使用自定义 [`OutputConfig`]（可指定画布尺寸与背景色）。
///
/// 背景如实按 [`OutputConfig::background`] 绘制（与 [`render_png_with_config`] 一致）；
/// 想要官方 mermaid 那样的透明底，把它设为 [`lievisual::geometry::Color::TRANSPARENT`]。
///
/// 与 [`build_diagram_with_config`] 呼应，避免调用方重复 parse+build 样板。
pub fn render_with_config(
    mermaid_text: &str,
    config: &OutputConfig,
) -> error::DiagramResult<String> {
    let diagram = MermaidParser::parse_mermaid(mermaid_text)?;
    let scene = build_diagram_with_config(&diagram, config)?;
    Ok(scene_ext::render_scene_svg(&scene))
}

/// 渲染 Mermaid 图为 PNG 字节，使用自定义 [`OutputConfig`]（可指定画布尺寸与背景色）。
pub fn render_png_with_config(
    mermaid_text: &str,
    config: &OutputConfig,
) -> error::DiagramResult<Vec<u8>> {
    let diagram = MermaidParser::parse_mermaid(mermaid_text)?;
    let scene = build_diagram_with_config(&diagram, config)?;
    Ok(scene_ext::render_scene_png(&scene))
}

// ——— 手绘风格（sketch，可选开启）———

/// 手绘选项与填充样式（转发自 lievisual 的 [`lievisual::sketch`] 模块）。
///
/// 手绘是**额外开启**的：[`render`] / [`render_png`] 的行为与开关前完全一致，
/// 只有显式调用 `render_sketch*` / `render_png_sketch*` 才会启用手绘。
pub use lievisual::{SketchFillStyle, SketchKind, SketchKinds, SketchOptions};

/// 共享入口：build（内部已 fit）→ roughen → 二次 fit。
///
/// `builder` 内部的 fit 用 [`builder::CANVAS_MARGIN`]，手绘抖动会略微顶出内容边缘，
/// 因此用**同一个边距**再贴合一次：已贴合的内容只被平移抖动溢出的几像素，不缩放。
///
/// 另外套用图表管线的默认小填充保护（[`SKETCH_MIN_FILL_AREA`]）：mermaid 的箭头三角形
/// （~60px²）、边标签白底（~500px²）这类小面积实底填充一旦排线会毁掉可读性，保持实底、
/// 只手绘化其轮廓。用户通过 [`SketchOptions::min_fill_area`] 显式指定时以用户为准
/// （`Some(0.0)` 关闭保护）。
fn build_sketch_scene(
    mermaid_text: &str,
    config: &OutputConfig,
    sketch: &SketchOptions,
) -> error::DiagramResult<lievisual::Scene> {
    let diagram = MermaidParser::parse_mermaid(mermaid_text)?;
    let mut scene = build_diagram_with_config(&diagram, config)?;
    let mut sketch = sketch.clone();
    if sketch.min_fill_area.is_none() {
        sketch.min_fill_area = Some(SKETCH_MIN_FILL_AREA);
    }
    lievisual::roughen_scene_fit(&mut scene, &sketch, builder::CANVAS_MARGIN);
    Ok(scene)
}

/// 见 [`build_sketch_scene`]：约 32×32 以下的填充不排线。
const SKETCH_MIN_FILL_AREA: f64 = 1000.0;

/// 渲染 Mermaid 图表为**手绘风格** SVG（rough.js 质感：抖动描边 + 排线填充）。
///
/// 在 [`render`] 的基础上多做一步：对 builder 产出的 [`lievisual::Scene`] 应用手绘 pass。
/// 默认选项 = rough.js 风格（roughness 0.7、hachure -41°），需要定制时用
/// [`render_sketch_with_config`] 传入 [`SketchOptions`]（可用 [`SketchKinds`] 选择只手绘部分图元）。
///
/// # 示例
///
/// ```
/// use liemermaid::render_sketch;
///
/// let svg = render_sketch(r#"flowchart TD
///     A[Start]
///     B[End]
///     A --> B
/// "#, 800, 600).expect("render failed");
/// assert!(svg.contains("<pattern"), "手绘填充应产生 <pattern>");
/// ```
pub fn render_sketch(mermaid_text: &str, width: u32, height: u32) -> error::DiagramResult<String> {
    render_sketch_with_config(
        mermaid_text,
        &OutputConfig {
            width: Some(width as f64),
            height: Some(height as f64),
            ..OutputConfig::default()
        },
        &SketchOptions::new().with_fill(SketchFillStyle::Hachure),
    )
}

/// 渲染手绘 SVG，使用自定义 [`OutputConfig`] 与 [`SketchOptions`]。
///
/// [`SketchOptions`] 的常用定制：
/// - `.with_fill(SketchFillStyle::…)` —— 启用/选择填充样式（`None` = 只抖描边，填充原样）
/// - `.with_kinds(...)` —— 按图元类型选择（如排除 `Line`/`Path` 让连线保持笔直）
/// - `.with_seed(n)` —— 固定种子，同图同输出（测试必须显式给）
pub fn render_sketch_with_config(
    mermaid_text: &str,
    config: &OutputConfig,
    sketch: &SketchOptions,
) -> error::DiagramResult<String> {
    let scene = build_sketch_scene(mermaid_text, config, sketch)?;
    Ok(scene_ext::render_scene_svg(&scene))
}

/// 渲染 Mermaid 图表为**手绘风格** PNG 字节（形态对齐 [`render_png`]：内容放大到目标尺寸）。
pub fn render_png_sketch(
    mermaid_text: &str,
    width: u32,
    height: u32,
) -> error::DiagramResult<Vec<u8>> {
    render_png_sketch_with_config(
        mermaid_text,
        &OutputConfig {
            width: Some(width as f64),
            height: Some(height as f64),
            upscale: true,
            ..OutputConfig::default()
        },
        &SketchOptions::new().with_fill(SketchFillStyle::Hachure),
    )
}

/// 渲染手绘 PNG，使用自定义 [`OutputConfig`] 与 [`SketchOptions`]。
pub fn render_png_sketch_with_config(
    mermaid_text: &str,
    config: &OutputConfig,
    sketch: &SketchOptions,
) -> error::DiagramResult<Vec<u8>> {
    let scene = build_sketch_scene(mermaid_text, config, sketch)?;
    Ok(scene_ext::render_scene_png(&scene))
}
