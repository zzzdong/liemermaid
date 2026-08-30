//! Stage 3: Materialize —— 视觉决策点（唯一消费主题常量的地方）。
//!
//! 消费 `Geograph` + `StyleIntent`，把"几何 + 样式意图"解析成具体颜色 / 线型，
//! 产出视觉自足的 [`SceneGraph`]。之后 `paint` 不再有任何 theme 依赖、不再有任何图类型判断。
//!
//! 支持 flowchart + state：节点按 [`ShapeKind`]（含 StartDot 实心圆 / EndDot 双环 /
//! Bar 横条），边按 [`EdgeKind`] 选 flowchart / state 主题，边标签绘制白底文本。

use std::collections::HashMap;

use lievisual::geometry::{Color, Point, Rect, Size};
use lievisual::scene::{Fill, LineCap, LineJoin, Stroke};
use lievisual::text::{FontWeight, RichSpan, TextAlign, TextBaseline, TextStyle};

use crate::builder::ir::{
    self, SceneGraph, SceneItem, StyleIntent,
    common::{ArrowKind, ArrowSpec},
    geograph::{GGNode, Geograph},
    shape::{EdgeEnds, ShapeGeometry, ShapeKind},
    unigraph::EdgeKind,
};
use crate::builder::theme;

const NODE_STROKE_WIDTH: f64 = 1.0;

/// 几何 + 视觉意图 → 视觉自足的场景图。
pub fn run(gg: &Geograph, _style: &StyleIntent) -> SceneGraph {
    let mut items = Vec::new();

    // —— pie（Radial 家族）：整体绘制（扇区 + 百分比标签 + 标题）——
    // PieSlice 节点存在即视为 pie，走专属渲染，不经过通用节点/边循环。
    let is_pie = gg
        .nodes
        .iter()
        .any(|n| matches!(n.detail, ir::common::NodeDetail::PieSlice { .. }));
    if is_pie {
        emit_pie(&mut items, gg);
        return SceneGraph {
            size: gg.size,
            background: gg.background,
            items,
        };
    }

    // —— gitgraph（Hierarchy 家族）：整体绘制（提交点 + 分支线 + 合并曲线 + 标签）——
    let is_gitgraph = gg
        .nodes
        .iter()
        .any(|n| matches!(n.detail, ir::common::NodeDetail::GitCommit { .. }));
    if is_gitgraph {
        emit_gitgraph(&mut items, gg);
        return SceneGraph {
            size: gg.size,
            background: gg.background,
            items,
        };
    }

    // —— sequence（Sequence 家族）：整体绘制（参与者盒 + 生命线 + 消息 + 备注 + 分组块）——
    // 参与者（Lifeline 节点）存在即视为 sequence，走专属渲染，不经过通用节点/边循环。
    let is_sequence = gg
        .nodes
        .iter()
        .any(|n| n.role == ir::common::NodeRole::Lifeline);
    if is_sequence {
        emit_sequence(&mut items, gg);
        return SceneGraph {
            size: gg.size,
            background: gg.background,
            items,
        };
    }

    // —— timeline（Linear 家族）：整体绘制（标题 + 时间轴 + 点 + 块 + 连线）——
    // TimelineSection 节点不参与普通节点渲染（下方循环中跳过）。
    let is_timeline = gg
        .nodes
        .iter()
        .any(|n| matches!(n.detail, ir::common::NodeDetail::TimelineSection { .. }));
    if is_timeline {
        emit_timeline(&mut items, gg);
    }

    // —— 子图容器：背景 + 边框 + 标题（z 低于节点，保证节点覆盖其上）——
    // flowchart subgraph：.cluster rect{fill:#ffffde;stroke:#aaaa33}
    // state 复合状态：.stateGroup .composit{fill:white} + stroke:#9370DB，标题在顶部中央。
    for c in &gg.containers {
        let r = c.bounds;
        let is_composite = c.kind == ir::common::ContainerKind::StateComposite;
        let (fill_color, stroke_color, title_align, title_pos) = if is_composite {
            (
                lievisual::geometry::Color::WHITE,
                theme::state::STROKE,
                TextAlign::Center,
                Point::new(r.min_x() + r.width() / 2.0, r.min_y() + 12.0),
            )
        } else {
            (
                theme::flowchart::SUBGRAPH_FILL,
                theme::flowchart::SUBGRAPH_STROKE,
                TextAlign::Left,
                Point::new(r.min_x() + 8.0, r.min_y() + 12.0),
            )
        };
        let container_stroke = Stroke {
            color: stroke_color,
            width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash_array: Vec::new(),
            dash_offset: 0.0,
            miter_limit: 4.0,
        };
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Rect {
                at: Point::new(r.min_x(), r.min_y()),
                size: Size::new(r.width(), r.height()),
            },
            fill: Some(Fill::Solid(fill_color)),
            stroke: Some(container_stroke.clone()),
            name: None,
            z: -1,
        });
        if let Some(title) = &c.title
            && !title.is_empty()
        {
            let title_style = theme::text_style(
                theme::TEXT_COLOR,
                theme::FONT_SIZE,
                title_align,
                TextBaseline::Middle,
            );
            items.push(SceneItem::Label {
                text: vec![lievisual::text::RichSpan::new(
                    title.clone(),
                    title_style.clone(),
                )],
                position: title_pos,
                style: title_style,
                anchor: ir::scenegraph::Anchor::Left,
                z: 0,
            });
        }
    }

    // —— 节点：形状 + 文本 ——
    for n in &gg.nodes {
        // 结构化节点（类框 / 实体框）走专用多栏绘制。
        // TimelineSection 由 emit_timeline 统一绘制（轴/点/块/连线）；SequenceNote
        // 属 Sequence 家族，引擎尚未接入（阶段 2 进行中），先回退到普通节点渲染占位。
        match &n.detail {
            ir::common::NodeDetail::Class { .. } => {
                emit_class_box(&mut items, n);
                continue;
            }
            ir::common::NodeDetail::Entity { .. } => {
                emit_entity_box(&mut items, n);
                continue;
            }
            ir::common::NodeDetail::TimelineSection { .. } => continue,
            ir::common::NodeDetail::SequenceNote { .. } => {}
            // pie / gitgraph 由 emit_pie / emit_gitgraph 整体绘制（本循环不可达，仅补穷尽）。
            ir::common::NodeDetail::PieSlice { .. } => {}
            ir::common::NodeDetail::GitCommit { .. } => {}
            ir::common::NodeDetail::None => {}
        }
        // 节点填充/描边按形状语义取色（state 与 flowchart 同色系，此处统一用 flowchart 常量）。
        let (fill, stroke) = node_fill_stroke(n.shape);
        match n.shape {
            // 双环（EndDot / DoubleCircle）：外圈描边 + 内圈实心小圆，各占一个 Shape。
            ShapeKind::EndDot | ShapeKind::DoubleCircle => {
                let outer = Stroke {
                    color: theme::state::STROKE,
                    width: 1.5,
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Miter,
                    dash_array: Vec::new(),
                    dash_offset: 0.0,
                    miter_limit: 4.0,
                };
                items.push(SceneItem::Shape {
                    geometry: ShapeGeometry::Ellipse {
                        center: n.center,
                        rx: n.size.width / 2.0,
                        ry: n.size.height / 2.0,
                    },
                    fill: None,
                    stroke: Some(outer),
                    name: None,
                    z: 0,
                });
                // 内圈实心小圆。
                let inner_r = (n.size.width / 2.0) * 0.6;
                items.push(SceneItem::Shape {
                    geometry: ShapeGeometry::Ellipse {
                        center: n.center,
                        rx: inner_r,
                        ry: inner_r,
                    },
                    fill: Some(Fill::Solid(theme::state::STROKE)),
                    stroke: None,
                    name: None,
                    z: 0,
                });
            }
            _ => {
                let geometry = shape_to_geometry(n.shape, n.center, n.size);
                items.push(SceneItem::Shape {
                    geometry,
                    fill: Some(fill),
                    stroke: Some(stroke),
                    name: None,
                    z: 0,
                });
            }
        }

        // 节点文本（Bar / StartDot / EndDot 无文本）。
        if let Some(label) = &n.label
            && !label.text.is_empty()
            && !matches!(n.shape, ShapeKind::Bar)
        {
            items.push(SceneItem::Label {
                text: label.spans.clone(),
                position: n.center,
                style: theme::text_style(
                    theme::flowchart::TEXT,
                    theme::FONT_SIZE,
                    TextAlign::Center,
                    TextBaseline::Middle,
                ),
                anchor: ir::scenegraph::Anchor::Center,
                z: 2,
            });
        }
    }

    // —— 边 ——
    for e in &gg.edges {
        // 线型决定颜色 / 宽度 / 虚线样式（Flow=实线，Dotted=虚线，Thick=粗线，Invisible=透明）。
        let (color, width) = edge_style(e.kind, e.line_kind);
        let dash_array = match e.line_kind {
            ir::common::LineKind::Dotted => vec![3.0, 4.0],
            _ => Vec::new(),
        };
        let color = if e.line_kind == ir::common::LineKind::Invisible {
            Color::new(0, 0, 0, 0)
        } else {
            color
        };
        let stroke = Stroke {
            color,
            width,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash_array,
            dash_offset: 0.0,
            miter_limit: 4.0,
        };
        let ends = edge_ends(e.kind, e.arrow);
        items.push(SceneItem::Edge {
            path: e.route.clone(),
            stroke: stroke.clone(),
            ends,
            z: 1,
        });

        // ER 基数符号（source / target 端，沿「远离节点」方向排列）。
        let (cs, ct) = e.cardinality;
        if let Some(c) = cs {
            emit_cardinality(
                &mut items,
                e.route.start(),
                e.route.first_direction(),
                c,
                &stroke,
                1,
            );
        }
        if let Some(c) = ct {
            let d = e.route.last_direction();
            emit_cardinality(
                &mut items,
                e.route.end(),
                Point::new(-d.x, -d.y),
                c,
                &stroke,
                1,
            );
        }

        // class 基数文本（"1" / "*" 等），在关系线两端一侧。
        if let Some(t) = &e.cardinality_text.0 {
            emit_cardinality_text(&mut items, e.route.start(), e.route.first_direction(), t, 2);
        }
        if let Some(t) = &e.cardinality_text.1 {
            let d = e.route.last_direction();
            emit_cardinality_text(&mut items, e.route.end(), Point::new(-d.x, -d.y), t, 2);
        }

        // 边标签：白底 + 居中文本（锚点来自 route 中点）。
        if let (Some(text), Some(anchor)) = (&e.label_text, &e.label_anchor)
            && !text.is_empty()
        {
            let ts = theme::text_style(
                theme::flowchart::TEXT,
                theme::FONT_SIZE,
                TextAlign::Center,
                TextBaseline::Middle,
            );
            let layout = lievisual::text::layout_text(
                &[lievisual::text::RichSpan::new(text.clone(), ts.clone())],
                None,
            );
            let pad_x = 4.0;
            let pad_y = 2.0;
            let bg = ShapeGeometry::Rect {
                at: Point::new(
                    anchor.x - layout.width / 2.0 - pad_x,
                    anchor.y - layout.height / 2.0 - pad_y,
                ),
                size: Size::new(layout.width + 2.0 * pad_x, layout.height + 2.0 * pad_y),
            };
            items.push(SceneItem::Shape {
                geometry: bg,
                // 官方 mermaid 边标签底色：rgba(232,232,232,0.8)。
                fill: Some(Fill::Solid(Color::new(232, 232, 232, 204))),
                stroke: None,
                name: Some("edge-label".to_string()),
                z: 1,
            });
            items.push(SceneItem::Label {
                text: vec![lievisual::text::RichSpan::new(text.clone(), ts.clone())],
                position: *anchor,
                style: ts,
                anchor: ir::scenegraph::Anchor::Center,
                z: 2,
            });
        }
    }

    SceneGraph {
        size: gg.size,
        background: gg.background,
        items,
    }
}

// 类框 / 实体框绘制常量（与 measure 阶段 `measure_structured_label` 对齐）。
const CLASS_PAD: f64 = 12.0;
const ENTITY_PAD: f64 = 14.0;
/// 类图成员 / 类名字号（官方 CSS: g.classGroup text{font-size:10px}）。
const SMALL_FONT: f64 = theme::class::MEMBER_FONT_SIZE;
const ATTR_LINE_H: f64 = 24.0;
/// 空栏占位高度（官方空类成员/方法栏各 18px）。
const SECTION_MIN_H: f64 = 18.0;
/// ER 实体属性 type 列与 name 列之间的间距。
const ER_ATTR_GAP: f64 = 24.0;

/// 绘制 ER 基数符号（`||` / `|o` / `}|` / `}o`）在边端点处。
/// `p` = 端点，`away` = 远离节点方向的单位向量，`card` = 基数类型。
fn emit_cardinality(
    items: &mut Vec<SceneItem>,
    p: Point,
    away: Point,
    card: ir::common::ErCardinality,
    stroke: &Stroke,
    z: i32,
) {
    use ir::common::ErCardinality::*;
    let perp = Point::new(-away.y, away.x);
    const SHORT_HALF: f64 = 4.0;
    const STEP: f64 = 6.0;
    let at = |idx: f64| Point::new(p.x + away.x * idx * STEP, p.y + away.y * idx * STEP);

    // 短线：沿 perp 方向、中心 `center`，半长 SHORT_HALF。
    let short = |items: &mut Vec<SceneItem>, center: Point| {
        let s = Point::new(
            center.x - perp.x * SHORT_HALF,
            center.y - perp.y * SHORT_HALF,
        );
        let e = Point::new(
            center.x + perp.x * SHORT_HALF,
            center.y + perp.y * SHORT_HALF,
        );
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[s, e]),
            stroke: stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z,
        });
    };
    // 空心圆：半径 3。
    let circle = |items: &mut Vec<SceneItem>, center: Point| {
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Ellipse {
                center,
                rx: 3.0,
                ry: 3.0,
            },
            fill: None,
            stroke: Some(stroke.clone()),
            name: None,
            z,
        });
    };
    // 大括号 `}`：3 点折线（top/mid/bot），mid 沿 away 凸出。
    let brace = |items: &mut Vec<SceneItem>, center: Point| {
        let top = Point::new(center.x - perp.x * 4.0, center.y - perp.y * 4.0);
        let mid = Point::new(center.x + away.x * 3.5, center.y + away.y * 3.5);
        let bot = Point::new(center.x + perp.x * 4.0, center.y + perp.y * 4.0);
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Polygon {
                points: vec![top, mid, bot],
            },
            fill: None,
            stroke: Some(stroke.clone()),
            name: None,
            z,
        });
    };

    match card {
        ExactlyOne => {
            short(items, at(0.0));
            short(items, at(1.0));
        }
        ZeroOrOne => {
            short(items, at(0.0));
            circle(items, at(1.0));
        }
        OneOrMany => {
            short(items, at(0.0));
            brace(items, at(1.0));
        }
        ZeroOrMany => {
            circle(items, at(0.0));
            brace(items, at(1.0));
        }
    }
}

/// 绘制 class 关系基数文本（`"1"` / `"*"` / `"many"` 等），放在关系线端点一侧。
fn emit_cardinality_text(items: &mut Vec<SceneItem>, p: Point, dir: Point, text: &str, z: i32) {
    // 沿垂直于边的方向偏移，避免压在关系线上。
    let perp = Point::new(-dir.y, dir.x);
    let pos = Point::new(p.x + perp.x * 12.0, p.y + perp.y * 12.0);
    let ts = TextStyle::new(theme::class::TEXT, theme::FONT_SIZE, theme::FONT_FAMILY)
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);
    items.push(SceneItem::Label {
        text: vec![RichSpan::new(text.to_string(), ts.clone())],
        position: pos,
        style: ts,
        anchor: ir::scenegraph::Anchor::Center,
        z,
    });
}

/// class 主题描边（官方 1px）。
fn class_stroke() -> Stroke {
    Stroke {
        color: theme::class::STROKE,
        width: 1.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    }
}

/// 绘制 UML 类框（三栏：header + attrs + methods）。
fn emit_class_box(items: &mut Vec<SceneItem>, n: &GGNode) {
    let half_w = n.size.width / 2.0;
    let half_h = n.size.height / 2.0;
    let rect = Rect::new(
        n.center.x - half_w,
        n.center.y - half_h,
        n.center.x + half_w,
        n.center.y + half_h,
    );
    let stroke = class_stroke();

    let ir::common::NodeDetail::Class {
        annotation,
        attrs,
        methods,
    } = &n.detail
    else {
        return;
    };
    let header_layout_h = n.label.as_ref().map(|l| l.layout.height).unwrap_or(20.0);
    let ann_h = if annotation.is_some() { 16.0 } else { 0.0 };
    let header_h = header_layout_h + 24.0 + ann_h;
    let attr_h = if attrs.is_empty() {
        SECTION_MIN_H
    } else {
        attrs.len() as f64 * ATTR_LINE_H
    };

    // 整体背景（body 色） + header 背景（HEADER_FILL）。
    items.push(SceneItem::Shape {
        geometry: ShapeGeometry::Rect {
            at: Point::new(rect.min_x(), rect.min_y()),
            size: n.size,
        },
        fill: Some(Fill::Solid(theme::class::FILL)),
        stroke: Some(stroke.clone()),
        name: None,
        z: 0,
    });
    items.push(SceneItem::Shape {
        geometry: ShapeGeometry::Rect {
            at: Point::new(rect.min_x(), rect.min_y()),
            size: Size::new(n.size.width, header_h),
        },
        fill: Some(Fill::Solid(theme::class::HEADER_FILL)),
        stroke: Some(stroke.clone()),
        name: None,
        z: 1,
    });

    // 注解（header 顶部）。
    if let Some(ann) = annotation {
        let ts = theme::text_style(
            theme::class::TEXT,
            SMALL_FONT,
            TextAlign::Center,
            TextBaseline::Middle,
        );
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(format!("«{}»", ann), ts.clone())],
            position: Point::new(n.center.x, rect.min_y() + 9.0),
            style: ts,
            anchor: ir::scenegraph::Anchor::Center,
            z: 2,
        });
    }
    // 类名（header 居中，官方 10px + 加粗）。
    if let Some(label) = &n.label {
        let name_y = rect.min_y() + header_h / 2.0 + if annotation.is_some() { 6.0 } else { 0.0 };
        let ts = theme::text_style(
            theme::class::TEXT,
            SMALL_FONT,
            TextAlign::Center,
            TextBaseline::Middle,
        )
        .with_weight(FontWeight::Bold);
        items.push(SceneItem::Label {
            text: label.spans.clone(),
            position: Point::new(n.center.x, name_y),
            style: ts,
            anchor: ir::scenegraph::Anchor::Center,
            z: 2,
        });
    }

    // 两条分隔线：名称栏底部 + 成员栏底部（官方恒画，即使成员/方法为空）。
    for dy in [rect.min_y() + header_h, rect.min_y() + header_h + attr_h] {
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Rect {
                at: Point::new(rect.min_x(), dy),
                size: Size::new(n.size.width, 1.0),
            },
            fill: Some(Fill::Solid(theme::class::SEPARATOR)),
            stroke: None,
            name: None,
            z: 1,
        });
    }

    // attrs 行（左对齐）。
    let mut line_y = rect.min_y() + header_h + 4.0;
    for a in attrs {
        let ts = theme::text_style(
            theme::class::TEXT,
            SMALL_FONT,
            TextAlign::Left,
            TextBaseline::Top,
        );
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(a.clone(), ts.clone())],
            position: Point::new(rect.min_x() + CLASS_PAD, line_y),
            style: ts,
            anchor: ir::scenegraph::Anchor::Left,
            z: 2,
        });
        line_y += ATTR_LINE_H;
    }

    // methods 行（左对齐，从成员栏分隔线下方开始）。
    line_y = rect.min_y() + header_h + attr_h + 4.0;
    for m in methods {
        let ts = theme::text_style(
            theme::class::TEXT,
            SMALL_FONT,
            TextAlign::Left,
            TextBaseline::Top,
        );
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(m.clone(), ts.clone())],
            position: Point::new(rect.min_x() + CLASS_PAD, line_y),
            style: ts,
            anchor: ir::scenegraph::Anchor::Left,
            z: 2,
        });
        line_y += ATTR_LINE_H;
    }
}

/// 绘制 ER 实体框（两栏：header + attrs）。
fn emit_entity_box(items: &mut Vec<SceneItem>, n: &GGNode) {
    let half_w = n.size.width / 2.0;
    let half_h = n.size.height / 2.0;
    let rect = Rect::new(
        n.center.x - half_w,
        n.center.y - half_h,
        n.center.x + half_w,
        n.center.y + half_h,
    );
    let stroke = Stroke {
        color: theme::er::STROKE,
        width: 1.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };

    let ir::common::NodeDetail::Entity { attrs } = &n.detail else {
        return;
    };
    let header_h = n.label.as_ref().map(|l| l.layout.height).unwrap_or(20.0) + 20.0;

    items.push(SceneItem::Shape {
        geometry: ShapeGeometry::Rect {
            at: Point::new(rect.min_x(), rect.min_y()),
            size: n.size,
        },
        fill: Some(Fill::Solid(theme::er::FILL)),
        stroke: Some(stroke.clone()),
        name: None,
        z: 0,
    });
    items.push(SceneItem::Shape {
        geometry: ShapeGeometry::Rect {
            at: Point::new(rect.min_x(), rect.min_y()),
            size: Size::new(n.size.width, header_h),
        },
        fill: Some(Fill::Solid(theme::er::HEADER_FILL)),
        stroke: Some(stroke.clone()),
        name: None,
        z: 1,
    });

    // 实体名（header 居中）。
    if let Some(label) = &n.label {
        let ts = theme::text_style(
            theme::er::TEXT,
            theme::FONT_SIZE,
            TextAlign::Center,
            TextBaseline::Middle,
        );
        items.push(SceneItem::Label {
            text: label.spans.clone(),
            position: Point::new(n.center.x, rect.min_y() + header_h / 2.0),
            style: ts,
            anchor: ir::scenegraph::Anchor::Center,
            z: 2,
        });
    }

    // 属性分 type / name 两列（16px，与官方 ER 属性字号一致）。
    let attr_ts = theme::text_style(
        theme::er::TEXT,
        theme::FONT_SIZE,
        TextAlign::Left,
        TextBaseline::Top,
    );
    let mut type_max = 0.0f64;
    for a in attrs {
        let l =
            lievisual::text::layout_text(&[RichSpan::new(a.type_.clone(), attr_ts.clone())], None);
        type_max = type_max.max(l.width);
    }
    let type_x = rect.min_x() + ENTITY_PAD;
    let name_x = type_x + type_max + ER_ATTR_GAP;

    // 分栏线（官方 golden：header 与属性区之间的横线 + type/name 之间的竖线，
    // 均为 `fill-rule="evenodd"` 的细分隔线）。
    let header_line_y = rect.min_y() + header_h;
    let divider_stroke = Stroke {
        color: theme::er::STROKE,
        width: 1.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };
    // header 底部横线（横跨整宽）。
    items.push(SceneItem::Edge {
        path: ir::geograph::line_route(&[
            Point::new(rect.min_x(), header_line_y),
            Point::new(rect.max_x(), header_line_y),
        ]),
        stroke: divider_stroke.clone(),
        ends: (EdgeEnds::None, EdgeEnds::None),
        z: 1,
    });
    // type / name 之间的竖线（从 header 底延伸到底边），仅在有属性时绘制。
    if !attrs.is_empty() {
        let div_x = name_x - ER_ATTR_GAP / 2.0;
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[
                Point::new(div_x, header_line_y),
                Point::new(div_x, rect.max_y()),
            ]),
            stroke: divider_stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 1,
        });
    }

    let mut line_y = rect.min_y() + header_h + 6.0;
    for a in attrs {
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(a.type_.clone(), attr_ts.clone())],
            position: Point::new(type_x, line_y),
            style: attr_ts.clone(),
            anchor: ir::scenegraph::Anchor::Left,
            z: 2,
        });
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(a.name.clone(), attr_ts.clone())],
            position: Point::new(name_x, line_y),
            style: attr_ts.clone(),
            anchor: ir::scenegraph::Anchor::Left,
            z: 2,
        });
        line_y += ATTR_LINE_H;
    }
}

/// 绘制 gitgraph 图表（Hierarchy 家族）：提交点 + 分支线 + 合并曲线 + 标签。
///
/// 几何由 engine `hierarchy_geometry` 产生：提交按声明序沿 x 推进、分支按容器序映射到 y 行；
/// 边（child → parent）留空路由，此处据两端节点中心画水平线（同分支）或曲线（跨分支）。
fn emit_gitgraph(items: &mut Vec<SceneItem>, gg: &Geograph) {
    use ir::common::NodeDetail;
    use ir::geograph::RouteSegment;

    let node_by_id: HashMap<&str, &GGNode> = gg.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    // 分支列表（顺序 = 容器序，含 main）与行映射。
    let branches: Vec<(String, Vec<String>)> = gg
        .containers
        .iter()
        .filter(|c| c.kind == ir::common::ContainerKind::GitBranch)
        .map(|c| (c.title.clone().unwrap_or_default(), c.member_ids.clone()))
        .collect();
    let branch_row: HashMap<&str, usize> = branches
        .iter()
        .enumerate()
        .map(|(i, (n, _))| (n.as_str(), i))
        .collect();
    let branch_color = |name: &str| -> Color {
        let i = branch_row.get(name).copied().unwrap_or(0);
        theme::gitgraph::BRANCH_COLORS[i % theme::gitgraph::BRANCH_COLORS.len()]
    };
    let git_stroke = |color: Color, width: f64| Stroke {
        color,
        width,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };
    let cr = theme::gitgraph::COMMIT_RADIUS;

    // —— 提交点（HIGHLIGHT 方框 / merge 白芯 / 普通实心）+ commit id 标签 + tag 标签 ——
    for n in &gg.nodes {
        let NodeDetail::GitCommit {
            branch,
            id,
            tag,
            commit_type,
            is_merge,
        } = &n.detail
        else {
            continue;
        };
        let color = branch_color(branch);
        let line_y = n.center.y;
        let is_highlight = commit_type.as_deref() == Some("HIGHLIGHT");
        if is_highlight {
            // HIGHLIGHT：深色外方框 + 浅色内方框（对齐官方 commit-highlight-outer / inner）。
            items.push(SceneItem::Shape {
                geometry: ShapeGeometry::Rect {
                    at: Point::new(n.center.x - 10.0, line_y - 10.0),
                    size: Size::new(20.0, 20.0),
                },
                fill: Some(Fill::Solid(theme::gitgraph::HIGHLIGHT_OUTER)),
                stroke: None,
                name: None,
                z: 1,
            });
            items.push(SceneItem::Shape {
                geometry: ShapeGeometry::Rect {
                    at: Point::new(n.center.x - 6.0, line_y - 6.0),
                    size: Size::new(12.0, 12.0),
                },
                fill: Some(Fill::Solid(theme::gitgraph::MERGE_INNER)),
                stroke: None,
                name: None,
                z: 1,
            });
        } else if *is_merge {
            // merge：分支色外圆 + 白芯（对齐官方 commit + commit-merge 双圆）。
            items.push(SceneItem::Shape {
                geometry: ShapeGeometry::Ellipse {
                    center: n.center,
                    rx: cr,
                    ry: cr,
                },
                fill: Some(Fill::Solid(color)),
                stroke: None,
                name: None,
                z: 1,
            });
            items.push(SceneItem::Shape {
                geometry: ShapeGeometry::Ellipse {
                    center: n.center,
                    rx: cr * 0.6,
                    ry: cr * 0.6,
                },
                fill: Some(Fill::Solid(theme::gitgraph::MERGE_INNER)),
                stroke: None,
                name: None,
                z: 1,
            });
        } else {
            items.push(SceneItem::Shape {
                geometry: ShapeGeometry::Ellipse {
                    center: n.center,
                    rx: cr,
                    ry: cr,
                },
                fill: Some(Fill::Solid(color)),
                stroke: None,
                name: None,
                z: 1,
            });
        }
        // commit id 标签：显式 id 优先；merge 无显式 id 不显示（官方 merge 无 id 时无 commit-label）；
        // 普通提交无 id 显示节点 id（官方为随机哈希，此处用 cN 近似）。
        let commit_label = id
            .clone()
            .or_else(|| if *is_merge { None } else { Some(n.id.clone()) });
        if let Some(text) = commit_label {
            let ts = TextStyle::new(
                theme::gitgraph::COMMIT_LABEL_FILL,
                theme::gitgraph::LABEL_FONT,
                theme::FONT_FAMILY,
            )
            .with_align(TextAlign::Center)
            .with_baseline(TextBaseline::Alphabetic);
            let tw = lievisual::text::layout_text(&[RichSpan::new(text.clone(), ts.clone())], None)
                .width;
            // 淡黄背景（官方 .commit-label-bkg #ffffde @0.5，包住文字：左 -2 / 右 +3，高 15）。
            items.push(SceneItem::Shape {
                geometry: ShapeGeometry::Rect {
                    at: Point::new(n.center.x - tw / 2.0 - 2.0, line_y + 13.5),
                    size: Size::new(tw + 5.0, 15.0),
                },
                fill: Some(Fill::Solid(theme::gitgraph::COMMIT_LABEL_BKG)),
                stroke: None,
                name: None,
                z: 0,
            });
            items.push(SceneItem::Label {
                text: vec![RichSpan::new(text, ts.clone())],
                position: Point::new(n.center.x, line_y + 25.0),
                style: ts,
                anchor: ir::scenegraph::Anchor::Center,
                z: 2,
            });
        }
        // tag 标签：位于 commit 上方，灰色「左尖角矩形」背景（官方 tag 标签的简化），文字居中。
        if let Some(tag_text) = tag {
            let ts = TextStyle::new(
                theme::gitgraph::TAG_LABEL_FILL,
                theme::gitgraph::LABEL_FONT,
                theme::FONT_FAMILY,
            )
            .with_align(TextAlign::Center)
            .with_baseline(TextBaseline::Alphabetic);
            let tw =
                lievisual::text::layout_text(&[RichSpan::new(tag_text.clone(), ts.clone())], None)
                    .width;
            // 背景盒：主体矩形 + 左侧尖角（指向 commit 一侧），包住文字（上下各留 ~2px）。
            let x_left = n.center.x - tw / 2.0 - 4.0;
            let x_right = n.center.x + tw / 2.0 + 4.0;
            let y_top = line_y - 24.5;
            let y_bottom = line_y - 10.5;
            let tip = 6.0;
            let cy = (y_top + y_bottom) / 2.0;
            items.push(SceneItem::Shape {
                geometry: ShapeGeometry::Polygon {
                    points: vec![
                        Point::new(x_left - tip, cy), // 左尖角
                        Point::new(x_left, y_top),
                        Point::new(x_right, y_top),
                        Point::new(x_right, y_bottom),
                        Point::new(x_left, y_bottom),
                    ],
                },
                fill: Some(Fill::Solid(theme::gitgraph::TAG_BKG)),
                stroke: Some(git_stroke(theme::gitgraph::TAG_BKG_STROKE, 1.0)),
                name: None,
                z: 0,
            });
            items.push(SceneItem::Label {
                text: vec![RichSpan::new(tag_text.clone(), ts.clone())],
                position: Point::new(n.center.x, line_y - 16.0),
                style: ts,
                anchor: ir::scenegraph::Anchor::Center,
                z: 2,
            });
        }
    }

    // —— 分支线 / 合并曲线（边：child → parent）——
    // 跨分支采用「直线 → 圆角贝塞尔 → 直线」的平滑连接，控制点落在与目标对齐的平行线 / 水平线上，
    // 曲线凸向 child 方向（不向内凹）：fork（child 在下）= 垂直下 → 弧（在 child 行平行线上
    // 对齐父 commit 点的控制点）→ 水平进 child；merge（child 在上）= 水平右 → 弧（在父行水平线
    // 上对齐 child commit 点的控制点）→ 垂直进 child（连线回到 commit）。
    let curve_r = theme::gitgraph::BRANCH_SPACING / 3.0;
    for e in &gg.edges {
        let (Some(child), Some(parent)) = (
            node_by_id.get(e.source.as_str()),
            node_by_id.get(e.target.as_str()),
        ) else {
            continue;
        };
        let (
            NodeDetail::GitCommit {
                branch: cb,
                is_merge: cm,
                ..
            },
            NodeDetail::GitCommit { branch: pb, .. },
        ) = (&child.detail, &parent.detail)
        else {
            continue;
        };
        if cb == pb {
            // 同分支：水平线（父分支色）。
            items.push(SceneItem::Edge {
                path: ir::geograph::line_route(&[parent.center, child.center]),
                stroke: git_stroke(branch_color(cb), theme::gitgraph::LINE_WIDTH),
                ends: (EdgeEnds::None, EdgeEnds::None),
                z: 0,
            });
        } else {
            // 跨分支：颜色 = fork 用子分支色、merge 用父分支色（对齐官方箭头）。
            let color = if *cm {
                branch_color(pb)
            } else {
                branch_color(cb)
            };
            let (pc, cc) = (parent.center, child.center);
            let mut path = ir::geograph::RoutePath::new();
            if cc.y >= pc.y {
                // fork：父（上）→ 垂直下 → 弧（控制点 = child 行平行线上对齐父 commit 点）
                // → 水平进 child（下）。曲线先落到 child 行的平行线上，再平滑弯向水平进 child。
                let a = Point::new(pc.x, cc.y - curve_r);
                let p1 = Point::new(pc.x, cc.y); // 平行线（child 行）上对齐父 commit 点
                let p2 = Point::new((cc.x - curve_r).max(pc.x), cc.y); // 平行线上 child 左侧，保证水平入
                path.push(RouteSegment::Line { from: pc, to: a });
                path.push(RouteSegment::CubicBezier {
                    p0: a,
                    p1,
                    p2,
                    p3: cc,
                });
            } else {
                // merge：父（下）→ 水平右 → 弧（控制点 = 父行水平线上对齐 child commit 点）
                // → 垂直进 child（上）。曲线先沿水平线走到 child 正下方，再垂直连线回到 commit。
                let a = Point::new(cc.x - curve_r, pc.y);
                let p1 = Point::new(cc.x, pc.y); // 水平线（父行）上对齐 child commit 点
                let p2 = Point::new(cc.x, (pc.y - curve_r).max(cc.y)); // child 列上 commit 下方，保证垂直入
                path.push(RouteSegment::Line { from: pc, to: a });
                path.push(RouteSegment::CubicBezier {
                    p0: a,
                    p1,
                    p2,
                    p3: cc,
                });
            }
            items.push(SceneItem::Edge {
                path,
                stroke: git_stroke(color, theme::gitgraph::LINE_WIDTH),
                ends: (EdgeEnds::None, EdgeEnds::None),
                z: 0,
            });
        }
    }

    // —— 分支虚线（每行贯穿，标识分支平行状态）——
    let min_x = gg
        .nodes
        .iter()
        .map(|n| n.center.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = gg
        .nodes
        .iter()
        .map(|n| n.center.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let branch_dash = Stroke {
        color: Color::new(51, 51, 51, 255), // #333（对齐官方 .branch stroke）
        width: 1.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: vec![2.0, 2.0],
        dash_offset: 0.0,
        miter_limit: 4.0,
    };

    // —— 分支标签块 + 尾部延伸线 ——
    for (i, (name, members)) in branches.iter().enumerate() {
        let color = theme::gitgraph::BRANCH_COLORS[i % theme::gitgraph::BRANCH_COLORS.len()];
        let y = theme::gitgraph::TOP_MARGIN + i as f64 * theme::gitgraph::BRANCH_SPACING;
        // 分支行虚线（贯穿内容区，标识该分支的平行轨道）。
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[
                Point::new(min_x - 10.0, y),
                Point::new(max_x + 40.0, y),
            ]),
            stroke: branch_dash.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 0,
        });
        let ts_label = TextStyle::new(
            Color::rgb(255, 255, 255),
            theme::FONT_SIZE,
            theme::FONT_FAMILY,
        )
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);
        let layout =
            lievisual::text::layout_text(&[RichSpan::new(name.clone(), ts_label.clone())], None);
        let pad_x = 12.0;
        let pad_y = 6.0;
        let lw = layout.width + pad_x * 2.0;
        let lh = layout.height + pad_y * 2.0;
        let lx = theme::gitgraph::LEFT_MARGIN - lw - 16.0;
        let ly = y - lh / 2.0;
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::RoundedRect {
                at: Point::new(lx, ly),
                size: Size::new(lw, lh),
                radius: 4.0,
            },
            fill: Some(Fill::Solid(color)),
            stroke: Some(git_stroke(color, 1.0)),
            name: None,
            z: 0,
        });
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(name.clone(), ts_label.clone())],
            position: Point::new(lx + lw / 2.0, ly + lh / 2.0),
            style: ts_label,
            anchor: ir::scenegraph::Anchor::Center,
            z: 2,
        });
        // 分支尾部延伸线（最后一个提交之后）。
        if let Some(last_id) = members.last()
            && let Some(last) = node_by_id.get(last_id.as_str())
        {
            items.push(SceneItem::Edge {
                path: ir::geograph::line_route(&[
                    Point::new(last.center.x + cr + 4.0, y),
                    Point::new(last.center.x + cr + 40.0, y),
                ]),
                stroke: git_stroke(Color::new(180, 180, 180, 255), 1.0),
                ends: (EdgeEnds::None, EdgeEnds::None),
                z: 0,
            });
        }
    }
}

/// 绘制 pie 图表（Radial 家族）：扇区（从 12 点方向顺时针）+ 百分比标签 + 标题。
///
/// 所有扇区共享圆心（原点），半径固定；fit_to_canvas 负责整体居中缩放。
fn emit_pie(items: &mut Vec<SceneItem>, gg: &Geograph) {
    let mut data: Vec<(String, f64)> = Vec::new();
    for n in &gg.nodes {
        if let ir::common::NodeDetail::PieSlice { label, value } = &n.detail {
            data.push((label.clone(), *value));
        }
    }
    if data.is_empty() {
        return;
    }
    let total: f64 = data.iter().map(|(_, v)| v).sum();
    if total <= 0.0 {
        return;
    }
    let center = Point::new(0.0, 0.0);
    let radius = theme::pie::RADIUS;

    // 标题（官方：`<text x="0" y="-200" class="pieTitleText">`，圆心正上方）。
    // 官方 `.pieTitleText` 用默认字母基线（`position.y` 即基线），故用 Alphabetic；
    // 若用 Top，渲染时基线会被下移一个 ascent，标题更靠近圆心而压到饼图。
    if let Some(title) = &gg.title
        && !title.is_empty()
    {
        let ts = theme::text_style(
            Color::BLACK,
            theme::pie::TITLE_FONT,
            TextAlign::Center,
            TextBaseline::Alphabetic,
        );
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(title.clone(), ts.clone())],
            position: Point::new(center.x, center.y - theme::pie::TITLE_DY),
            style: ts,
            anchor: ir::scenegraph::Anchor::Center,
            z: 2,
        });
    }

    // 外圈（官方 `<circle r="186" class="pieOuterCircle"/>`：黑描边、无填充）。
    items.push(SceneItem::Shape {
        geometry: ShapeGeometry::Ellipse {
            center,
            rx: theme::pie::OUTER_RADIUS,
            ry: theme::pie::OUTER_RADIUS,
        },
        fill: None,
        stroke: Some(Stroke {
            color: theme::pie::OUTER_STROKE,
            width: theme::pie::OUTER_STROKE_WIDTH,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash_array: Vec::new(),
            dash_offset: 0.0,
            miter_limit: 4.0,
        }),
        name: None,
        z: 1,
    });

    let slice_stroke = Stroke {
        color: Color::rgb(255, 255, 255),
        width: 2.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };
    let slice_ts = theme::text_style(
        Color::BLACK,
        theme::pie::LABEL_FONT,
        TextAlign::Center,
        TextBaseline::Middle,
    );

    // 各扇区：从 12 点钟方向（-π/2）顺时针推进。
    let mut start = -std::f64::consts::FRAC_PI_2;
    for (idx, (_label, value)) in data.iter().enumerate() {
        let sweep = 2.0 * std::f64::consts::PI * value / total;
        let color = theme::pie::COLORS[idx % theme::pie::COLORS.len()];
        // ShapeGeometry::Pie 的第 4 字段即 sweep 角（paint 直传 Element::pie）。
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Pie {
                center,
                radius,
                start_angle: start,
                end_angle: sweep,
            },
            fill: Some(Fill::Solid(color)),
            stroke: Some(slice_stroke.clone()),
            name: None,
            z: 0,
        });

        // 扇区标签：官方只放**百分比**（`40%`），名称交给右侧图例。
        let mid = start + sweep / 2.0;
        let lr = radius * theme::pie::LABEL_RADIUS_RATIO;
        let lp = Point::new(center.x + lr * mid.cos(), center.y + lr * mid.sin());
        let pct = format!("{}%", (value / total * 100.0).round() as i64);
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(pct, slice_ts.clone())],
            position: lp,
            style: slice_ts.clone(),
            anchor: ir::scenegraph::Anchor::Center,
            z: 2,
        });

        start += sweep;
    }

    // 图例（官方 `<g class="legend" transform="translate(216, y)">`：色块 + 名称，
    // 竖直居中排布；`showData` 时名称后附 `[数值]`）。
    let n = data.len() as f64;
    let legend_x = center.x + radius + theme::pie::LEGEND_DX;
    let legend_top = center.y - (n - 1.0) / 2.0 * theme::pie::LEGEND_ROW_H;
    let legend_ts = theme::text_style(
        Color::BLACK,
        theme::pie::LABEL_FONT,
        TextAlign::Left,
        TextBaseline::Middle,
    );
    for (idx, (label, value)) in data.iter().enumerate() {
        let color = theme::pie::COLORS[idx % theme::pie::COLORS.len()];
        let row_y = legend_top + idx as f64 * theme::pie::LEGEND_ROW_H;
        let swatch_top = row_y - theme::pie::LEGEND_SWATCH / 2.0;
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Rect {
                at: Point::new(legend_x, swatch_top),
                size: Size::new(theme::pie::LEGEND_SWATCH, theme::pie::LEGEND_SWATCH),
            },
            fill: Some(Fill::Solid(color)),
            stroke: Some(Stroke {
                color,
                width: 1.0,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                dash_array: Vec::new(),
                dash_offset: 0.0,
                miter_limit: 4.0,
            }),
            name: None,
            z: 1,
        });
        let text = if gg.show_data {
            format!("{} [{}]", label, *value as i64)
        } else {
            label.clone()
        };
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(text, legend_ts.clone())],
            position: Point::new(legend_x + theme::pie::LEGEND_TEXT_DX, row_y),
            style: legend_ts.clone(),
            anchor: ir::scenegraph::Anchor::Left,
            z: 2,
        });
    }
}

/// 绘制 sequence 图表（Sequence 家族）：参与者盒 + 生命线 + 消息箭头 + 备注 + 分组块。
fn emit_sequence(items: &mut Vec<SceneItem>, gg: &Geograph) {
    let lifelines: Vec<&GGNode> = gg
        .nodes
        .iter()
        .filter(|n| n.role == ir::common::NodeRole::Lifeline)
        .collect();
    if lifelines.is_empty() {
        return;
    }

    // 生命线底端：最后一行消息 / 备注之下。
    let msg_bottom = gg
        .edges
        .iter()
        .flat_map(|e| e.route.anchors())
        .map(|p| p.y)
        .fold(0.0f64, f64::max);
    let note_bottom = gg
        .nodes
        .iter()
        .filter(|n| matches!(n.detail, ir::common::NodeDetail::SequenceNote { .. }))
        .map(|n| n.center.y + n.size.height / 2.0)
        .fold(0.0f64, f64::max);
    // 官方 golden：最后一条消息线 y=199、底部 actor 盒 y=219，间距 20。
    // 生命线止于底部盒上沿，底部盒在 content_bottom 处。
    let content_bottom = msg_bottom.max(note_bottom) + 20.0;

    // 官方 golden：`<line class="actor-line" stroke-width="0.5px" stroke="#999"/>`——细**实线**。
    let lifeline_stroke = Stroke {
        color: theme::sequence::LIFELINE,
        width: theme::sequence::LIFELINE_WIDTH,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };

    // —— 参与者盒 + 生命线 ——
    for n in &lifelines {
        let half_w = n.size.width / 2.0;
        let half_h = n.size.height / 2.0;
        let rect = Rect::new(
            n.center.x - half_w,
            n.center.y - half_h,
            n.center.x + half_w,
            n.center.y + half_h,
        );
        // 官方 golden：`<rect fill="#eaeaea" stroke="#666" rx="3" ry="3" class="actor"/>`。
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::RoundedRect {
                at: Point::new(rect.min_x(), rect.min_y()),
                size: n.size,
                radius: theme::sequence::ACTOR_RADIUS,
            },
            fill: Some(Fill::Solid(theme::sequence::ACTOR_FILL)),
            stroke: Some(Stroke {
                color: theme::sequence::ACTOR_STROKE,
                width: 1.0,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                dash_array: Vec::new(),
                dash_offset: 0.0,
                miter_limit: 4.0,
            }),
            name: None,
            z: 0,
        });
        if let Some(label) = &n.label {
            let ts = theme::text_style(
                theme::sequence::ACTOR_TEXT,
                theme::FONT_SIZE,
                TextAlign::Center,
                TextBaseline::Middle,
            );
            items.push(SceneItem::Label {
                text: label.spans.clone(),
                position: n.center,
                style: ts,
                anchor: ir::scenegraph::Anchor::Center,
                z: 2,
            });
        }
        // 生命线（单条虚线，从顶部盒底向下延伸到底部盒上沿）。
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[
                Point::new(n.center.x, n.center.y + half_h),
                Point::new(n.center.x, content_bottom),
            ]),
            stroke: lifeline_stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 0,
        });
    }

    // —— 底部参与者盒（官方 `<rect class="actor actor-bottom">`，位于生命线终点）——
    for n in &lifelines {
        let half_w = n.size.width / 2.0;
        let bottom_y = content_bottom;
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::RoundedRect {
                at: Point::new(n.center.x - half_w, bottom_y),
                size: n.size,
                radius: theme::sequence::ACTOR_RADIUS,
            },
            fill: Some(Fill::Solid(theme::sequence::ACTOR_FILL)),
            stroke: Some(Stroke {
                color: theme::sequence::ACTOR_STROKE,
                width: 1.0,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                dash_array: Vec::new(),
                dash_offset: 0.0,
                miter_limit: 4.0,
            }),
            name: None,
            z: 0,
        });
        if let Some(label) = &n.label {
            let ts = theme::text_style(
                theme::sequence::ACTOR_TEXT,
                theme::FONT_SIZE,
                TextAlign::Center,
                TextBaseline::Middle,
            );
            items.push(SceneItem::Label {
                text: label.spans.clone(),
                position: Point::new(n.center.x, bottom_y + n.size.height / 2.0),
                style: ts,
                anchor: ir::scenegraph::Anchor::Center,
                z: 2,
            });
        }
    }

    // —— 激活条（`A->>+B` / `A-->>-B`）：官方 `<rect fill="#EDF2AE" stroke="#666" width="10"/>` ——
    for a in &gg.activations {
        let y0 = a.y0.min(a.y1);
        let y1 = a.y0.max(a.y1);
        if (y1 - y0).abs() < 1.0 {
            continue;
        }
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Rect {
                at: Point::new(a.x - theme::sequence::ACTIVATION_WIDTH / 2.0, y0),
                size: Size::new(theme::sequence::ACTIVATION_WIDTH, y1 - y0),
            },
            fill: Some(Fill::Solid(theme::sequence::ACTIVATION_FILL)),
            stroke: Some(Stroke {
                color: theme::sequence::ACTIVATION_STROKE,
                width: 1.0,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                dash_array: Vec::new(),
                dash_offset: 0.0,
                miter_limit: 4.0,
            }),
            name: None,
            z: 0,
        });
    }

    // —— 消息边：水平线 + 箭头 + 标签 ——
    for e in &gg.edges {
        let (color, _w) = edge_style(e.kind, e.line_kind);
        // 官方 golden：消息线 `stroke-width="2"`，虚线消息 `stroke-dasharray: 3, 3`。
        let width = theme::sequence::MESSAGE_WIDTH;
        let dash_array = match e.line_kind {
            ir::common::LineKind::Dotted => theme::sequence::MESSAGE_DASH.to_vec(),
            _ => Vec::new(),
        };
        let stroke = Stroke {
            color,
            width,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash_array,
            dash_offset: 0.0,
            miter_limit: 4.0,
        };
        let ends = edge_ends(e.kind, e.arrow);
        items.push(SceneItem::Edge {
            path: e.route.clone(),
            stroke: stroke.clone(),
            ends,
            z: 1,
        });
        if let (Some(text), Some(anchor)) = (&e.label_text, &e.label_anchor)
            && !text.is_empty()
        {
            let ts = theme::text_style(
                theme::sequence::TEXT,
                theme::FONT_SIZE,
                TextAlign::Center,
                TextBaseline::Bottom,
            );
            items.push(SceneItem::Label {
                text: vec![RichSpan::new(text.clone(), ts.clone())],
                position: *anchor,
                style: ts,
                anchor: ir::scenegraph::Anchor::Center,
                z: 2,
            });
        }
    }

    // —— 备注盒 ——
    for n in &gg.nodes {
        let ir::common::NodeDetail::SequenceNote { text, .. } = &n.detail else {
            continue;
        };
        let half_w = n.size.width / 2.0;
        let half_h = n.size.height / 2.0;
        let rect = Rect::new(
            n.center.x - half_w,
            n.center.y - half_h,
            n.center.x + half_w,
            n.center.y + half_h,
        );
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::RoundedRect {
                at: Point::new(rect.min_x(), rect.min_y()),
                size: n.size,
                radius: theme::sequence::ACTOR_RADIUS,
            },
            fill: Some(Fill::Solid(theme::sequence::NOTE_FILL)),
            stroke: Some(Stroke {
                color: theme::sequence::NOTE_STROKE,
                width: 1.0,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                dash_array: Vec::new(),
                dash_offset: 0.0,
                miter_limit: 4.0,
            }),
            name: None,
            z: 0,
        });
        let ts = theme::text_style(
            theme::sequence::TEXT,
            theme::FONT_SIZE,
            TextAlign::Center,
            TextBaseline::Middle,
        );
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(text.clone(), ts.clone())],
            position: n.center,
            style: ts,
            anchor: ir::scenegraph::Anchor::Center,
            z: 2,
        });
    }

    // —— 分组块边框 + 标签（loop / alt / opt / par） ——
    for c in &gg.containers {
        if c.kind != ir::common::ContainerKind::SequenceBlock {
            continue;
        }
        let r = c.bounds;
        let stroke = Stroke {
            color: theme::sequence::BLOCK_STROKE,
            width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash_array: vec![6.0, 4.0],
            dash_offset: 0.0,
            miter_limit: 4.0,
        };
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::RoundedRect {
                at: Point::new(r.min_x(), r.min_y()),
                size: Size::new(r.width(), r.height()),
                radius: theme::NODE_RADIUS,
            },
            fill: None,
            stroke: Some(stroke),
            name: None,
            z: -1,
        });
        if let Some(label) = &c.title {
            let ts = theme::text_style(
                theme::sequence::BLOCK_TEXT,
                theme::FONT_SIZE * 0.85,
                TextAlign::Left,
                TextBaseline::Middle,
            );
            let origin = Point::new(r.min_x() + 8.0, r.min_y() + 12.0);
            // 官方把标签拆成两段文本：`loop` + `[Each item]`（并列排布，非合并为
            // `loop [Each item]`），与 golden 的 `<text>` 集合保持一致。
            let (head, tail) = split_block_label(label);
            match tail {
                Some(tail) => {
                    let w = lievisual::text::layout_text(
                        &[RichSpan::new(head.clone(), ts.clone())],
                        None,
                    )
                    .width;
                    items.push(SceneItem::Label {
                        text: vec![RichSpan::new(head, ts.clone())],
                        position: origin,
                        style: ts.clone(),
                        anchor: ir::scenegraph::Anchor::Left,
                        z: 0,
                    });
                    items.push(SceneItem::Label {
                        text: vec![RichSpan::new(tail, ts.clone())],
                        position: Point::new(origin.x + w + 4.0, origin.y),
                        style: ts,
                        anchor: ir::scenegraph::Anchor::Left,
                        z: 0,
                    });
                }
                None => items.push(SceneItem::Label {
                    text: vec![RichSpan::new(head, ts.clone())],
                    position: origin,
                    style: ts,
                    anchor: ir::scenegraph::Anchor::Left,
                    z: 0,
                }),
            }
        }
    }
}

/// 拆分分组块标签为「关键字」+「方括号描述」两段（官方两段独立文本）。
///
/// `"loop [Each item]"` → `("loop", Some("[Each item]]"))`；无方括号时返回 `("", None)`。
fn split_block_label(label: &str) -> (String, Option<String>) {
    match label.find(" [") {
        Some(i) => (label[..i].to_string(), Some(label[i + 1..].to_string())),
        None => (label.to_string(), None),
    }
}

/// 绘制 timeline 图表（Linear 家族）：标题 + 时间轴 + 时间点 + section 块 + 事件块 + 连线。
///
/// 坐标约定（由 engine 的 `linear_centers` 产生）：LR（默认）时各 section 节点中心
/// 在同一水平线（时间轴）上、x 递增；TD 时在同一垂直线（时间轴）上、y 递增。
/// section 块在轴上方 / 左侧，事件块在轴下方 / 右侧。
fn emit_timeline(items: &mut Vec<SceneItem>, gg: &Geograph) {
    let sections: Vec<&GGNode> = gg
        .nodes
        .iter()
        .filter(|n| matches!(n.detail, ir::common::NodeDetail::TimelineSection { .. }))
        .collect();
    if sections.is_empty() {
        return;
    }
    use ir::common::NodeDetail;

    let axis_stroke = Stroke {
        color: theme::timeline::LINE,
        width: theme::timeline::LINE_WIDTH,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };
    let dot_stroke = Stroke {
        color: theme::timeline::LINE,
        width: 1.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };

    // 主轴方向由节点分布推断：LR（水平轴）→ x 差异主导；TD（垂直轴）→ y 差异主导。
    let horizontal = if sections.len() >= 2 {
        let a = sections[0].center;
        let b = sections[1].center;
        (b.y - a.y).abs() < (b.x - a.x).abs()
    } else {
        true
    };
    let axis_c = if horizontal {
        sections[0].center.y
    } else {
        sections[0].center.x
    };
    let mut min_a = f64::INFINITY;
    let mut max_a = f64::NEG_INFINITY;
    for s in &sections {
        let c = if horizontal { s.center.x } else { s.center.y };
        min_a = min_a.min(c);
        max_a = max_a.max(c);
    }
    let span_mid = (min_a + max_a) / 2.0;

    // 标题（时间轴上方居中）。
    if let Some(title) = &gg.title
        && !title.is_empty()
    {
        let ts = TextStyle::new(theme::timeline::TITLE, 22.0, theme::FONT_FAMILY)
            .with_align(TextAlign::Center)
            .with_baseline(TextBaseline::Top);
        // 标题位于 section 标题块（轴上方最远处）之上。
        let title_off = theme::timeline::TASK_DY
            + theme::timeline::BLOCK_H
            + theme::timeline::SECTION_DY
            + theme::timeline::BLOCK_H / 2.0
            + 40.0;
        let pos = if horizontal {
            Point::new(span_mid, axis_c - title_off)
        } else {
            Point::new(axis_c - title_off, span_mid)
        };
        items.push(SceneItem::Label {
            text: vec![RichSpan::new(title.clone(), ts.clone())],
            position: pos,
            style: ts,
            anchor: ir::scenegraph::Anchor::Center,
            z: 2,
        });
    }

    // 主轴线（带末端箭头）。
    let arr = theme::timeline::ARROW_SIZE;
    if horizontal {
        let y = axis_c;
        let x0 = min_a - theme::timeline::BLOCK_W / 2.0 - 20.0;
        let x1 = max_a + theme::timeline::BLOCK_W / 2.0 + 20.0;
        let tip = x1 - arr * 1.5;
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[Point::new(x0, y), Point::new(tip, y)]),
            stroke: axis_stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 0,
        });
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[
                Point::new(tip, y),
                Point::new(tip - arr * 0.7, y - arr * 0.5),
            ]),
            stroke: axis_stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 0,
        });
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[
                Point::new(tip, y),
                Point::new(tip - arr * 0.7, y + arr * 0.5),
            ]),
            stroke: axis_stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 0,
        });
    } else {
        let x = axis_c;
        let y0 = min_a - theme::timeline::BLOCK_W / 2.0 - 20.0;
        let y1 = max_a + theme::timeline::BLOCK_W / 2.0 + 20.0;
        let tip = y1 - arr * 1.5;
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[Point::new(x, y0), Point::new(x, tip)]),
            stroke: axis_stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 0,
        });
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[
                Point::new(x, tip),
                Point::new(x - arr * 0.5, tip - arr * 0.7),
            ]),
            stroke: axis_stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 0,
        });
        items.push(SceneItem::Edge {
            path: ir::geograph::line_route(&[
                Point::new(x, tip),
                Point::new(x + arr * 0.5, tip - arr * 0.7),
            ]),
            stroke: axis_stroke.clone(),
            ends: (EdgeEnds::None, EdgeEnds::None),
            z: 0,
        });
    }

    // 每个 section 列：时间点 + section 块（轴上方）+ 事件块（轴下方）+ 连线。
    for (idx, s) in sections.iter().enumerate() {
        let color = theme::timeline::BLOCK_COLORS[idx % theme::timeline::BLOCK_COLORS.len()];
        let (cx, cy) = if horizontal {
            (s.center.x, axis_c)
        } else {
            (axis_c, s.center.y)
        };
        let NodeDetail::TimelineSection { events } = &s.detail else {
            continue;
        };
        let label_text = s.label.as_ref().map(|l| l.text.clone()).unwrap_or_default();
        let dot = Point::new(cx, cy);

        // 时间点（实心圆）。
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Ellipse {
                center: dot,
                rx: theme::timeline::DOT_R,
                ry: theme::timeline::DOT_R,
            },
            fill: Some(Fill::Solid(theme::timeline::DOT)),
            stroke: Some(dot_stroke.clone()),
            name: None,
            z: 1,
        });

        // section 标题块（轴上方最远处）。官方 layout：标题在最顶部，
        // 下方近轴是时间点块，时间轴把「时间点」与「事件」隔开。
        let sec_center = if horizontal {
            Point::new(
                cx,
                cy - theme::timeline::TASK_DY
                    - theme::timeline::BLOCK_H
                    - theme::timeline::SECTION_DY,
            )
        } else {
            Point::new(
                cx - theme::timeline::TASK_DY
                    - theme::timeline::BLOCK_H
                    - theme::timeline::SECTION_DY,
                cy,
            )
        };
        emit_timeline_block(items, sec_center, color, &label_text, 14.0);
        emit_timeline_connector(items, dot, sec_center);

        // 时间点块（轴上方近轴处，events[0]）→ 事件块（轴下方，events[1..]）。
        // 官方 golden：时间点（如 2020）与标题同在轴上方，事件在轴下方，时间轴分隔内容。
        if let Some((first, rest)) = events.split_first() {
            let task_center = if horizontal {
                Point::new(cx, cy - theme::timeline::TASK_DY)
            } else {
                Point::new(cx - theme::timeline::TASK_DY, cy)
            };
            emit_timeline_block(items, task_center, color, first, 13.0);
            emit_timeline_connector(items, dot, task_center);

            for (j, ev) in rest.iter().enumerate() {
                let off = theme::timeline::EVENT_DY
                    + j as f64 * (theme::timeline::BLOCK_H + theme::timeline::EVENT_GAP);
                let ev_center = if horizontal {
                    Point::new(cx, cy + off)
                } else {
                    Point::new(cx + off, cy)
                };
                emit_timeline_block(items, ev_center, color, ev, 13.0);
                emit_timeline_connector(items, dot, ev_center);
            }
        }
    }
}

/// 画一个 timeline 任务块（section / event）：圆角彩色矩形 + 居中文本。
fn emit_timeline_block(
    items: &mut Vec<SceneItem>,
    center: Point,
    color: Color,
    text: &str,
    font_size: f64,
) {
    let at = Point::new(
        center.x - theme::timeline::BLOCK_W / 2.0,
        center.y - theme::timeline::BLOCK_H / 2.0,
    );
    let stroke = Stroke {
        color: theme::timeline::BLOCK_STROKE,
        width: theme::timeline::BLOCK_STROKE_W,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };
    items.push(SceneItem::Shape {
        geometry: ShapeGeometry::RoundedRect {
            at,
            size: Size::new(theme::timeline::BLOCK_W, theme::timeline::BLOCK_H),
            radius: theme::timeline::BLOCK_RX,
        },
        fill: Some(Fill::Solid(color)),
        stroke: Some(stroke),
        name: None,
        z: 0,
    });
    let ts = theme::text_style(
        theme::timeline::BLOCK_TEXT,
        font_size,
        TextAlign::Center,
        TextBaseline::Middle,
    );
    items.push(SceneItem::Label {
        text: vec![RichSpan::new(text.to_string(), ts.clone())],
        position: center,
        style: ts,
        anchor: ir::scenegraph::Anchor::Center,
        z: 2,
    });
}

/// 时间点 → 任务块的连接线（虚线 + 指向块的箭头）。
fn emit_timeline_connector(items: &mut Vec<SceneItem>, dot: Point, block: Point) {
    // 同列垂直连（LR）或同行水平连（TD），由 dot 与 block 的坐标关系推断。
    let vert = (dot.x - block.x).abs() < 1e-6;
    let (dir, from_c, to_c) = if vert {
        let dir = if block.y < dot.y { -1.0 } else { 1.0 };
        (
            dir,
            dot.y + dir * theme::timeline::DOT_R,
            block.y - dir * theme::timeline::BLOCK_H / 2.0,
        )
    } else {
        let dir = if block.x < dot.x { -1.0 } else { 1.0 };
        (
            dir,
            dot.x + dir * theme::timeline::DOT_R,
            block.x - dir * theme::timeline::BLOCK_W / 2.0,
        )
    };
    let (from, tip) = if vert {
        (
            Point::new(dot.x, from_c),
            Point::new(dot.x, to_c - dir * 6.0),
        )
    } else {
        (
            Point::new(from_c, dot.y),
            Point::new(to_c - dir * 6.0, dot.y),
        )
    };
    let stroke = Stroke {
        color: theme::timeline::LINE,
        width: theme::timeline::CONNECTOR_W,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: vec![6.0, 4.0],
        dash_offset: 0.0,
        miter_limit: 4.0,
    };
    items.push(SceneItem::Edge {
        path: ir::geograph::line_route(&[from, tip]),
        stroke: stroke.clone(),
        ends: (EdgeEnds::None, EdgeEnds::None),
        z: 0,
    });
    // 箭头两翼（尖端在 tip，指向块）。
    let asz = 6.0;
    let (w1, w2) = if vert {
        (
            Point::new(tip.x - asz * 0.6, tip.y + dir * asz * 0.8),
            Point::new(tip.x + asz * 0.6, tip.y + dir * asz * 0.8),
        )
    } else {
        (
            Point::new(tip.x + dir * asz * 0.8, tip.y - asz * 0.6),
            Point::new(tip.x + dir * asz * 0.8, tip.y + asz * 0.6),
        )
    };
    items.push(SceneItem::Edge {
        path: ir::geograph::line_route(&[tip, w1]),
        stroke: stroke.clone(),
        ends: (EdgeEnds::None, EdgeEnds::None),
        z: 0,
    });
    items.push(SceneItem::Edge {
        path: ir::geograph::line_route(&[tip, w2]),
        stroke,
        ends: (EdgeEnds::None, EdgeEnds::None),
        z: 0,
    });
}

/// 节点填充/描边按形状语义取色。
fn node_fill_stroke(shape: ShapeKind) -> (Fill, Stroke) {
    let default_stroke = Stroke {
        color: theme::flowchart::STROKE,
        width: NODE_STROKE_WIDTH,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };
    match shape {
        // StartDot：实心深色圆（官方 `.node circle.state-start{fill:#333333}`）。
        ShapeKind::StartDot => (
            Fill::Solid(theme::state::SPECIAL),
            Stroke {
                width: 0.0,
                ..default_stroke
            },
        ),
        // Bar：fork/join 横条，实心深色（官方 `.node .fork-join{fill:#333333}`）。
        ShapeKind::Bar => (
            Fill::Solid(theme::state::SPECIAL),
            Stroke {
                width: 0.0,
                ..default_stroke
            },
        ),
        _ => (Fill::Solid(theme::flowchart::FILL), default_stroke),
    }
}

/// 边颜色/宽度按语义与线型取（flowchart 与 state 同色，class/er 用各自主题色）。
/// Thick 加粗，其余（Solid/Dotted/Invisible）用常规宽度。
fn edge_style(kind: EdgeKind, line: ir::common::LineKind) -> (Color, f64) {
    let width = if line == ir::common::LineKind::Thick {
        theme::EDGE_WIDTH_THICK
    } else {
        theme::EDGE_WIDTH
    };
    let color = match kind {
        EdgeKind::ClassExtends
        | EdgeKind::ClassComposition
        | EdgeKind::ClassAggregation
        | EdgeKind::ClassAssociation
        | EdgeKind::ClassDependency
        | EdgeKind::ClassRealization
        | EdgeKind::ClassLink
        | EdgeKind::ClassDashed => theme::class::EDGE,
        _ => theme::flowchart::EDGE,
    };
    (color, width)
}

/// 单端 ArrowKind → EdgeEnds（起止两端分别查表）。
fn map_ends(kind: ArrowKind) -> EdgeEnds {
    match kind {
        ArrowKind::Arrow => EdgeEnds::Arrow,
        ArrowKind::Circle => EdgeEnds::Circle,
        ArrowKind::Cross => EdgeEnds::Cross,
        ArrowKind::None => EdgeEnds::None,
    }
}

/// 边的起止标记：class 关系据 [`EdgeKind`] 生成特殊端点装饰（三角 / 菱形），
/// 其余据 [`ArrowSpec`] 映射。extract 阶段 class 关系的 arrow 为占位（None/Arrow），
/// 真正的三角 / 菱形在此据 kind 补出。
fn edge_ends(kind: EdgeKind, arrow: ArrowSpec) -> (EdgeEnds, EdgeEnds) {
    match kind {
        EdgeKind::ClassExtends => (EdgeEnds::Triangle, EdgeEnds::None),
        EdgeKind::ClassComposition => (EdgeEnds::DiamondFilled, EdgeEnds::None),
        EdgeKind::ClassAggregation => (EdgeEnds::DiamondHollow, EdgeEnds::None),
        EdgeKind::ClassAssociation => (EdgeEnds::None, EdgeEnds::TriangleFilled),
        EdgeKind::ClassDependency => (EdgeEnds::None, EdgeEnds::TriangleFilled),
        _ => (map_ends(arrow.start), map_ends(arrow.end)),
    }
}

/// 据 ShapeKind + center + size 生成抽象几何描述（paint 据此选 Element 变体）。
fn shape_to_geometry(shape: ShapeKind, center: Point, size: Size) -> ShapeGeometry {
    let half_w = size.width / 2.0;
    let half_h = size.height / 2.0;
    let tl = Point::new(center.x - half_w, center.y - half_h);
    match shape {
        ShapeKind::Rectangle | ShapeKind::Bar | ShapeKind::QuadrantCell => {
            ShapeGeometry::Rect { at: tl, size }
        }
        ShapeKind::Rounded | ShapeKind::Subroutine => ShapeGeometry::RoundedRect {
            at: tl,
            size,
            // 官方默认节点圆角 radius=5（theme::NODE_RADIUS）。
            radius: if shape == ShapeKind::Subroutine {
                2.0
            } else {
                theme::NODE_RADIUS
            },
        },
        ShapeKind::Stadium => ShapeGeometry::Stadium { at: tl, size },
        ShapeKind::Diamond => ShapeGeometry::Polygon {
            points: vec![
                Point::new(center.x, center.y - half_h),
                Point::new(center.x + half_w, center.y),
                Point::new(center.x, center.y + half_h),
                Point::new(center.x - half_w, center.y),
            ],
        },
        ShapeKind::Hexagon => {
            let w = half_w * 0.5;
            ShapeGeometry::Polygon {
                points: vec![
                    Point::new(center.x - w, center.y - half_h),
                    Point::new(center.x + w, center.y - half_h),
                    Point::new(center.x + half_w, center.y),
                    Point::new(center.x + w, center.y + half_h),
                    Point::new(center.x - w, center.y + half_h),
                    Point::new(center.x - half_w, center.y),
                ],
            }
        }
        ShapeKind::Circle | ShapeKind::StartDot | ShapeKind::EndDot | ShapeKind::DoubleCircle => {
            ShapeGeometry::Ellipse {
                center,
                rx: half_w,
                ry: half_h,
            }
        }
        ShapeKind::Asymmetric => {
            let skew = half_w * 0.25;
            ShapeGeometry::Polygon {
                points: vec![
                    Point::new(tl.x + skew, tl.y),
                    Point::new(tl.x + size.width, tl.y),
                    Point::new(tl.x + size.width - skew, tl.y + size.height),
                    Point::new(tl.x, tl.y + size.height),
                ],
            }
        }
        ShapeKind::Parallelogram => {
            let skew = half_w * 0.25;
            ShapeGeometry::Polygon {
                points: vec![
                    Point::new(tl.x + skew, tl.y),
                    Point::new(tl.x + size.width + skew, tl.y),
                    Point::new(tl.x + size.width - skew, tl.y + size.height),
                    Point::new(tl.x - skew, tl.y + size.height),
                ],
            }
        }
        ShapeKind::Trapezoid => {
            let x = half_w * 0.25;
            ShapeGeometry::Polygon {
                points: vec![
                    Point::new(center.x - half_w + x, center.y - half_h),
                    Point::new(center.x + half_w - x, center.y - half_h),
                    Point::new(center.x + half_w, center.y + half_h),
                    Point::new(center.x - half_w, center.y + half_h),
                ],
            }
        }
        ShapeKind::Cylinder => {
            let ry = half_h * 0.2;
            let top = center.y - half_h + ry;
            let bottom = center.y + half_h - ry;
            let mut pts = Vec::new();
            let steps = 12;
            for i in 0..=steps {
                let t = std::f64::consts::PI * (i as f64 / steps as f64);
                let x = center.x - half_w * (t.cos());
                let y = top - ry * (t.sin());
                pts.push(Point::new(x, y));
            }
            pts.push(Point::new(center.x + half_w, bottom));
            for i in 0..=steps {
                let t = std::f64::consts::PI * (i as f64 / steps as f64);
                let x = center.x + half_w * (t.cos());
                let y = bottom + ry * (t.sin());
                pts.push(Point::new(x, y));
            }
            pts.push(Point::new(center.x - half_w, top));
            ShapeGeometry::Polygon { points: pts }
        }
        ShapeKind::PieSlice => ShapeGeometry::Pie {
            center,
            radius: half_w.max(half_h),
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        },
    }
}
