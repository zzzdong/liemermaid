pub mod ast;
pub mod diagram_builder;
pub mod error;
pub mod option;
pub mod parser;
pub mod render;
pub mod text;
pub mod visual;
pub use ast::Diagram;
pub use parser::MermaidParser;

use diagram_builder::{build_diagram_with_config, types::OutputConfig};
use render::SvgRenderer;

/// 渲染 Mermaid 图表为 SVG 字符串
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
///     A[Start] --> B[End]
/// "#, 800, 600).expect("render failed");
/// ```
pub fn render(mermaid_text: &str, width: u32, height: u32) -> error::DiagramResult<String> {
    let diagram = MermaidParser::parse_mermaid(mermaid_text)
        .map_err(|e| error::DiagramError::LayoutError(format!("parse error: {}", e)))?;
    
    // 使用用户指定的尺寸创建配置
    let config = OutputConfig {
        width: width as f64,
        height: height as f64,
        ..OutputConfig::default()
    };
    
    let elements = build_diagram_with_config(&diagram, &config)?;
    let renderer = SvgRenderer::new();
    renderer.render(&elements, width, height)
}