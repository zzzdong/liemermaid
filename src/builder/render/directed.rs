//! `DirectedRenderer`：flowchart / state 的有向图渲染。
//!
//! 拿 `PlacedGraph`（纯几何）+ AST（`Flowchart` / `StateDiagram`），
//! 回查节点形状 / 文本 / 箭头类型，组合绘制成 `SceneNode`。不修改坐标。

use lievisual::geometry::{BezPath, Point, Rect, Size};
use lievisual::scene::SceneNode;
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

use crate::ast::{Diagram, Flowchart, NodeShape, StateDiagram};
use crate::builder::layout::ir::{LineKind, PlacedGraph, ShapeHint};
use crate::builder::layout::measure::measure_nodes;
use crate::builder::layout::recognize::all_flowchart_nodes;
use crate::builder::layout::state_nodes::collect_state_nodes;
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

/// 绘制边标签（`A -->|label| B`）：白色背景 + 居中文本。
fn draw_edge_label(elements: &mut Vec<SceneNode>, center: Point, text: &str) {
    let ts = vir::TextStyle::new(
        theme::flowchart::TEXT,
        theme::FONT_SIZE,
        theme::FONT_FAMILY.to_string(),
    )
    .with_align(TextAlign::Center)
    .with_baseline(TextBaseline::Middle);
    let layout = layout_text(&[RichSpan::new(text.to_string(), ts.clone())], None);
    let pad_x = 4.0;
    let pad_y = 2.0;
    let bg = Rect::new(
        center.x - layout.width / 2.0 - pad_x,
        center.y - layout.height / 2.0 - pad_y,
        center.x + layout.width / 2.0 + pad_x,
        center.y + layout.height / 2.0 + pad_y,
    );
    elements.push(vir::rect_node(
        bg,
        None,
        vir::fs_both(Color::new(1.0, 1.0, 1.0, 1.0), theme::flowchart::TEXT, 1.0),
        Z_AXIS,
    ));
    let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
    elements.push(vir::text_node(
        text.to_string(),
        Point::new(center.x + x_off, center.y + y_off),
        ts.with_align(TextAlign::Left).with_baseline(TextBaseline::Top),
        0.0,
        None,
        Z_LABEL,
    ));
}

/// 把布局 `ShapeHint` 映射为渲染 `NodeShape`（state 图：Rect → 圆角矩形，Circle → 圆）。
fn state_node_shape(hint: ShapeHint) -> Option<NodeShape> {
    match hint {
        ShapeHint::Rect | ShapeHint::Rounded => Some(NodeShape::Rounded),
        ShapeHint::Circle => Some(NodeShape::Circle),
        ShapeHint::Diamond => Some(NodeShape::Diamond),
        ShapeHint::Bar => None,
    }
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
        // 线型（虚线 / 不可见）取自与 edge_routes 同序的 `placed.edge_kinds`。
        for (ei, route) in placed.edge_routes.iter().enumerate() {
            if route.len() < 2 {
                continue;
            }
            // 不可见边（`~~~`）：占位，不绘制
            let is_invisible = placed
                .edge_kinds
                .get(ei)
                .map(|k| *k == LineKind::Invisible)
                .unwrap_or(false);
            if is_invisible {
                continue;
            }
            let is_dashed = placed
                .edge_kinds
                .get(ei)
                .map(|k| *k == LineKind::Dashed)
                .unwrap_or(false);
            let is_no_arrow = placed
                .edge_kinds
                .get(ei)
                .map(|k| *k == LineKind::NoArrow)
                .unwrap_or(false);
            let is_thick = placed
                .edge_kinds
                .get(ei)
                .map(|k| *k == LineKind::Thick)
                .unwrap_or(false);
            let width = if is_thick {
                theme::EDGE_WIDTH * 2.5
            } else {
                theme::EDGE_WIDTH
            };
            let stroke = if is_dashed {
                vir::dashed_stroke(theme::flowchart::EDGE, width, vec![6.0, 4.0])
            } else {
                vir::stroke(theme::flowchart::EDGE, width)
            };
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
            // 箭头：终点附近（取最后一个折点的方向）；无箭头边（`---`）跳过
            if !is_no_arrow {
                let last = route.len() - 1;
                let tip = route[last];
                let prev = route[last - 1];
                let dx = tip.x - prev.x;
                let dy = tip.y - prev.y;
                let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                let dir = Point::new(dx / len, dy / len);
                draw_arrow_head(&mut elements, &tip, &dir, &stroke, false);
            }
            // 边标签：`A -->|label| B`，画在边路径中点（白底）
            if let Some(label) = fc.edges.get(ei).and_then(|e| e.label.as_deref()) {
                if !label.is_empty() {
                    let mid = route[route.len() / 2];
                    let mid2 = route[(route.len() - 1) / 2];
                    let lm = Point::new((mid.x + mid2.x) / 2.0, (mid.y + mid2.y) / 2.0 - 8.0);
                    draw_edge_label(&mut elements, lm, label);
                }
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
        sd: &StateDiagram,
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

        // 节点：元数据（顺序 = `PlacedGraph.positions`）来自 `collect_state_nodes`，
        // 与 convert 的 `LayoutGraph.nodes` 一致，含 label / 形状 / 尺寸。
        // 按官方 state 样式区分：start 实心圆 / end 空心双层圆 / fork-join 横条 / 其余圆角矩形。
        let nodes = collect_state_nodes(sd);
        let style = NodeRenderStyle {
            fill: theme::state::FILL,
            stroke: theme::state::STROKE,
            stroke_width: 2.0,
        };
        let edge_color = theme::state::STROKE;
        for (i, center) in placed.positions.iter().enumerate() {
            let Some(info) = nodes.get(i) else { continue };
            if info.id == "__start__" {
                // 开始：实心圆
                let r = info.size.width / 2.0;
                elements.push(vir::circle_node(
                    *center,
                    r,
                    vir::fs_both(edge_color, edge_color, 1.5),
                    Z_SERIES,
                ));
                continue;
            }
            if info.id == "__end__" {
                // 结束：空心双层圆（外圈描边 + 内实心小圆）
                let outer_r = info.size.width / 2.0;
                elements.push(vir::circle_node(
                    *center,
                    outer_r,
                    vir::fs_stroke(edge_color, 2.0),
                    Z_SERIES,
                ));
                let inner_r = outer_r * 0.6;
                elements.push(vir::circle_node(
                    *center,
                    inner_r,
                    vir::fs_both(edge_color, edge_color, 1.5),
                    Z_SERIES,
                ));
                continue;
            }
            if info.shape == ShapeHint::Bar {
                // fork / join：水平横条
                let rect = Rect::new(
                    center.x - info.size.width / 2.0,
                    center.y - info.size.height / 2.0,
                    center.x + info.size.width / 2.0,
                    center.y + info.size.height / 2.0,
                );
                elements.push(vir::rect_node(
                    rect,
                    None,
                    vir::fs_both(edge_color, edge_color, 1.5),
                    Z_SERIES,
                ));
                continue;
            }
            // 普通状态：圆角矩形 + label
            let shape = state_node_shape(info.shape);
            draw_node_shape(&mut elements, *center, info.size, shape, &style);
            let text = info
                .label
                .as_deref()
                .unwrap_or_else(|| info.id.as_str());
            if !text.is_empty() {
                draw_node_label(&mut elements, *center, text, info.size.width - 10.0);
            }
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
