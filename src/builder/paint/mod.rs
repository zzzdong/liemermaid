//! Stage 4: Paint —— 零分支纯翻译。
//!
//! `fn run(&SceneGraph) -> Scene` 不接收 `&Diagram`、不接收 `&Theme`，
//! 结构上杜绝回查 AST。这是"渲染解耦"痛点的彻底解决。
//!
//! P0.3：支持 Rect/RoundedRect/Polygon/Ellipse 形状 + 直线边 + 富文本；
//! 箭头标记（EdgeEnds）留 P1.3 补充（当前边仅画线，不画箭头头部）。

use lievisual::geometry::{Point, Rect, Vec2};
use lievisual::scene::{Element, Fill, FillStrokeStyle, Scene, SceneNode, Stroke};

use crate::builder::ir::{
    geograph::{RoutePath, RouteSegment},
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
                name,
                z,
            } => {
                let style = FillStrokeStyle {
                    fill: fill.clone(),
                    stroke: stroke.clone(),
                };
                let element = geometry_to_element(geometry, style);
                let node = SceneNode::new(element).with_z(*z);
                if let Some(n) = name {
                    node.with_name(n)
                } else {
                    node
                }
            }
            SceneItem::Edge { path, stroke, ends, z } => {
                // P1.3：多段折线 + 起止标记（箭头/圆/叉）。
                let nodes = run_edge_nodes(path, stroke, ends, *z);
                
                if nodes.is_empty() {
                    continue;
                } else if nodes.len() == 1 {
                    nodes.into_iter().next().unwrap()
                } else {
                    SceneNode::group(nodes).with_z(*z)
                }
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
                    .filter_map(|c| run_item(c))
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
            name,
            z,
        } => {
            let style = FillStrokeStyle {
                fill: fill.clone(),
                stroke: stroke.clone(),
            };
            let node = SceneNode::new(geometry_to_element(geometry, style)).with_z(*z);
            Some(if let Some(n) = name { node.with_name(n) } else { node })
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

/// 把一条已路由边翻译为若干 SceneNode（贝塞尔曲线 + 起止标记）。
///
/// `ends: (start, end)`：起、终两端各自的标记类型；普通 flowchart 边起点为 None，
/// 只在有箭头语义的一端画标记（修复"普通 `-->` 边出现双向箭头"的问题）。
///
/// `path` 为路由段序列（直线/贝塞尔段），**按段类型直接描边**（不做折线→弧线的
/// 后处理）。箭头方向取自路由首/末段的**端切线**，与曲线切向严格对齐
/// （修复"箭头看不完整/方向错位"）。
fn run_edge_nodes(
    path: &RoutePath,
    stroke: &Stroke,
    ends: &(EdgeEnds, EdgeEnds),
    z: i32,
) -> Vec<SceneNode> {
    let mut nodes = Vec::new();
    if path.is_empty() {
        return nodes;
    }
    // 曲线本体：把路由段翻译成 BezPath（Line→line_to, CubicBezier→curve_to）。
    nodes.push(
        SceneNode::new(Element::Path {
            path: path_to_bezpath(path),
            style: FillStrokeStyle {
                fill: None,
                stroke: Some(stroke.clone()),
            },
            closed: false,
        })
        .with_z(z),
    );

    // 终点标记：末段方向（指向端口）。
    let (start_ends, end_ends) = *ends;
    if end_ends != EdgeEnds::None {
        let d = path.last_direction();
        let tip = path.end();
        let from = Point::new(tip.x - d.x * 16.0, tip.y - d.y * 16.0);
        nodes.extend(arrow_element(from, tip, &end_ends, stroke, z));
    }
    // 起点标记：首段方向反向（从起点向外）。
    if start_ends != EdgeEnds::None {
        let d = path.first_direction();
        let tip = path.start();
        let from = Point::new(tip.x + d.x * 16.0, tip.y + d.y * 16.0);
        nodes.extend(arrow_element(from, tip, &start_ends, stroke, z));
    }

    nodes
}

/// 把路由段序列（Line/CubicBezier）翻译成连续 `BezPath`。
fn path_to_bezpath(path: &RoutePath) -> lievisual::geometry::BezPath {
    use lievisual::geometry::BezPath;
    let mut bp = BezPath::new();
    let mut pen_set = false;
    for seg in path.iter() {
        match seg {
            RouteSegment::Line { from, to } => {
                if !pen_set {
                    bp.move_to((from.x, from.y));
                    pen_set = true;
                }
                bp.line_to((to.x, to.y));
            }
            RouteSegment::CubicBezier { p0, p1, p2, p3 } => {
                if !pen_set {
                    bp.move_to((p0.x, p0.y));
                    pen_set = true;
                }
                bp.curve_to((p1.x, p1.y), (p2.x, p2.y), (p3.x, p3.y));
            }
        }
    }
    bp
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
    let hollow = FillStrokeStyle {
        fill: None,
        stroke: Some(stroke.clone()),
    };
    let solid = FillStrokeStyle {
        fill: Some(Fill::Solid(stroke.color)),
        stroke: Some(stroke.clone()),
    };
    // 三角（空心 / 实心）：尖端在 tip，底边沿 -u 方向外扩。
    let triangle = |f: &FillStrokeStyle| -> Vec<SceneNode> {
        let left = Point::new(tip.x - ux * size + px * size * 0.5, tip.y - uy * size + py * size * 0.5);
        let right = Point::new(tip.x - ux * size - px * size * 0.5, tip.y - uy * size - py * size * 0.5);
        vec![SceneNode::new(Element::polygon(vec![left, tip, right], f.clone())).with_z(z)]
    };
    // 菱形（空心 / 实心）：front 沿 -u（离开端点）、back 沿 +u（进入端点）。
    let diamond = |f: &FillStrokeStyle| -> Vec<SceneNode> {
        let front = Point::new(tip.x - ux * size, tip.y - uy * size);
        let back = Point::new(tip.x + ux * size, tip.y + uy * size);
        let p1 = Point::new(tip.x + px * size * 0.7, tip.y + py * size * 0.7);
        let p2 = Point::new(tip.x - px * size * 0.7, tip.y - py * size * 0.7);
        vec![SceneNode::new(Element::polygon(vec![front, p1, back, p2], f.clone())).with_z(z)]
    };
    match ends {
        // 官方 flowchart / state 箭头为**实心**（`.arrowheadPath{fill:#333333}`，
        // marker path `M 0 0 L 10 5 L 0 10 z` 闭合填充）；class 继承的空心三角
        // 才是 `fill:none`。
        EdgeEnds::Arrow | EdgeEnds::TriangleFilled => out.extend(triangle(&solid)),
        EdgeEnds::Triangle => out.extend(triangle(&hollow)),
        EdgeEnds::DiamondFilled => out.extend(diamond(&solid)),
        EdgeEnds::DiamondHollow => out.extend(diamond(&hollow)),
        EdgeEnds::Circle => {
            let c = Point::new(tip.x - ux * size * 0.5, tip.y - uy * size * 0.5);
            out.push(SceneNode::new(Element::ellipse(c, Vec2::new(size * 0.4, size * 0.4), 0.0, hollow)).with_z(z));
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
