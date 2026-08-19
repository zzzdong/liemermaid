use lievisual::geometry::{Color, Point};

/// 风格的默认配置
pub const DEFAULT_WIDTH: f64 = 800.0;
pub const DEFAULT_HEIGHT: f64 = 600.0;

/// 调色板（用于多种图表的系列色）
pub const PALETTE: &[Color] = &[
    Color::new(31_f64 / 255.0, 119_f64 / 255.0, 180_f64 / 255.0, 1.0), // 蓝
    Color::new(255_f64 / 255.0, 127_f64 / 255.0, 14_f64 / 255.0, 1.0), // 橙
    Color::new(44_f64 / 255.0, 160_f64 / 255.0, 44_f64 / 255.0, 1.0),  // 绿
    Color::new(148_f64 / 255.0, 103_f64 / 255.0, 189_f64 / 255.0, 1.0), // 紫
    Color::new(140_f64 / 255.0, 86_f64 / 255.0, 75_f64 / 255.0, 1.0),  // 棕
    Color::new(227_f64 / 255.0, 119_f64 / 255.0, 194_f64 / 255.0, 1.0), // 粉
    Color::new(127_f64 / 255.0, 127_f64 / 255.0, 127_f64 / 255.0, 1.0), // 灰
    Color::new(188_f64 / 255.0, 189_f64 / 255.0, 34_f64 / 255.0, 1.0), // 黄绿
    Color::new(23_f64 / 255.0, 190_f64 / 255.0, 207_f64 / 255.0, 1.0), // 青
    Color::new(214_f64 / 255.0, 39_f64 / 255.0, 40_f64 / 255.0, 1.0),  // 红
];

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
