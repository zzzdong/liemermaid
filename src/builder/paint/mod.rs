//! Stage 4: Paint —— 零分支纯翻译。
//!
//! `fn run(&SceneGraph) -> Scene` 不接收 `&Diagram`、不接收 `&Theme`，
//! 结构上杜绝回查 AST。这是"渲染解耦"痛点的彻底解决。
//!
//! P0.3：支持 Rect/RoundedRect/Polygon/Ellipse 形状 + 直线边 + 富文本；
//! 箭头标记（EdgeEnds）留 P1.3 补充（当前边仅画线，不画箭头头部）。

use lievisual::geometry::{Point, Rect, Vec2};
use lievisual::scene::{Element, FillStrokeStyle, Scene, SceneNode, Stroke};

use crate::builder::ir::{
    scenegraph::{SceneGraph, SceneItem},
    shape::{EdgeEnds, ShapeGeometry},
};

/// 视觉自足的场景图 → lievisual 原语集合。
pub fn run(sg: &SceneGraph) -> Scene {
    let mut scene = Scene::new(sg.size.width, sg.size.height).with_background(sg.background);

    for item in &sg.items {
        let node: SceneNode = match item {
            SceneItem::Shape {
                geometry,
                fill,
                stroke,
                z,
            } => {
                let style = FillStrokeStyle {
                    fill: fill.clone(),
                    stroke: stroke.clone(),
                };
                let element = geometry_to_element(geometry, style);
                SceneNode::new(element).with_z(*z)
            }
            SceneItem::Edge { path, stroke, ends, z } => {
                // P1.3：多段折线 + 起止标记（箭头/圆/叉）。
                let nodes = run_edge_nodes(path, stroke, ends, *z);
                let node = if nodes.is_empty() {
                    continue;
                } else if nodes.len() == 1 {
                    nodes.into_iter().next().unwrap()
                } else {
                    SceneNode::group(nodes).with_z(*z)
                };
                node
            }
            SceneItem::Label {
                text,
                position,
                style,
                anchor: _anchor,
                z,
            } => {
                let element = Element::rich_text(text.clone(), *position, style.clone());
                SceneNode::new(element).with_z(*z)
            }
            SceneItem::Group { children, z } => {
                let children: Vec<SceneNode> = children
                    .iter()
                    .filter_map(|c| match run_item(c) {
                        Some(n) => Some(n),
                        None => None,
                    })
                    .collect();
                SceneNode::group(children).with_z(*z)
            }
        };
        scene.push_node(node);
    }

    scene
}

/// 单个 item → SceneNode（供 Group 递归）。
fn run_item(item: &SceneItem) -> Option<SceneNode> {
    match item {
        SceneItem::Shape {
            geometry,
            fill,
            stroke,
            z,
        } => {
            let style = FillStrokeStyle {
                fill: fill.clone(),
                stroke: stroke.clone(),
            };
            Some(SceneNode::new(geometry_to_element(geometry, style)).with_z(*z))
        }
        SceneItem::Edge { path, stroke, ends, z } => {
            let nodes = run_edge_nodes(path, stroke, ends, *z);
            if nodes.is_empty() {
                None
            } else if nodes.len() == 1 {
                Some(nodes.into_iter().next().unwrap())
            } else {
                Some(SceneNode::group(nodes).with_z(*z))
            }
        }
        SceneItem::Label {
            text,
            position,
            style,
            anchor: _anchor,
            z,
        } => Some(SceneNode::new(Element::rich_text(text.clone(), *position, style.clone())).with_z(*z)),
        SceneItem::Group { children, z } => {
            let children: Vec<SceneNode> = children.iter().filter_map(run_item).collect();
            Some(SceneNode::group(children).with_z(*z))
        }
    }
}

/// 抽象几何 → lievisual Element。
fn geometry_to_element(geometry: &ShapeGeometry, style: FillStrokeStyle) -> Element {
    match geometry {
        ShapeGeometry::Rect { at, size } => {
            let rect = Rect::new(at.x, at.y, at.x + size.width, at.y + size.height);
            Element::rect(rect, style)
        }
        ShapeGeometry::RoundedRect { at, size, radius } => {
            let rect = Rect::new(at.x, at.y, at.x + size.width, at.y + size.height);
            Element::rounded_rect(rect, *radius, style)
        }
        ShapeGeometry::Stadium { at, size } => {
            let rect = Rect::new(at.x, at.y, at.x + size.width, at.y + size.height);
            // stadium：圆角半径取半高，呈现药丸形。
            let r = (size.height / 2.0).min(size.width / 2.0);
            Element::rounded_rect(rect, r, style)
        }
        ShapeGeometry::Diamond { center, size } => {
            let hw = size.width / 2.0;
            let hh = size.height / 2.0;
            let pts = vec![
                Point::new(center.x, center.y - hh),
                Point::new(center.x + hw, center.y),
                Point::new(center.x, center.y + hh),
                Point::new(center.x - hw, center.y),
            ];
            Element::polygon(pts, style)
        }
        ShapeGeometry::Polygon { points } => Element::polygon(points.clone(), style),
        ShapeGeometry::Ellipse { center, rx, ry } => {
            Element::ellipse(*center, Vec2::new(*rx, *ry), 0.0, style)
        }
        ShapeGeometry::Path { ops } => {
            // lievisual 0.1.2 无原生 Path 变体，几何层 PathOp 已展开为足够采样点，
            // 此处收集所有 MoveTo/LineTo/ArcTo 端点为折线多边形渲染。
            let mut pts: Vec<Point> = Vec::new();
            for op in ops {
                match op {
                    crate::builder::ir::shape::PathOp::MoveTo(p)
                    | crate::builder::ir::shape::PathOp::LineTo(p)
                    | crate::builder::ir::shape::PathOp::ArcTo(p, _) => pts.push(*p),
                    crate::builder::ir::shape::PathOp::CurveTo(_, _, p) => pts.push(*p),
                }
            }
            if pts.len() >= 3 {
                Element::polygon(pts, style)
            } else {
                Element::rect(Rect::new(0.0, 0.0, 1.0, 1.0), style)
            }
        }
        ShapeGeometry::Pie {
            center,
            radius,
            start_angle,
            end_angle,
        } => Element::pie(*center, *radius, *start_angle, *end_angle, style),
    }
}

/// 把一条已路由边翻译为若干 SceneNode（折线 + 起止标记）。
fn run_edge_nodes(path: &[Point], stroke: &Stroke, ends: &EdgeEnds, z: i32) -> Vec<SceneNode> {
    let mut nodes = Vec::new();
    if path.len() < 2 {
        return nodes;
    }
    // 折线本体（正交/样条统一用 polyline，paint 不做二次平滑）。
    nodes.push(SceneNode::new(Element::poly(path.to_vec(), stroke.clone())).with_z(z));

    // 终点标记（基于最后一段方向）。
    let last = path[path.len() - 1];
    let prev = path[path.len() - 2];
    nodes.extend(arrow_element(prev, last, ends, stroke, z));

    // 起点标记（基于第一段方向，反向）。
    let first = path[0];
    let next = path[1];
    nodes.extend(arrow_element(next, first, ends, stroke, z));

    nodes
}

/// 在 `tip` 处、沿 `from→tip` 方向画一个标记（Arrow/Circle/Cross）。
fn arrow_element(from: Point, tip: Point, ends: &EdgeEnds, stroke: &Stroke, z: i32) -> Vec<SceneNode> {
    let mut out = Vec::new();
    let dx = tip.x - from.x;
    let dy = tip.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return out;
    }
    let ux = dx / len;
    let uy = dy / len;
    // 垂直单位向量。
    let px = -uy;
    let py = ux;
    let size = 8.0;
    let style = FillStrokeStyle {
        fill: None,
        stroke: Some(stroke.clone()),
    };
    match ends {
        EdgeEnds::Arrow => {
            let left = Point::new(tip.x - ux * size + px * size * 0.5, tip.y - uy * size + py * size * 0.5);
            let right = Point::new(tip.x - ux * size - px * size * 0.5, tip.y - uy * size - py * size * 0.5);
            let pts = vec![left, tip, right];
            out.push(SceneNode::new(Element::polygon(pts, style)).with_z(z));
        }
        EdgeEnds::Circle => {
            let c = Point::new(tip.x - ux * size * 0.5, tip.y - uy * size * 0.5);
            out.push(SceneNode::new(Element::ellipse(c, Vec2::new(size * 0.4, size * 0.4), 0.0, style)).with_z(z));
        }
        EdgeEnds::Cross => {
            // 以 tip 为交点的两条对角线（沿/垂直边方向）。
            let ox = ux * size * 0.5;
            let oy = uy * size * 0.5;
            let qx = px * size * 0.4;
            let qy = py * size * 0.4;
            let a1 = Point::new(tip.x - ox + qx, tip.y - oy + qy);
            let a2 = Point::new(tip.x + ox - qx, tip.y + oy - qy);
            let b1 = Point::new(tip.x - ox - qx, tip.y - oy - qy);
            let b2 = Point::new(tip.x + ox + qx, tip.y + oy + qy);
            out.push(SceneNode::new(Element::line(a1, a2, stroke.clone())).with_z(z));
            out.push(SceneNode::new(Element::line(b1, b2, stroke.clone())).with_z(z));
        }
        _ => {}
    }
    out
}
