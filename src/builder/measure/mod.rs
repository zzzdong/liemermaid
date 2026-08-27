//! Stage 1.5: Measure —— 测量所有文本，把尺寸写回 UG 得到 UG'。
//!
//! 必须在 Layout 之前完成：solver（分层 / 网格 / 泳道）需要节点包围盒才能排布。
//! 节点标签经 lievisual 文本测量 + [`ShapeKind`] 几何约束推算尺寸；
//! 边标签（`label_text`）同样在此测量，填入 [`UGEdge::label`]。
//!
//! 字体尺寸等样式常量直接引用 `crate::builder::theme` 的 const（现状无 Theme 结构体）。

use lievisual::geometry::{Color, Size};
use lievisual::text::{RichSpan, TextAlign, TextBaseline, TextStyle, layout_text};

use crate::builder::theme;
use crate::builder::ir::{self, common::*};

const MIN_NODE_WIDTH: f64 = theme::NODE_MIN_W;
const MIN_NODE_HEIGHT: f64 = theme::NODE_MIN_H;
const NODE_PAD_X: f64 = theme::NODE_PAD_X;
const NODE_PAD_Y: f64 = theme::NODE_PAD_Y;
const FONT_SIZE: f64 = theme::FONT_SIZE;
const FONT_FAMILY: &str = theme::FONT_FAMILY;

/// 测量 UG 中所有节点与边标签，返回带尺寸的 UG'（结构同 UG，label 转为 Measured）。
pub fn measure_all(mut ug: ir::Unigraph) -> ir::Unigraph {
    ug.nodes = ug
        .nodes
        .into_iter()
        .map(|mut n| {
            let shape = n.shape;
            let hint = n.size_hint.clone();
            n.label = measure_node_label(&n.label, shape, hint);
            n
        })
        .collect();

    // 边标签：measure 阶段统一测量（label_text → MeasuredLabel）。
    for e in ug.edges.iter_mut() {
        if let Some(text) = e.label_text.clone() {
            let style = TextStyle::new(Color::BLACK, FONT_SIZE, FONT_FAMILY)
                .with_align(TextAlign::Center)
                .with_baseline(TextBaseline::Middle);
            let layout = layout_text(&[RichSpan::new(text.clone(), style.clone())], None);
            let size = Size::new(layout.width, layout.height);
            e.label = Some(ir::common::MeasuredLabel {
                text: text.clone(),
                spans: vec![RichSpan::new(text, style)],
                layout,
                size,
            });
        }
    }

    ug
}

/// 测量单个节点标签：按 `SizeHint`（Fixed 优先）与 `ShapeKind` 几何约束推算包围盒。
fn measure_node_label(
    label: &LabelOrMeasured,
    shape: ir::shape::ShapeKind,
    size_hint: SizeHint,
) -> LabelOrMeasured {
    // 已测量（如第二次调用）：直接返回，避免重复测量。
    if let LabelOrMeasured::Measured(_) = label {
        return label.clone();
    }
    let LabelOrMeasured::Spec(spec) = label else {
        return label.clone();
    };
    let text = spec.text.clone();

    // 固定尺寸节点（state 的 start/end/bar）：直接给默认尺寸，不依赖文本。
    if let SizeHint::Fixed(fixed) = size_hint {
        let style = TextStyle::new(Color::BLACK, FONT_SIZE, FONT_FAMILY)
            .with_align(TextAlign::Center)
            .with_baseline(TextBaseline::Middle);
        let layout = layout_text(&[RichSpan::new(text.clone(), style.clone())], None);
        return LabelOrMeasured::Measured(MeasuredLabel {
            text: text.clone(),
            spans: vec![RichSpan::new(text, style)],
            layout,
            size: fixed,
        });
    }

    let text_style = TextStyle::new(Color::BLACK, FONT_SIZE, FONT_FAMILY)
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);
    let layout = layout_text(&[RichSpan::new(text.clone(), text_style.clone())], None);
    let text_w = layout.width;
    let text_h = layout.height;

    // 节点尺寸由文字排版结果决定；不同形状有各自几何约束，必须在测量阶段落实。
    let size = match shape {
        ir::shape::ShapeKind::Circle
        | ir::shape::ShapeKind::DoubleCircle
        | ir::shape::ShapeKind::StartDot
        | ir::shape::ShapeKind::EndDot => {
            // 圆 / 双圆 / start / end：强制正方形，直径 = 文字最大维度 + 留白。
            let pad = NODE_PAD_X;
            let d = (text_w.max(text_h) + 2.0 * pad).max(MIN_NODE_HEIGHT);
            Size::new(d, d)
        }
        ir::shape::ShapeKind::Stadium => {
            // 跑道形：两端半圆直径 = 高。
            let h = (text_h + 2.0 * NODE_PAD_Y).max(MIN_NODE_HEIGHT);
            let r = h / 2.0;
            let w = (text_w + 2.0 * r + 8.0).max(MIN_NODE_WIDTH);
            Size::new(w, h)
        }
        ir::shape::ShapeKind::Cylinder => {
            let ellipse = NODE_PAD_Y * 2.0;
            let h = (text_h + 2.0 * NODE_PAD_Y + ellipse).max(MIN_NODE_HEIGHT);
            let w = (text_w + 2.0 * NODE_PAD_X).max(MIN_NODE_WIDTH);
            Size::new(w, h)
        }
        ir::shape::ShapeKind::Bar => Size::new(100.0, 10.0),
        // 矩形类：按 shape 乘数 + padding。
        _ => {
            let (min_w, min_h, pad_x, pad_y) = shape_multiplier(shape);
            Size::new(
                min_w.max(text_w + 2.0 * pad_x),
                min_h.max(text_h + 2.0 * pad_y),
            )
        }
    };

    LabelOrMeasured::Measured(MeasuredLabel {
        text: text.clone(),
        spans: vec![RichSpan::new(text, text_style)],
        layout,
        size,
    })
}

/// 据 ShapeKind 返回 (min_w, min_h, pad_x, pad_y)，与旧 `layout/measure.rs` 的几何约束对齐。
fn shape_multiplier(shape: ir::shape::ShapeKind) -> (f64, f64, f64, f64) {
    use ir::shape::ShapeKind as S;
    match shape {
        S::Diamond => (
            MIN_NODE_WIDTH * 1.4,
            MIN_NODE_HEIGHT * 1.4,
            NODE_PAD_X * 1.6,
            NODE_PAD_Y * 1.6,
        ),
        S::Hexagon => (
            MIN_NODE_WIDTH * 1.3,
            MIN_NODE_HEIGHT * 1.2,
            NODE_PAD_X * 1.4,
            NODE_PAD_Y * 1.2,
        ),
        S::Cylinder => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT * 1.2,
            NODE_PAD_X * 1.2,
            NODE_PAD_Y * 1.5,
        ),
        S::Stadium => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.5,
            NODE_PAD_Y,
        ),
        S::Asymmetric => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.3,
            NODE_PAD_Y,
        ),
        S::Parallelogram => (
            MIN_NODE_WIDTH * 1.2,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.5,
            NODE_PAD_Y,
        ),
        S::Trapezoid => (
            MIN_NODE_WIDTH * 1.2,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.4,
            NODE_PAD_Y,
        ),
        _ => (MIN_NODE_WIDTH, MIN_NODE_HEIGHT, NODE_PAD_X, NODE_PAD_Y),
    }
}
