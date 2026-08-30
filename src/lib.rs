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
//! 与官方 mermaid 一致：`width` / `height` 是**上限**而非固定画布。输出 SVG 的
//! 根节点为 `width="100%"` + 贴合内容包围盒的 `viewBox`；内容超出上限时等比缩小，
//! 内容装得下时**不放大**（画布贴合内容，只留少量边距）。
//!
//! # 示例
//!
//! ```
//! use liemermaid::render;
//!
//! let svg = render("flowchart TD\nA[Start] --> B[End]", 800, 600).unwrap();
//! assert!(svg.starts_with("<svg"));
//! assert!(svg.contains(r##"width="100%""##));
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
