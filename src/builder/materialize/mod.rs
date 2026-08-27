//! Stage 3: Materialize —— 视觉决策点（唯一消费主题常量的地方）。
//!
//! 消费 `Geograph` + `StyleIntent`，把"几何 + 样式意图"解析成具体颜色 / 线型，
//! 产出视觉自足的 [`SceneGraph`]。之后 `paint` 不再有任何 theme 依赖、不再有任何图类型判断。
//!
//! P0.3：仅实现 flowchart 的矩形/菱形/圆 + 直线边 + 节点文本；其余形状/线型细化留 P1.3。

use lievisual::geometry::{Point, Size};
use lievisual::scene::{Fill, LineCap, LineJoin, Stroke};

use crate::builder::ir::{
    self,
    common::ArrowKind,
    geograph::Geograph,
    shape::{EdgeEnds, ShapeGeometry, ShapeKind},
    SceneGraph, SceneItem, StyleIntent,
};
use crate::builder::theme;

const NODE_STROKE_WIDTH: f64 = 1.0;

/// 几何 + 视觉意图 → 视觉自足的场景图。
pub fn run(gg: &Geograph, _style: &StyleIntent) -> SceneGraph {
    let mut items = Vec::new();

    // —— 节点：形状 + 文本 ——
    for n in &gg.nodes {
        let geometry = shape_to_geometry(n.shape, n.center, n.size);
        let fill = Fill::Solid(theme::flowchart::FILL);
        let stroke = Stroke {
            color: theme::flowchart::STROKE,
            width: NODE_STROKE_WIDTH,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash_array: Vec::new(),
            dash_offset: 0.0,
            miter_limit: 4.0,
        };
        items.push(SceneItem::Shape {
            geometry,
            fill: Some(fill),
            stroke: Some(stroke),
            z: 0,
        });

        // 节点文本
        if let Some(label) = &n.label {
            items.push(SceneItem::Label {
                text: label.spans.clone(),
                position: n.center,
                style: lievisual::text::TextStyle::new(
                    theme::flowchart::TEXT,
                    theme::FONT_SIZE,
                    theme::FONT_FAMILY,
                )
                .with_align(lievisual::text::TextAlign::Center)
                .with_baseline(lievisual::text::TextBaseline::Middle),
                anchor: ir::scenegraph::Anchor::Center,
                z: 2,
            });
        }
    }

    // —— 边 ——
    for e in &gg.edges {
        let stroke = Stroke {
            color: theme::flowchart::EDGE,
            width: theme::EDGE_WIDTH,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash_array: Vec::new(),
            dash_offset: 0.0,
            miter_limit: 4.0,
        };
        let ends = match e.arrow.end {
            ArrowKind::Arrow => EdgeEnds::Arrow,
            ArrowKind::Circle => EdgeEnds::Circle,
            ArrowKind::Cross => EdgeEnds::Cross,
            ArrowKind::None => EdgeEnds::None,
        };
        items.push(SceneItem::Edge {
            path: e.route.clone(),
            stroke,
            ends,
            z: 1,
        });
    }

    SceneGraph {
        size: gg.size,
        background: gg.background,
        items,
    }
}

/// 据 ShapeKind + center + size 生成抽象几何描述（paint 据此选 Element 变体）。
fn shape_to_geometry(shape: ShapeKind, center: Point, size: Size) -> ShapeGeometry {
    let half_w = size.width / 2.0;
    let half_h = size.height / 2.0;
    let tl = Point::new(center.x - half_w, center.y - half_h);
    match shape {
        ShapeKind::Rectangle => ShapeGeometry::Rect { at: tl, size },
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
        ShapeKind::Circle | ShapeKind::StartDot => ShapeGeometry::Ellipse {
            center,
            rx: half_w,
            ry: half_h,
        },
        ShapeKind::DoubleCircle | ShapeKind::EndDot => {
            // 双环：外圈实心 + 内圈镂空（用 Path 画两个同心椭圆环，paint 用 stroke 表现）。
            ShapeGeometry::Ellipse {
                center,
                rx: half_w,
                ry: half_h,
            }
        }
        ShapeKind::Bar => ShapeGeometry::Rect { at: tl, size },
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
            // 圆柱：用多边形采样近似（顶部椭圆弧 + 两侧 + 底部椭圆弧）。
            let ry = half_h * 0.2;
            let top = center.y - half_h + ry;
            let bottom = center.y + half_h - ry;
            let mut pts = Vec::new();
            let steps = 12;
            // 顶弧（左→右，向上凸起）
            for i in 0..=steps {
                let t = std::f64::consts::PI * (i as f64 / steps as f64);
                let x = center.x - half_w * (t.cos());
                let y = top - ry * (t.sin());
                pts.push(Point::new(x, y));
            }
            // 右侧下落到底弧
            pts.push(Point::new(center.x + half_w, bottom));
            // 底弧（右→左，向下凸起）
            for i in 0..=steps {
                let t = std::f64::consts::PI * (i as f64 / steps as f64);
                let x = center.x + half_w * (t.cos());
                let y = bottom + ry * (t.sin());
                pts.push(Point::new(x, y));
            }
            // 回到左侧顶部
            pts.push(Point::new(center.x - half_w, top));
            ShapeGeometry::Polygon { points: pts }
        }
        ShapeKind::PieSlice => ShapeGeometry::Pie {
            center,
            radius: half_w.max(half_h),
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        },
        ShapeKind::QuadrantCell => ShapeGeometry::Rect { at: tl, size },
    }
}
