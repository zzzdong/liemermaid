use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use lievisual::geometry::{Point, Rect};
use vello_cpu::kurbo::BezPath;

use crate::{
    ast::{Direction, Flowchart, NodeShape},
    builder::types::OutputConfig,
    error::DiagramResult,
    vir::{self,
        draw_arrow_head, theme, Stroke, TextAlign, TextBaseline,
        Z_AXIS, Z_LABEL, Z_SERIES, Z_SUBGRAPH, Z_SUBGRAPH_LABEL,
    },
};
use lievisual::scene::SceneNode;
use lievisual::text::{compute_text_offset, layout_text, RichSpan};

use super::layout::{
    edges::route_edges,
    layers::assign_layers,
    measure::{measure_groups, measure_nodes},
    position::compute_positions,
    recognize::{all_flowchart_nodes, compute_flowchart_back_edges, recognize_structure},
    sugiyama::{NodeSize, SugiyamaConfig, SugiyamaLayout, SugiyamaResult},
    types::{
        Layout, LayoutEdge, LayoutEngine, LayoutMetadata, LayoutNode, NodeMetrics, NodePosition,
        NodeStyle, RoutedEdge, Size, LayoutSubgraph,
    },
};

const NODE_FONT_SIZE: f64 = theme::FONT_SIZE;

/// 画布边距
const MARGIN: f64 = 40.0;

/// 子图标题区高度（容器框顶部留白）
const SUBGRAPH_TITLE_H: f64 = 22.0;

fn edge_stroke() -> Stroke { vir::stroke(theme::flowchart::EDGE, theme::EDGE_WIDTH) }

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
        let size = metrics
            .get(node_id)
            .map(|m| m.size)
            .unwrap_or(Size::new(140.0, 50.0));
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

/// 将 TB 坐标系的 Sugiyama 结果按方向旋转/镜像，对齐 dagre 的 rankdir 语义。
///
/// sugiyama 内部始终在 TB（y 为层主轴）坐标系布局；dagre 对 LR/RL/DT 的处理
/// 是先按 TB 算布局，最后整体旋转画布。这里等价地：
///   - TD: 不变
///   - DT: 上下镜像 (x, y) -> (x, -y)
///   - LR: 转置   (x, y) -> (y, x)
///   - RL: 转置+镜像 (x, y) -> (-y, x)
/// 变换后整体平移使坐标非负（与 dagre 的 bounding box 一致）。
/// LR/RL 同时互换节点矩形宽高，使旋转后矩形方向与坐标排列匹配。
fn transform_sugiyama_direction(result: &mut SugiyamaResult, direction: Direction) {
    use lievisual::geometry::{Point};

    let map = |p: Point| -> Point {
        match direction {
            Direction::TB | Direction::TD => p,
            Direction::BT => Point::new(p.x, -p.y),
            Direction::LR => Point::new(p.y, p.x),
            Direction::RL => Point::new(-p.y, p.x),
        }
    };

    // 先纯映射，再求全局 min 以平移
    let mut mapped_pos: HashMap<NodeIndex, Point> = HashMap::new();
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for (n, p) in result.positions.iter() {
        let q = map(*p);
        min_x = min_x.min(q.x);
        min_y = min_y.min(q.y);
        mapped_pos.insert(*n, q);
    }
    let mut mapped_routes: HashMap<(NodeIndex, NodeIndex), Vec<Point>> = HashMap::new();
    for (k, pts) in result.edge_routes.iter() {
        let qs: Vec<Point> = pts.iter().map(|&p| {
            let q = map(p);
            min_x = min_x.min(q.x);
            min_y = min_y.min(q.y);
            q
        }).collect();
        mapped_routes.insert(*k, qs);
    }

    // 平移使非负
    let off_x = if min_x.is_finite() { -min_x } else { 0.0 };
    let off_y = if min_y.is_finite() { -min_y } else { 0.0 };
    for q in mapped_pos.values_mut() {
        q.x += off_x;
        q.y += off_y;
    }
    for pts in mapped_routes.values_mut() {
        for q in pts.iter_mut() {
            q.x += off_x;
            q.y += off_y;
        }
    }

    result.positions = mapped_pos;
    result.edge_routes = mapped_routes;

    // LR/RL 互换矩形宽高
    if matches!(direction, Direction::LR | Direction::RL) {
        for s in result.sizes.values_mut() {
            std::mem::swap(&mut s.width, &mut s.height);
        }
    }
}

/// 将 Sugiyama 布局结果渲染为流程图 VisualElement
fn render_sugiyama_flowchart(
    fc: &Flowchart,
    result: &super::layout::sugiyama::SugiyamaResult,
    graph: &DiGraph<String, ()>,
    indices: &HashMap<String, NodeIndex>,
    node_metrics: &HashMap<String, NodeMetrics>,
) -> Vec<SceneNode> {
    // 构建 node_id → center 映射
    let mut node_centers: HashMap<String, Point> = HashMap::new();
    for (idx, pos) in &result.positions {
        let id = &graph[*idx];
        node_centers.insert(id.clone(), *pos);
    }

    let mut elements = Vec::new();

    // 绘制边（使用 Sugiyama 路由结果）
    for edge in fc.edges.iter() {
        if let (Some(&from_idx), Some(&to_idx)) =
            (indices.get(&edge.source), indices.get(&edge.target))
            && let Some(route) = result.edge_routes.get(&(from_idx, to_idx))
            && route.len() >= 2
        {
            elements.push(vir::polyline_node(route.clone(), edge_stroke(), Z_AXIS));
            // 箭头：最后一段方向
            let last = route.last().unwrap();
            let prev = route[route.len() - 2];
            let dx = last.x - prev.x;
            let dy = last.y - prev.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                let ud = Point::new(dx / len, dy / len);
                draw_arrow_head(&mut elements, last, &ud, &edge_stroke());
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

    let all_nodes = all_flowchart_nodes(fc);
    let nodes: Vec<LayoutNode> = all_nodes
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

    // 子图容器：由成员节点包围盒外扩 padding 得到
    let subgraph_padding = 24.0;
    let subgraphs: Vec<LayoutSubgraph> = fc
        .subgraphs
        .iter()
        .map(|sg| {
            let mut sg_min_x = f64::MAX;
            let mut sg_min_y = f64::MAX;
            let mut sg_max_x = f64::MIN;
            let mut sg_max_y = f64::MIN;
            for member in &sg.nodes {
                if let Some(ln) = nodes.iter().find(|n| n.id == member.id) {
                    sg_min_x = sg_min_x.min(ln.bounds.min_x());
                    sg_min_y = sg_min_y.min(ln.bounds.min_y());
                    sg_max_x = sg_max_x.max(ln.bounds.max_x());
                    sg_max_y = sg_max_y.max(ln.bounds.max_y());
                }
            }
            if sg_min_x == f64::MAX {
                sg_min_x = 0.0;
                sg_min_y = 0.0;
                sg_max_x = 0.0;
                sg_max_y = 0.0;
            }
            let bounds = Rect::new(
                sg_min_x - subgraph_padding,
                sg_min_y - subgraph_padding - SUBGRAPH_TITLE_H,
                sg_max_x + subgraph_padding,
                sg_max_y + subgraph_padding,
            );
            LayoutSubgraph {
                title: sg.title.clone(),
                member_ids: sg.nodes.iter().map(|n| n.id.clone()).collect(),
                bounds,
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
            curved: false,
        })
        .collect();

    Layout {
        nodes,
        edges,
        size: content_size,
        metadata: LayoutMetadata { direction },
        subgraphs,
    }
}

/// 将 Layout IR 渲染为 VisualElement
fn render_layout(layout: &Layout) -> Vec<SceneNode> {
    let mut elements = Vec::new();

    // 计算居中偏移
    let (offset_x, offset_y) = if layout.nodes.is_empty() && layout.subgraphs.is_empty() {
        (0.0, 0.0)
    } else {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for node in &layout.nodes {
            min_x = min_x.min(node.bounds.min_x());
            min_y = min_y.min(node.bounds.min_y());
            max_x = max_x.max(node.bounds.max_x());
            max_y = max_y.max(node.bounds.max_y());
        }
        for sg in &layout.subgraphs {
            min_x = min_x.min(sg.bounds.min_x());
            min_y = min_y.min(sg.bounds.min_y());
            max_x = max_x.max(sg.bounds.max_x());
            max_y = max_y.max(sg.bounds.max_y());
        }

        let offset_x = MARGIN - min_x;
        let offset_y = MARGIN - min_y;

        (offset_x, offset_y)
    };

    // 先绘制子图容器框（位于节点与边之下）
    for sg in &layout.subgraphs {
        let rect = Rect::new(
            sg.bounds.min_x() + offset_x,
            sg.bounds.min_y() + offset_y,
            sg.bounds.max_x() + offset_x,
            sg.bounds.max_y() + offset_y,
        );
        let style = vir::fs_stroke(theme::flowchart::SUBGRAPH_STROKE, theme::EDGE_WIDTH);
        elements.push(vir::rect_node(rect, Some(8.0), style, Z_SUBGRAPH));

        if let Some(title) = &sg.title {
            let title_style = vir::text_style(
                theme::flowchart::SUBGRAPH_TITLE,
                NODE_FONT_SIZE,
                theme::FONT_FAMILY,
                TextAlign::Left,
                TextBaseline::Top,
            );
            let title_position = Point::new(
                rect.min_x() + 10.0,
                rect.min_y() + 6.0,
            );
            elements.push(vir::text_node(
                title.clone(),
                title_position,
                title_style.with_align(TextAlign::Left).with_baseline(TextBaseline::Top),
                0.0,
                None,
                Z_SUBGRAPH_LABEL,
            ));
        }
    }

    // 绘制边
    for edge in &layout.edges {
        if edge.path.len() >= 2 {
            let pts: Vec<Point> = edge
                .path
                .iter()
                .map(|p| Point::new(p.x + offset_x, p.y + offset_y))
                .collect();
            if edge.curved && pts.len() >= 2 {
                // 贝塞尔曲线：用首尾与中点控制
                let mut path = BezPath::new();
                path.move_to(pts[0]);
                if pts.len() == 2 {
                    path.line_to(pts[1]);
                } else {
                    let first = pts[0];
                    let last = pts[pts.len() - 1];
                    let mid = Point::new((first.x + last.x) / 2.0, (first.y + last.y) / 2.0);
                    path.curve_to(
                        Point::new(mid.x, first.y),
                        Point::new(mid.x, last.y),
                        last,
                    );
                }
                elements.push(vir::path_node(path, vir::fs_stroke(theme::flowchart::EDGE, theme::EDGE_WIDTH), Z_AXIS));
            } else {
                elements.push(vir::polyline_node(pts.clone(), edge_stroke(), Z_AXIS));
            }
            // 箭头：最后一段方向
            let last = pts.last().unwrap();
            let prev = pts[pts.len() - 2];
            let dx = last.x - prev.x;
            let dy = last.y - prev.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                let ud = Point::new(dx / len, dy / len);
                draw_arrow_head(&mut elements, last, &ud, &edge_stroke());
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
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
        let fc = self.flowchart;
        let direction = fc.direction.clone().unwrap_or(Direction::TD);

        // Pass 1: 结构识别
        let tree = recognize_structure(fc);

        // Pass 2: 尺寸测量（两种路径都需要，含 subgraph 内部节点）
        let all_nodes = all_flowchart_nodes(fc);
        let node_metrics = measure_nodes(&all_nodes, config);

        // 检测是否可用 Sugiyama：无子图的流程图统一使用 Sugiyama 优化布局，
        // 覆盖所有方向（TD/DT/LR/RL）；方向差异在布局后做坐标变换对齐 dagre。
        if !has_subgraphs(fc) {
            let (graph, _indices) = build_flowchart_graph(fc);

            // LR/RL 下节点矩形在视觉上"横放"，等价于把宽高互换后按 TB 布局再转置
            let swap = matches!(direction, Direction::LR | Direction::RL);

            // 构建节点尺寸映射
            let mut sugiyama_sizes: HashMap<NodeIndex, NodeSize> = HashMap::new();
            for node in &fc.nodes {
                if let Some(&idx) = _indices.get(&node.id) {
                    let nm = &node_metrics[&node.id];
                    let (width, height) = if swap {
                        (nm.size.height, nm.size.width)
                    } else {
                        (nm.size.width, nm.size.height)
                    };
                    sugiyama_sizes.insert(idx, NodeSize { width, height });
                }
            }

            // 运行 Sugiyama 4 阶段布局（内部统一 TB 坐标系）
            let sconfig = SugiyamaConfig::default();
            let sugiyama = SugiyamaLayout::new(sconfig, &graph);
            let mut result = sugiyama.layout(&sugiyama_sizes);

            // 按方向旋转/镜像坐标与边路径，对齐 dagre 的 rankdir 语义
            transform_sugiyama_direction(&mut result, direction);

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
) -> DiagramResult<Vec<SceneNode>> {
    FlowchartEngine::new(fc).layout(config)
}

/// 根据 LayoutNode 绘制节点（支持不同形状和动态尺寸）
fn draw_layout_node(
    elements: &mut Vec<SceneNode>,
    node: &LayoutNode,
    offset_x: f64,
    offset_y: f64,
) {
    let size = Size::new(node.bounds.width(), node.bounds.height());
    let center = Point::new(
        (node.bounds.min_x() + node.bounds.max_x()) / 2.0 + offset_x,
        (node.bounds.min_y() + node.bounds.max_y()) / 2.0 + offset_y,
    );

    let rect = Rect::new(
        center.x - size.width / 2.0,
        center.y - size.height / 2.0,
        center.x + size.width / 2.0,
        center.y + size.height / 2.0,
    );

    let fill = node.style.fill_color.unwrap_or(theme::flowchart::FILL);
    let stroke = node.style.stroke_color.unwrap_or(theme::flowchart::STROKE);
    let style = vir::fs_both(fill, stroke, node.style.stroke_width);

    match node.shape {
        Some(NodeShape::Circle) => {
            let radius = size.width.min(size.height) / 2.0;
            elements.push(vir::circle_node(center, radius, style, Z_SERIES));
        }
        Some(NodeShape::DoubleCircle) => {
            let outer_r = size.width.min(size.height) / 2.0;
            let inner_r = outer_r * 0.75;
            elements.push(vir::circle_node(center, outer_r, style.clone(), Z_SERIES));
            elements.push(vir::circle_node(center, inner_r, vir::fs_stroke(stroke, 2.0), Z_SERIES));
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
                let a = std::f64::consts::FRAC_PI_2 * i as f64 / segments as f64
                    - std::f64::consts::FRAC_PI_2;
                path.line_to(Point::new(
                    center.x + w - r + r * a.cos(),
                    center.y + r * a.sin(),
                ));
            }
            path.line_to(Point::new(center.x - w + r, center.y + h));
            for i in 0..=segments {
                let a = std::f64::consts::FRAC_PI_2 * i as f64 / segments as f64
                    + std::f64::consts::FRAC_PI_2;
                path.line_to(Point::new(
                    center.x - w + r + r * a.cos(),
                    center.y + r * a.sin(),
                ));
            }
            path.close_path();
            elements.push(vir::path_node(path, style, Z_SERIES));
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
            elements.push(vir::path_node(body, vir::fs_both(fill, stroke, 2.0), Z_SERIES));
            let mut top = BezPath::new();
            top.move_to(Point::new(center.x - w, center.y - h * 0.7));
            for i in 0..=ellipse_segments {
                let a = std::f64::consts::PI * i as f64 / ellipse_segments as f64;
                top.line_to(Point::new(
                    center.x - w + w * (1.0 + a.cos()),
                    center.y - h * 0.7 + (h * 0.3) * a.sin(),
                ));
            }
            top.close_path();
            elements.push(vir::path_node(top, vir::fs_both(fill, stroke, 2.0), Z_SERIES));
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
            elements.push(vir::path_node(path, style, Z_SERIES));
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
            elements.push(vir::path_node(path, style, Z_SERIES));
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
            elements.push(vir::path_node(path, style, Z_SERIES));
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
            elements.push(vir::path_node(path, style, Z_SERIES));
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
            elements.push(vir::path_node(path, style, Z_SERIES));
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
            elements.push(vir::path_node(path, style, Z_SERIES));
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
            elements.push(vir::path_node(path, style, Z_SERIES));
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
            elements.push(vir::path_node(path, style, Z_SERIES));
        }
        _ => {
            elements.push(vir::rect_node(rect, Some(theme::NODE_RADIUS), style, Z_SERIES));
        }
    }

    // 节点文本
    let text = node.label.as_deref().unwrap_or(&node.id);
    let text_style = vir::text_style(
        theme::flowchart::TEXT,
        NODE_FONT_SIZE,
        theme::FONT_FAMILY,
        TextAlign::Center,
        TextBaseline::Middle,
    );
    let max_w = if size.width > 20.0 {
        Some(size.width - 10.0)
    } else {
        None
    };
    let layout = layout_text(&[RichSpan::new(text.to_string(), text_style.clone())], max_w);

    let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
    let text_position = Point::new(center.x + x_off, center.y + y_off);

    elements.push(vir::text_node(
        text.to_string(),
        text_position,
        text_style.with_align(TextAlign::Left).with_baseline(TextBaseline::Top),
        0.0,
        max_w,
        Z_LABEL,
    ));
}

#[cfg(test)]
mod direction_transform_tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn sample_result() -> SugiyamaResult {
        use lievisual::geometry::{Point};
        let a = NodeIndex::new(0);
        let b = NodeIndex::new(1);
        let c = NodeIndex::new(2);
        let mut positions = HashMap::new();
        positions.insert(a, Point::new(0.0, 0.0));
        positions.insert(b, Point::new(0.0, 100.0));
        positions.insert(c, Point::new(0.0, 200.0));
        let mut sizes = HashMap::new();
        for n in [a, b, c] {
            sizes.insert(n, NodeSize { width: 100.0, height: 40.0 });
        }
        let mut edge_routes = HashMap::new();
        edge_routes.insert((a, c), vec![Point::new(0.0, 20.0), Point::new(0.0, 180.0)]);
        SugiyamaResult {
            positions,
            sizes,
            layers: HashMap::new(),
            layer_nodes: HashMap::new(),
            edge_routes,
            feedback_arcs: HashSet::new(),
            sccs: Vec::new(),
            scc_id: HashMap::new(),
        }
    }

    #[test]
    fn lr_transposes_rank_to_x_axis() {
        let mut r = sample_result();
        transform_sugiyama_direction(&mut r, Direction::LR);
        let a = r.positions[&NodeIndex::new(0)];
        let b = r.positions[&NodeIndex::new(1)];
        let c = r.positions[&NodeIndex::new(2)];
        // rank 沿 x 递增（A<B<C），同 rank 同 x
        assert!(b.x > a.x, "rank should increase along x under LR");
        assert!(c.x > b.x, "rank should increase along x under LR");
        assert_eq!(a.y, b.y);
        assert_eq!(b.y, c.y);
        // 非负 + 矩形宽高互换
        assert!(a.x >= 0.0 && a.y >= 0.0);
        assert_eq!(r.sizes[&NodeIndex::new(0)].width, 40.0);
        assert_eq!(r.sizes[&NodeIndex::new(0)].height, 100.0);
    }

    #[test]
    fn bt_flips_y() {
        let mut r = sample_result();
        transform_sugiyama_direction(&mut r, Direction::BT);
        let a = r.positions[&NodeIndex::new(0)];
        let c = r.positions[&NodeIndex::new(2)];
        // BT: rank0 在底部（y 更大），rank2 在顶部（y 更小）
        assert!(a.y > c.y, "BT should put rank0 below rank2");
        assert!(a.y >= 0.0 && c.y >= 0.0);
        // BT 不互换尺寸
        assert_eq!(r.sizes[&NodeIndex::new(0)].width, 100.0);
    }

    #[test]
    fn td_unchanged() {
        let mut r = sample_result();
        let before = r.positions.clone();
        transform_sugiyama_direction(&mut r, Direction::TD);
        assert_eq!(r.positions, before);
    }
}
