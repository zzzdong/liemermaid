//! Stage 1.5: Measure —— 测量所有文本，把尺寸写回 UG 得到 UG'。
//!
//! 必须在 Layout 之前完成：solver（分层 / 网格 / 泳道）需要节点包围盒才能排布。
//! 节点标签经 lievisual 文本测量 + [`ShapeKind`] 几何约束推算尺寸；
//! 边标签（P0.3 暂未使用）留待 P1.3。
//!
//! 字体尺寸等样式常量直接引用 `crate::builder::theme` 的 const（现状无 Theme 结构体，
//! 见 redesign-task-plan.md §0 事实核对），故无需传 theme 参数。

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

/// 测量 UG 中所有节点标签，返回带尺寸的 UG'（结构同 UG，label 转为 Measured）。
pub fn measure_all(ug: ir::Unigraph) -> ir::Unigraph {
    let nodes = ug
        .nodes
        .into_iter()
        .map(|mut n| {
            if let LabelOrMeasured::Spec(spec) = &n.label {
                let text = spec.text.clone();
                let text_style = TextStyle::new(Color::BLACK, FONT_SIZE, FONT_FAMILY)
                    .with_align(TextAlign::Center)
                    .with_baseline(TextBaseline::Middle);
                let layout = layout_text(&[RichSpan::new(text.clone(), text_style.clone())], None);
                let text_w = layout.width;
                let text_h = layout.height;

                // P0.3 简化：所有形状用默认矩形尺寸推算；
                // 圆/菱形/圆柱等精确几何约束留 P1.3（measure 细化）。
                let size = Size::new(
                    MIN_NODE_WIDTH.max(text_w + 2.0 * NODE_PAD_X),
                    MIN_NODE_HEIGHT.max(text_h + 2.0 * NODE_PAD_Y),
                );

                n.label = LabelOrMeasured::Measured(ir::common::MeasuredLabel {
                    text,
                    spans: vec![RichSpan::new(spec.text.clone(), text_style)],
                    layout,
                    size,
                });
            }
            n
        })
        .collect();

    ir::Unigraph {
        family: ug.family,
        direction: ug.direction,
        nodes,
        edges: ug.edges,
        meta: ug.meta,
    }
}
