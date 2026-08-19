use crate::{
    ast::{
        MessageArrow, NotePlacement, SequenceBlock, SequenceBlockKind, SequenceDiagram,
        SequenceItem, SequenceStatement,
    },
    builder::{layout::types::LayoutEngine, types::OutputConfig},
    error::DiagramResult,
    vir::{self, Element, SceneNode, TextAlign, TextBaseline, Z_AXIS, Z_LABEL, Z_SERIES, theme},
};
use lievisual::geometry::Point;
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

const BOX_HEIGHT: f64 = 40.0;
const BOX_MIN_WIDTH: f64 = 80.0;
const PAD_X: f64 = 16.0;
const COL_GAP: f64 = 40.0;
const LIFELINE_DASH: f64 = 6.0;
const MESSAGE_SPACING: f64 = 50.0;
const NOTE_HEIGHT: f64 = 36.0;
const NOTE_HEIGHT_BR: f64 = 36.0;
const NOTE_GAP: f64 = 14.0;
const FONT_SIZE: f64 = theme::FONT_SIZE;

pub struct SequenceEngine<'a> {
    seq: &'a SequenceDiagram,
}

impl<'a> SequenceEngine<'a> {
    pub fn new(seq: &'a SequenceDiagram) -> Self {
        Self { seq }
    }
}

impl<'a> LayoutEngine for SequenceEngine<'a> {
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
        Ok(build_sequence_elements(self.seq, config))
    }
}

pub fn build_sequence_elements(seq: &SequenceDiagram, config: &OutputConfig) -> Vec<SceneNode> {
    let mut elements = Vec::new();

    if seq.participants.is_empty() {
        return elements;
    }

    // ---- 计算各参与者的列宽和中心 x ----
    let name_to_idx: std::collections::HashMap<&str, usize> = seq
        .participants
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.as_str(), i))
        .collect();

    let mut col_widths: Vec<f64> = Vec::with_capacity(seq.participants.len());
    for p in &seq.participants {
        let display_name = p.alias.as_deref().unwrap_or(&p.name);
        let text_style = vir::text_style(
            theme::sequence::TEXT,
            FONT_SIZE,
            theme::FONT_FAMILY,
            TextAlign::Left,
            TextBaseline::Top,
        );
        let layout = layout_text(
            &[RichSpan::new(display_name.to_string(), text_style.clone())],
            None,
        );
        let text_w = layout.width;
        col_widths.push(BOX_MIN_WIDTH.max(text_w + PAD_X * 2.0));
    }

    let start_x = 20.0;

    let mut col_centers: Vec<f64> = Vec::with_capacity(seq.participants.len());
    let mut cur_x = start_x;
    for w in &col_widths {
        col_centers.push(cur_x + w / 2.0);
        cur_x += w + COL_GAP;
    }

    let box_top = 20.0;
    let box_bottom = box_top + BOX_HEIGHT;

    // ---- 绘制参与者盒子 ----
    for (i, p) in seq.participants.iter().enumerate() {
        let cx = col_centers[i];
        let bw = col_widths[i];
        let display_name = p.alias.as_deref().unwrap_or(&p.name);

        let rect = lievisual::geometry::Rect::new(cx - bw / 2.0, box_top, bw, box_bottom - box_top);
        elements.push(
            SceneNode::from(Element::RoundedRect {
                rect,
                radius: theme::NODE_RADIUS,
                style: vir::fs_both(
                    theme::sequence::ACTOR_FILL,
                    theme::sequence::ACTOR_STROKE,
                    2.0,
                ),
            })
            .with_z(Z_SERIES),
        );

        let ts = vir::text_style(
            theme::sequence::TEXT,
            FONT_SIZE,
            theme::FONT_FAMILY,
            TextAlign::Left,
            TextBaseline::Top,
        );
        let layout = layout_text(
            &[RichSpan::new(display_name.to_string(), ts)],
            Some(bw - 8.0),
        );
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
        elements.push(vir::text_node(
            display_name.to_string(),
            Point::new(cx + x_off, (box_top + box_bottom) / 2.0 + y_off),
            vir::text_style(
                theme::sequence::TEXT,
                FONT_SIZE,
                theme::FONT_FAMILY,
                TextAlign::Center,
                TextBaseline::Middle,
            ),
            0.0,
            Some(bw - 8.0),
            Z_LABEL,
        ));
    }

    // ---- 收集有序的语句行（含分组块），统一计算垂直布局 ----
    const BLOCK_LABEL_H: f64 = 24.0;
    const BLOCK_INDENT: f64 = 24.0;
    const BLOCK_PAD: f64 = 10.0;

    struct SeqRow {
        kind: u8, // 0=message, 1=note
        fi: usize,
        ti: usize,
        y: f64,
        text: String,
        arrow: MessageArrow,
        placement: NotePlacement,
        targets: Vec<usize>,
        depth: usize,
    }
    struct SeqBlock {
        y_top: f64,
        y_bottom: f64,
        depth: usize,
        label: String,
        kind: SequenceBlockKind,
    }

    let mut rows: Vec<SeqRow> = Vec::new();
    let mut blocks: Vec<SeqBlock> = Vec::new();
    let mut cur_y = box_bottom + 30.0;

    fn push_statement(
        stmts: &[SequenceItem],
        depth: usize,
        name_to_idx: &std::collections::HashMap<&str, usize>,
        rows: &mut Vec<SeqRow>,
        blocks: &mut Vec<SeqBlock>,
        cur_y: &mut f64,
        block_label_h: f64,
        msg_spacing: f64,
        note_h: f64,
        note_gap: f64,
    ) {
        for item in stmts {
            match item {
                SequenceItem::Message(msg) => {
                    let fi = name_to_idx.get(msg.from.as_str()).copied().unwrap_or(0);
                    let ti = name_to_idx.get(msg.to.as_str()).copied().unwrap_or(0);
                    rows.push(SeqRow {
                        kind: 0,
                        fi,
                        ti,
                        y: *cur_y,
                        text: msg.text.clone().unwrap_or_default(),
                        arrow: msg.arrow,
                        placement: NotePlacement::Over,
                        targets: Vec::new(),
                        depth,
                    });
                    *cur_y += msg_spacing;
                }
                SequenceItem::Note(note) => {
                    let indices: Vec<usize> = note
                        .targets
                        .iter()
                        .map(|t| name_to_idx.get(t.as_str()).copied().unwrap_or(0))
                        .collect();
                    rows.push(SeqRow {
                        kind: 1,
                        fi: 0,
                        ti: 0,
                        y: *cur_y,
                        text: note.text.clone(),
                        arrow: MessageArrow::Solid,
                        placement: note.placement,
                        targets: indices,
                        depth,
                    });
                    *cur_y += note_h + note_gap;
                }
                SequenceItem::Block(block) => {
                    let y_top = *cur_y;
                    *cur_y += block_label_h;
                    let label = block_label(block);
                    push_statement(
                        &block.items,
                        depth + 1,
                        name_to_idx,
                        rows,
                        blocks,
                        cur_y,
                        block_label_h,
                        msg_spacing,
                        note_h,
                        note_gap,
                    );
                    *cur_y += BLOCK_PAD;
                    blocks.push(SeqBlock {
                        y_top,
                        y_bottom: *cur_y,
                        depth,
                        label,
                        kind: block.kind.clone(),
                    });
                }
            }
        }
    }

    fn block_label(block: &SequenceBlock) -> String {
        let prefix = match block.kind {
            SequenceBlockKind::Loop => "loop",
            SequenceBlockKind::Alt => "alt",
            SequenceBlockKind::Opt => "opt",
            SequenceBlockKind::Par => "par",
        };
        match &block.label {
            Some(l) if !l.trim().is_empty() => format!("{} [{}]", prefix, l.trim()),
            _ => prefix.to_string(),
        }
    }

    for stmt in &seq.statements {
        match stmt {
            SequenceStatement::Message(msg) => {
                let fi = name_to_idx.get(msg.from.as_str()).copied().unwrap_or(0);
                let ti = name_to_idx.get(msg.to.as_str()).copied().unwrap_or(0);
                rows.push(SeqRow {
                    kind: 0,
                    fi,
                    ti,
                    y: cur_y,
                    text: msg.text.clone().unwrap_or_default(),
                    arrow: msg.arrow,
                    placement: NotePlacement::Over,
                    targets: Vec::new(),
                    depth: 0,
                });
                cur_y += MESSAGE_SPACING;
            }
            SequenceStatement::Note(note) => {
                let indices: Vec<usize> = note
                    .targets
                    .iter()
                    .map(|t| name_to_idx.get(t.as_str()).copied().unwrap_or(0))
                    .collect();
                rows.push(SeqRow {
                    kind: 1,
                    fi: 0,
                    ti: 0,
                    y: cur_y,
                    text: note.text.clone(),
                    arrow: MessageArrow::Solid,
                    placement: note.placement,
                    targets: indices,
                    depth: 0,
                });
                cur_y += NOTE_HEIGHT_BR;
            }
            SequenceStatement::Block(block) => {
                let y_top = cur_y;
                cur_y += BLOCK_LABEL_H;
                let label = block_label(block);
                push_statement(
                    &block.items,
                    1,
                    &name_to_idx,
                    &mut rows,
                    &mut blocks,
                    &mut cur_y,
                    BLOCK_LABEL_H,
                    MESSAGE_SPACING,
                    NOTE_HEIGHT_BR,
                    NOTE_GAP,
                );
                cur_y += BLOCK_PAD;
                blocks.push(SeqBlock {
                    y_top,
                    y_bottom: cur_y,
                    depth: 0,
                    label,
                    kind: block.kind.clone(),
                });
            }
        }
    }

    // ---- 生命线（虚线分段绘制） ----
    let lifeline_bottom = cur_y;
    for cx in &col_centers {
        let mut y = box_bottom;
        while y < lifeline_bottom {
            let end = (y + LIFELINE_DASH).min(lifeline_bottom);
            elements.push(vir::line_node(
                Point::new(*cx, y),
                Point::new(*cx, end),
                vir::stroke(theme::sequence::LIFELINE, 0.0),
                Z_AXIS,
            ));
            y += LIFELINE_DASH;
        }
    }

    // ---- 绘制分组块边框 + 标签 ----
    for b in &blocks {
        let x0 = col_centers[0] - col_widths[0] / 2.0 - BLOCK_PAD + b.depth as f64 * BLOCK_INDENT;
        let x1 = *col_centers.last().unwrap()
            + col_widths.last().unwrap() / 2.0
            + BLOCK_PAD
            + b.depth as f64 * BLOCK_INDENT;
        let rect = lievisual::geometry::Rect::new(x0, b.y_top, x1 - x0, b.y_bottom - b.y_top);
        elements.push(
            SceneNode::from(Element::RoundedRect {
                rect,
                radius: theme::NODE_RADIUS,
                style: vir::fs_both(
                    theme::sequence::BLOCK_FILL,
                    theme::sequence::BLOCK_STROKE,
                    1.0,
                ),
            })
            .with_z(Z_AXIS),
        );
        // 标签
        let ts = vir::text_style(
            theme::sequence::BLOCK_TEXT,
            FONT_SIZE * 0.85,
            theme::FONT_FAMILY,
            TextAlign::Left,
            TextBaseline::Top,
        );
        let layout = layout_text(&[RichSpan::new(b.label.clone(), ts.clone())], None);
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Left, TextBaseline::Middle);
        elements.push(vir::text_node(
            b.label.clone(),
            Point::new(x0 + 8.0 + x_off, b.y_top + BLOCK_LABEL_H / 2.0 + y_off),
            vir::text_style(
                theme::sequence::BLOCK_TEXT,
                FONT_SIZE * 0.85,
                theme::FONT_FAMILY,
                TextAlign::Left,
                TextBaseline::Top,
            ),
            0.0,
            None,
            Z_LABEL,
        ));
    }

    // ---- 绘制消息 ----
    for r in &rows {
        if r.kind == 1 {
            continue; // 备注稍后统一绘制
        }
        let base_from = col_centers[r.fi];
        let base_to = col_centers[r.ti];
        let from_x = base_from + r.depth as f64 * BLOCK_INDENT;
        let to_x = base_to + r.depth as f64 * BLOCK_INDENT;
        let arrow_y = r.y + MESSAGE_SPACING * 0.3;

        if r.fi == r.ti {
            elements.push(vir::polyline_node(
                vec![Point::new(from_x, arrow_y)],
                vir::stroke(theme::sequence::EDGE, 1.5),
                Z_AXIS,
            ));
        } else {
            let dir = if to_x > from_x { 1.0 } else { -1.0 };
            let stroke = vir::stroke(theme::sequence::EDGE, 1.5);

            elements.push(vir::line_node(
                Point::new(from_x, arrow_y),
                Point::new(to_x, arrow_y),
                stroke.clone(),
                Z_AXIS,
            ));

            match r.arrow {
                MessageArrow::Solid | MessageArrow::Dashed => {}
                MessageArrow::SolidTip | MessageArrow::DashedTip => {
                    let sz = 8.0;
                    elements.push(vir::line_node(
                        Point::new(to_x, arrow_y),
                        Point::new(to_x - dir * sz, arrow_y - sz * 0.5),
                        stroke.clone(),
                        Z_AXIS,
                    ));
                    elements.push(vir::line_node(
                        Point::new(to_x, arrow_y),
                        Point::new(to_x - dir * sz, arrow_y + sz * 0.5),
                        stroke,
                        Z_AXIS,
                    ));
                }
                MessageArrow::Cross => {
                    let sz = 5.0;
                    let tip_x = to_x - dir * sz;
                    elements.push(vir::line_node(
                        Point::new(tip_x, arrow_y - sz),
                        Point::new(tip_x, arrow_y + sz),
                        stroke,
                        Z_AXIS,
                    ));
                }
                MessageArrow::Open => {
                    let sz = 8.0;
                    elements.push(vir::line_node(
                        Point::new(to_x - dir * sz, arrow_y - sz * 0.5),
                        Point::new(to_x, arrow_y),
                        stroke.clone(),
                        Z_AXIS,
                    ));
                    elements.push(vir::line_node(
                        Point::new(to_x - dir * sz, arrow_y + sz * 0.5),
                        Point::new(to_x, arrow_y),
                        stroke,
                        Z_AXIS,
                    ));
                }
                MessageArrow::Both => {
                    // 终点三角箭头
                    let sz = 8.0;
                    elements.push(vir::line_node(
                        Point::new(to_x, arrow_y),
                        Point::new(to_x - dir * sz, arrow_y - sz * 0.5),
                        stroke.clone(),
                        Z_AXIS,
                    ));
                    elements.push(vir::line_node(
                        Point::new(to_x, arrow_y),
                        Point::new(to_x - dir * sz, arrow_y + sz * 0.5),
                        stroke.clone(),
                        Z_AXIS,
                    ));
                    // 起点反向箭头
                    elements.push(vir::line_node(
                        Point::new(from_x, arrow_y),
                        Point::new(from_x + dir * sz, arrow_y - sz * 0.5),
                        stroke.clone(),
                        Z_AXIS,
                    ));
                    elements.push(vir::line_node(
                        Point::new(from_x, arrow_y),
                        Point::new(from_x + dir * sz, arrow_y + sz * 0.5),
                        stroke,
                        Z_AXIS,
                    ));
                }
            }
        }

        // 消息文本
        if !r.text.is_empty() {
            let mid_x = (from_x + to_x) / 2.0;
            let ts = vir::text_style(
                theme::sequence::TEXT,
                FONT_SIZE,
                theme::FONT_FAMILY,
                TextAlign::Left,
                TextBaseline::Top,
            );
            let layout = layout_text(&[RichSpan::new(r.text.clone(), ts.clone())], Some(200.0));
            let (x_off, y_off) =
                compute_text_offset(&layout, TextAlign::Center, TextBaseline::Bottom);
            elements.push(vir::text_node(
                r.text.clone(),
                Point::new(mid_x + x_off, arrow_y - 4.0 + y_off),
                vir::text_style(
                    theme::sequence::TEXT,
                    FONT_SIZE,
                    theme::FONT_FAMILY,
                    TextAlign::Left,
                    TextBaseline::Top,
                ),
                0.0,
                Some(200.0),
                Z_LABEL,
            ));
        }
    }

    // ---- 绘制备注 ----
    for r in &rows {
        if r.kind != 1 {
            continue;
        }
        let note_y = r.y + 5.0;

        let (min_ref_x, max_ref_x) = if r.targets.is_empty() {
            (0.0, 0.0)
        } else {
            let mn = r
                .targets
                .iter()
                .map(|i| col_centers[*i] - col_widths[*i] / 2.0)
                .fold(f64::MAX, f64::min);
            let mx = r
                .targets
                .iter()
                .map(|i| col_centers[*i] + col_widths[*i] / 2.0)
                .fold(f64::MIN, f64::max);
            (mn, mx)
        };

        let (nx, nw) = match r.placement {
            NotePlacement::LeftOf => {
                let w = (min_ref_x - 10.0).clamp(100.0, 200.0);
                (min_ref_x - w - 10.0, w)
            }
            NotePlacement::RightOf => {
                let w = (config.width - max_ref_x - 10.0).clamp(100.0, 200.0);
                (max_ref_x + 10.0, w)
            }
            NotePlacement::Over => {
                let span = max_ref_x - min_ref_x;
                let w = (span + 30.0).max(160.0);
                (min_ref_x - (w - span) / 2.0, w)
            }
        };

        let note_rect = lievisual::geometry::Rect::new(nx, note_y, nw, NOTE_HEIGHT_BR);
        elements.push(
            SceneNode::from(Element::RoundedRect {
                rect: note_rect,
                radius: theme::NODE_RADIUS,
                style: vir::fs_both(
                    theme::sequence::NOTE_FILL,
                    theme::sequence::NOTE_STROKE,
                    1.5,
                ),
            })
            .with_z(Z_SERIES),
        );

        let ts = vir::text_style(
            theme::sequence::TEXT,
            FONT_SIZE,
            theme::FONT_FAMILY,
            TextAlign::Left,
            TextBaseline::Top,
        );
        let layout = layout_text(
            &[RichSpan::new(r.text.clone(), ts.clone())],
            Some(nw - 10.0),
        );
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
        elements.push(vir::text_node(
            r.text.clone(),
            Point::new(nx + nw / 2.0 + x_off, note_y + NOTE_HEIGHT_BR / 2.0 + y_off),
            vir::text_style(
                theme::sequence::TEXT,
                FONT_SIZE,
                theme::FONT_FAMILY,
                TextAlign::Left,
                TextBaseline::Top,
            ),
            0.0,
            Some(nw - 10.0),
            Z_LABEL,
        ));
    }

    elements
}
