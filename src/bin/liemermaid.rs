use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use liemermaid::render;
use liemermaid::render_png;

#[derive(ValueEnum, Clone, Debug)]
enum Format {
    Png,
    Svg,
}

/// 从文件扩展名推断输出格式
fn infer_format_from_ext(path: &Path) -> Option<Format> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("png") => Some(Format::Png),
        Some("svg") => Some(Format::Svg),
        _ => None,
    }
}

/// 将 Mermaid 语法文件 (.mmd) 渲染为 PNG 或 SVG 图像
#[derive(Parser, Debug)]
#[command(name = "liemermaid")]
#[command(about = "Render Mermaid (.mmd) diagram to PNG/SVG images")]
struct Args {
    /// 输入 Mermaid 文件 (.mmd)
    #[arg(short, long, value_name = "FILE")]
    input: PathBuf,

    /// 输出图像文件路径（格式由扩展名推断：.png, .svg）
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,

    /// 输出格式（覆盖扩展名推断）
    #[arg(short, long, value_enum)]
    format: Option<Format>,

    /// 画布宽度（像素）
    #[arg(short = 'W', long, default_value_t = 800)]
    width: u32,

    /// 画布高度（像素）
    #[arg(short = 'H', long, default_value_t = 600)]
    height: u32,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 读取 mmd 文本
    let mmd = std::fs::read_to_string(&args.input)
        .map_err(|e| anyhow::anyhow!("读取输入文件失败 {}: {}", args.input.display(), e))?;

    // 推断输出格式
    let format = args
        .format
        .clone()
        .or_else(|| infer_format_from_ext(&args.output))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "无法从扩展名 '{}' 推断输出格式。请使用 -f/--format 或支持的后缀 (.png, .svg)。",
                args.output
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("(无)")
            )
        })?;

    match format {
        Format::Svg => {
            let svg = render(&mmd, args.width, args.height)
                .map_err(|e| anyhow::anyhow!("渲染 SVG 失败: {}", e))?;
            std::fs::write(&args.output, &svg)
                .map_err(|e| anyhow::anyhow!("写入文件失败 {}: {}", args.output.display(), e))?;
            println!("SVG 已保存至: {}", args.output.display());
        }
        Format::Png => {
            let png = render_png(&mmd, args.width, args.height)
                .map_err(|e| anyhow::anyhow!("渲染 PNG 失败: {}", e))?;
            std::fs::write(&args.output, &png)
                .map_err(|e| anyhow::anyhow!("写入文件失败 {}: {}", args.output.display(), e))?;
            println!("PNG 已保存至: {}", args.output.display());
        }
    }

    Ok(())
}
