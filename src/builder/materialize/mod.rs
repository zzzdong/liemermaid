//! Stage 3: Materialize —— 视觉决策点（唯一消费主题常量的地方）。
//!
//! 消费 `Geograph` + `StyleIntent`，把"几何 + 样式意图"解析成具体颜色 / 线型，
//! 产出视觉自足的 [`SceneGraph`]。之后 `paint` 不再有任何 theme 依赖、不再有任何图类型判断。
//!
//! 支持 flowchart + state：节点按 [`ShapeKind`]（含 StartDot 实心圆 / EndDot 双环 /
//! Bar 横条），边按 [`EdgeKind`] 选 flowchart / state 主题，边标签绘制白底文本。

use lievisual::geometry::{Color, Point, Size};
use lievisual::scene::{Fill, LineCap, LineJoin, Stroke};
use lievisual::text::{TextAlign, TextBaseline, TextStyle};

use crate::builder::ir::{
    self,
    common::ArrowKind,
    geograph::Geograph,
    shape::{EdgeEnds, ShapeGeometry, ShapeKind},
    unigraph::EdgeKind,
    SceneGraph, SceneItem, StyleIntent,
};
use crate::builder::theme;

const NODE_STROKE_WIDTH: f64 = 1.0;

/// 几何 + 视觉意图 → 视觉自足的场景图。
pub fn run(gg: &Geograph, _style: &StyleIntent) -> SceneGraph {
    let mut items = Vec::new();

    // —— 子图容器：背景 + 边框 + 标题（z 低于节点，保证节点覆盖其上）——
    let container_stroke = Stroke {
        color: theme::flowchart::STROKE,
        width: 1.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        miter_limit: 4.0,
    };
    for c in &gg.containers {
        let r = c.bounds;
        items.push(SceneItem::Shape {
            geometry: ShapeGeometry::Rect {
                at: Point::new(r.min_x(), r.min_y()),
                size: Size::new(r.width(), r.height()),
            },
            fill: Some(Fill::Solid(Color::new(0.98, 0.98, 1.0, 0.5))),
            stroke: Some(container_stroke.clone()),
            name: None,
            z: -1,
        });
        if let Some(title) = &c.title
            && !title.is_empty()
        {
            items.push(SceneItem::Label {
                text: vec![lievisual::text::RichSpan::new(
                    title.clone(),
                    TextStyle::new(
                        theme::flowchart::STROKE,
                        theme::FONT_SIZE,
                        theme::FONT_FAMILY,
                    )
                    .with_align(TextAlign::Left)
                    .with_baseline(TextBaseline::Middle),
                )],
                position: Point::new(r.min_x() + 8.0, r.min_y() + 12.0),
                style: TextStyle::new(
                    theme::flowchart::STROKE,
                    theme::FONT_SIZE,
                    theme::FONT_FAMILY,
                )
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Middle),
                anchor: ir::scenegraph::Anchor::Left,
                z: 0,
            });
        }
    }

    // —— 节点：形状 + 文本 ——
    for n in &gg.nodes {
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
                style: TextStyle::new(
                    theme::flowchart::TEXT,
                    theme::FONT_SIZE,
                    theme::FONT_FAMILY,
                )
                .with_align(TextAlign::Center)
                .with_baseline(TextBaseline::Middle),
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
            Color::new(0.0, 0.0, 0.0, 0.0)
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
        let ends = (map_ends(e.arrow.start), map_ends(e.arrow.end));
        items.push(SceneItem::Edge {
            path: e.route.clone(),
            stroke,
            ends,
            z: 1,
        });

        // 边标签：白底 + 居中文本（锚点来自 route 中点）。
        if let (Some(text), Some(anchor)) = (&e.label_text, &e.label_anchor)
            && !text.is_empty()
        {
            let ts = TextStyle::new(
                theme::flowchart::TEXT,
                theme::FONT_SIZE,
                theme::FONT_FAMILY,
            )
            .with_align(TextAlign::Center)
            .with_baseline(TextBaseline::Middle);
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
                size: Size::new(
                    layout.width + 2.0 * pad_x,
                    layout.height + 2.0 * pad_y,
                ),
            };
            items.push(SceneItem::Shape {
                geometry: bg,
                fill: Some(Fill::Solid(Color::new(1.0, 1.0, 1.0, 1.0))),
                stroke: None,
                name: Some("edge-label".to_string()),
                z: 1,
            });
            items.push(SceneItem::Label {
                text: vec![lievisual::text::RichSpan::new(text.clone(), ts)],
                position: *anchor,
                style: TextStyle::new(
                    theme::flowchart::TEXT,
                    theme::FONT_SIZE,
                    theme::FONT_FAMILY,
                )
                .with_align(TextAlign::Center)
                .with_baseline(TextBaseline::Middle),
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
        // StartDot：实心深色圆（用描边色填充）。
        ShapeKind::StartDot => (
            Fill::Solid(theme::state::STROKE),
            Stroke { width: 0.0, ..default_stroke },
        ),
        // Bar：fork/join 横条，实心深色。
        ShapeKind::Bar => (
            Fill::Solid(theme::state::STROKE),
            Stroke { width: 0.0, ..default_stroke },
        ),
        _ => (Fill::Solid(theme::flowchart::FILL), default_stroke),
    }
}

/// 边颜色/宽度按语义与线型取（flowchart 与 state 同色，此处统一用 flowchart 常量）。
/// Thick 加粗，其余（Solid/Dotted/Invisible）用常规宽度。
fn edge_style(_kind: EdgeKind, line: ir::common::LineKind) -> (Color, f64) {
    let width = if line == ir::common::LineKind::Thick {
        theme::EDGE_WIDTH * 2.5
    } else {
        theme::EDGE_WIDTH
    };
    (theme::flowchart::EDGE, width)
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
            radius: if shape == ShapeKind::Subroutine { 2.0 } else { 8.0 },
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
