//! Stage 1.5: Measure —— 测量所有文本，把尺寸写回 UG 得到 UG'。
//!
//! 必须在 Layout 之前完成：solver（分层 / 网格 / 泳道）需要节点包围盒才能排布。
//! 节点标签经 lievisual 文本测量 + [`ShapeKind`](crate::builder::ir::shape::ShapeKind)
//! 几何约束推算尺寸；边标签（`label_text`）同样在此测量，填入
//! [`UGEdge::label`](crate::builder::ir::unigraph::UGEdge::label)。
//!
//! 字体尺寸等样式常量直接引用 `crate::builder::theme` 的 const（现状无 Theme 结构体）。

use lievisual::geometry::{Color, Size};
use lievisual::text::{RichSpan, TextAlign, TextBaseline, TextStyle, layout_text};

use crate::builder::ir::{self, common::*};
use crate::builder::theme;

const MIN_NODE_WIDTH: f64 = theme::NODE_MIN_W;
const MIN_NODE_HEIGHT: f64 = theme::NODE_MIN_H;
const NODE_PAD_X: f64 = theme::NODE_PAD_X;
const NODE_PAD_Y: f64 = theme::NODE_PAD_Y;
const FONT_SIZE: f64 = theme::FONT_SIZE;

/// 带官方行高的文本样式（见 [`theme::text_style`]）。
fn text_style(size: f64, align: TextAlign, baseline: TextBaseline) -> TextStyle {
    theme::text_style(Color::BLACK, size, align, baseline)
}

/// 测量 UG 中所有节点与边标签，返回带尺寸的 UG'（结构同 UG，label 转为 Measured）。
pub fn measure_all(mut ug: ir::Unigraph) -> ir::Unigraph {
    ug.nodes = ug
        .nodes
        .into_iter()
        .map(|mut n| {
            let shape = n.shape;
            let hint = n.size_hint.clone();
            // 结构化节点（类框 / 实体框 / 时间轴列 / 备注）走多栏测量；
            // PieSlice / GitCommit 是普通单标签节点（几何由 materialize 从数据计算）。
            n.label = match &n.detail {
                NodeDetail::None | NodeDetail::PieSlice { .. } | NodeDetail::GitCommit { .. } => {
                    measure_node_label(&n.label, shape, hint)
                }
                detail => measure_structured_label(&n.label, detail),
            };
            n
        })
        .collect();

    // 边标签：measure 阶段统一测量（label_text → MeasuredLabel）。
    for e in ug.edges.iter_mut() {
        if let Some(text) = e.label_text.clone() {
            let style = text_style(FONT_SIZE, TextAlign::Center, TextBaseline::Middle);
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
        let style = text_style(FONT_SIZE, TextAlign::Center, TextBaseline::Middle);
        let layout = layout_text(&[RichSpan::new(text.clone(), style.clone())], None);
        return LabelOrMeasured::Measured(MeasuredLabel {
            text: text.clone(),
            spans: vec![RichSpan::new(text, style)],
            layout,
            size: fixed,
        });
    }

    let text_style = text_style(FONT_SIZE, TextAlign::Center, TextBaseline::Middle);
    let layout = layout_text(&[RichSpan::new(text.clone(), text_style.clone())], None);
    let text_w = layout.width;
    let text_h = layout.height;

    // 节点尺寸由文字排版结果决定；不同形状有各自几何约束，必须在测量阶段落实。
    // 显式 padding（`SizeHint::Padded`）优先，否则取形状自带 padding 表。
    let (pad_x, pad_y, square) = match size_hint {
        SizeHint::Padded { pad_x, pad_y } => (pad_x, pad_y, false),
        _ => shape_padding(shape),
    };
    let size = if shape == ir::shape::ShapeKind::Cylinder {
        // 圆柱：顶部/底部各有一段椭圆弧（占 10% 高），正文区仍需容纳文字 + padding。
        let w = text_w + 2.0 * pad_x;
        let h = (text_h + 2.0 * pad_y) / 0.8;
        Size::new(w, h)
    } else if square {
        // 圆 / 双圆 / 菱形：强制正方形，边长 = 文字最大维度 + 两侧 padding。
        let side = text_w.max(text_h) + 2.0 * pad_x.max(pad_y);
        Size::new(side, side)
    } else {
        Size::new(text_w + 2.0 * pad_x, text_h + 2.0 * pad_y)
    };
    let size = Size::new(
        size.width.max(MIN_NODE_WIDTH),
        size.height.max(MIN_NODE_HEIGHT),
    );

    LabelOrMeasured::Measured(MeasuredLabel {
        text: text.clone(),
        spans: vec![RichSpan::new(text, text_style)],
        layout,
        size,
    })
}

// 类框 / 实体框的尺寸常量（对齐官方 mermaid 类图）。
const CLASS_PAD: f64 = 12.0;
const ENTITY_PAD: f64 = 18.0;
/// 类图成员 / 类名字号（官方 CSS: g.classGroup text{font-size:10px}）。
const SMALL_FONT: f64 = theme::class::MEMBER_FONT_SIZE;
/// 空栏占位高度（官方空类成员/方法栏各 18px）。
const SECTION_MIN_H: f64 = 18.0;
/// 成员/属性行高（16px 字号 × 1.5 行高）。
const ATTR_LINE_H: f64 = 24.0;
/// ER 实体属性 type 列与 name 列之间的间距。
const ER_ATTR_GAP: f64 = 32.0;

/// 结构化节点（类框 / 实体框）的多栏尺寸测量。
///
/// 类框 = header（类名 + 注解）+ attrs 栏 + methods 栏；实体框 = header + attrs 栏。
/// 返回的 MeasuredLabel：`size` = 整框总尺寸（供布局），`text`/`spans` = header 文本
/// （供 materialize 绘制标题），各栏文本由 [`NodeDetail`] 单独携带。
fn measure_structured_label(label: &LabelOrMeasured, detail: &NodeDetail) -> LabelOrMeasured {
    let LabelOrMeasured::Spec(spec) = label else {
        return label.clone();
    };
    let name = spec.text.clone();

    // 类名与成员同字号（官方 10px），颜色在 materialize 阶段注入。
    let header_style = text_style(SMALL_FONT, TextAlign::Center, TextBaseline::Middle);
    let header_layout = layout_text(&[RichSpan::new(name.clone(), header_style.clone())], None);

    let small_style = text_style(SMALL_FONT, TextAlign::Left, TextBaseline::Top);

    let (width, height) = match detail {
        NodeDetail::Class {
            annotation,
            attrs,
            methods,
        } => {
            let ann_layout = annotation.as_ref().map(|a| {
                layout_text(
                    &[RichSpan::new(format!("«{}»", a), small_style.clone())],
                    None,
                )
            });
            let header_h = header_layout.height
                + 24.0
                + ann_layout.as_ref().map(|l| l.height + 4.0).unwrap_or(0.0);

            let attr_h = if attrs.is_empty() {
                SECTION_MIN_H
            } else {
                attrs.len() as f64 * ATTR_LINE_H
            };
            let method_h = if methods.is_empty() {
                SECTION_MIN_H
            } else {
                methods.len() as f64 * ATTR_LINE_H
            };

            let mut max_w = header_layout.width + CLASS_PAD * 2.0;
            if let Some(al) = &ann_layout {
                max_w = max_w.max(al.width + CLASS_PAD * 2.0);
            }
            for line in attrs.iter().chain(methods.iter()) {
                let l = layout_text(&[RichSpan::new(line.clone(), small_style.clone())], None);
                max_w = max_w.max(l.width + CLASS_PAD * 2.0);
            }

            (max_w, header_h + attr_h + method_h)
        }
        NodeDetail::Entity { attrs } => {
            let header_h = header_layout.height + 20.0;
            let attr_h = (attrs.len() as f64 * ATTR_LINE_H).max(18.0);

            // 属性分 type / name / constraint 三列（官方 ER 属性继承根字号 16px）。
            let attr_style = text_style(FONT_SIZE, TextAlign::Left, TextBaseline::Top);
            let mut type_w = 0.0f64;
            let mut name_w = 0.0f64;
            let mut constraint_w = 0.0f64;
            for a in attrs {
                let tl = layout_text(&[RichSpan::new(a.type_.clone(), attr_style.clone())], None);
                let nl = layout_text(&[RichSpan::new(a.name.clone(), attr_style.clone())], None);
                type_w = type_w.max(tl.width);
                name_w = name_w.max(nl.width);
                if let Some(c) = &a.constraint {
                    let cl = layout_text(&[RichSpan::new(c.clone(), attr_style.clone())], None);
                    constraint_w = constraint_w.max(cl.width);
                }
            }
            let attrs_w = if attrs.is_empty() {
                0.0
            } else {
                type_w + ER_ATTR_GAP + name_w
                    + if constraint_w > 0.0 {
                        ER_ATTR_GAP + constraint_w
                    } else {
                        0.0
                    }
            };
            let max_w = header_layout.width.max(attrs_w) + ENTITY_PAD * 2.0;

            (max_w, header_h + attr_h)
        }
        NodeDetail::TimelineSection { events } => {
            // 时间轴列：尺寸覆盖「section 块（上）+ 事件块区（下）+ 时间点/连线留白」。
            // 具体视觉常量由 materialize 使用，这里仅给出能容纳全列内容的包围盒。
            let sec_w =
                layout_text(&[RichSpan::new(name.clone(), small_style.clone())], None).width;
            let mut max_ev = 0.0f64;
            for ev in events {
                let l = layout_text(&[RichSpan::new(ev.clone(), small_style.clone())], None);
                max_ev = max_ev.max(l.width);
            }
            let events_h =
                events.len() as f64 * (theme::timeline::BLOCK_H + theme::timeline::EVENT_GAP);
            (
                (sec_w.max(max_ev) + CLASS_PAD * 2.0).max(theme::timeline::BLOCK_W),
                theme::timeline::BLOCK_H + events_h + 40.0,
            )
        }
        NodeDetail::SequenceNote { text, .. } => {
            let tl = layout_text(&[RichSpan::new(text.clone(), small_style.clone())], None);
            (
                tl.width + CLASS_PAD * 2.0,
                tl.height.max(theme::timeline::BLOCK_H * 0.8) + CLASS_PAD * 2.0,
            )
        }
        NodeDetail::None | NodeDetail::PieSlice { .. } | NodeDetail::GitCommit { .. } => {
            // 调用处已按 None / PieSlice / GitCommit 分派到单标签测量，此处仅防御回退。
            let size = Size::new(
                header_layout.width + 2.0 * NODE_PAD_X,
                header_layout.height + 2.0 * NODE_PAD_Y,
            );
            return LabelOrMeasured::Measured(MeasuredLabel {
                text: name.clone(),
                spans: vec![RichSpan::new(name, header_style)],
                layout: header_layout,
                size,
            });
        }
    };

    LabelOrMeasured::Measured(MeasuredLabel {
        text: name.clone(),
        spans: vec![RichSpan::new(name, header_style)],
        layout: header_layout,
        size: Size::new(width, height),
    })
}

/// 形状 → `(pad_x, pad_y, square)`：节点包围盒 = 文本排版盒 + 各形状 padding。
///
/// `square = true` 时包围盒强制正方形（圆 / 双圆 / 菱形），边长取
/// `max(text_w, text_h) + 2 × max(pad_x, pad_y)`。
///
/// 数值**实测自官方 mermaid golden**（`tests/golden/golden/*.svg`），例如
/// `flowchart__shapes` 中 `Start` → 93.80×54（文本 33.80×24），即 rect padding
/// = (30, 15)；`Circle` → r=27.95（文本 40.89×24），即直径 = 文本宽 + 15。
fn shape_padding(shape: ir::shape::ShapeKind) -> (f64, f64, bool) {
    use ir::shape::ShapeKind as S;
    match shape {
        // rect / rounded：W = text_w + 60，H = text_h + 30。
        S::Rectangle | S::Rounded | S::QuadrantCell => (NODE_PAD_X, NODE_PAD_Y, false),
        // 圆 / 双圆：直径 = max(text_w, text_h) + 15。
        S::Circle | S::DoubleCircle | S::StartDot | S::EndDot => (7.5, 7.5, true),
        // 菱形：正方形包围盒，边长 = max(text_w, text_h) + 54。
        S::Diamond => (27.0, 27.0, true),
        // 圆柱（Database）：正文区同 rect，另加上下椭圆弧（见调用处 /0.8）。
        S::Cylinder => (NODE_PAD_X, NODE_PAD_Y, false),
        // 以下各形状官方高度统一为 text_h + 15、宽度各不同（实测 golden）。
        S::Stadium => (12.5, 7.5, false),
        S::Subroutine => (15.5, 7.5, false),
        S::Parallelogram | S::Trapezoid => (17.25, 7.5, false),
        S::Asymmetric => (17.25, 7.5, false),
        S::Hexagon => (25.0, 15.0, false),
        S::Bar => (0.0, 0.0, false),
        S::PieSlice => (0.0, 0.0, false),
    }
}
