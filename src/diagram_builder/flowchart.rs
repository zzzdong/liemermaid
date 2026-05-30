use std::collections::{HashMap, HashSet};

use vello_cpu::kurbo::{Point, Rect};

use crate::{
    ast::{Direction, Flowchart, Node, NodeShape, Edge},
    diagram_builder::types::OutputConfig,
    error::DiagramResult,
    text::{create_text_layout, compute_text_offset},
    visual::{
        Color, FillStrokeStyle, StrokeStyle, TextAlign, TextBaseline, TextStyle, VisualElement,
        Z_AXIS, Z_LABEL, Z_SERIES,
    },
};

use super::layout::{
    edges::route_edges,
    layers::assign_layers,
    measure::{measure_groups, measure_nodes},
    position::compute_positions,
    recognize::recognize_structure,
    types::{NodeMetrics, NodePosition, Size},
};

const NODE_FONT_SIZE: f64 = 13.0;

const EDGE_STROKE: StrokeStyle = StrokeStyle {
    color: Color {
        r: 68,
        g: 68,
        b: 68,
        a: 255,
    },
    width: 2.0,
};
const NODE_FILL: Color = Color::new(240, 248, 255);
const NODE_STROKE: Color = Color::new(68, 114, 196);

/// 流程图构建入口：7-Pass 布局管线
///
/// Pass 1: 结构识别 (recognize_structure)
/// Pass 2: 尺寸测量 (measure_nodes + measure_groups)
/// Pass 3: 层级分配 (assign_layers)
/// Pass 5: 几何定位 (compute_positions)
/// Pass 7: 边路由 (route_edges)
/// Pass 6: 画布适配（使用 config 指定的尺寸，做居中适配）
pub fn build_flowchart_elements(
    fc: &Flowchart,
    config: &OutputConfig,
) -> DiagramResult<Vec<VisualElement>> {
    let direction = fc.direction.clone().unwrap_or(Direction::TD);

    // Pass 1: 结构识别
    let tree = recognize_structure(fc);

    // Pass 2: 尺寸测量
    let node_metrics = measure_nodes(&fc.nodes, config);
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

    // 计算整体 bounding box 并居中到画布
    let (offset_x, offset_y) = compute_center_offset(&node_positions, &node_metrics, config);

    // 计算回边对（传递给边路由）
    let back_edge_pairs = compute_back_edge_pairs(&fc.edges, &layers);

    // Pass 7: 边路由
    let routed_edges = route_edges(&fc.edges, &node_positions, &layers, &back_edge_pairs, direction.clone());

    // 生成 VisualElement
    let mut elements = Vec::new();

    // 绘制边（先画边，后画节点）
    for re in &routed_edges {
        if re.route.len() >= 2 {
            let pts: Vec<Point> = re
                .route
                .iter()
                .map(|p| Point::new(p.x + offset_x, p.y + offset_y))
                .collect();
            elements.push(VisualElement::Polyline {
                points: pts,
                style: EDGE_STROKE,
                z_index: Z_AXIS,
            });
        }
    }

    // 绘制节点
    for node in &fc.nodes {
        if let Some(pos) = node_positions.get(&node.id) {
            let nm = node_metrics.get(&node.id);
            draw_node_with_metrics(
                &mut elements,
                node,
                pos,
                nm,
                offset_x,
                offset_y,
            );
        }
    }

    Ok(elements)
}

/// 计算整体在画布中的居中偏移
fn compute_center_offset(
    positions: &HashMap<String, NodePosition>,
    metrics: &HashMap<String, NodeMetrics>,
    config: &OutputConfig,
) -> (f64, f64) {
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

    if min_x == f64::MAX {
        return (0.0, 0.0);
    }

    let content_w = max_x - min_x;
    let content_h = max_y - min_y;

    let offset_x = (config.width - content_w) / 2.0 - min_x;
    let offset_y = (config.height - content_h) / 2.0 - min_y;

    (offset_x, offset_y)
}

/// 计算回边集合（基于原始图结构和BFS层级）
fn compute_back_edge_pairs(edges: &[Edge], _layers: &HashMap<String, usize>) -> HashSet<(String, String)> {
    // 直接用 BFS 重新计算层级来判断回边
    let mut in_deg: HashMap<String, usize> = HashMap::new();
    let mut out_edges: HashMap<String, Vec<String>> = HashMap::new();
    for edge in edges {
        out_edges.entry(edge.source.clone()).or_default().push(edge.target.clone());
        *in_deg.entry(edge.target.clone()).or_insert(0) += 1;
        in_deg.entry(edge.source.clone()).or_insert(0);
    }

    let mut bfs_layers: HashMap<String, usize> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for (node, &deg) in &in_deg {
        if deg == 0 {
            bfs_layers.insert(node.clone(), 0);
            queue.push_back(node.clone());
        }
    }
    if queue.is_empty() {
        if let Some(first) = in_deg.keys().next() {
            bfs_layers.insert(first.clone(), 0);
            queue.push_back(first.clone());
        }
    }
    while let Some(cur) = queue.pop_front() {
        let cur_layer = bfs_layers[&cur];
        if let Some(targets) = out_edges.get(&cur) {
            for t in targets {
                let new_layer = cur_layer + 1;
                let existing = bfs_layers.get(t).copied().unwrap_or(usize::MAX);
                if new_layer < existing {
                    bfs_layers.insert(t.clone(), new_layer);
                    queue.push_back(t.clone());
                }
            }
        }
    }

    let mut back_edges = HashSet::new();
    for edge in edges {
        let from_layer = bfs_layers.get(&edge.source).copied().unwrap_or(0);
        let to_layer = bfs_layers.get(&edge.target).copied().unwrap_or(0);
        if from_layer > to_layer {
            back_edges.insert((edge.source.clone(), edge.target.clone()));
        }
    }
    back_edges
}

/// 根据 NodeMetrics 绘制节点（支持不同形状和动态尺寸）
fn draw_node_with_metrics(
    elements: &mut Vec<VisualElement>,
    node: &Node,
    pos: &NodePosition,
    nm: Option<&NodeMetrics>,
    offset_x: f64,
    offset_y: f64,
) {
    let size = nm.map(|m| m.size).unwrap_or(Size::new(140.0, 50.0));
    let center = Point::new(pos.center.x + offset_x, pos.center.y + offset_y);

    let rect = Rect::new(
        center.x - size.width / 2.0,
        center.y - size.height / 2.0,
        center.x + size.width / 2.0,
        center.y + size.height / 2.0,
    );

    let style = FillStrokeStyle::new()
        .with_fill(NODE_FILL)
        .with_stroke(NODE_STROKE, 2.0);

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
        Some(NodeShape::Diamond) => {
            let points = vec![
                Point::new(center.x, center.y - size.height / 2.0),
                Point::new(center.x + size.width / 2.0, center.y),
                Point::new(center.x, center.y + size.height / 2.0),
                Point::new(center.x - size.width / 2.0, center.y),
            ];
            elements.push(VisualElement::Polyline {
                points: vec![points[0], points[1], points[2], points[3], points[0]],
                style: StrokeStyle {
                    color: NODE_STROKE,
                    width: 2.0,
                },
                z_index: Z_SERIES,
            });
        }
        Some(NodeShape::Hexagon) => {
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let q = w * 0.3;
            let points = vec![
                Point::new(center.x - w + q, center.y - h),
                Point::new(center.x + w - q, center.y - h),
                Point::new(center.x + w, center.y),
                Point::new(center.x + w - q, center.y + h),
                Point::new(center.x - w + q, center.y + h),
                Point::new(center.x - w, center.y),
            ];
            elements.push(VisualElement::Polyline {
                points: vec![
                    points[0], points[1], points[2], points[3], points[4], points[5], points[0],
                ],
                style: StrokeStyle {
                    color: NODE_STROKE,
                    width: 2.0,
                },
                z_index: Z_SERIES,
            });
        }
        _ => {
            elements.push(VisualElement::Rect {
                rect,
                style,
                z_index: Z_SERIES,
            });
        }
    }

    // 节点文本
    let text = node.text.as_deref().unwrap_or(&node.id);
    let text_style = TextStyle {
        font_size: NODE_FONT_SIZE,
        align: TextAlign::Center,
        vertical_align: TextBaseline::Middle,
        color: Color::new(34, 34, 34),
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
