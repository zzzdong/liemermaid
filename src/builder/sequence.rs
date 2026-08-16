use vello_cpu::kurbo::Point;

use crate::{
    ast::{MessageArrow, NotePlacement, SequenceDiagram},
    builder::{layout::types::LayoutEngine, types::OutputConfig},
    error::DiagramResult,
    text::{compute_text_offset, create_text_layout},
    visual::{
        FillStrokeStyle, StrokeStyle, TextAlign, TextBaseline, TextStyle, VisualElement, Z_AXIS,
        Z_LABEL, Z_SERIES, theme,
    },
};

const BOX_HEIGHT: f64 = 40.0;
const BOX_MIN_WIDTH: f64 = 80.0;
const PAD_X: f64 = 16.0;
const COL_GAP: f64 = 40.0;
const LIFELINE_DASH: f64 = 6.0;
const LIFELINE_GAP: f64 = 4.0;
const MESSAGE_SPACING: f64 = 50.0;
const NOTE_HEIGHT: f64 = 36.0;
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
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<VisualElement>> {
        Ok(build_sequence_elements(self.seq, config))
    }
}

pub fn build_sequence_elements(seq: &SequenceDiagram, config: &OutputConfig) -> Vec<VisualElement> {
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
        let text_style = TextStyle {
            font_size: FONT_SIZE,
            align: TextAlign::Center,
            vertical_align: TextBaseline::Middle,
            ..Default::default()
        };
        let layout = create_text_layout(display_name, &text_style, None);
        let text_w = layout.width() as f64;
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

        let rect = vello_cpu::kurbo::Rect::new(cx - bw / 2.0, box_top, cx + bw / 2.0, box_bottom);
        elements.push(VisualElement::Rect {
            rect,
            radius: Some(theme::NODE_RADIUS),
            style: FillStrokeStyle::new()
                .with_fill(theme::sequence::ACTOR_FILL)
                .with_stroke(theme::sequence::ACTOR_STROKE, 2.0),
            z_index: Z_SERIES,
        });

        let ts = TextStyle {
            font_size: FONT_SIZE,
            font_family: theme::FONT_FAMILY.to_string(),
            align: TextAlign::Center,
            vertical_align: TextBaseline::Middle,
            color: theme::sequence::TEXT,
            ..Default::default()
        };
        let layout = create_text_layout(display_name, &ts, Some(bw - 8.0));
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
        elements.push(VisualElement::TextRun {
            text: display_name.to_string(),
            position: Point::new(cx + x_off, (box_top + box_bottom) / 2.0 + y_off),
            style: TextStyle {
                align: TextAlign::Left,
                vertical_align: TextBaseline::Top,
                ..ts
            },
            rotation: 0.0,
            max_width: Some(bw - 8.0),
            layout: Some(Box::new(layout)),
            z_index: Z_LABEL,
        });
    }

    // ---- 计算消息/备注占用的垂直空间 ----
    let mut message_entries: Vec<(usize, usize, f64, String, MessageArrow)> = Vec::new();
    let mut note_entries: Vec<(f64, String, NotePlacement, Vec<usize>)> = Vec::new();

    let mut cur_y = box_bottom + 30.0;

    // 交错排列消息和备注（按输入顺序）
    // 先收集所有元素的 y 位置
    for msg in &seq.messages {
        let fi = name_to_idx.get(msg.from.as_str()).copied().unwrap_or(0);
        let ti = name_to_idx.get(msg.to.as_str()).copied().unwrap_or(0);
        message_entries.push((
            fi,
            ti,
            cur_y,
            msg.text.clone().unwrap_or_default(),
            msg.arrow,
        ));
        cur_y += MESSAGE_SPACING;
    }

    for note in &seq.notes {
        let indices: Vec<usize> = note
            .targets
            .iter()
            .map(|t| name_to_idx.get(t.as_str()).copied().unwrap_or(0))
            .collect();
        note_entries.push((cur_y, note.text.clone(), note.placement, indices));
        cur_y += NOTE_HEIGHT + NOTE_GAP;
    }

    // ---- 生命线（虚线分段绘制） ----
    let lifeline_bottom = cur_y;
    for cx in &col_centers {
        let mut y = box_bottom;
        while y < lifeline_bottom {
            let end = (y + LIFELINE_DASH).min(lifeline_bottom);
            elements.push(VisualElement::Line {
                start: Point::new(*cx, y),
                end: Point::new(*cx, end),
                style: StrokeStyle {
                    color: theme::sequence::LIFELINE,
                    width: 1.5,
                },
                z_index: Z_AXIS,
            });
            y = end + LIFELINE_GAP;
        }
    }

    // ---- 绘制消息 ----
    for (fi, ti, y, text, arrow) in &message_entries {
        let from_x = col_centers[*fi];
        let to_x = col_centers[*ti];
        let arrow_y = *y + MESSAGE_SPACING * 0.3;

        if fi == ti {
            let loop_x = from_x + 20.0;
            elements.push(VisualElement::Polyline {
                points: vec![
                    Point::new(from_x, arrow_y),
                    Point::new(loop_x, arrow_y),
                    Point::new(loop_x, arrow_y + 10.0),
                    Point::new(from_x, arrow_y + 10.0),
                ],
                style: StrokeStyle {
                    color: theme::sequence::EDGE,
                    width: 1.5,
                },
                z_index: Z_AXIS,
            });
        } else {
            let dir = if to_x > from_x { 1.0 } else { -1.0 };
            let stroke = StrokeStyle {
                color: theme::sequence::EDGE,
                width: 1.5,
            };

            elements.push(VisualElement::Line {
                start: Point::new(from_x, arrow_y),
                end: Point::new(to_x, arrow_y),
                style: stroke.clone(),
                z_index: Z_AXIS,
            });

            match arrow {
                MessageArrow::Solid | MessageArrow::Dashed => {}
                MessageArrow::SolidTip | MessageArrow::DashedTip => {
                    let sz = 8.0;
                    elements.push(VisualElement::Line {
                        start: Point::new(to_x, arrow_y),
                        end: Point::new(to_x - dir * sz, arrow_y - sz * 0.5),
                        style: stroke.clone(),
                        z_index: Z_AXIS,
                    });
                    elements.push(VisualElement::Line {
                        start: Point::new(to_x, arrow_y),
                        end: Point::new(to_x - dir * sz, arrow_y + sz * 0.5),
                        style: stroke,
                        z_index: Z_AXIS,
                    });
                }
                MessageArrow::Cross => {
                    let sz = 5.0;
                    let tip_x = to_x - dir * sz;
                    elements.push(VisualElement::Line {
                        start: Point::new(tip_x, arrow_y - sz),
                        end: Point::new(tip_x, arrow_y + sz),
                        style: stroke,
                        z_index: Z_AXIS,
                    });
                }
                MessageArrow::Open => {
                    let sz = 8.0;
                    elements.push(VisualElement::Line {
                        start: Point::new(to_x - dir * sz, arrow_y - sz * 0.5),
                        end: Point::new(to_x, arrow_y),
                        style: stroke.clone(),
                        z_index: Z_AXIS,
                    });
                    elements.push(VisualElement::Line {
                        start: Point::new(to_x - dir * sz, arrow_y + sz * 0.5),
                        end: Point::new(to_x, arrow_y),
                        style: stroke,
                        z_index: Z_AXIS,
                    });
                }
            }
        }

        // 消息文本
        if !text.is_empty() {
            let mid_x = (from_x + to_x) / 2.0;
            let ts = TextStyle {
                font_size: 12.0,
                font_family: theme::FONT_FAMILY.to_string(),
                align: TextAlign::Center,
                vertical_align: TextBaseline::Bottom,
                color: theme::sequence::TEXT,
                ..Default::default()
            };
            let layout = create_text_layout(text, &ts, Some(200.0));
            let (x_off, y_off) =
                compute_text_offset(&layout, TextAlign::Center, TextBaseline::Bottom);
            elements.push(VisualElement::TextRun {
                text: text.clone(),
                position: Point::new(mid_x + x_off, arrow_y - 4.0 + y_off),
                style: TextStyle {
                    align: TextAlign::Left,
                    vertical_align: TextBaseline::Top,
                    ..ts
                },
                rotation: 0.0,
                max_width: Some(200.0),
                layout: Some(Box::new(layout)),
                z_index: Z_LABEL,
            });
        }
    }

    // ---- 绘制备注 ----
    for (y, text, placement, target_indices) in &note_entries {
        let note_y = *y + 5.0;

        let (min_ref_x, max_ref_x) = if target_indices.is_empty() {
            (0.0, 0.0)
        } else {
            let mn = target_indices
                .iter()
                .map(|i| col_centers[*i] - col_widths[*i] / 2.0)
                .fold(f64::MAX, f64::min);
            let mx = target_indices
                .iter()
                .map(|i| col_centers[*i] + col_widths[*i] / 2.0)
                .fold(f64::MIN, f64::max);
            (mn, mx)
        };

        let (nx, nw) = match placement {
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

        let note_rect = vello_cpu::kurbo::Rect::new(nx, note_y, nx + nw, note_y + NOTE_HEIGHT);
        elements.push(VisualElement::Rect {
            rect: note_rect,
            radius: Some(theme::NODE_RADIUS),
            style: FillStrokeStyle::new()
                .with_fill(theme::sequence::NOTE_FILL)
                .with_stroke(theme::sequence::NOTE_STROKE, 1.5),
            z_index: Z_SERIES,
        });

        let ts = TextStyle {
            font_size: 12.0,
            font_family: theme::FONT_FAMILY.to_string(),
            align: TextAlign::Center,
            vertical_align: TextBaseline::Middle,
            color: theme::sequence::TEXT,
            ..Default::default()
        };
        let layout = create_text_layout(text, &ts, Some(nw - 10.0));
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
        elements.push(VisualElement::TextRun {
            text: text.clone(),
            position: Point::new(nx + nw / 2.0 + x_off, note_y + NOTE_HEIGHT / 2.0 + y_off),
            style: TextStyle {
                align: TextAlign::Left,
                vertical_align: TextBaseline::Top,
                ..ts
            },
            rotation: 0.0,
            max_width: Some(nw - 10.0),
            layout: Some(Box::new(layout)),
            z_index: Z_LABEL,
        });
    }

    elements
}
