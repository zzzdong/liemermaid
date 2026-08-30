use lievisual::geometry::{Color, Point};

/// 风格的默认配置
pub const DEFAULT_WIDTH: f64 = 800.0;
pub const DEFAULT_HEIGHT: f64 = 600.0;

/// 全局输出配置
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// 画布宽度上限/目标（pt）。`None` 表示不限宽度。
    pub width: Option<f64>,
    /// 画布高度上限/目标（pt）。`None` 表示不限高度。
    pub height: Option<f64>,
    /// 背景色，写入生成的 [`lievisual::Scene`]（`scene.background`），由渲染后端绘制
    pub background: Color,
    /// 是否把内容**放大**到 `width`/`height`（而非仅作上限）。
    ///
    /// 矢量 SVG 保持官方 mermaid 语义（`scale ≤ 1`，绝不放大）；但 PNG 位图若内容
    /// 自然尺寸偏小，被宿主放大到页宽后会发虚，故位图路径需放大到目标宽度提分辨率。
    pub upscale: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            width: Some(DEFAULT_WIDTH),
            height: Some(DEFAULT_HEIGHT),
            background: Color::new(255, 255, 255, 255),
            upscale: false,
        }
    }
}

/// 居中辅助：计算矩形区域内居中绘制文本时的偏移量
pub fn center_in_rect(
    text_w: f64,
    text_h: f64,
    rect_x: f64,
    rect_y: f64,
    rect_w: f64,
    rect_h: f64,
) -> Point {
    Point::new(
        rect_x + (rect_w - text_w) / 2.0,
        rect_y + (rect_h - text_h) / 2.0,
    )
}
