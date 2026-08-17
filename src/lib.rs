pub mod ast;
pub mod builder;
pub mod error;
pub mod option;
pub mod parser;
pub mod scene_ext;
pub mod vir;
pub use ast::Diagram;
pub use parser::MermaidParser;

use builder::{build_diagram_with_config, types::OutputConfig};

/// 渲染 Mermaid 图表为 SVG 字符串。
///
/// builder 直接产出 [`lievisual::Scene`]，本函数交由 lievisual 的矢量后端
/// （`SvgRenderer`）输出。这是唯一的渲染路径。
///
/// # 参数
/// - `mermaid_text`: Mermaid 语法文本
/// - `width`: SVG 宽度
/// - `height`: SVG 高度
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
    let diagram = MermaidParser::parse_mermaid(mermaid_text)
        .map_err(|e| error::DiagramError::LayoutError(format!("parse error: {e}")))?;

    // 使用用户指定的尺寸创建配置
    let config = OutputConfig {
        width: width as f64,
        height: height as f64,
        ..OutputConfig::default()
    };

    let scene = build_diagram_with_config(&diagram, &config)?;
    Ok(scene_ext::render_scene_svg(&scene))
}

/// 渲染 Mermaid 图为 PNG 位图字节（供 liepress 等宿主嵌入 PDF/PNG/SVG/DOCX）。
///
/// 形态与 liecharts 的 `render_png` 对齐：`render_png(text, w, h) -> Result<Vec<u8>>`。
/// 底层同样转换为 [`lievisual::Scene`]，交由 lievisual 的 vello_cpu 后端（`VelloPixmapRenderer`）栅格化并编码 PNG。
pub fn render_png(mermaid_text: &str, width: u32, height: u32) -> error::DiagramResult<Vec<u8>> {
    let diagram = MermaidParser::parse_mermaid(mermaid_text)
        .map_err(|e| error::DiagramError::LayoutError(format!("parse error: {e}")))?;

    let config = OutputConfig {
        width: width as f64,
        height: height as f64,
        ..OutputConfig::default()
    };

    let scene = build_diagram_with_config(&diagram, &config)?;
    Ok(scene_ext::render_scene_png(&scene))
}
