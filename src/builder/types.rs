use vello_cpu::kurbo::Point;

use lievisual::geometry::Color;

/// 风格的默认配置
pub const DEFAULT_WIDTH: f64 = 800.0;
pub const DEFAULT_HEIGHT: f64 = 600.0;
pub const DEFAULT_FONT_SIZE: f64 = 14.0;
pub const DEFAULT_FONT_FAMILY: &str = "sans-serif";

/// 调色板（用于多种图表的系列色）
pub const PALETTE: &[Color] = &[
    Color::new(31 as f64 / 255.0, 119 as f64 / 255.0, 180 as f64 / 255.0, 1.0),  // 蓝
    Color::new(255 as f64 / 255.0, 127 as f64 / 255.0, 14 as f64 / 255.0, 1.0),  // 橙
    Color::new(44 as f64 / 255.0, 160 as f64 / 255.0, 44 as f64 / 255.0, 1.0),   // 绿
    Color::new(148 as f64 / 255.0, 103 as f64 / 255.0, 189 as f64 / 255.0, 1.0), // 紫
    Color::new(140 as f64 / 255.0, 86 as f64 / 255.0, 75 as f64 / 255.0, 1.0),   // 棕
    Color::new(227 as f64 / 255.0, 119 as f64 / 255.0, 194 as f64 / 255.0, 1.0), // 粉
    Color::new(127 as f64 / 255.0, 127 as f64 / 255.0, 127 as f64 / 255.0, 1.0), // 灰
    Color::new(188 as f64 / 255.0, 189 as f64 / 255.0, 34 as f64 / 255.0, 1.0),  // 黄绿
    Color::new(23 as f64 / 255.0, 190 as f64 / 255.0, 207 as f64 / 255.0, 1.0),  // 青
    Color::new(214 as f64 / 255.0, 39 as f64 / 255.0, 40 as f64 / 255.0, 1.0),   // 红
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
            background: Color::new(255 as f64 / 255.0, 255 as f64 / 255.0, 255 as f64 / 255.0, 1.0),
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
