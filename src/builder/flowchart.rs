use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use vello_cpu::kurbo::{BezPath, Point, Rect};

use crate::{
    ast::{Direction, Flowchart, NodeShape},
    builder::types::OutputConfig,
    error::DiagramResult,
    text::{create_text_layout, compute_text_offset},
    visual::{
        draw_arrow_head, theme,
        FillStrokeStyle, StrokeStyle, TextAlign, TextBaseline, TextStyle, VisualElement,
        Z_AXIS, Z_LABEL, Z_SERIES,
    },
};

use super::layout::{
    edges::route_edges,
    layers::assign_layers,
    measure::{measure_groups, measure_nodes},
    position::compute_positions,
    recognize::{recognize_structure, compute_flowchart_back_edges},
    sugiyama::{NodeSize, SugiyamaConfig, SugiyamaLayout},
    types::{
        Layout, LayoutEdge, LayoutEngine, LayoutMetadata, LayoutNode, NodeMetrics, NodePosition,
        NodeStyle, RoutedEdge, Size,
    },
};

const NODE_FONT_SIZE: f64 = theme::FONT_SIZE;

/// 画布边距
const MARGIN: f64 = 40.0;

const EDGE_STROKE: StrokeStyle = StrokeStyle {
    color: theme::flowchart::EDGE,
    width: theme::EDGE_WIDTH,
};

/// 计算内容的包围盒尺寸（不包含画布边距）
fn compute_content_bounds(
    positions: &HashMap<String, NodePosition>,
    metrics: &HashMap<String, NodeMetrics>,
) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for (node_id, pos) in positions {
        let size = metrics.get(node_id).map(|m| m.size).unwrap_or(Size::new(140.0, 50.0));
        min_x = min_x.min(pos.center.x - size.width / 2.0);
        min_y = min_y.min(pos.center.y - size.height / 2.0);
        max_x = max_x.max(pos.center.x + size.width / 2.0);
        max_y = max_y.max(pos.center.y + size.height / 2.0);
    }

    (min_x, min_y, max_x, max_y)
}

/// 从 Flowchart 构建 petgraph 有向图（用于 Sugiyama 布局）
fn build_flowchart_graph(fc: &Flowchart) -> (DiGraph<String, ()>, HashMap<String, NodeIndex>) {
    let mut graph = DiGraph::new();
    let mut indices = HashMap::new();
    for node in &fc.nodes {
        let idx = graph.add_node(node.id.clone());
        indices.insert(node.id.clone(), idx);
    }
    for edge in &fc.edges {
        if let (Some(&from), Some(&to)) = (indices.get(&edge.source), indices.get(&edge.target)) {
            graph.add_edge(from, to, ());
        }
    }
    (graph, indices)
}

/// 检查流程图是否有子图（subgraph），决定是否可用 Sugiyama
/// Sugiyama 处理平面有向图，无法处理带显式子图边界的复杂结构
fn has_subgraphs(fc: &Flowchart) -> bool {
    !fc.subgraphs.is_empty()
}

/// 将 Sugiyama 布局结果渲染为流程图 VisualElement
fn render_sugiyama_flowchart(
    fc: &Flowchart,
    result: &super::layout::sugiyama::SugiyamaResult,
    graph: &DiGraph<String, ()>,
    indices: &HashMap<String, NodeIndex>,
    node_metrics: &HashMap<String, NodeMetrics>,
) -> Vec<VisualElement> {
    // 构建 node_id → center 映射
    let mut node_centers: HashMap<String, Point> = HashMap::new();
    for (idx, pos) in &result.positions {
        let id = &graph[*idx];
        node_centers.insert(id.clone(), *pos);
    }

    let mut elements = Vec::new();

    // 绘制边（使用 Sugiyama 路由结果）
    for edge in fc.edges.iter() {
        if let (Some(&from_idx), Some(&to_idx)) = (indices.get(&edge.source), indices.get(&edge.target)) {
            if let Some(route) = result.edge_routes.get(&(from_idx, to_idx)) {
                if route.len() >= 2 {
                    elements.push(VisualElement::Polyline {
                        points: route.clone(),
                        style: EDGE_STROKE,
                        z_index: Z_AXIS,
                    });
                    // 箭头：最后一段方向
                    let last = route.last().unwrap();
                    let prev = route[route.len() - 2];
                    let dx = last.x - prev.x;
                    let dy = last.y - prev.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let ud = Point::new(dx / len, dy / len);
                        draw_arrow_head(&mut elements, last, &ud, &EDGE_STROKE);
                    }
                }
            }
        }
    }

    // 绘制节点
    for node in &fc.nodes {
        if let Some(&center) = node_centers.get(&node.id) {
            let nm = node_metrics.get(&node.id);
            let size = nm.map(|m| m.size).unwrap_or(Size::new(140.0, 50.0));
            let bounds = Rect::new(
                center.x - size.width / 2.0,
                center.y - size.height / 2.0,
                center.x + size.width / 2.0,
                center.y + size.height / 2.0,
            );
            let layout_node = LayoutNode {
                id: node.id.clone(),
                bounds,
                ports: vec![],
                label: node.text.clone().or(Some(node.id.clone())),
                shape: node.shape.clone(),
                style: NodeStyle::default(),
            };
            // 调用已有的绘制函数（偏移为 0，因为 Sugiyama 坐标已经是绝对坐标）
            draw_layout_node(&mut elements, &layout_node, 0.0, 0.0);
        }
    }

    elements
}

/// 将布局管道内部数据转换为统一 Layout IR
fn build_layout(
    fc: &Flowchart,
    node_positions: &HashMap<String, NodePosition>,
    node_metrics: &HashMap<String, NodeMetrics>,
    routed_edges: &[RoutedEdge],
    direction: Direction,
) -> Layout {
    let (min_x, min_y, max_x, max_y) = compute_content_bounds(node_positions, node_metrics);

    let content_size = if min_x == f64::MAX {
        Size::new(0.0, 0.0)
    } else {
        Size::new(max_x - min_x, max_y - min_y)
    };

    let nodes: Vec<LayoutNode> = fc
        .nodes
        .iter()
        .map(|node| {
            let nm = node_metrics.get(&node.id);
            let pos = node_positions.get(&node.id);
            let size = nm.map(|m| m.size).unwrap_or(Size::new(140.0, 50.0));
            let center = pos.map(|p| p.center).unwrap_or(Point::new(0.0, 0.0));

            let bounds = Rect::new(
                center.x - size.width / 2.0,
                center.y - size.height / 2.0,
                center.x + size.width / 2.0,
                center.y + size.height / 2.0,
            );

            let ports = nm
                .map(|m| {
                    vec![
                        Point::new(center.x + m.anchors.top.x, center.y + m.anchors.top.y),
                        Point::new(center.x + m.anchors.bottom.x, center.y + m.anchors.bottom.y),
                        Point::new(center.x + m.anchors.left.x, center.y + m.anchors.left.y),
                        Point::new(center.x + m.anchors.right.x, center.y + m.anchors.right.y),
                    ]
                })
                .unwrap_or_default();

            LayoutNode {
                id: node.id.clone(),
                bounds,
                ports,
                label: node.text.clone(),
                shape: node.shape.clone(),
                style: NodeStyle::default(),
            }
        })
        .collect();

    let edges: Vec<LayoutEdge> = routed_edges
        .iter()
        .map(|re| LayoutEdge {
            from: re.edge.source.clone(),
            to: re.edge.target.clone(),
            path: re.route.clone(),
            arrow_at_end: true,
            label: re.edge.label.clone(),
            label_position: re.label_position.map(|(p, _)| p),
        })
        .collect();

    Layout {
        nodes,
        edges,
        size: content_size,
        metadata: LayoutMetadata { direction },
    }
}

/// 将 Layout IR 渲染为 VisualElement
fn render_layout(layout: &Layout) -> Vec<VisualElement> {
    let mut elements = Vec::new();

    // 计算居中偏移
    let (offset_x, offset_y) = if layout.nodes.is_empty() {
        (0.0, 0.0)
    } else {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for node in &layout.nodes {
            min_x = min_x.min(node.bounds.x0);
            min_y = min_y.min(node.bounds.y0);
            max_x = max_x.max(node.bounds.x1);
            max_y = max_y.max(node.bounds.y1);
        }

        let offset_x = MARGIN - min_x;
        let offset_y = MARGIN - min_y;

        (offset_x, offset_y)
    };

    // 绘制边
    for edge in &layout.edges {
        if edge.path.len() >= 2 {
            let pts: Vec<Point> = edge
                .path
                .iter()
                .map(|p| Point::new(p.x + offset_x, p.y + offset_y))
                .collect();
            elements.push(VisualElement::Polyline {
                points: pts.clone(),
                style: EDGE_STROKE,
                z_index: Z_AXIS,
            });
            // 箭头：最后一段方向
            let last = pts.last().unwrap();
            let prev = pts[pts.len() - 2];
            let dx = last.x - prev.x;
            let dy = last.y - prev.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                let ud = Point::new(dx / len, dy / len);
                draw_arrow_head(&mut elements, last, &ud, &EDGE_STROKE);
            }
        }
    }

    // 绘制节点
    for node in &layout.nodes {
        draw_layout_node(&mut elements, node, offset_x, offset_y);
    }

    elements
}

/// FlowchartEngine：流程图布局引擎，实现 LayoutEngine trait
///
/// 封装 7-Pass 布局管线：
///   Pass 1: 结构识别 (recognize_structure)
///   Pass 2: 尺寸测量 (measure_nodes + measure_groups)
///   Pass 3: 层级分配 (assign_layers)
///   Pass 5: 几何定位 (compute_positions)
///   Pass 7: 边路由 (route_edges)
///   Pass 6: 画布适配
pub struct FlowchartEngine<'a> {
    flowchart: &'a Flowchart,
}

impl<'a> FlowchartEngine<'a> {
    pub fn new(flowchart: &'a Flowchart) -> Self {
        Self { flowchart }
    }
}

impl<'a> LayoutEngine for FlowchartEngine<'a> {
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<VisualElement>> {
        let fc = self.flowchart;
        let direction = fc.direction.clone().unwrap_or(Direction::TD);

        // Pass 1: 结构识别
        let tree = recognize_structure(fc);

        // Pass 2: 尺寸测量（两种路径都需要）
        let node_metrics = measure_nodes(&fc.nodes, config);

        // 检测是否可用 Sugiyama：无子图的流程图使用 Sugiyama 优化布局
        if !has_subgraphs(fc) && direction == Direction::TD {
            let (graph, _indices) = build_flowchart_graph(fc);

            // 构建节点尺寸映射
            let mut sugiyama_sizes: HashMap<NodeIndex, NodeSize> = HashMap::new();
            for node in &fc.nodes {
                if let Some(&idx) = _indices.get(&node.id) {
                    let nm = &node_metrics[&node.id];
                    sugiyama_sizes.insert(idx, NodeSize {
                        width: nm.size.width,
                        height: nm.size.height,
                    });
                }
            }

            // 运行 Sugiyama 4 阶段布局
            let sconfig = SugiyamaConfig::default();
            let sugiyama = SugiyamaLayout::new(sconfig, &graph);
            let result = sugiyama.layout(&sugiyama_sizes);

            let elements = render_sugiyama_flowchart(fc, &result, &graph, &_indices, &node_metrics);
            return Ok(elements);
        }

        // ---- 原有管线（有子图或非 TD 方向时使用） ----
        let group_metrics = measure_groups(&tree, &node_metrics);

        // Pass 3: 层级分配
        let layers = assign_layers(&tree);

        // Pass 5: 几何定位
        let node_positions = compute_positions(
            &tree,
            &node_metrics,
            &group_metrics,
            config,
            direction.clone(),
        );

        // 计算回边对（传递给边路由）
        let back_edge_pairs = compute_flowchart_back_edges(fc);

        // Pass 7: 边路由
        let routed_edges = route_edges(
            &fc.edges,
            &node_positions,
            &layers,
            &back_edge_pairs,
            direction.clone(),
        );

        // 构建统一 Layout IR
        let layout = build_layout(fc, &node_positions, &node_metrics, &routed_edges, direction);

        // 渲染为 VisualElement
        Ok(render_layout(&layout))
    }
}

/// 流程图构建入口：创建 FlowchartEngine 并执行布局管线
pub fn build_flowchart_elements(
    fc: &Flowchart,
    config: &OutputConfig,
) -> DiagramResult<Vec<VisualElement>> {
    FlowchartEngine::new(fc).layout(config)
}

/// 根据 LayoutNode 绘制节点（支持不同形状和动态尺寸）
fn draw_layout_node(
    elements: &mut Vec<VisualElement>,
    node: &LayoutNode,
    offset_x: f64,
    offset_y: f64,
) {
    let size = Size::new(
        node.bounds.width(),
        node.bounds.height(),
    );
    let center = Point::new(
        (node.bounds.x0 + node.bounds.x1) / 2.0 + offset_x,
        (node.bounds.y0 + node.bounds.y1) / 2.0 + offset_y,
    );

    let rect = Rect::new(
        center.x - size.width / 2.0,
        center.y - size.height / 2.0,
        center.x + size.width / 2.0,
        center.y + size.height / 2.0,
    );

    let fill = node.style.fill_color.unwrap_or(theme::flowchart::FILL);
    let stroke = node.style.stroke_color.unwrap_or(theme::flowchart::STROKE);
    let style = FillStrokeStyle::new().with_fill(fill).with_stroke(stroke, node.style.stroke_width);

    match node.shape {
        Some(NodeShape::Circle) => {
            let radius = size.width.min(size.height) / 2.0;
            elements.push(VisualElement::Circle {
                center,
                radius,
                style,
                z_index: Z_SERIES,
            });
        }
        Some(NodeShape::DoubleCircle) => {
            let outer_r = size.width.min(size.height) / 2.0;
            let inner_r = outer_r * 0.75;
            elements.push(VisualElement::Circle {
                center,
                radius: outer_r,
                style: style.clone(),
                z_index: Z_SERIES,
            });
            elements.push(VisualElement::Circle {
                center,
                radius: inner_r,
                style: FillStrokeStyle::new().with_stroke(stroke, 2.0),
                z_index: Z_SERIES,
            });
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
                let a = std::f64::consts::FRAC_PI_2 * i as f64 / segments as f64 - std::f64::consts::FRAC_PI_2;
                path.line_to(Point::new(center.x + w - r + r * a.cos(), center.y + r * a.sin()));
            }
            path.line_to(Point::new(center.x - w + r, center.y + h));
            for i in 0..=segments {
                let a = std::f64::consts::FRAC_PI_2 * i as f64 / segments as f64 + std::f64::consts::FRAC_PI_2;
                path.line_to(Point::new(center.x - w + r + r * a.cos(), center.y + r * a.sin()));
            }
            path.close_path();
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
        }
        Some(NodeShape::Cylinder) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let ellipse_segments = 16;
            let mut body = BezPath::new();
            body.move_to(Point::new(center.x - w, center.y - h * 0.7));
            body.line_to(Point::new(center.x + w, center.y - h * 0.7));
            body.line_to(Point::new(center.x + w, center.y + h));
            body.line_to(Point::new(center.x - w, center.y + h));
            body.close_path();
            elements.push(VisualElement::Path {
                path: body,
                style: FillStrokeStyle::new().with_fill(fill).with_stroke(stroke, 2.0),
                z_index: Z_SERIES,
            });
            let mut top = BezPath::new();
            top.move_to(Point::new(center.x - w, center.y - h * 0.7));
            for i in 0..=ellipse_segments {
                let a = std::f64::consts::PI * i as f64 / ellipse_segments as f64;
                top.line_to(Point::new(center.x - w + w * (1.0 + a.cos()), center.y - h * 0.7 + (h * 0.3) * a.sin()));
            }
            top.close_path();
            elements.push(VisualElement::Path {
                path: top,
                style: FillStrokeStyle::new().with_fill(fill).with_stroke(stroke, 2.0),
                z_index: Z_SERIES,
            });
        }
        Some(NodeShape::Subroutine) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let notch = 10.0;
            let mut path = BezPath::new();
            path.move_to(Point::new(center.x - w + notch, center.y - h));
            path.line_to(Point::new(center.x + w - notch, center.y - h));
            path.line_to(Point::new(center.x + w, center.y));
            path.line_to(Point::new(center.x + w - notch, center.y + h));
            path.line_to(Point::new(center.x - w + notch, center.y + h));
            path.line_to(Point::new(center.x - w, center.y));
            path.close_path();
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
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
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
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
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
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
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
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
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
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
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
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
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
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
            elements.push(VisualElement::Path { path, style, z_index: Z_SERIES });
        }
        _ => {
            elements.push(VisualElement::Rect {
                rect,
                radius: Some(theme::NODE_RADIUS),
                style,
                z_index: Z_SERIES,
            });
        }
    }

    // 节点文本
    let text = node.label.as_deref().unwrap_or(&node.id);
    let text_style = TextStyle {
        font_size: NODE_FONT_SIZE,
        font_family: theme::FONT_FAMILY.to_string(),
        align: TextAlign::Center,
        vertical_align: TextBaseline::Middle,
        color: theme::flowchart::TEXT,
        ..Default::default()
    };
    let max_w = if size.width > 20.0 { Some(size.width - 10.0) } else { None };
    let layout = create_text_layout(text, &text_style, max_w);

    let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
    let text_position = Point::new(center.x + x_off, center.y + y_off);

    elements.push(VisualElement::TextRun {
        text: text.to_string(),
        position: text_position,
        style: TextStyle {
            align: TextAlign::Left,
            vertical_align: TextBaseline::Top,
            ..text_style
        },
        rotation: 0.0,
        max_width: max_w,
        layout: Some(layout),
        z_index: Z_LABEL,
    });
}