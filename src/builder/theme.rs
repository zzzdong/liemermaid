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
// Stadium 为跑道形：两端半圆直径 = 节点高度（半圆半径 = 半高），故不再用固定小圆角
pub const STADIUM_RADIUS: f64 = 8.0;

// ---- 连线通用（对齐 mermaid 默认主题）----
pub const EDGE_COLOR: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
pub const EDGE_WIDTH: f64 = 2.0;
pub const TEXT_COLOR: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333

// ==================== Flowchart（对齐 mermaid 默认主题）====================
pub mod flowchart {
    use super::Color;
    // 官方默认主题：节点填充 primaryColor=#ECECFF，描边 nodeBorder=#9370DB
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR; // #333333 与官方 lineColor 一致
    pub const SUBGRAPH_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const SUBGRAPH_TITLE: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
}

// ==================== State（对齐 mermaid 默认主题）====================
pub mod state {
    use super::Color;
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const START_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const END_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
}

// ==================== Class（对齐 mermaid 默认主题）====================
pub mod class {
    use super::Color;
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const HEADER_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const SEPARATOR: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const DIAMOND_FILL: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
}

// ==================== Sequence（对齐 mermaid 默认主题）====================
pub mod sequence {
    use super::Color;
    // 官方默认主题：actorBkg=primaryColor=#ECECFF，actorBorder=nodeBorder=#9370DB
    pub const ACTOR_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const ACTOR_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const LIFELINE: Color = Color::new(153.0 / 255.0, 153.0 / 255.0, 153.0 / 255.0, 1.0); // #999 官方 lifeline 灰
    // 官方 activationBkgColor=#f4f4f4 / activationBorderColor=#666（灰），保持与官方一致
    pub const ACTIVATION: Color = Color::new(102.0 / 255.0, 102.0 / 255.0, 102.0 / 255.0, 1.0); // #666
    pub const NOTE_FILL: Color = Color::new(237.0 / 255.0, 242.0 / 255.0, 174.0 / 255.0, 1.0); // #EDF2AE 官方 noteBkgColor
    pub const NOTE_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const BLOCK_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const BLOCK_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const BLOCK_TEXT: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
}

// ==================== ER（对齐 mermaid 默认主题）====================
pub mod er {
    use super::Color;
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const HEADER_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
}

// ==================== Timeline（对齐 mermaid 默认主题）====================
// 官方时间线使用主题色：轴线/节点 nodeBorder=#9370DB，强调 #aaaa33，文字 #333。
pub mod timeline {
    use super::Color;
    pub const LINE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB 与节点描边一致
    pub const ACCENT: Color = Color::new(170.0 / 255.0, 170.0 / 255.0, 51.0 / 255.0, 1.0); // #aaaa33 官方强调色
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const TITLE: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
}

// ==================== Git Graph (多分支) ====================
// 官方默认主题：分支色由 adjust(primaryColor=#ECECFF, {h,...}) 派生（蓝紫协调系），
// 而非彩虹调色板。下方 8 色为 khroma adjust 同款算法算出的确切值。
pub mod gitgraph {
    use super::Color;
    pub const BRANCH_COLORS: [Color; 8] = [
        Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0), // git0 = primaryColor #ECECFF
        Color::new(255.0 / 255.0, 255.0 / 255.0, 222.0 / 255.0, 1.0), // git1 = secondaryColor #ffffde
        Color::new(249.0 / 255.0, 255.0 / 255.0, 236.0 / 255.0, 1.0), // git2 = adjust(primary,{h:-160})
        Color::new(236.0 / 255.0, 246.0 / 255.0, 255.0 / 255.0, 1.0), // git3 = adjust(primary,{h:-30})
        Color::new(236.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.0), // git4 = adjust(primary,{h:-60})
        Color::new(236.0 / 255.0, 255.0 / 255.0, 246.0 / 255.0, 1.0), // git5 = adjust(primary,{h:-90})
        Color::new(255.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0), // git6 = adjust(primary,{h:+60})
        Color::new(255.0 / 255.0, 236.0 / 255.0, 236.0 / 255.0, 1.0), // git7 = adjust(primary,{h:+120})
    ];
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const COMMIT_STROKE: Color = Color::new(255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.0);
}

// ==================== Pie (多色轮盘，对齐 mermaid 默认主题) ====================
// 官方 pie 段色由 adjust(primaryColor=#ECECFF / secondaryColor=#ffffde / tertiaryColor,
// {h,l}) 派生（khroma 同款算法）。下方 12 色为算出的确切值。
pub mod pie {
    use super::Color;
    pub const COLORS: [Color; 12] = [
        Color::new(236.0 / 255.0, 236.0 / 255.0, 255.0 / 255.0, 1.0), // pie1 = primaryColor #ECECFF
        Color::new(255.0 / 255.0, 255.0 / 255.0, 222.0 / 255.0, 1.0), // pie2 = secondaryColor #ffffde
        Color::new(185.0 / 255.0, 255.0 / 255.0, 32.0 / 255.0, 1.0),  // pie3 = adjust(tertiary,{l:-40})
        Color::new(185.0 / 255.0, 185.0 / 255.0, 255.0 / 255.0, 1.0), // pie4 = adjust(primary,{l:-10})
        Color::new(255.0 / 255.0, 255.0 / 255.0, 69.0 / 255.0, 1.0),  // pie5 = adjust(secondary,{l:-30})
        Color::new(217.0 / 255.0, 255.0 / 255.0, 134.0 / 255.0, 1.0), // pie6 = adjust(tertiary,{l:-20})
        Color::new(255.0 / 255.0, 134.0 / 255.0, 255.0 / 255.0, 1.0), // pie7 = adjust(primary,{h:+60,l:-20})
        Color::new(32.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.0),  // pie8 = adjust(primary,{h:-60,l:-40})
        Color::new(255.0 / 255.0, 32.0 / 255.0, 32.0 / 255.0, 1.0),   // pie9 = adjust(primary,{h:120,l:-40})
        Color::new(255.0 / 255.0, 32.0 / 255.0, 255.0 / 255.0, 1.0),  // pie10 = adjust(primary,{h:+60,l:-40})
        Color::new(32.0 / 255.0, 255.0 / 255.0, 144.0 / 255.0, 1.0),  // pie11 = adjust(primary,{h:-90,l:-40})
        Color::new(255.0 / 255.0, 83.0 / 255.0, 83.0 / 255.0, 1.0),   // pie12 = adjust(primary,{h:120,l:-30})
    ];
}
