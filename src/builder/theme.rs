//! 各图表的配色与排版常量。
//!
//! 从旧 `visual::theme` 迁移而来，现独立为 builder 层的主题模块；
//! `materialize` 直接引用 `crate::builder::theme` 的常量。

use lievisual::Color;
use lievisual::text::{TextAlign, TextBaseline, TextStyle};

// ---- 渲染层级（SceneNode.z_index）----
// 数值约定：背景 < 容器 < 数据图元 < 标签 < 标题。materialize 是唯一消费方。

/// 画布背景。
pub const Z_BACKGROUND: i32 = 0;
/// 子图 / 复合状态等容器框（位于节点与边之下）。
pub const Z_SUBGRAPH: i32 = 5;
/// 网格 / 参考线（如 timeline 轴线）。
pub const Z_GRID: i32 = 10;
/// 数据图元（节点、扇区、时间轴块）。
pub const Z_SERIES: i32 = 20;
/// 节点 / 边标签。
pub const Z_LABEL: i32 = 40;
/// 图表标题。
pub const Z_TITLE: i32 = 50;

// ---- 基础 ----
pub const BACKGROUND: Color = Color::new(255, 255, 255, 255);
// 官方默认主题 fontFamily='"trebuchet ms", verdana, arial, sans-serif'，fontSize='16px'。
// 测量（measure.rs）与绘制（各 builder）共用以下字体配置，保证节点尺寸一致。
pub const FONT_FAMILY: &str = "'trebuchet ms', verdana, arial, sans-serif";
pub const FONT_SIZE: f64 = 16.0;
/// 官方 CSS `line-height: 1.5`（`#my-svg .label{line-height:1.5}`），
/// 直接影响节点包围盒高度（foreignObject 高度 = 16 × 1.5 = 24）。
pub const LINE_HEIGHT: f64 = 1.5;
// 官方默认 radius=5
pub const NODE_RADIUS: f64 = 5.0;
// Stadium 为跑道形：两端半圆直径 = 节点高度（半圆半径 = 半高），故不再用固定小圆角
pub const STADIUM_RADIUS: f64 = 5.0;

// ---- 节点尺寸（测量与绘制共用单一来源，对齐官方默认主题）----
// 官方 mermaid 单栏节点包围盒 = 文本排版盒 + 各形状 padding（实测 golden）：
//   rect / rounded：W = text_w + 60，H = text_h + 30（如 "Start" → 93.8×54）。
//   故基准 padding 为 PAD_X=30 / PAD_Y=15；**无 120×60 最小尺寸**（官方节点随文本收缩）。
pub const NODE_PAD_X: f64 = 30.0;
pub const NODE_PAD_Y: f64 = 15.0;
/// 节点尺寸的绝对下限（仅防止退化成 0，远小于官方默认，不干预正常排版）。
pub const NODE_MIN_W: f64 = 16.0;
pub const NODE_MIN_H: f64 = 16.0;

// ---- 连线通用（对齐 mermaid 默认主题）----
pub const EDGE_COLOR: Color = Color::new(51, 51, 51, 255); // #333333
/// 官方 `.edge-thickness-normal{stroke-width:1px}`。
pub const EDGE_WIDTH: f64 = 1.0;
/// 官方 `.edge-thickness-thick{stroke-width:3.5px}`（`==>` 粗线）。
pub const EDGE_WIDTH_THICK: f64 = 3.5;
// 官方节点文本颜色（CSS: .nodeLabel{color:#131300}），非 #333。
pub const TEXT_COLOR: Color = Color::new(19, 19, 0, 255); // #131300

/// 带官方行高（`line-height: 1.5`）的文本样式。
///
/// 官方 mermaid 的 `<foreignObject>` 高度 = `font-size × 1.5`（16px → 24px），
/// 直接决定节点包围盒高度。不设行高时 lievisual 返回字体固有行高（≈18.6px），
/// 节点会比官方矮约 5px。**measure 与 materialize 必须共用**，否则文本垂直位置会偏。
pub fn text_style(color: Color, size: f64, align: TextAlign, baseline: TextBaseline) -> TextStyle {
    TextStyle::new(color, size, FONT_FAMILY)
        .with_align(align)
        .with_baseline(baseline)
        .with_line_height(size * LINE_HEIGHT)
}

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
    /// 官方边标签背景：`.labelBkg{background-color:rgba(232,232,232,0.5)}`
    /// ——半透明浅灰（#e8e8e8 @ 0.5），不是纯白。
    pub const EDGE_LABEL_BG: Color = Color::new(232, 232, 232, 128);
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
    /// 官方 state 特殊节点（start 实心圆 / fork-join 横条）颜色：
    /// `.node circle.state-start{fill:#333333;stroke:#333333}`、
    /// `.node .fork-join{fill:#333333;stroke:#333333}`——深色，非紫色。
    pub const SPECIAL: Color = Color::new(51, 51, 51, 255); // #333333
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
    /// 官方 class 边标签背景：`.labelBkg{background:#ECECFF}`（**不透明**，与节点填充同色）。
    pub const EDGE_LABEL_BG: Color = Color::new(236, 236, 255, 255);
    /// 官方类图文本字号：继承根 `#my-svg{font-size:16px}`（foreignObject 行高 24 = 16×1.5）。
    pub const MEMBER_FONT_SIZE: f64 = 16.0;
}

// ==================== Sequence（对齐 mermaid 默认主题）====================
pub mod sequence {
    use super::Color;
    // 官方默认：参与者盒淡紫填充 + 紫色描边。
    pub const ACTOR_FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const ACTOR_STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const ACTOR_RADIUS: f64 = 3.0;
    pub const FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    // 官方 `.messageText{fill:#333;stroke:none}`（消息/备注文本用 #333，非节点标签的 #131300）。
    pub const TEXT: Color = Color::new(51, 51, 51, 255); // #333
    // 官方 `.actor>tspan{fill:black}`（参与者盒内名字用纯黑）。
    pub const ACTOR_TEXT: Color = Color::new(0, 0, 0, 255); // #000
    pub const EDGE: Color = super::EDGE_COLOR;
    // 官方 golden 计算样式（浏览器实测，CSS 覆盖内联属性后的真实呈现）：
    // `.actor-line{stroke:#9370DB}` + `#my-svg line{stroke-width:2px}`，无 dasharray
    // ——即**实线** 2px 紫色（内联 `stroke="#999" stroke-width="0.5px"` 被 CSS 覆盖）。
    pub const LIFELINE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const LIFELINE_WIDTH: f64 = 2.0;
    /// 生命线为实线（官方无 stroke-dasharray）。
    pub const LIFELINE_DASH: [f64; 0] = [];
    /// 官方 golden 计算样式：`.messageLine0{stroke-width:1.5;stroke:#333}`
    /// （内联 `stroke-width="2"` 被 CSS 覆盖为 1.5px）。
    pub const MESSAGE_WIDTH: f64 = 1.5;
    /// 官方虚线消息 `style="stroke-dasharray: 3, 3;"`（内联 style 优先于 CSS 的 2,2）。
    pub const MESSAGE_DASH: [f64; 2] = [3.0, 3.0];
    // 官方默认主题 activation 计算样式：`.activation0{fill:#f4f4f4;stroke:#666}`
    // （内联 `fill="#EDF2AE"` 被 CSS 覆盖）。
    pub const ACTIVATION_FILL: Color = Color::new(244, 244, 244, 255); // #f4f4f4
    pub const ACTIVATION_STROKE: Color = Color::new(102, 102, 102, 255); // #666
    pub const ACTIVATION_WIDTH: f64 = 10.0;
    // 官方备注框计算样式：`.note{stroke:#aaaa33;fill:#fff5ad}`（内联 #EDF2AE/#666 被覆盖）。
    pub const NOTE_FILL: Color = Color::new(255, 245, 173, 255); // #fff5ad
    pub const NOTE_STROKE: Color = Color::new(170, 170, 51, 255); // #aaaa33
    /// 官方 `.noteText{fill:black}`。
    pub const NOTE_TEXT: Color = Color::new(0, 0, 0, 255); // #000
    /// 官方 `.labelText,.loopText{fill:black}`（分组块标签为黑色，非 #333）。
    pub const BLOCK_TEXT: Color = Color::new(0, 0, 0, 255); // #000
    pub const BLOCK_FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const BLOCK_STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
}

// ==================== ER（对齐 mermaid 默认主题）====================
pub mod er {
    use super::Color;
    pub const FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const HEADER_FILL: Color = Color::new(236, 236, 255, 255); // #ECECFF
    /// attribute 斑马纹：奇数行（1st/3rd/...）用白色。
    pub const ATTR_ROW_LIGHT: Color = Color::new(255, 255, 255, 255); // #FFFFFF
    /// attribute 斑马纹：偶数行（2nd/4th/...）用淡紫（与实体填充同色）。
    pub const ATTR_ROW_DARK: Color = Color::new(236, 236, 255, 255); // #ECECFF
    pub const STROKE: Color = Color::new(147, 112, 219, 255); // #9370DB
    pub const TEXT: Color = super::TEXT_COLOR;
    pub const EDGE: Color = super::EDGE_COLOR;
    /// 官方 ER 边标签背景：`.labelBkg{background-color:rgba(248.67,255,236,0.5)}`
    /// ——半透明淡黄绿（≈ #f8ffec @ 0.5）。
    pub const EDGE_LABEL_BG: Color = Color::new(248, 255, 236, 128);
}

// ==================== Timeline（对齐官方 timeline golden 实测）====================
// 官方 golden `timeline__basic` 实测：
// - 列距 200（块宽 190 + 列间隙 10）；section / task（period）块 190×65.8，
//   仅**顶角**圆（r=5），底边另有 3px 互补色线；event 块 190×50（更矮）；
// - 主轴 4px 黑色实线 + 末端箭头，**轴上无圆点**；
// - 每列一条 2px 黑虚线（dash 5,5）从 task 块底穿过主轴延伸到 event 块下方；
// - 标题 `font-size:4ex;font-weight:bold` 左对齐于轴起点上方；
// - 配色 = 官方 cScale 调色板（section i → 第 i 项），白字仅紫罗兰系（0/3）。
pub mod timeline {
    use super::Color;

    /// 主轴 / 连接虚线颜色（官方黑色）。
    pub const LINE: Color = Color::new(0, 0, 0, 255);
    pub const AXIS_WIDTH: f64 = 4.0;
    pub const CONNECTOR_W: f64 = 2.0;
    pub const ARROW_SIZE: f64 = 10.0;
    /// 主轴左端 / 右端超出首 / 末列中心的距离。
    pub const AXIS_HEAD: f64 = 50.0;
    pub const AXIS_TAIL: f64 = 250.0;
    /// 连接虚线末端越过最后一个 event 块底的距离。
    pub const CONNECTOR_TAIL: f64 = 60.0;
    /// 块底线宽度（官方 `.section-N line{stroke-width:3}`）。
    pub const BLOCK_LINE_W: f64 = 3.0;
    /// 标题（官方 `4ex` 粗体，黑色）。
    pub const TITLE: Color = Color::new(0, 0, 0, 255);
    pub const TITLE_SIZE: f64 = 64.0;
    /// 块文本字号（官方默认 16px）。
    pub const BLOCK_TEXT_SIZE: f64 = 16.0;

    // 官方 cScale 调色板（section i → 第 i 项，HSL 转 RGB）。
    pub const BLOCK_COLORS: [Color; 12] = [
        Color::new(134, 134, 255, 255), // hsl(240,100%,76.3%) #8686FF
        Color::new(255, 255, 120, 255), // hsl(60,100%,73.5%)  #FFFF78
        Color::new(215, 255, 134, 255), // hsl(80,100%,76.3%)  #D7FF86
        Color::new(194, 134, 255, 255), // hsl(270,100%,76.3%) #C286FF
        Color::new(255, 134, 255, 255), // hsl(300,100%,76.3%) #FF86FF
        Color::new(255, 134, 194, 255), // hsl(330,100%,76.3%) #FF86C2
        Color::new(255, 134, 134, 255), // hsl(0,100%,76.3%)   #FF8686
        Color::new(255, 194, 134, 255), // hsl(30,100%,76.3%)  #FFC286
        Color::new(194, 255, 134, 255), // hsl(90,100%,76.3%)  #C2FF86
        Color::new(134, 255, 194, 255), // hsl(150,100%,76.3%) #86FFC2
        Color::new(134, 255, 255, 255), // hsl(180,100%,76.3%) #86FFFF
        Color::new(134, 194, 255, 255), // hsl(210,100%,76.3%) #86C2FF
    ];
    /// 官方块底线颜色（section 色相的**互补色**提亮，`.section-N line`）。
    pub const BLOCK_LINE_COLORS: [Color; 12] = [
        Color::new(255, 255, 185, 255), // hsl(60,100%,86.3%)  #FFFFB9
        Color::new(171, 171, 255, 255), // hsl(240,100%,83.5%) #ABABFF
        Color::new(208, 185, 255, 255), // hsl(260,100%,86.3%) #D0B9FF
        Color::new(220, 255, 185, 255), // hsl(90,100%,86.3%)  #DCFFB9
        Color::new(185, 255, 185, 255), // hsl(120,100%,86.3%) #B9FFB9
        Color::new(185, 255, 220, 255), // hsl(150,100%,86.3%) #B9FFDC
        Color::new(185, 255, 255, 255), // hsl(180,100%,86.3%) #B9FFFF
        Color::new(185, 220, 255, 255), // hsl(210,100%,86.3%) #B9DCFF
        Color::new(220, 185, 255, 255), // hsl(270,100%,86.3%) #DCB9FF
        Color::new(255, 185, 220, 255), // hsl(330,100%,86.3%) #FFB9DC
        Color::new(255, 185, 185, 255), // hsl(0,100%,86.3%)   #FFB9B9
        Color::new(255, 220, 185, 255), // hsl(30,100%,86.3%)  #FFDCB9
    ];
    /// 白字的调色板下标（官方 `.section--1/.section-2 text{fill:#ffffff}`）。
    pub const BLOCK_TEXT_WHITE: [usize; 2] = [0, 3];
    pub const TEXT_WHITE: Color = Color::new(255, 255, 255, 255);
    pub const TEXT_BLACK: Color = Color::new(0, 0, 0, 255);

    /// 第 idx 列的块填充色。
    pub fn block_color(idx: usize) -> Color {
        BLOCK_COLORS[idx % BLOCK_COLORS.len()]
    }
    /// 第 idx 列的块文本色（紫罗兰系白字，其余黑字）。
    pub fn block_text_color(idx: usize) -> Color {
        let i = idx % BLOCK_COLORS.len();
        if BLOCK_TEXT_WHITE.contains(&i) {
            TEXT_WHITE
        } else {
            TEXT_BLACK
        }
    }
    /// 第 idx 列的块底线颜色。
    pub fn block_line_color(idx: usize) -> Color {
        BLOCK_LINE_COLORS[idx % BLOCK_LINE_COLORS.len()]
    }

    // 布局尺寸（官方 golden 实测）。
    // 结构：section 块（轴上方 200）→ period/task 块（轴上方 83）→ 主轴 →
    // event 块（轴下方 109 起垂直堆叠）；列心相距 200（块宽 190 + 间隙 10）。
    pub const BLOCK_W: f64 = 190.0;
    pub const BLOCK_H: f64 = 65.8;
    pub const EVENT_H: f64 = 50.0;
    /// 块顶角圆角半径（底角为直角）。
    pub const BLOCK_RX: f64 = 5.0;
    /// 列心间距（官方块宽 190 + 列间隙 10）。
    pub const COL_PITCH: f64 = 200.0;
    /// section 块中心相对主轴的偏移（轴上方）。
    pub const SECTION_DY: f64 = 200.0;
    /// period（task）块中心相对主轴的偏移（轴上方）。
    pub const PERIOD_DY: f64 = 83.0;
    /// 首个 event 块中心相对主轴的偏移（轴下方）。
    pub const EVENT_DY: f64 = 109.0;
    /// 同列多个 event 块之间的间距。
    pub const EVENT_GAP: f64 = 50.0;
}

// ==================== Git Graph (多分支) ====================
// 官方默认主题：高饱和度深色系（深蓝/金黄/绿/红…），在白底上清晰可见。
pub mod gitgraph {
    use super::Color;
    pub const BRANCH_COLORS: [Color; 8] = [
        Color::new(0, 0, 204, 255),    // git0 深蓝 #0000CC (main)
        Color::new(255, 200, 0, 255),  // git1 金黄 #FFC800 (develop)
        Color::new(34, 139, 34, 255),  // git2 森林绿 #228B22
        Color::new(220, 20, 60, 255),  // git3 猩红 #DC143C
        Color::new(148, 0, 211, 255),  // git4 紫罗兰 #9400D3
        Color::new(30, 144, 255, 255), // git5 道奇蓝 #1E90FF
        Color::new(255, 140, 0, 255),  // git6 深橙 #FF8C00
        Color::new(50, 205, 50, 255),  // git7 酸橙绿 #32CD32
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
    /// 官方 golden：扇区路径半径 185，外圈 `<circle r="186" class="pieOuterCircle"/>`。
    pub const RADIUS: f64 = 185.0;
    pub const OUTER_RADIUS: f64 = 186.0;
    pub const OUTER_STROKE: Color = Color::new(0, 0, 0, 255); // 官方 `.pieOuterCircle{stroke:black}`
    pub const OUTER_STROKE_WIDTH: f64 = 2.0;
    /// 扇区分隔描边（官方 `.pieCircle{stroke:black;stroke-width:2px}` —— 黑线
    /// 才能在相邻浅色扇区之间看清切分边界，白线在白底上不可见）。
    pub const SLICE_STROKE: Color = Color::new(0, 0, 0, 255);
    pub const SLICE_STROKE_WIDTH: f64 = 2.0;
    /// 扇区百分比标签所在半径（官方实测 = 0.75 × 半径）。
    pub const LABEL_RADIUS_RATIO: f64 = 0.75;
    /// 图例色块尺寸与排版（官方 `<rect width="18" height="18"/>` + `<text x="22">`）。
    pub const LEGEND_SWATCH: f64 = 18.0;
    pub const LEGEND_TEXT_DX: f64 = 22.0;
    pub const LEGEND_ROW_H: f64 = 22.0;
    /// 图例左边缘相对圆心的 x 偏移（官方 = 半径 + 31）。
    pub const LEGEND_DX: f64 = 31.0;
    /// 标题基线相对圆心的 y 偏移（官方 `<text x="0" y="-200" class="pieTitleText">`）。
    pub const TITLE_DY: f64 = 200.0;
    pub const TITLE_FONT: f64 = 24.0;
    pub const LABEL_FONT: f64 = 16.0;
    pub const COLORS: [Color; 12] = [
        Color::new(236, 236, 255, 255), // pie1 = primaryColor #ECECFF
        Color::new(255, 255, 222, 255), // pie2 = secondaryColor #ffffde
        Color::new(185, 255, 32, 255),  // pie3 = adjust(tertiary,{l:-40})
        Color::new(185, 185, 255, 255), // pie4 = adjust(primary,{l:-10})
        Color::new(255, 255, 69, 255),  // pie5 = adjust(secondary,{l:-30})
        Color::new(217, 255, 134, 255), // pie6 = adjust(tertiary,{l:-20})
        Color::new(255, 134, 255, 255), // pie7 = adjust(primary,{h:+60,l:-20})
        Color::new(32, 255, 255, 255),  // pie8 = adjust(primary,{h:-60,l:-40})
        Color::new(255, 32, 32, 255),   // pie9 = adjust(primary,{h:120,l:-40})
        Color::new(255, 32, 255, 255),  // pie10 = adjust(primary,{h:+60,l:-40})
        Color::new(32, 255, 144, 255),  // pie11 = adjust(primary,{h:-90,l:-40})
        Color::new(255, 83, 83, 255),   // pie12 = adjust(primary,{h:120,l:-30})
    ];
}
