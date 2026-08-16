//! 验证 `liemermaid::render_png` 能产出合法、非空的 PNG 位图字节，
//! 供 liepress 等宿主按 liecharts 的 `render_png` 形态集成。

use liemermaid::render_png;

const FLOWCHART: &str = r#"flowchart TD
A["Start"]
B["Process Data"]
C["End"]
A --> B
B --> C
"#;

#[test]
fn render_png_produces_valid_png_bytes() {
    let bytes = render_png(FLOWCHART, 800, 600).expect("render_png should succeed");

    // 非空输出
    assert!(!bytes.is_empty(), "PNG 字节不应为空");

    // PNG 文件签名：89 50 4E 47 0D 0A 1A 0A
    assert_eq!(
        &bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "输出应为合法 PNG（以 PNG 签名开头）"
    );

    // PNG 数据量应合理（远大于一个 IHDR 头，说明有实际像素）
    assert!(bytes.len() > 1024, "PNG 字节数过小，可能未真正绘制内容");
}
