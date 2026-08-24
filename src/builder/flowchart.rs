use std::collections::HashMap;

use lievisual::geometry::{Point, Rect};
use petgraph::graph::{DiGraph, NodeIndex};
use vello_cpu::kurbo::BezPath;

use crate::{
    ast::{ArrowType, Direction, Flowchart, NodeShape}, builder::types::OutputConfig, error::DiagramResult,     vir::{
        self, Color, Stroke, TextAlign, TextBaseline, Z_AXIS, Z_LABEL, Z_SERIES, Z_SUBGRAPH, Z_SUBGRAPH_LABEL, draw_arrow_circle, draw_arrow_cross, draw_arrow_head, theme,
    },
};
use lievisual::scene::SceneNode;
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

use super::layout::{
    measure::measure_nodes,
    recognize::all_flowchart_nodes,
    sugiyama::{NodeSize, SugiyamaResult},
    types::{
        Layout, LayoutEdge, LayoutEngine, LayoutMetadata, LayoutNode, LayoutSubgraph, NodeMetrics,
        NodeStyle, Size,
    },
};
use crate::builder::dagre_layout;

const NODE_FONT_SIZE: f64 = theme::FONT_SIZE;

/// 画布边距
const MARGIN: f64 = 40.0;

/// 子图标题区高度（容器框顶部留白）
const SUBGRAPH_TITLE_H: f64 = 22.0;

fn edge_stroke() -> Stroke {
    vir::stroke(theme::flowchart::EDGE, theme::EDGE_WIDTH)
}

/// 根据箭头类型返回对应的描边样式：
/// - `==>`(Thick) 加粗；其余保持默认线宽。
fn edge_stroke_for(arrow: &ArrowType) -> Stroke {
    match arrow {
        ArrowType::Thick => vir::stroke(theme::flowchart::EDGE, theme::EDGE_WIDTH * 1.6),
        _ => vir::stroke(theme::flowchart::EDGE, theme::EDGE_WIDTH),
    }
}

/// 把折线按固定步长离散成密集点序列（用于虚线采样）
fn sample_polyline(route: &[Point], step: f64) -> Vec<Point> {
    if route.len() < 2 {
        return route.to_vec();
    }
    let mut pts = vec![route[0]];
    for i in 1..route.len() {
        let a = route[i - 1];
        let b = route[i];
        let len = a.distance(b);
        if len <= 1e-6 {
            continue;
        }
        let n = (len / step).ceil() as usize;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            pts.push(Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
        }
    }
    pts
}

/// 绘制虚线边：沿 route 折线按 dash/gap 周期切分出短直线段，终点自绘箭头。
fn draw_dashed_edge(elements: &mut Vec<SceneNode>, route: &[Point]) {
    const DASH: f64 = 6.0;
    const GAP: f64 = 4.0;
    const STEP: f64 = 2.0;
    let dense = sample_polyline(route, STEP);
    if dense.len() < 2 {
        return;
    }
    let stroke = edge_stroke_for(&ArrowType::Dotted);
    let mut drawing = true; // 当前处于 dash 段还是 gap 段
    let mut phase: f64 = 0.0; // 当前段内已走过的距离（用于判断是否跨越边界）
    let mut cur = dense[0];
    let mut last_dir = Point::new(0.0, 0.0);
    let mut last_point = dense[dense.len() - 1];
    for i in 1..dense.len() {
        let p = dense[i];
        let seg = cur.distance(p.clone());
        if seg <= 1e-6 {
            cur = p;
            continue;
        }
        let mut pos = 0.0;
        while pos < seg {
            let remain = if drawing { DASH - phase } else { GAP - phase };
            let take = remain.min(seg - pos);
            let next = Point::new(
                cur.x + (p.x - cur.x) * (pos + take) / seg,
                cur.y + (p.y - cur.y) * (pos + take) / seg,
            );
            if drawing {
                elements.push(vir::polyline_node(vec![cur, next], stroke.clone(), Z_AXIS));
                last_dir = Point::new(next.x - cur.x, next.y - cur.y);
                last_point = next;
            }
            pos += take;
            phase += take;
            let limit = if drawing { DASH } else { GAP };
            if phase >= limit - 1e-9 {
                phase = 0.0;
                drawing = !drawing;
            }
            cur = next;
        }
        cur = p;
    }
    let len = (last_dir.x * last_dir.x + last_dir.y * last_dir.y).sqrt();
    if len > 0.0 {
        let ud = Point::new(last_dir.x / len, last_dir.y / len);
        draw_arrow_head(elements, &last_point, &ud, &edge_stroke(), false);
    }
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
#[cfg(test)]
fn transform_sugiyama_direction(result: &mut SugiyamaResult, direction: Direction) {
    use lievisual::geometry::Point;

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
        let qs: Vec<Point> = pts
            .iter()
            .map(|&p| {
                let q = map(p);
                min_x = min_x.min(q.x);
                min_y = min_y.min(q.y);
                q
            })
            .collect();
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
        // Sugiyama 可能在内部插入虚拟节点，其索引可能超出 graph 节点范围；
        // 仅保留真实存在的节点位置。
        if let Some(id) = graph.node_weight(*idx) {
            node_centers.insert(id.clone(), *pos);
        }
    }

    let mut elements = Vec::new();

    // 沿折线按比例 t∈[0,1] 取坐标
    fn polyline_point_at(pts: &[Point], t: f64) -> Point {
        if pts.len() < 2 {
            return pts.first().copied().unwrap_or_default();
        }
        let mut seg_lens = Vec::new();
        let mut total = 0.0;
        for w in pts.windows(2) {
            let d = (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
            seg_lens.push(d);
            total += d;
        }
        if total == 0.0 {
            return pts[pts.len() / 2];
        }
        let target = t.clamp(0.0, 1.0) * total;
        let mut acc = 0.0;
        for (i, &d) in seg_lens.iter().enumerate() {
            if acc + d >= target {
                let r = if d > 0.0 { (target - acc) / d } else { 0.0 };
                return Point::new(
                    pts[i].x + (pts[i + 1].x - pts[i].x) * r,
                    pts[i].y + (pts[i + 1].y - pts[i].y) * r,
                );
            }
            acc += d;
        }
        *pts.last().unwrap()
    }

    // 边标签：半透明灰底 + 文本（对齐官方 edgeLabelBackground=rgba(232,232,232,0.8)）
    fn draw_edge_label(elements: &mut Vec<SceneNode>, pos: Point, text: &str) {
        // 官方默认 edgeLabelBackground = rgba(232,232,232,0.8)
        let label_bg = Color::new(232.0 / 255.0, 232.0 / 255.0, 232.0 / 255.0, 0.8);
        let label_bg_stroke = Color::new(200.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0, 1.0);
        let style = vir::text_style(
            theme::flowchart::TEXT,
            NODE_FONT_SIZE,
            theme::FONT_FAMILY,
            TextAlign::Center,
            TextBaseline::Middle,
        );
        let layout = layout_text(&[RichSpan::new(text.to_string(), style.clone())], None);
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
        let w = layout.width + 8.0;
        let h = layout.height + 4.0;
        elements.push(
            vir::rect_node(
                Rect::new(
                    pos.x - w / 2.0,
                    pos.y - h / 2.0,
                    pos.x + w / 2.0,
                    pos.y + h / 2.0,
                ),
                Some(2.0),
                vir::fs_both(label_bg, label_bg_stroke, 1.0),
                Z_LABEL,
            )
            ,
        );
        elements.push(vir::text_node(
            text.to_string(),
            Point::new(pos.x + x_off, pos.y + y_off),
            style
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Top),
            0.0,
            None,
            Z_LABEL,
        ));
    }

    for edge in fc.edges.iter() {
        if let (Some(&from_idx), Some(&to_idx)) =
            (indices.get(&edge.source), indices.get(&edge.target))
            && let Some(route) = result.edge_routes.get(&(from_idx, to_idx))
        {
            if route.len() >= 2 {
                let arrow = &edge.arrow_type;
                if let ArrowType::Dotted = arrow {
                    // 虚线：沿折线切分短直线段
                    draw_dashed_edge(&mut elements, &route);
                } else {
                    elements.push(vir::curved_edge_node(route.clone(), edge_stroke_for(arrow), Z_AXIS));
                }
                // 边标签
                if let Some(label_text) = &edge.label {
                    let mid = polyline_point_at(route, 0.5);
                    draw_edge_label(&mut elements, mid, label_text);
                }
                let last = route.last().unwrap();
                let first = route.first().unwrap();
                let n = route.len();
                let prev = &route[n - 2];
                let dx = last.x - prev.x;
                let dy = last.y - prev.y;
                let len = (dx * dx + dy * dy).sqrt();
                let tip_dir = if len > 0.0 {
                    Point::new(dx / len, dy / len)
                } else {
                    Point::new(0.0, 0.0)
                };
                // 终点标记：特殊形状自绘，普通箭头用 draw_arrow_head 自绘（IR 不含箭头概念）
                match arrow {
                    ArrowType::NoArrow | ArrowType::Invisible => {}
                    ArrowType::Circle | ArrowType::MultiCircle => {
                        draw_arrow_circle(&mut elements, last, &edge_stroke());
                    }
                    ArrowType::Cross | ArrowType::MultiCross => {
                        draw_arrow_cross(&mut elements, last, &edge_stroke());
                    }
                    _ => {
                        if len > 0.0 {
                            draw_arrow_head(&mut elements, last, &tip_dir, &edge_stroke(), false);
                        }
                    }
                }
                // 双向箭头：起点也画一个反向标记
                if arrow == &ArrowType::Both
                    || arrow == &ArrowType::MultiCircle
                    || arrow == &ArrowType::MultiCross
                {
                    let next = route[1];
                    let dx = first.x - next.x;
                    let dy = first.y - next.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let ud = Point::new(dx / len, dy / len);
                        match arrow {
                            ArrowType::MultiCircle => {
                                draw_arrow_circle(&mut elements, first, &edge_stroke());
                            }
                            ArrowType::MultiCross => {
                                draw_arrow_cross(&mut elements, first, &edge_stroke());
                            }
                            _ => {
                                draw_arrow_head(&mut elements, first, &ud, &edge_stroke(), false);
                            }
                        }
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
        // 官方 default 主题：subgraph 容器 clusterBkg=#FFFFDE（浅黄）+ 描边 #9370DB
        let subgraph_fill = Color::new(255.0 / 255.0, 255.0 / 255.0, 222.0 / 255.0, 1.0); // #FFFFDE
        let style = vir::fs_both(subgraph_fill, theme::flowchart::SUBGRAPH_STROKE, theme::EDGE_WIDTH);
        elements.push(vir::rect_node(rect, Some(theme::NODE_RADIUS), style, Z_SUBGRAPH));

        if let Some(title) = &sg.title {
            let title_style = vir::text_style(
                theme::flowchart::SUBGRAPH_TITLE,
                NODE_FONT_SIZE,
                theme::FONT_FAMILY,
                TextAlign::Left,
                TextBaseline::Top,
            );
            let title_position = Point::new(rect.min_x() + 10.0, rect.min_y() + 6.0);
            elements.push(
                vir::text_node(
                    title.clone(),
                    title_position,
                    title_style
                        .with_align(TextAlign::Left)
                        .with_baseline(TextBaseline::Top),
                    0.0,
                    None,
                    Z_SUBGRAPH_LABEL,
                )
                ,
            );
        }
    }

    // 绘制边：曲线 + 自绘箭头
    for edge in &layout.edges {
        if edge.path.len() >= 2 {
            let pts: Vec<Point> = edge
                .path
                .iter()
                .map(|p| Point::new(p.x + offset_x, p.y + offset_y))
                .collect();
            elements.push(vir::curved_edge_node(pts.clone(), edge_stroke(), Z_AXIS));
            let n = pts.len();
            if n >= 2 {
                let last = &pts[n - 1];
                let prev = &pts[n - 2];
                let dx = last.x - prev.x;
                let dy = last.y - prev.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.0 {
                    let ud = Point::new(dx / len, dy / len);
                    draw_arrow_head(&mut elements, last, &ud, &edge_stroke(), false);
                }
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
/// 布局由 dagre 求解（节点尺寸经 measure_nodes 预测量后传入），
/// 不再走旧的自研 7-Pass 管线。
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

        // Pass 1: 结构识别（dagre 内部完成，这里不再需要单独调用）

        // Pass 2: 尺寸测量（两种路径都需要，含 subgraph 内部节点）
        let all_nodes = all_flowchart_nodes(fc);
        let node_metrics = measure_nodes(&all_nodes, config);

        // 统一用 dagre（dagrejs/dagre 的 Rust 移植）做分层布局，与 mermaid 官方
        // 布局引擎一致；dagre 原生支持所有方向，并内建 compound 模式处理 subgraph。
        let dagre_result = run_dagre_layout(fc, &node_metrics, &direction);

        if !has_subgraphs(fc) {
            // 无子图：构造 SugiyamaResult 复用现有渲染（保留特殊箭头等细节）
            let (graph, indices) = build_flowchart_graph(fc);
            let result = sugiyama_result_from_dagre(fc, &dagre_result, &indices, &node_metrics);
            let elements = render_sugiyama_flowchart(fc, &result, &graph, &indices, &node_metrics);
            return Ok(elements);
        }

        // 有子图：构造统一 Layout IR（含子图容器框）并渲染
        let layout = layout_from_dagre(fc, &dagre_result, &node_metrics, direction);
        Ok(render_layout(&layout))
    }
}

/// 构建 dagre 所需的节点尺寸映射（按 id，含 subgraph 内部节点）
fn run_dagre_layout(
    fc: &Flowchart,
    node_metrics: &HashMap<String, NodeMetrics>,
    direction: &Direction,
) -> dagre_layout::DagreLayout {
    let swap = matches!(direction, Direction::LR | Direction::RL);
    let mut sizes: HashMap<String, dagre_layout::NodeSize> = HashMap::new();
    for id in node_metrics.keys() {
        let nm = &node_metrics[id];
        let (width, height) = if swap {
            (nm.size.height, nm.size.width)
        } else {
            (nm.size.width, nm.size.height)
        };
        sizes.insert(id.clone(), dagre_layout::NodeSize { width, height });
    }
    dagre_layout::run_dagre(fc, &sizes, direction)
}

/// 把 dagre 布局结果转换为 `SugiyamaResult`（无子图路径复用现有渲染）
fn sugiyama_result_from_dagre(
    fc: &Flowchart,
    dagre_result: &dagre_layout::DagreLayout,
    indices: &HashMap<String, NodeIndex>,
    node_metrics: &HashMap<String, NodeMetrics>,
) -> SugiyamaResult {
    use lievisual::geometry::Point;
    use std::collections::HashSet;

    let mut positions = HashMap::new();
    let mut sizes = HashMap::new();
    for (id, &idx) in indices {
        if let Some(c) = dagre_result.centers.get(id) {
            positions.insert(idx, Point { x: c.x, y: c.y });
        }
        if let Some(nm) = node_metrics.get(id) {
            sizes.insert(
                idx,
                NodeSize {
                    width: nm.size.width,
                    height: nm.size.height,
                },
            );
        }
    }

    let mut edge_routes = HashMap::new();
    for edge in &fc.edges {
        if let (Some(&s), Some(&t)) = (indices.get(&edge.source), indices.get(&edge.target)) {
            if let Some(route) = dagre_result
                .edge_routes
                .get(&(edge.source.clone(), edge.target.clone()))
            {
                edge_routes.insert(
                    (s, t),
                    route.iter().map(|p| Point { x: p.x, y: p.y }).collect(),
                );
            }
        }
    }

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

/// 把 dagre 布局结果转换为统一 `Layout` IR（有子图路径，含子图容器框）
fn layout_from_dagre(
    fc: &Flowchart,
    dagre_result: &dagre_layout::DagreLayout,
    node_metrics: &HashMap<String, NodeMetrics>,
    direction: Direction,
) -> Layout {
    // 节点
    let mut nodes: Vec<LayoutNode> = Vec::new();
    let mut node_bounds: HashMap<String, Rect> = HashMap::new();
    for node in all_flowchart_nodes(fc) {
        let Some(c) = dagre_result.centers.get(&node.id) else {
            continue;
        };
        let nm = node_metrics
            .get(&node.id)
            .map(|m| m.size)
            .unwrap_or(Size::new(60.0, 30.0));
        let bounds = Rect::new(
            c.x - nm.width / 2.0,
            c.y - nm.height / 2.0,
            c.x + nm.width / 2.0,
            c.y + nm.height / 2.0,
        );
        node_bounds.insert(node.id.clone(), bounds);
        let ports = vec![
            Point::new(bounds.min_x(), c.y),
            Point::new(bounds.max_x(), c.y),
            Point::new(c.x, bounds.min_y()),
            Point::new(c.x, bounds.max_y()),
        ];
        nodes.push(LayoutNode {
            id: node.id.clone(),
            bounds,
            ports,
            label: node.text.clone(),
            shape: node.shape.clone(),
            style: NodeStyle::default(),
        });
    }

    // 边
    let edges: Vec<LayoutEdge> = fc
        .edges
        .iter()
        .chain(fc.subgraphs.iter().flat_map(|sg| sg.edges.iter()))
        .map(|edge| {
            let path = dagre_result
                .edge_routes
                .get(&(edge.source.clone(), edge.target.clone()))
                .cloned()
                .unwrap_or_else(|| {
                    // 退化：直线连接两端中心
                    let s = dagre_result.centers.get(&edge.source);
                    let t = dagre_result.centers.get(&edge.target);
                    match (s, t) {
                        (Some(a), Some(b)) => vec![*a, *b],
                        _ => vec![],
                    }
                });
            LayoutEdge {
                from: edge.source.clone(),
                to: edge.target.clone(),
                path,
                arrow_at_end: true,
                label: edge.label.clone(),
                label_position: None,
                curved: true,
            }
        })
        .collect();

    // 子图容器：成员节点包围盒外扩 padding
    let subgraph_padding = 24.0;
    let subgraphs: Vec<LayoutSubgraph> = fc
        .subgraphs
        .iter()
        .map(|sg| {
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_x = f64::MIN;
            let mut max_y = f64::MIN;
            for member in &sg.nodes {
                if let Some(b) = node_bounds.get(&member.id) {
                    min_x = min_x.min(b.min_x());
                    min_y = min_y.min(b.min_y());
                    max_x = max_x.max(b.max_x());
                    max_y = max_y.max(b.max_y());
                }
            }
            if min_x == f64::MAX {
                min_x = 0.0;
                min_y = 0.0;
                max_x = 0.0;
                max_y = 0.0;
            }
            let bounds = Rect::new(
                min_x - subgraph_padding,
                min_y - subgraph_padding - SUBGRAPH_TITLE_H,
                max_x + subgraph_padding,
                max_y + subgraph_padding,
            );
            LayoutSubgraph {
                title: sg.title.clone(),
                member_ids: sg.nodes.iter().map(|n| n.id.clone()).collect(),
                bounds,
            }
        })
        .collect();

    Layout {
        nodes,
        edges,
        size: Size::new(0.0, 0.0),
        metadata: LayoutMetadata { direction },
        subgraphs,
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
            // measure 阶段已保证 size 为正方形且含留白，半径直接取半宽
            let radius = size.width / 2.0;
            elements.push(vir::circle_node(center, radius, style, Z_SERIES));
        }
        Some(NodeShape::DoubleCircle) => {
            let outer_r = size.width / 2.0;
            let inner_r = outer_r * 0.75;
            elements.push(vir::circle_node(center, outer_r, style.clone(), Z_SERIES));
            elements.push(vir::circle_node(
                center,
                inner_r,
                vir::fs_stroke(stroke, 2.0),
                Z_SERIES,
            ));
        }
        Some(NodeShape::Stadium) => {
            // Stadium（跑道形）：上下直线 + 左右半圆，半圆半径 = 半高
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
            elements.push(vir::path_node(path, style, Z_SERIES));
        }
        Some(NodeShape::Cylinder) => {
            // 数据库/圆柱：矩形主体 + 顶部椭圆盖 + 底部下凸弧
            let w = size.width / 2.0;
            let h = size.height / 2.0;
            let ry = (2.0 * h) * 0.13;
            let segments = 16;
            let left = center.x - w;
            let right = center.x + w;
            let top_y = center.y - h;
            let bot_y = center.y + h;
            // 主体（填充 + 描边）
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
            elements.push(vir::path_node(
                body,
                vir::fs_both(fill, stroke, node.style.stroke_width),
                Z_SERIES,
            ));
            // 顶部椭圆盖（盖在主体上方，仅描边即可，但填充以覆盖接缝）
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
            elements.push(
                vir::path_node(
                    top,
                    vir::fs_both(fill, stroke, node.style.stroke_width),
                    Z_SERIES,
                )
                ,
            );
        }
        Some(NodeShape::Subroutine) => {
            // 子程序：矩形 + 左右两侧内嵌竖条
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
            elements.push(vir::path_node(outer, style.clone(), Z_SERIES));
            let notch = 8.0;
            let inner = vir::stroke(stroke, node.style.stroke_width);
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
        Some(NodeShape::Rounded) => {
            // 圆角矩形：([]) 语法，对齐官方 rx=5
            elements.push(vir::rect_node(rect, Some(theme::NODE_RADIUS), style, Z_SERIES));
        }
        Some(NodeShape::Rectangle) | _ => {
            // 默认矩形（[]）：官方基本矩形为直角（rx=0）
            elements.push(vir::rect_node(rect, None, style, Z_SERIES));
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
    let layout = layout_text(
        &[RichSpan::new(text.to_string(), text_style.clone())],
        max_w,
    );

    let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
    let text_position = Point::new(center.x + x_off, center.y + y_off);

    elements.push(
        vir::text_node(
            text.to_string(),
            text_position,
            text_style
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Top),
            0.0,
            max_w,
            Z_LABEL,
        )
        ,
    );
}

#[cfg(test)]
mod direction_transform_tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn sample_result() -> SugiyamaResult {
        use lievisual::geometry::Point;
        let a = NodeIndex::new(0);
        let b = NodeIndex::new(1);
        let c = NodeIndex::new(2);
        let mut positions = HashMap::new();
        positions.insert(a, Point::new(0.0, 0.0));
        positions.insert(b, Point::new(0.0, 100.0));
        positions.insert(c, Point::new(0.0, 200.0));
        let mut sizes = HashMap::new();
        for n in [a, b, c] {
            sizes.insert(
                n,
                NodeSize {
                    width: 100.0,
                    height: 40.0,
                },
            );
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
