use lievisual::geometry::{Color, Point};

/// 风格的默认配置
pub const DEFAULT_WIDTH: f64 = 800.0;
pub const DEFAULT_HEIGHT: f64 = 600.0;

/// 全局输出配置
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// 画布宽度（pt）
    pub width: f64,
    /// 画布高度（pt）
    pub height: f64,
    /// 背景色，写入生成的 [`lievisual::Scene`]（`scene.background`），由渲染后端绘制
    pub background: Color,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            background: Color::new(255_f64 / 255.0, 255_f64 / 255.0, 255_f64 / 255.0, 1.0),
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
