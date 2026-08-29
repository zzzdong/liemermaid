//! 各图表的配色与排版常量。
//!
//! 从旧 `visual::theme` 迁移而来，现独立为 builder 层的主题模块，
//! 由各图表 builder 通过 `crate::vir::theme`（重导出）引用。

use lievisual::Color;

// ---- 基础 ----
pub const BACKGROUND: Color = Color::new(255, 255, 255, 255);
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
pub const EDGE_COLOR: Color = Color::new(51, 51, 51, 255); // #333333
// 官方 flowchart 连线更细（约 1.5px），原 2.0 视觉偏粗。
pub const EDGE_WIDTH: f64 = 1.5;
// 官方节点文本颜色（CSS: .nodeLabel{color:#131300}），非 #333。
pub const TEXT_COLOR: Color = Color::new(19, 19, 0, 255); // #131300

// ==================== Flowchart（对齐 mermaid 默认主题）====================
pub mod flowchart {
    use super::Color;
    // 官方默认主题：节点填充 primaryColor=#ECECFF，描边 nodeBorder=#9370DB
    pub const FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR; // #333333 与官方 lineColor 一致
    pub const SUBGRAPH_STROKE: Color = Color::new(170, 170, 51, 255); // #AAAA33 官方 cluster 边框
    pub const SUBGRAPH_FILL: Color = Color::new(255, 255, 222, 255); // #FFFFDE 官方 cluster 填充（淡黄）
    pub const SUBGRAPH_TITLE: Color = Color::new(51, 51, 51, 255); // #333333
}

// ==================== State（对齐 mermaid 默认主题）====================
pub mod state {
    use super::Color;
    pub const FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const START_FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const END_STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
}

// ==================== Class（对齐 mermaid 默认主题）====================
pub mod class {
    use super::Color;
    pub const FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const HEADER_FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    // 官方类图文本（类名 + 成员）为深色（CSS: .nodeLabel{color:#131300}），非紫色。
    pub const TEXT: Color = Color::new(19, 19, 0, 255); // #131300
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const SEPARATOR: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const DIAMOND_FILL: Color = Color::new(147, 112, 219, 255); // #9370DB
    /// 官方类图文本字号：继承根 `#my-svg{font-size:16px}`（foreignObject 行高 24 = 16×1.5）。
    pub const MEMBER_FONT_SIZE: f64 = 16.0;
}

// ==================== Sequence（对齐 mermaid 默认主题）====================
pub mod sequence {
    use super::Color;
    // 官方默认主题：actorBkg=primaryColor=#ECECFF，actorBorder=nodeBorder=#9370DB
    pub const ACTOR_FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const ACTOR_STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    pub const LIFELINE: Color = Color::new(153, 153, 153, 255); // #999 官方 lifeline 灰
    // 官方 activationBkgColor=#f4f4f4（浅灰填充）/ activationBorderColor=#666（深灰描边）
    pub const ACTIVATION_FILL: Color = Color::new(244, 244, 244, 255); // #f4f4f4
    pub const ACTIVATION_STROKE: Color =
        Color::new(102, 102, 102, 255); // #666
    pub const NOTE_FILL: Color = Color::new(237, 242, 174, 255); // #EDF2AE 官方 noteBkgColor
    pub const NOTE_STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const BLOCK_FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const BLOCK_STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const BLOCK_TEXT: Color = Color::new(51, 51, 51, 255); // #333333
}

// ==================== ER（对齐 mermaid 默认主题）====================
pub mod er {
    use super::Color;
    pub const FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const HEADER_FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
}

// ==================== Timeline（对齐 mermaid 默认主题）====================
// 官方时间线配色：轴线/节点 lineColor=#333，任务块调色板循环，文字 #333。
pub mod timeline {
    use super::Color;
    pub const LINE: Color = Color::new(51, 51, 51, 255); // #333333 时间线+轴线
    pub const DOT: Color = Color::new(51, 51, 51, 255); // #333333 时间线圆点
    pub const BLOCK_STROKE: Color = Color::new(51, 51, 51, 255); // #333333 任务块描边
    pub const BLOCK_TEXT: Color = Color::new(51, 51, 51, 255); // #333333 任务块文字
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const TITLE: Color = Color::new(51, 51, 51, 255); // #333333

    // 任务块填充调色板（官方 timeline 循环配色，蓝/黄/绿协调系）
    pub const BLOCK_COLORS: [Color; 9] = [
        Color::new(147, 112, 219, 255), // #9370DB 紫
        Color::new(255, 243, 176, 255), // #FFF3B0 黄
        Color::new(144, 238, 144, 255), // #90EE90 绿
        Color::new(173, 216, 230, 255), // #ADD8E6 浅蓝
        Color::new(255, 218, 185, 255), // #FFDAB9 桃色
        Color::new(221, 160, 221, 255), // #DDA0DD 梅红
        Color::new(240, 255, 240, 255), // #F0FFF0 蜜瓜绿
        Color::new(255, 228, 196, 255), // #FFE4C4 鹿皮色
        Color::new(230, 230, 250, 255), // #E6E6FA 薰衣草
    ];

    // 布局尺寸（测量与绘制共用单一来源）
    pub const BLOCK_W: f64 = 100.0;
    pub const BLOCK_H: f64 = 44.0;
    pub const BLOCK_RX: f64 = 6.0;
    pub const DOT_R: f64 = 7.0;
    pub const TITLE_Y: f64 = 25.0; // 标题 Y 位置
    pub const LINE_Y: f64 = 130.0; // 时间线 Y 位置（从画布顶部计）
    pub const SECTION_DY: f64 = 60.0; // section 块在时间线上方的距离
    pub const EVENT_DY: f64 = 75.0; // event 块在时间线下方首个位置的距离
    pub const EVENT_GAP: f64 = 15.0; // 同列多个 event 块之间的间距
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
        Color::new(0, 0, 204, 255), // git0 深蓝 #0000CC (main)
        Color::new(255, 200, 0, 255), // git1 金黄 #FFC800 (develop)
        Color::new(34, 139, 34, 255), // git2 森林绿 #228B22
        Color::new(220, 20, 60, 255), // git3 猩红 #DC143C
        Color::new(148, 0, 211, 255), // git4 紫罗兰 #9400D3
        Color::new(30, 144, 255, 255), // git5 道奇蓝 #1E90FF
        Color::new(255, 140, 0, 255), // git6 深橙 #FF8C00
        Color::new(50, 205, 50, 255), // git7 酸橙绿 #32CD32
    ];
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const COMMIT_STROKE: Color = Color::new(255, 255, 255, 255);

    // 布局尺寸（横向布局，测量与绘制共用单一来源）
    pub const COMMIT_RADIUS: f64 = 10.0; // commit 圆点半径（对齐官方 circle r=10）
    pub const BRANCH_SPACING: f64 = 60.0; // 分支行间距（Y 方向）
    pub const COMMIT_SPACING: f64 = 70.0; // commit 间距（X 方向）
    pub const LABEL_OFFSET: f64 = 20.0; // 标签偏移
    pub const LEFT_MARGIN: f64 = 120.0; // 左边距（留出分支标签空间）
    pub const TOP_MARGIN: f64 = 35.0; // 上边距
    pub const LINE_WIDTH: f64 = 4.0; // 连线宽度（加粗更醒目）

    // commit id / tag 标签样式（对齐官方 mermaid 默认主题）
    pub const LABEL_FONT: f64 = 10.0; // 官方 .commit-label / .tag-label font-size:10px
    pub const COMMIT_LABEL_FILL: Color = Color::new(0, 0, 33, 255); // .commit-label #000021
    pub const COMMIT_LABEL_BKG: Color = Color::new(255, 255, 222, 128); // .commit-label-bkg #ffffde @ opacity 0.5
    pub const TAG_LABEL_FILL: Color = Color::new(19, 19, 0, 255); // .tag-label #131300
    pub const TAG_BKG: Color = Color::new(224, 224, 224, 255); // tag 背景浅灰 #E0E0E0
    pub const TAG_BKG_STROKE: Color = Color::new(184, 184, 184, 255); // tag 背景描边 #B8B8B8
    pub const HIGHLIGHT_OUTER: Color = Color::new(19, 19, 0, 255); // .commit-highlight-outer hsl(60,100%,3.7%)≈#131300
    pub const MERGE_INNER: Color = Color::new(236, 236, 255, 255); // .commit-merge / .commit-highlight-inner #ECECFF
}

// ==================== Pie (多色轮盘，对齐 mermaid 默认主题) ====================
// 官方 pie 段色由 adjust(primaryColor=#ECECFF / secondaryColor=#ffffde / tertiaryColor,
// {h,l}) 派生（khroma 同款算法）。下方 12 色为算出的确切值。
pub mod pie {
    use super::Color;
    pub const COLORS: [Color; 12] = [
        Color::new(236, 236, 255, 255), // pie1 = primaryColor #ECECFF
        Color::new(255, 255, 222, 255),           // pie2 = secondaryColor #ffffde
        Color::new(185, 255, 32, 255),  // pie3 = adjust(tertiary,{l:-40})
        Color::new(185, 185, 255, 255), // pie4 = adjust(primary,{l:-10})
        Color::new(255, 255, 69, 255),            // pie5 = adjust(secondary,{l:-30})
        Color::new(217, 255, 134, 255), // pie6 = adjust(tertiary,{l:-20})
        Color::new(255, 134, 255, 255),           // pie7 = adjust(primary,{h:+60,l:-20})
        Color::new(32, 255, 255, 255),            // pie8 = adjust(primary,{h:-60,l:-40})
        Color::new(255, 32, 32, 255),   // pie9 = adjust(primary,{h:120,l:-40})
        Color::new(255, 32, 255, 255),            // pie10 = adjust(primary,{h:+60,l:-40})
        Color::new(32, 255, 144, 255),  // pie11 = adjust(primary,{h:-90,l:-40})
        Color::new(255, 83, 83, 255),   // pie12 = adjust(primary,{h:120,l:-30})
    ];
}
