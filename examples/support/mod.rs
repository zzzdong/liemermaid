//! examples 公共输出辅助：统一写入 `examples/out/` 并打印产物路径。
#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

use liemermaid::{OutputConfig, render, render_png, render_with_config};
use lievisual::geometry::Color;

/// 产物目录（`*.svg` / `*.png` 已被 .gitignore 忽略）。
pub const OUT_DIR: &str = "examples/out";

/// 确保产物目录存在。
pub fn out_dir() -> PathBuf {
    let dir = PathBuf::from(OUT_DIR);
    fs::create_dir_all(&dir).expect("create examples/out");
    dir
}

/// 渲染 SVG 并写入 `examples/out/{name}.svg`，返回路径。
pub fn svg(name: &str, source: &str, width: u32, height: u32) -> PathBuf {
    let path = out_dir().join(format!("{name}.svg"));
    let content = render(source, width, height).expect("render svg");
    fs::write(&path, content).expect("write svg");
    println!("SVG: {}", path.display());
    path
}

/// 渲染 PNG 并写入 `examples/out/{name}.png`，返回路径。
pub fn png(name: &str, source: &str, width: u32, height: u32) -> PathBuf {
    let path = out_dir().join(format!("{name}.png"));
    let bytes = render_png(source, width, height).expect("render png");
    fs::write(&path, bytes).expect("write png");
    println!("PNG: {}", path.display());
    path
}

/// 同时输出 SVG + PNG。
pub fn both(name: &str, source: &str, width: u32, height: u32) {
    svg(name, source, width, height);
    png(name, source, width, height);
}

/// 渲染透明背景 SVG（官方 mermaid 语义）。
pub fn svg_transparent(name: &str, source: &str, width: u32, height: u32) -> PathBuf {
    let path = out_dir().join(format!("{name}.svg"));
    let config = OutputConfig {
        width: Some(width as f64),
        height: Some(height as f64),
        background: Color::TRANSPARENT,
        ..OutputConfig::default()
    };
    let content = render_with_config(source, &config).expect("render transparent svg");
    fs::write(&path, content).expect("write svg");
    println!("SVG (transparent): {}", path.display());
    path
}
