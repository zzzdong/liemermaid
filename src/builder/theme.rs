//! 各图表的配色与排版常量。
//!
//! 从旧 `visual::theme` 迁移而来，现独立为 builder 层的主题模块，
//! 由各图表 builder 通过 `crate::vir::theme`（重导出）引用。

use lievisual::Color;

// ---- 基础 ----
pub const BACKGROUND: Color = Color::new(1.0, 1.0, 1.0, 1.0);
// 官方默认主题 fontFamily='"trebuchet ms", verdana, arial, sans-serif'，fontSize='16px'。
// 测量（measure.rs）与绘制（各 builder）共用以下字体配置，保证节点尺寸一致。
pub const FONT_FAMILY: &str = "'trebuchet ms', verdana, arial, sans-serif";
pub const FONT_SIZE: f64 = 16.0;
// 官方默认 radius=5
pub const NODE_RADIUS: f64 = 5.0;
// Stadium 为跑道形：两端半圆直径 = 节点高度（半圆半径 = 半高），故不再用固定小圆角
pub const STADIUM_RADIUS: f64 = 5.0;

// ---- 节点尺寸（测量与绘制共用单一来源，对齐官方默认主题）----
pub const NODE_MIN_W: f64 = 120.0;
pub const NODE_MIN_H: f64 = 60.0;
pub const NODE_PAD_X: f64 = 22.0;
pub const NODE_PAD_Y: f64 = 12.0;

// ---- 连线通用（对齐 mermaid 默认主题）----
pub const EDGE_COLOR: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
// 官方 flowchart 连线更细（约 1.5px），原 2.0 视觉偏粗。
pub const EDGE_WIDTH: f64 = 1.5;
pub const TEXT_COLOR: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333

// ==================== Flowchart（对齐 mermaid 默认主题）====================
pub mod flowchart {
    use super::Color;
    // 官方默认主题：节点填充 primaryColor=#ECECFF，描边 nodeBorder=#9370DB
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR; // #333333 与官方 lineColor 一致
    pub const SUBGRAPH_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const SUBGRAPH_TITLE: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
}

// ==================== State（对齐 mermaid 默认主题）====================
pub mod state {
    use super::Color;
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const START_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const END_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
}

// ==================== Class（对齐 mermaid 默认主题）====================
pub mod class {
    use super::Color;
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const HEADER_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
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
    pub const ACTOR_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const ACTOR_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const LIFELINE: Color = Color::new(153.0 / 255.0, 153.0 / 255.0, 153.0 / 255.0, 1.0); // #999 官方 lifeline 灰
    // 官方 activationBkgColor=#f4f4f4（浅灰填充）/ activationBorderColor=#666（深灰描边）
    pub const ACTIVATION_FILL: Color = Color::new(244.0 / 255.0, 244.0 / 255.0, 244.0 / 255.0, 1.0); // #f4f4f4
    pub const ACTIVATION_STROKE: Color = Color::new(102.0 / 255.0, 102.0 / 255.0, 102.0 / 255.0, 1.0); // #666
    pub const NOTE_FILL: Color = Color::new(237.0 / 255.0, 242.0 / 255.0, 174.0 / 255.0, 1.0); // #EDF2AE 官方 noteBkgColor
    pub const NOTE_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const BLOCK_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const BLOCK_STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const BLOCK_TEXT: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333
}

// ==================== ER（对齐 mermaid 默认主题）====================
pub mod er {
    use super::Color;
    pub const FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const HEADER_FILL: Color = Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0); // #ECECFF
    pub const STROKE: Color = Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
}

// ==================== Timeline（对齐 mermaid 默认主题）====================
// 官方时间线配色：轴线/节点 lineColor=#333，任务块调色板循环，文字 #333。
pub mod timeline {
    use super::Color;
    pub const LINE: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333 时间线+轴线
    pub const DOT: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333 时间线圆点
    pub const BLOCK_STROKE: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333 任务块描边
    pub const BLOCK_TEXT: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333 任务块文字
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const TITLE: Color = Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0); // #333333

    // 任务块填充调色板（官方 timeline 循环配色，蓝/黄/绿协调系）
    pub const BLOCK_COLORS: [Color; 9] = [
        Color::new(147.0 / 255.0, 112.0 / 255.0, 219.0 / 255.0, 1.0), // #9370DB 紫
        Color::new(255.0 / 255.0, 243.0 / 255.0, 176.0 / 255.0, 1.0), // #FFF3B0 黄
        Color::new(144.0 / 255.0, 238.0 / 255.0, 144.0 / 255.0, 1.0), // #90EE90 绿
        Color::new(173.0 / 255.0, 216.0 / 255.0, 230.0 / 255.0, 1.0), // #ADD8E6 浅蓝
        Color::new(255.0 / 255.0, 218.0 / 255.0, 185.0 / 255.0, 1.0), // #FFDAB9 桃色
        Color::new(221.0 / 255.0, 160.0 / 255.0, 221.0 / 255.0, 1.0), // #DDA0DD 梅红
        Color::new(240.0 / 255.0, 255.0 / 255.0, 240.0 / 255.0, 1.0), // #F0FFF0 蜜瓜绿
        Color::new(255.0 / 255.0, 228.0 / 255.0, 196.0 / 255.0, 1.0), // #FFE4C4 鹿皮色
        Color::new(230.0 / 255.0, 230.0 / 255.0, 250.0 / 255.0, 1.0), // #E6E6FA 薰衣草
    ];

    // 布局尺寸（测量与绘制共用单一来源）
    pub const BLOCK_W: f64 = 100.0;
    pub const BLOCK_H: f64 = 44.0;
    pub const BLOCK_RX: f64 = 6.0;
    pub const DOT_R: f64 = 7.0;
    pub const TITLE_Y: f64 = 25.0;          // 标题 Y 位置
    pub const LINE_Y: f64 = 130.0;         // 时间线 Y 位置（从画布顶部计）
    pub const SECTION_DY: f64 = 60.0;      // section 块在时间线上方的距离
    pub const EVENT_DY: f64 = 75.0;        // event 块在时间线下方首个位置的距离
    pub const EVENT_GAP: f64 = 15.0;       // 同列多个 event 块之间的间距
    pub const LINE_WIDTH: f64 = 2.5;
    pub const BLOCK_STROKE_W: f64 = 1.5;
    pub const CONNECTOR_W: f64 = 1.5;
    pub const ARROW_SIZE: f64 = 8.0;
    pub const LEFT_MARGIN: f64 = 60.0;
    pub const RIGHT_MARGIN: f64 = 60.0;
}

// ==================== Git Graph (多分支) ====================
// 官方默认主题：高饱和度深色系（深蓝/金黄/绿/红…），在白底上清晰可见。
pub mod gitgraph {
    use super::Color;
    pub const BRANCH_COLORS: [Color; 8] = [
        Color::new(0.0, 0.0, 204.0 / 255.0, 1.0), // git0 深蓝 #0000CC (main)
        Color::new(255.0 / 255.0, 200.0 / 255.0, 0.0, 1.0), // git1 金黄 #FFC800 (develop)
        Color::new(34.0 / 255.0, 139.0 / 255.0, 34.0 / 255.0, 1.0), // git2 森林绿 #228B22
        Color::new(220.0 / 255.0, 20.0 / 255.0, 60.0 / 255.0, 1.0), // git3 猩红 #DC143C
        Color::new(148.0 / 255.0, 0.0, 211.0 / 255.0, 1.0), // git4 紫罗兰 #9400D3
        Color::new(30.0 / 255.0, 144.0 / 255.0, 255.0 / 255.0, 1.0), // git5 道奇蓝 #1E90FF
        Color::new(255.0 / 255.0, 140.0 / 255.0, 0.0, 1.0), // git6 深橙 #FF8C00
        Color::new(50.0 / 255.0, 205.0 / 255.0, 50.0 / 255.0, 1.0), // git7 酸橙绿 #32CD32
    ];
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const COMMIT_STROKE: Color = Color::new(1.0, 1.0, 1.0, 1.0);

    // 布局尺寸（横向布局，测量与绘制共用单一来源）
    pub const COMMIT_RADIUS: f64 = 9.0;      // commit 圆点半径
    pub const BRANCH_SPACING: f64 = 60.0;     // 分支行间距（Y 方向）
    pub const COMMIT_SPACING: f64 = 70.0;     // commit 间距（X 方向）
    pub const LABEL_OFFSET: f64 = 20.0;       // 标签偏移
    pub const LEFT_MARGIN: f64 = 120.0;       // 左边距（留出分支标签空间）
    pub const TOP_MARGIN: f64 = 35.0;         // 上边距
    pub const LINE_WIDTH: f64 = 4.0;          // 连线宽度（加粗更醒目）
}

// ==================== Pie (多色轮盘，对齐 mermaid 默认主题) ====================
// 官方 pie 段色由 adjust(primaryColor=#ECECFF / secondaryColor=#ffffde / tertiaryColor,
// {h,l}) 派生（khroma 同款算法）。下方 12 色为算出的确切值。
pub mod pie {
    use super::Color;
    pub const COLORS: [Color; 12] = [
        Color::new(236.0 / 255.0, 236.0 / 255.0, 1.0, 1.0), // pie1 = primaryColor #ECECFF
        Color::new(1.0, 1.0, 222.0 / 255.0, 1.0), // pie2 = secondaryColor #ffffde
        Color::new(185.0 / 255.0, 1.0, 32.0 / 255.0, 1.0),  // pie3 = adjust(tertiary,{l:-40})
        Color::new(185.0 / 255.0, 185.0 / 255.0, 1.0, 1.0), // pie4 = adjust(primary,{l:-10})
        Color::new(1.0, 1.0, 69.0 / 255.0, 1.0),  // pie5 = adjust(secondary,{l:-30})
        Color::new(217.0 / 255.0, 1.0, 134.0 / 255.0, 1.0), // pie6 = adjust(tertiary,{l:-20})
        Color::new(1.0, 134.0 / 255.0, 1.0, 1.0), // pie7 = adjust(primary,{h:+60,l:-20})
        Color::new(32.0 / 255.0, 1.0, 1.0, 1.0),  // pie8 = adjust(primary,{h:-60,l:-40})
        Color::new(1.0, 32.0 / 255.0, 32.0 / 255.0, 1.0),   // pie9 = adjust(primary,{h:120,l:-40})
        Color::new(1.0, 32.0 / 255.0, 1.0, 1.0),  // pie10 = adjust(primary,{h:+60,l:-40})
        Color::new(32.0 / 255.0, 1.0, 144.0 / 255.0, 1.0),  // pie11 = adjust(primary,{h:-90,l:-40})
        Color::new(1.0, 83.0 / 255.0, 83.0 / 255.0, 1.0),   // pie12 = adjust(primary,{h:120,l:-30})
    ];
}
