//! `DirectedRenderer`：flowchart / state 的有向图渲染。
//!
//! 拿 `PlacedGraph`（纯几何）+ AST（`Flowchart` / `StateDiagram`），
//! 回查节点形状 / 文本 / 箭头类型，组合绘制成 `SceneNode`。不修改坐标。

use lievisual::geometry::{BezPath, Point, Rect, Size};
use lievisual::scene::SceneNode;
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

use crate::ast::{Diagram, Flowchart, NodeShape, StateDiagram};
use crate::builder::layout::ir::PlacedGraph;
use crate::builder::layout::measure::measure_nodes;
use crate::builder::layout::recognize::all_flowchart_nodes;
use crate::builder::types::OutputConfig;
use crate::vir::{
    self, Color, TextAlign, TextBaseline, Z_AXIS, Z_LABEL, Z_SERIES, Z_SUBGRAPH, draw_arrow_head,
    theme,
};

/// 节点渲染样式。
pub struct NodeRenderStyle {
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width: f64,
}

impl Default for NodeRenderStyle {
    fn default() -> Self {
        Self {
            fill: theme::flowchart::FILL,
            stroke: theme::flowchart::STROKE,
            stroke_width: 2.0,
        }
    }
}

/// 按节点形状绘制。纯函数：只依赖 center / size / shape / style。
pub fn draw_node_shape(
    elements: &mut Vec<SceneNode>,
    center: Point,
    size: Size,
    shape: Option<NodeShape>,
    style: &NodeRenderStyle,
) {
    let fs = vir::fs_both(style.fill, style.stroke, style.stroke_width);
    let rect = Rect::new(
        center.x - size.width / 2.0,
        center.y - size.height / 2.0,
        center.x + size.width / 2.0,
        center.y + size.height / 2.0,
    );

    match shape {
        Some(NodeShape::Circle) => {
            let radius = size.width / 2.0;
            elements.push(vir::circle_node(center, radius, fs, Z_SERIES));
        }
        Some(NodeShape::DoubleCircle) => {
            let outer_r = size.width / 2.0;
            let inner_r = outer_r * 0.75;
            elements.push(vir::circle_node(center, outer_r, fs, Z_SERIES));
            elements.push(vir::circle_node(
                center,
                inner_r,
                vir::fs_stroke(style.stroke, 2.0),
                Z_SERIES,
            ));
        }
        Some(NodeShape::Stadium) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let r = h;
            let segments = 12;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x - w + r, center.y - h));
            path.line_to(Point::new(center.x + w - r, center.y - h));
            for i in 0..=segments {
                let a = -std::f64::consts::FRAC_PI_2
                    + std::f64::consts::FRAC_PI_2 * 2.0 * i as f64 / segments as f64;
                path.line_to(Point::new(
                    center.x + w - r + r * a.cos(),
                    center.y + r * a.sin(),
                ));
            }
            path.line_to(Point::new(center.x - w + r, center.y + h));
            for i in 0..=segments {
                let a = std::f64::consts::FRAC_PI_2
                    + std::f64::consts::FRAC_PI_2 * 2.0 * i as f64 / segments as f64;
                path.line_to(Point::new(
                    center.x - w + r + r * a.cos(),
                    center.y + r * a.sin(),
                ));
            }
            path.close_path();
            elements.push(vir::path_node(path, fs, Z_SERIES));
        }
        Some(NodeShape::Cylinder) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let ry = (2.0 * h) * 0.13;
            let segments = 16;
            let left = center.x - w;
            let right = center.x + w;
            let top_y = center.y - h;
            let bot_y = center.y + h;
            let mut body = BezPath::new();
            body.move_to(Point::new(left, top_y + ry));
            body.line_to(Point::new(left, bot_y - ry));
            for i in 0..=segments {
                let a = std::f64::consts::PI * i as f64 / segments as f64;
                body.line_to(Point::new(
                    center.x - w * a.cos(),
                    bot_y - ry + ry * a.sin(),
                ));
            }
            body.line_to(Point::new(right, top_y + ry));
            body.close_path();
            elements.push(vir::path_node(body, fs.clone(), Z_SERIES));
            let mut top = BezPath::new();
            top.move_to(Point::new(left, top_y + ry));
            for i in 0..=segments {
                let a = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
                top.line_to(Point::new(
                    center.x - w * a.cos(),
                    top_y + ry + ry * a.sin(),
                ));
            }
            top.close_path();
            elements.push(vir::path_node(top, fs.clone(), Z_SERIES));
        }
        Some(NodeShape::Subroutine) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let left = center.x - w;
            let right = center.x + w;
            let top = center.y - h;
            let bot = center.y + h;
            let mut outer = BezPath::new();
            outer.move_to(Point::new(left, top));
            outer.line_to(Point::new(right, top));
            outer.line_to(Point::new(right, bot));
            outer.line_to(Point::new(left, bot));
            outer.close_path();
            elements.push(vir::path_node(outer, fs.clone(), Z_SERIES));
            let notch = 8.0;
            let inner = vir::stroke(style.stroke, style.stroke_width);
            elements.push(vir::line_node(
                Point::new(left + notch, top),
                Point::new(left + notch, bot),
                inner.clone(),
                Z_SERIES,
            ));
            elements.push(vir::line_node(
                Point::new(right - notch, top),
                Point::new(right - notch, bot),
                inner,
                Z_SERIES,
            ));
        }
        Some(NodeShape::Diamond) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x, center.y - h));
            path.line_to(Point::new(center.x + w, center.y));
            path.line_to(Point::new(center.x, center.y + h));
            path.line_to(Point::new(center.x - w, center.y));
            path.close_path();
            elements.push(vir::path_node(path, fs, Z_SERIES));
        }
        Some(NodeShape::Hexagon) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let inset = w * 0.3;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x - w + inset, center.y - h));
            path.line_to(Point::new(center.x + w - inset, center.y - h));
            path.line_to(Point::new(center.x + w, center.y));
            path.line_to(Point::new(center.x + w - inset, center.y + h));
            path.line_to(Point::new(center.x - w + inset, center.y + h));
            path.line_to(Point::new(center.x - w, center.y));
            path.close_path();
            elements.push(vir::path_node(path, fs, Z_SERIES));
        }
        Some(NodeShape::Asymmetric) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let q = w * 0.3;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x - w + q, center.y - h));
            path.line_to(Point::new(center.x + w - q, center.y - h));
            path.line_to(Point::new(center.x + w, center.y));
            path.line_to(Point::new(center.x + w - q, center.y + h));
            path.line_to(Point::new(center.x - w + q, center.y + h));
            path.line_to(Point::new(center.x - w, center.y));
            path.close_path();
            elements.push(vir::path_node(path, fs, Z_SERIES));
        }
        Some(NodeShape::Parallelogram) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let skew = w * 0.25;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x - w + skew, center.y - h));
            path.line_to(Point::new(center.x + w, center.y - h));
            path.line_to(Point::new(center.x + w - skew, center.y + h));
            path.line_to(Point::new(center.x - w, center.y + h));
            path.close_path();
            elements.push(vir::path_node(path, fs, Z_SERIES));
        }
        Some(NodeShape::ParallelogramAlt) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let skew = w * 0.25;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x - w, center.y - h));
            path.line_to(Point::new(center.x + w - skew, center.y - h));
            path.line_to(Point::new(center.x + w, center.y + h));
            path.line_to(Point::new(center.x - w + skew, center.y + h));
            path.close_path();
            elements.push(vir::path_node(path, fs, Z_SERIES));
        }
        Some(NodeShape::Trapezoid) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let inset = w * 0.2;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x - w + inset, center.y - h));
            path.line_to(Point::new(center.x + w - inset, center.y - h));
            path.line_to(Point::new(center.x + w, center.y + h));
            path.line_to(Point::new(center.x - w, center.y + h));
            path.close_path();
            elements.push(vir::path_node(path, fs, Z_SERIES));
        }
        Some(NodeShape::TrapezoidAlt) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let inset = w * 0.2;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x - w, center.y - h));
            path.line_to(Point::new(center.x + w, center.y - h));
            path.line_to(Point::new(center.x + w - inset, center.y + h));
            path.line_to(Point::new(center.x - w + inset, center.y + h));
            path.close_path();
            elements.push(vir::path_node(path, fs, Z_SERIES));
        }
        Some(NodeShape::Rounded) => {
            elements.push(vir::rect_node(rect, Some(theme::NODE_RADIUS), fs, Z_SERIES));
        }
        // Rectangle 及 None（无形状）：矩形兜底
        Some(NodeShape::Rectangle) | None => {
            elements.push(vir::rect_node(rect, None, fs, Z_SERIES));
        }
    }
}

/// 画节点文本（居中）。
fn draw_node_label(elements: &mut Vec<SceneNode>, center: Point, text: &str, max_w: f64) {
    if text.is_empty() {
        return;
    }
    let style = vir::text_style(
        theme::flowchart::TEXT,
        theme::FONT_SIZE,
        theme::FONT_FAMILY,
        TextAlign::Center,
        TextBaseline::Middle,
    );
    let layout = layout_text(
        &[RichSpan::new(text.to_string(), style.clone())],
        Some(max_w),
    );
    let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
    elements.push(vir::text_node(
        text.to_string(),
        Point::new(center.x + x_off, center.y + y_off),
        style
            .with_align(TextAlign::Left)
            .with_baseline(TextBaseline::Top),
        0.0,
        Some(max_w),
        Z_LABEL,
    ));
}

/// `DirectedRenderer`。
pub struct DirectedRenderer;

impl DirectedRenderer {
    /// 渲染 flowchart（无子图：`placed.edge_routes` 与 `fc.edges` 同序）。
    pub fn render_flowchart(
        placed: &PlacedGraph,
        fc: &Flowchart,
        config: &OutputConfig,
    ) -> Vec<SceneNode> {
        let mut elements = Vec::new();

        // 节点：顺序 = all_flowchart_nodes = LayoutGraph.nodes 顺序
        let nodes = all_flowchart_nodes(fc);
        let metrics = measure_nodes(&nodes, config);
        for (i, node) in nodes.iter().enumerate() {
            if i >= placed.positions.len() {
                break;
            }
            let center = placed.positions[i];
            let size = metrics
                .get(&node.id)
                .map(|m| Size::new(m.size.width, m.size.height))
                .unwrap_or(Size::new(120.0, 60.0));
            let style = NodeRenderStyle::default();
            draw_node_shape(&mut elements, center, size, node.shape.clone(), &style);
            // 标签：优先用显式文本，否则回退到节点 id（与 measure_node 一致）
            let label = node.text.as_deref().unwrap_or(&node.id);
            draw_node_label(&mut elements, center, label, size.width - 10.0);
        }

        // 边：遍历 `placed.edge_routes`（布局层的权威边输出，含子图跨组边）。
        // 注意：`fc.edges` 顺序与 `edge_routes` 不一定一致（subgraph 时 GroupedDirected
        // 会把组内边 + 跨组边合并），故直接按 edge_routes 绘制。
        for route in &placed.edge_routes {
            if route.len() >= 2 {
                let stroke = vir::stroke(theme::flowchart::EDGE, theme::EDGE_WIDTH);
                // 4 点（起、控制1、控制2、终）：直接用三次贝塞尔形成平滑 S 曲线
                // （避免 Catmull-Rom 对 4 点折线的摆动/阶梯圆角问题）
                if route.len() == 4 {
                    elements.push(vir::cubic_bezier_edge(
                        route[0],
                        route[1],
                        route[2],
                        route[3],
                        stroke.clone(),
                        Z_AXIS,
                    ));
                } else {
                    elements.push(vir::curved_edge_node(route.clone(), stroke.clone(), Z_AXIS));
                }
                // 箭头：终点附近（取最后一个折点的方向）
                let last = route.len() - 1;
                let tip = route[last];
                let prev = route[last - 1];
                let dx = tip.x - prev.x;
                let dy = tip.y - prev.y;
                let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                let dir = Point::new(dx / len, dy / len);
                draw_arrow_head(&mut elements, &tip, &dir, &stroke, false);
            }
        }

        // 子图框
        let subgraph_fill = Color::new(1.0, 1.0, 222.0 / 255.0, 1.0); // #FFFFDE
        for bound in &placed.group_bounds {
            elements.push(vir::rect_node(
                *bound,
                None,
                vir::fs_both(
                    subgraph_fill,
                    theme::flowchart::SUBGRAPH_STROKE,
                    theme::EDGE_WIDTH,
                ),
                Z_SUBGRAPH,
            ));
        }

        elements
    }

    /// 渲染 state diagram（基础版：节点 + 边）。
    pub fn render_state(
        placed: &PlacedGraph,
        _sd: &StateDiagram,
        _config: &OutputConfig,
    ) -> Vec<SceneNode> {
        let mut elements = Vec::new();

        // 边：`placed.edge_routes` 与 `LayoutGraph.edges` 同序（state 的 transitions）
        let edge_stroke = vir::stroke(theme::state::EDGE, theme::EDGE_WIDTH);
        for route in &placed.edge_routes {
            if route.len() >= 2 {
                elements.push(vir::curved_edge_node(
                    route.clone(),
                    edge_stroke.clone(),
                    Z_AXIS,
                ));
                let last = route.len() - 1;
                let tip = route[last];
                let prev = route[last - 1];
                let dx = tip.x - prev.x;
                let dy = tip.y - prev.y;
                let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                let dir = Point::new(dx / len, dy / len);
                draw_arrow_head(&mut elements, &tip, &dir, &edge_stroke, false);
            }
        }

        // 节点
        for center in &placed.positions {
            let style = NodeRenderStyle {
                fill: theme::state::FILL,
                stroke: theme::state::STROKE,
                stroke_width: 2.0,
            };
            draw_node_shape(
                &mut elements,
                *center,
                Size::new(120.0, 60.0),
                Some(NodeShape::Rounded),
                &style,
            );
        }
        elements
    }

    /// Directed 家族（`Flowchart` / `State`）的统一渲染入口。
    pub fn render(
        placed: &PlacedGraph,
        diagram: &Diagram,
        config: &OutputConfig,
    ) -> Vec<SceneNode> {
        match diagram {
            Diagram::Flowchart(fc) => Self::render_flowchart(placed, fc, config),
            Diagram::State(sd) => Self::render_state(placed, sd, config),
            _ => Vec::new(),
        }
    }
}
