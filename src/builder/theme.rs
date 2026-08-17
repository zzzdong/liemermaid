//! 各图表的配色与排版常量。
//!
//! 从旧 `visual::theme` 迁移而来，现独立为 builder 层的主题模块，
//! 由各图表 builder 通过 `crate::vir::theme`（重导出）引用。

use lievisual::Color;

// ---- 基础 ----
pub const BACKGROUND: Color = Color::new(255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.0);
pub const FONT_FAMILY: &str = "Segoe UI, system-ui, -apple-system, sans-serif";
pub const FONT_SIZE: f64 = 13.0;
pub const NODE_RADIUS: f64 = 6.0;

// ---- 连线通用 ----
pub const EDGE_COLOR: Color = Color::new(148.0 / 255.0, 163.0 / 255.0, 184.0 / 255.0, 1.0); // slate-400
pub const EDGE_WIDTH: f64 = 2.0;
pub const TEXT_COLOR: Color = Color::new(30.0 / 255.0, 41.0 / 255.0, 59.0 / 255.0, 1.0); // slate-800

// ==================== Flowchart (蓝) ====================
pub mod flowchart {
    use super::Color;
    pub const FILL: Color = Color::new(238.0 / 255.0, 242.0 / 255.0, 255.0 / 255.0, 1.0); // indigo-50
    pub const STROKE: Color = Color::new(99.0 / 255.0, 102.0 / 255.0, 241.0 / 255.0, 1.0); // indigo-500
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const SUBGRAPH_STROKE: Color = Color::new(148.0 / 255.0, 163.0 / 255.0, 184.0 / 255.0, 1.0); // slate-400
    pub const SUBGRAPH_TITLE: Color = Color::new(71.0 / 255.0, 85.0 / 255.0, 105.0 / 255.0, 1.0); // slate-600
}

// ==================== State (绿) ====================
pub mod state {
    use super::Color;
    pub const FILL: Color = Color::new(240.0 / 255.0, 253.0 / 255.0, 244.0 / 255.0, 1.0); // green-50
    pub const STROKE: Color = Color::new(34.0 / 255.0, 197.0 / 255.0, 94.0 / 255.0, 1.0); // green-500
    pub const TEXT: Color = Color::new(22.0 / 255.0, 101.0 / 255.0, 52.0 / 255.0, 1.0); // green-800
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const START_FILL: Color = Color::new(22.0 / 255.0, 101.0 / 255.0, 52.0 / 255.0, 1.0); // green-800
    pub const END_STROKE: Color = Color::new(34.0 / 255.0, 197.0 / 255.0, 94.0 / 255.0, 1.0); // green-500
}

// ==================== Class (紫) ====================
pub mod class {
    use super::Color;
    pub const FILL: Color = Color::new(255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.0); // white
    pub const HEADER_FILL: Color = Color::new(250.0 / 255.0, 245.0 / 255.0, 255.0 / 255.0, 1.0); // purple-50
    pub const STROKE: Color = Color::new(168.0 / 255.0, 85.0 / 255.0, 247.0 / 255.0, 1.0); // purple-500
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const SEPARATOR: Color = Color::new(214.0 / 255.0, 188.0 / 255.0, 250.0 / 255.0, 1.0); // purple-200
    pub const DIAMOND_FILL: Color = Color::new(168.0 / 255.0, 85.0 / 255.0, 247.0 / 255.0, 1.0); // purple-500
}

// ==================== Sequence (天蓝) ====================
pub mod sequence {
    use super::Color;
    pub const ACTOR_FILL: Color = Color::new(240.0 / 255.0, 249.0 / 255.0, 255.0 / 255.0, 1.0); // sky-50
    pub const ACTOR_STROKE: Color = Color::new(14.0 / 255.0, 165.0 / 255.0, 233.0 / 255.0, 1.0); // sky-500
    pub const FILL: Color = Color::new(240.0 / 255.0, 249.0 / 255.0, 255.0 / 255.0, 1.0); // sky-50
    pub const STROKE: Color = Color::new(14.0 / 255.0, 165.0 / 255.0, 233.0 / 255.0, 1.0); // sky-500
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const LIFELINE: Color = Color::new(203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0); // slate-300
    pub const NOTE_FILL: Color = Color::new(254.0 / 255.0, 252.0 / 255.0, 232.0 / 255.0, 1.0); // yellow-50
    pub const NOTE_STROKE: Color = Color::new(234.0 / 255.0, 179.0 / 255.0, 8.0 / 255.0, 1.0); // yellow-500
}

// ==================== ER (琥珀) ====================
pub mod er {
    use super::Color;
    pub const FILL: Color = Color::new(255.0 / 255.0, 251.0 / 255.0, 235.0 / 255.0, 1.0); // amber-50
    pub const HEADER_FILL: Color = Color::new(254.0 / 255.0, 243.0 / 255.0, 199.0 / 255.0, 1.0); // amber-100
    pub const STROKE: Color = Color::new(245.0 / 255.0, 158.0 / 255.0, 11.0 / 255.0, 1.0); // amber-500
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
}

// ==================== Timeline (粉) ====================
pub mod timeline {
    use super::Color;
    pub const LINE: Color = Color::new(236.0 / 255.0, 72.0 / 255.0, 153.0 / 255.0, 1.0); // pink-500
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const TITLE: Color = Color::new(30.0 / 255.0, 41.0 / 255.0, 59.0 / 255.0, 1.0); // slate-800
}

// ==================== Git Graph (多分支) ====================
pub mod gitgraph {
    use super::Color;
    pub const BRANCH_COLORS: [Color; 8] = [
        Color::new(99.0 / 255.0, 102.0 / 255.0, 241.0 / 255.0, 1.0),  // indigo-500
        Color::new(249.0 / 255.0, 115.0 / 255.0, 22.0 / 255.0, 1.0),  // orange-500
        Color::new(34.0 / 255.0, 197.0 / 255.0, 94.0 / 255.0, 1.0),   // green-500
        Color::new(234.0 / 255.0, 179.0 / 255.0, 8.0 / 255.0, 1.0),   // yellow-500
        Color::new(168.0 / 255.0, 85.0 / 255.0, 247.0 / 255.0, 1.0),  // purple-500
        Color::new(6.0 / 255.0, 182.0 / 255.0, 212.0 / 255.0, 1.0),   // cyan-500
        Color::new(148.0 / 255.0, 163.0 / 255.0, 184.0 / 255.0, 1.0), // slate-400
        Color::new(236.0 / 255.0, 72.0 / 255.0, 153.0 / 255.0, 1.0),  // pink-500
    ];
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const COMMIT_STROKE: Color = Color::new(255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.0);
}

// ==================== Pie (多色轮盘) ====================
pub mod pie {
    use super::Color;
    pub const COLORS: [Color; 10] = [
        Color::new(99.0 / 255.0, 102.0 / 255.0, 241.0 / 255.0, 1.0), // indigo-500
        Color::new(14.0 / 255.0, 165.0 / 255.0, 233.0 / 255.0, 1.0), // sky-500
        Color::new(249.0 / 255.0, 115.0 / 255.0, 22.0 / 255.0, 1.0), // orange-500
        Color::new(34.0 / 255.0, 197.0 / 255.0, 94.0 / 255.0, 1.0),  // green-500
        Color::new(168.0 / 255.0, 85.0 / 255.0, 247.0 / 255.0, 1.0), // purple-500
        Color::new(234.0 / 255.0, 179.0 / 255.0, 8.0 / 255.0, 1.0),  // yellow-500
        Color::new(236.0 / 255.0, 72.0 / 255.0, 153.0 / 255.0, 1.0), // pink-500
        Color::new(6.0 / 255.0, 182.0 / 255.0, 212.0 / 255.0, 1.0),  // cyan-500
        Color::new(239.0 / 255.0, 68.0 / 255.0, 68.0 / 255.0, 1.0),  // red-500
        Color::new(20.0 / 255.0, 184.0 / 255.0, 166.0 / 255.0, 1.0), // teal-500
    ];
}
