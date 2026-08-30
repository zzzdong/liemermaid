use liemermaid::{
    FontSource, parse_generic_family, register_font, register_font_generic, render, render_png,
};
use wasm_bindgen::prelude::*;

/// 渲染 Mermaid 源文本为 SVG 字符串（仅前端演示包装，错误转为字符串）。
///
/// `width` / `height` 作为画布**上限**：内容超出时等比缩小，装得下时不放大。
#[wasm_bindgen]
pub fn render_mermaid(text: &str, width: u32, height: u32) -> Result<String, String> {
    render(text, width, height).map_err(|e| e.to_string())
}

/// 渲染 Mermaid 源文本为 PNG 位图字节（前端用于 PNG 下载）。
#[wasm_bindgen]
pub fn render_mermaid_png(text: &str, width: u32, height: u32) -> Result<Vec<u8>, String> {
    render_png(text, width, height).map_err(|e| e.to_string())
}

/// 注册自定义字体（从 JS 传入字节数据），供图表文本按 `font_family` 名引用。
#[wasm_bindgen]
pub fn register_font_bytes(name: &str, bytes: &[u8]) -> Result<(), String> {
    register_font(FontSource::Memory(bytes.to_vec()), Some(name)).map_err(|e| e.to_string())
}

/// 注册字体到通用 `sans-serif` 家族，使显式 `font-family: sans-serif` 也能命中该字体
/// （parley 把 `sans-serif` 当作通用关键字，普通按名注册无法命中）。
#[wasm_bindgen]
pub fn register_font_sans_serif_bytes(name: &str, bytes: &[u8]) -> Result<(), String> {
    let generic = parse_generic_family("sans-serif")
        .ok_or_else(|| "unknown generic family: sans-serif".to_string())?;
    register_font_generic(
        FontSource::Memory(bytes.to_vec()),
        Some(name),
        Some(generic),
    )
    .map_err(|e| e.to_string())
}
