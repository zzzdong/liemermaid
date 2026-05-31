use vello_cpu::kurbo::Point;

use crate::visual::Color;

/// 风格的默认配置
pub const DEFAULT_WIDTH: f64 = 800.0;
pub const DEFAULT_HEIGHT: f64 = 600.0;
pub const DEFAULT_FONT_SIZE: f64 = 14.0;
pub const DEFAULT_FONT_FAMILY: &str = "sans-serif";

/// 调色板（用于多种图表的系列色）
pub const PALETTE: &[Color] = &[
    Color::new(31, 119, 180),   // 蓝
    Color::new(255, 127, 14),   // 橙
    Color::new(44, 160, 44),    // 绿
    Color::new(148, 103, 189),  // 紫
    Color::new(140, 86, 75),    // 棕
    Color::new(227, 119, 194),  // 粉
    Color::new(127, 127, 127),  // 灰
    Color::new(188, 189, 34),   // 黄绿
    Color::new(23, 190, 207),   // 青
    Color::new(214, 39, 40),    // 红
];

/// 全局输出配置
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// 画布宽度（pt）
    pub width: f64,
    /// 画布高度（pt）
    pub height: f64,
    /// 背景色
    pub background: Color,
    /// 默认字体
    pub font_family: String,
    /// 默认字号
    pub font_size: f64,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            background: Color::new(255, 255, 255),
            font_family: DEFAULT_FONT_FAMILY.to_string(),
            font_size: DEFAULT_FONT_SIZE,
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