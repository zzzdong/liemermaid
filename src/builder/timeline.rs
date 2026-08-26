use lievisual::geometry::{Color, Point, Rect};

use crate::{
    ast::TimelineDiagram,
    builder::{layout::types::LayoutEngine, types::OutputConfig},
    error::DiagramResult,
    vir::{self, SceneNode, TextAlign, TextBaseline, Z_AXIS, Z_LABEL, Z_SERIES, Z_TITLE, theme},
};
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

const TITLE_SIZE: f64 = 22.0;

pub struct TimelineEngine<'a> {
    timeline: &'a TimelineDiagram,
}

impl<'a> TimelineEngine<'a> {
    pub fn new(timeline: &'a TimelineDiagram) -> Self {
        Self { timeline }
    }
}

impl<'a> LayoutEngine for TimelineEngine<'a> {
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
        Ok(build_timeline_elements(self.timeline, config))
    }
}

/// 取第 index 个 section 的任务块填充色（来自 theme 调色板，循环）
fn section_color(index: usize) -> Color {
    let colors = theme::timeline::BLOCK_COLORS;
    colors[index % colors.len()]
}

/// 画一个带圆角的彩色矩形块（section 或 event 任务块），样式全部来自 theme
fn draw_task_block(
    elements: &mut Vec<SceneNode>,
    cx: f64,
    cy: f64,
    color: Color,
    text: &str,
    font_size: f64,
) {
    let w = theme::timeline::BLOCK_W;
    let h = theme::timeline::BLOCK_H;
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    let stroke = theme::timeline::BLOCK_STROKE;
    let text_color = theme::timeline::BLOCK_TEXT;

    // 圆角矩形背景
    elements.push(vir::rect_node(
        Rect::new(x, y, x + w, y + h),
        Some(theme::timeline::BLOCK_RX),
        vir::fs_both(color, stroke, theme::timeline::BLOCK_STROKE_W),
        Z_SERIES,
    ));

    // 文字居中
    let ts = vir::text_style(
        text_color,
        font_size,
        theme::FONT_FAMILY.to_string(),
        TextAlign::Center,
        TextBaseline::Middle,
    );
    let layout = layout_text(
        &[RichSpan::new(text.to_string(), ts.clone())],
        Some(w - 16.0),
    );
    let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
    elements.push(vir::text_node(
        text.to_string(),
        Point::new(cx + x_off, cy + y_off),
        ts.with_align(TextAlign::Left)
            .with_baseline(TextBaseline::Top),
        0.0,
        Some(w - 16.0),
        Z_LABEL,
    ));
}

pub fn build_timeline_elements(
    timeline: &TimelineDiagram,
    config: &OutputConfig,
) -> Vec<SceneNode> {
    let mut elements = Vec::new();

    if timeline.sections.is_empty() {
        return elements;
    }

    // Title
    let cur_y = theme::timeline::TITLE_Y;
    if let Some(title) = &timeline.title {
        let ts = vir::text_style(
            theme::timeline::TITLE,
            TITLE_SIZE,
            theme::FONT_FAMILY.to_string(),
            TextAlign::Center,
            TextBaseline::Top,
        );
        let layout = layout_text(
            &[RichSpan::new(title.to_string(), ts.clone())],
            Some(config.width - 80.0),
        );
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Top);
        elements.push(vir::text_node(
            title.clone(),
            Point::new(config.width / 2.0 + x_off, cur_y + y_off),
            ts.clone()
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Top),
            0.0,
            Some(config.width - 80.0),
            Z_TITLE,
        ));
    }

    // Timeline horizontal line (粗线 + 右端箭头)
    let line_y = theme::timeline::LINE_Y;
    let left_margin = theme::timeline::LEFT_MARGIN;
    let right_margin = config.width - theme::timeline::RIGHT_MARGIN;
    let arrow_end = right_margin - theme::timeline::ARROW_SIZE * 1.5;

    // 主轴线
    elements.push(vir::line_node(
        Point::new(left_margin, line_y),
        Point::new(arrow_end, line_y),
        vir::stroke(theme::timeline::LINE, theme::timeline::LINE_WIDTH),
        Z_AXIS,
    ));
    // 右端箭头
    let arr_sz = theme::timeline::ARROW_SIZE;
    elements.push(vir::line_node(
        Point::new(arrow_end, line_y),
        Point::new(arrow_end - arr_sz * 0.7, line_y - arr_sz * 0.5),
        vir::stroke(theme::timeline::LINE, theme::timeline::LINE_WIDTH),
        Z_AXIS,
    ));
    elements.push(vir::line_node(
        Point::new(arrow_end, line_y),
        Point::new(arrow_end - arr_sz * 0.7, line_y + arr_sz * 0.5),
        vir::stroke(theme::timeline::LINE, theme::timeline::LINE_WIDTH),
        Z_AXIS,
    ));

    // Sections and events — 官方布局：section 块在时间线上方，event 块在下方
    let n = timeline.sections.len();
    let col_w = (right_margin - left_margin) / n as f64;

    // 上排：section 彩色矩形块
    let section_block_y = line_y - theme::timeline::SECTION_DY;
    // 下排：event 彩色矩形块
    let event_block_y = line_y + theme::timeline::EVENT_DY;

    for (i, section) in timeline.sections.iter().enumerate() {
        let cx = left_margin + col_w * (i as f64 + 0.5);
        let color = section_color(i);

        // === 时间点：实心圆 ===
        elements.push(vir::circle_node(
            Point::new(cx, line_y),
            theme::timeline::DOT_R,
            vir::fs_fill(theme::timeline::DOT),
            Z_SERIES,
        ));

        // === 上排：Section 彩色矩形块 ===
        draw_task_block(
            &mut elements,
            cx,
            section_block_y,
            color,
            &section.name,
            14.0,
        );

        // 从时间点到上排块的垂直连接线（虚线 + 下端箭头）
        let conn_top = line_y - theme::timeline::DOT_R;
        let conn_bot = section_block_y + theme::timeline::BLOCK_H / 2.0;
        elements.push(vir::line_node(
            Point::new(cx, conn_top),
            Point::new(cx, conn_bot - 6.0),
            vir::dashed_stroke(
                theme::timeline::LINE,
                theme::timeline::CONNECTOR_W,
                [6.0, 4.0].to_vec(),
            ),
            Z_AXIS,
        ));
        // 向上箭头（指向 section 块底部）
        let asz = 6.0;
        elements.push(vir::line_node(
            Point::new(cx, conn_bot - 6.0),
            Point::new(cx - asz * 0.6, conn_bot - 6.0 + asz * 0.8),
            vir::stroke(theme::timeline::LINE, theme::timeline::CONNECTOR_W),
            Z_AXIS,
        ));
        elements.push(vir::line_node(
            Point::new(cx, conn_bot - 6.0),
            Point::new(cx + asz * 0.6, conn_bot - 6.0 + asz * 0.8),
            vir::stroke(theme::timeline::LINE, theme::timeline::CONNECTOR_W),
            Z_AXIS,
        ));

        // === 下排：Event 彩色矩形块 ===
        for (j, event) in section.events.iter().enumerate() {
            let ey =
                event_block_y + j as f64 * (theme::timeline::BLOCK_H + theme::timeline::EVENT_GAP);
            draw_task_block(&mut elements, cx, ey, color, event, 13.0);

            // 从时间点到下排块的垂直连接线（虚线 + 向下箭头）
            let e_conn_top = line_y + theme::timeline::DOT_R;
            let e_conn_bot = ey - theme::timeline::BLOCK_H / 2.0;
            elements.push(vir::line_node(
                Point::new(cx, e_conn_top),
                Point::new(cx, e_conn_bot - 6.0),
                vir::dashed_stroke(
                    theme::timeline::LINE,
                    theme::timeline::CONNECTOR_W,
                    [6.0, 4.0].to_vec(),
                ),
                Z_AXIS,
            ));
            // 向下箭头（指向 event 块顶部）
            let easz = 6.0;
            elements.push(vir::line_node(
                Point::new(cx, e_conn_bot - 6.0),
                Point::new(cx - easz * 0.6, e_conn_bot - 6.0 + easz * 0.8),
                vir::stroke(theme::timeline::LINE, theme::timeline::CONNECTOR_W),
                Z_AXIS,
            ));
            elements.push(vir::line_node(
                Point::new(cx, e_conn_bot - 6.0),
                Point::new(cx + easz * 0.6, e_conn_bot - 6.0 + easz * 0.8),
                vir::stroke(theme::timeline::LINE, theme::timeline::CONNECTOR_W),
                Z_AXIS,
            ));
        }
    }

    elements
}
