use std::collections::{HashMap, HashSet};

use lievisual::geometry::{Point, Rect, Transform};
use petgraph::graph::{DiGraph, NodeIndex};

use crate::{
    ast::{State, StateDiagram},
    builder::{
        layout::sugiyama::{NodeSize, SugiyamaConfig, SugiyamaLayout},
        layout::types::LayoutEngine,
        types::OutputConfig,
    },
    error::DiagramResult,
    vir::{self, TextAlign, TextBaseline, Z_AXIS, Z_LABEL, Z_SERIES, theme},
};
use lievisual::geometry::Color;
use lievisual::scene::SceneNode;
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

const STATE_PAD_X: f64 = 18.0;
const STATE_PAD_Y: f64 = 10.0;
const FONT_SIZE: f64 = theme::FONT_SIZE;
const SMALL_FONT: f64 = 11.0;
const COMPOSITE_PAD: f64 = 16.0;
const COMPOSITE_TITLE_H: f64 = 22.0;
const Z_INNER: i32 = Z_SERIES + 1;

/// State diagram node kinds for layout
#[derive(Debug, Clone)]
enum StateNode {
    Start,
    End,
    /// fork / join 伪节点：只画形状，不显示文本标签（与官方 mermaid 一致）。
    Fork,
    Join,
    /// 复合状态：渲染为带标题的嵌套容器，内部递归放置子状态图。
    Composite,
    Normal {
        description: Option<String>,
        label: Option<String>,
    },
}

pub struct StateEngine<'a> {
    diagram: &'a StateDiagram,
}

impl<'a> StateEngine<'a> {
    pub fn new(diagram: &'a StateDiagram) -> Self {
        Self { diagram }
    }
}

impl<'a> LayoutEngine for StateEngine<'a> {
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
        Ok(build_state_elements(self.diagram, config))
    }
}

pub fn build_state_elements(diagram: &StateDiagram, _config: &OutputConfig) -> Vec<SceneNode> {
    let config = SugiyamaConfig::default();
    render_state_diagram(diagram, &config).0
}

/// 渲染单个状态图（递归用于复合状态）。返回图元与包围盒尺寸，坐标已归一化到 (0,0) 起。
fn render_state_diagram(diagram: &StateDiagram, config: &SugiyamaConfig) -> (Vec<SceneNode>, f64, f64) {
    let mut elements = Vec::new();

    if diagram.transitions.is_empty() && diagram.states.is_empty() {
        return (elements, 0.0, 0.0);
    }

    // ---- Collect all state nodes ----
    let mut state_map: HashMap<String, StateNode> = HashMap::new();
    let mut out_edges: HashMap<String, Vec<(String, Option<String>)>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    // 显示名：state "label" as id 中的 label（存于 State::Simple.description）
    let state_labels: HashMap<String, String> = diagram
        .states
        .iter()
        .filter_map(|s| match s {
            State::Simple { id, description } => {
                description.clone().map(|d| (id.clone(), d))
            }
            _ => None,
        })
        .collect();

    // fork / join / composite 节点集合：用于决定节点种类（官方 mermaid 中这些节点
    // 不显示普通文本标签，fork/join 只画形状，composite 渲染为带标题的嵌套容器）。
    let fork_ids: HashSet<String> = diagram
        .states
        .iter()
        .filter_map(|s| match s {
            State::Fork { id } => Some(id.clone()),
            _ => None,
        })
        .collect();
    let join_ids: HashSet<String> = diagram
        .states
        .iter()
        .filter_map(|s| match s {
            State::Join { id } => Some(id.clone()),
            _ => None,
        })
        .collect();
    let composite_inner: HashMap<String, StateDiagram> = diagram
        .states
        .iter()
        .filter_map(|s| match s {
            State::Composite { id, inner } => Some((id.clone(), (**inner).clone())),
            _ => None,
        })
        .collect();

    // 预计算复合状态内部子图的包围盒尺寸（用于主布局中为其预留容器大小）。
    let mut composite_bounds: HashMap<String, (f64, f64)> = HashMap::new();
    for (id, inner) in &composite_inner {
        let (_, iw, ih) = render_state_diagram(inner, config);
        composite_bounds.insert(id.clone(), (iw, ih));
    }

    // 根据状态 id 选择布局节点种类。
    let make_node = |id: &str, lbl: Option<String>| -> StateNode {
        if fork_ids.contains(id) {
            StateNode::Fork
        } else if join_ids.contains(id) {
            StateNode::Join
        } else if composite_inner.contains_key(id) {
            StateNode::Composite
        } else {
            StateNode::Normal {
                description: None,
                label: lbl,
            }
        }
    };

    // Internal keys to distinguish start [*] from end [*]
    const START_KEY: &str = "__start__";
    const END_KEY: &str = "__end__";

    // Process transitions to discover all states
    for t in &diagram.transitions {
        let from_key = if t.from == "[*]" {
            START_KEY.to_string()
        } else {
            t.from.clone()
        };
        let to_key = if t.to == "[*]" {
            END_KEY.to_string()
        } else {
            t.to.clone()
        };

        // Register states
        if t.from == "[*]" {
            state_map
                .entry(START_KEY.to_string())
                .or_insert(StateNode::Start);
        } else {
            let lbl = state_labels.get(&t.from).cloned();
            state_map.entry(t.from.clone()).or_insert_with(|| make_node(&t.from, lbl));
        }

        if t.to == "[*]" {
            state_map
                .entry(END_KEY.to_string())
                .or_insert(StateNode::End);
        } else {
            let lbl = state_labels.get(&t.to).cloned();
            state_map.entry(t.to.clone()).or_insert_with(|| make_node(&t.to, lbl));
        }

        // Build edge list using internal keys
        out_edges
            .entry(from_key.clone())
            .or_default()
            .push((to_key.clone(), t.label.clone()));
        *in_degree.entry(to_key).or_insert(0) += 1;
        in_degree.entry(from_key).or_insert(0);
    }

    // ---- Topological sort for layout order ----
    let topo_order = topological_sort(&in_degree, &out_edges);

    // ---- Measure each node ----
    let node_ids: Vec<String> = topo_order
        .iter()
        .filter(|id| state_map.contains_key(*id))
        .cloned()
        .collect();

    struct NodeLayout {
        width: f64,
        height: f64,
        label: String,
    }

    let mut node_layouts: HashMap<String, NodeLayout> = HashMap::new();
    for id in &node_ids {
        let node = &state_map[id];
        let (label, desc) = match node {
            StateNode::Start => ("Start".to_string(), None),
            StateNode::End => ("End".to_string(), None),
            StateNode::Fork => (String::new(), None),
            // join 节点显示其 id 作为标签（官方 mermaid 行为）。
            StateNode::Join => (id.clone(), None),
            StateNode::Composite => (id.clone(), None),
            StateNode::Normal { label, description, .. } => {
                (label.clone().unwrap_or_else(|| id.clone()), description.clone())
            }
        };

        // Measure text
        let ts = vir::text_style(
            Color::BLACK,
            FONT_SIZE,
            String::new(),
            TextAlign::Center,
            TextBaseline::Middle,
        );
        let layout = layout_text(&[RichSpan::new(label.to_string(), ts.clone())], None);
        let text_w = layout.width;
        let text_h = layout.height;

        let desc_text_h = if let Some(d) = &desc {
            let dl = layout_text(
                &[RichSpan::new(
                    d.to_string(),
                    vir::text_style(
                        Color::BLACK,
                        SMALL_FONT,
                        String::new(),
                        TextAlign::Center,
                        TextBaseline::Middle,
                    ),
                )],
                None,
            );
            dl.height + 4.0
        } else {
            0.0
        };

        match node {
            StateNode::Start => {
                let r = 16.0;
                node_layouts.insert(
                    id.clone(),
                    NodeLayout {
                        width: r * 2.0 + 20.0,
                        height: r * 2.0 + 20.0,
                        label: label.clone(),
                    },
                );
            }
            StateNode::End => {
                let r = 16.0;
                node_layouts.insert(
                    id.clone(),
                    NodeLayout {
                        width: r * 2.0 + 24.0,
                        height: r * 2.0 + 24.0,
                        label: label.clone(),
                    },
                );
            }
            StateNode::Fork => {
                // fork/join 节点的形状（粗横线）不需要文本，给一个与官方相近的尺寸。
                let w = 40.0;
                let h = 24.0;
                node_layouts.insert(
                    id.clone(),
                    NodeLayout {
                        width: w,
                        height: h,
                        label,
                    },
                );
            }
            StateNode::Join => {
                let w = 40.0;
                let h = 24.0;
                node_layouts.insert(
                    id.clone(),
                    NodeLayout {
                        width: w,
                        height: h,
                        label,
                    },
                );
            }
            StateNode::Composite => {
                // 容器尺寸 = 内部子图包围盒 + 标题高度 + 内边距。
                let (iw, ih) = composite_bounds
                    .get(id)
                    .copied()
                    .unwrap_or((120.0, 80.0));
                let w = iw + COMPOSITE_PAD * 2.0;
                let h = ih + COMPOSITE_TITLE_H + COMPOSITE_PAD * 2.0;
                node_layouts.insert(
                    id.clone(),
                    NodeLayout {
                        width: w,
                        height: h,
                        label,
                    },
                );
            }
            StateNode::Normal { .. } => {
                let w = text_w.max(80.0) + STATE_PAD_X * 2.0;
                let h = (text_h + desc_text_h).max(40.0) + STATE_PAD_Y * 2.0;
                node_layouts.insert(
                    id.clone(),
                    NodeLayout {
                        width: w,
                        height: h,
                        label,
                    },
                );
            }
        }
    }

    // ---- Sugiyama layered layout ----
    let mut graph = DiGraph::new();
    let mut node_indices: HashMap<String, NodeIndex> = HashMap::new();
    for id in &node_ids {
        let idx = graph.add_node(id.clone());
        node_indices.insert(id.clone(), idx);
    }
    for (from, edges) in &out_edges {
        if let Some(from_idx) = node_indices.get(from) {
            for (to, _) in edges {
                if let Some(to_idx) = node_indices.get(to) {
                    graph.add_edge(*from_idx, *to_idx, ());
                }
            }
        }
    }

    // Build node sizes for Sugiyama
    let mut sugiyama_node_sizes: HashMap<NodeIndex, NodeSize> = HashMap::new();
    for id in &node_ids {
        if let Some(idx) = node_indices.get(id) {
            let nl = &node_layouts[id];
            sugiyama_node_sizes.insert(
                *idx,
                NodeSize {
                    width: nl.width,
                    height: nl.height,
                },
            );
        }
    }

    // Run Sugiyama 4-phase layout
    let config = SugiyamaConfig {
        crossing_iterations: 10,
        ..Default::default()
    };
    let sugiyama = SugiyamaLayout::new(config.clone(), &graph);
    let result = sugiyama.layout(&sugiyama_node_sizes);

    // Convert petgraph NodeIndex positions back to string-keyed positions
    let mut positions: HashMap<String, Point> = HashMap::new();
    for (idx, pos) in &result.positions {
        // Sugiyama 布局可能在内部插入虚拟节点，其索引可能超出 graph 节点范围；
        // 仅保留真实存在的节点位置，跳过越界/虚拟索引。
        if let Some(id) = graph.node_weight(*idx) {
            positions.insert(id.clone(), *pos);
        }
    }

    // ---- Draw transitions with routing ----
    for t in &diagram.transitions {
        let from_key = if t.from == "[*]" {
            START_KEY.to_string()
        } else {
            t.from.clone()
        };
        let to_key = if t.to == "[*]" {
            END_KEY.to_string()
        } else {
            t.to.clone()
        };
        let from_pos = positions.get(&from_key);
        let to_pos = positions.get(&to_key);
        if let (Some(fp), Some(tp)) = (from_pos, to_pos) {
            let from_nl = &node_layouts[&from_key];
            let to_nl = &node_layouts[&to_key];
            // 节点边界偏移（Start 实心圆 r=10, End 双环 r=12, Fork/Join 横线, Normal 矩形半高）
            let from_node = &state_map[&from_key];
            let to_node = &state_map[&to_key];
            let from_bottom = match from_node {
                StateNode::Start => fp.y + 10.0,
                StateNode::End => fp.y + 12.0,
                StateNode::Fork | StateNode::Join => fp.y + 3.0, // 横线半宽
                _ => fp.y + from_nl.height / 2.0,
            };
            let to_top = match to_node {
                StateNode::Start => tp.y - 10.0,
                StateNode::End => tp.y - 12.0,
                StateNode::Fork | StateNode::Join => tp.y - 3.0,
                _ => tp.y - to_nl.height / 2.0,
            };
            let mid_y = (from_bottom + to_top) / 2.0;

            let stroke = vir::stroke(theme::state::EDGE, 1.5);

            // 检测回环边：目标节点在源节点上方
            let is_back_edge = to_top < from_bottom - 1.0;

            if is_back_edge {
                // 回环边：使用二次贝塞尔曲线绕行右侧，避免与正向边重叠
                // 控制点向右偏移，偏移量与垂直距离成正比
                let dx = (tp.x - fp.x).abs().max(40.0);
                let _dy = (from_bottom - to_top).max(20.0);
                let ctrl_x = fp.x.max(tp.x) + dx * 0.6;
                let ctrl_y = mid_y;

                use lievisual::geometry::BezPath;
                let mut path = BezPath::new();
                path.move_to(lievisual::geometry::Point::new(fp.x, from_bottom));
                path.quad_to(
                    lievisual::geometry::Point::new(ctrl_x, ctrl_y),
                    lievisual::geometry::Point::new(tp.x, to_top),
                );
                elements.push(vir::path_node(path, vir::fs_stroke(stroke.color, stroke.width), Z_AXIS));

                // 箭头：沿曲线末端切线方向
                let sz = 7.0;
                // 切线方向近似：终点 → 控制点的反方向
                let end_dx = tp.x - ctrl_x;
                let end_dy = to_top - ctrl_y;
                let end_l = (end_dx * end_dx + end_dy * end_dy).sqrt().max(1e-6);
                let ux = end_dx / end_l;
                let uy = end_dy / end_l;
                elements.push(vir::line_node(
                    lievisual::geometry::Point::new(tp.x, to_top),
                    lievisual::geometry::Point::new(tp.x - sz * (ux * 0.4 + uy * 0.5), to_top - sz * (uy * 0.4 - ux * 0.5)),
                    stroke.clone(),
                    Z_AXIS,
                ));
                elements.push(vir::line_node(
                    lievisual::geometry::Point::new(tp.x, to_top),
                    lievisual::geometry::Point::new(tp.x + sz * (ux * 0.4 - uy * 0.5), to_top - sz * (uy * 0.4 + ux * 0.5)),
                    stroke,
                    Z_AXIS,
                ));

                // 标签放在控制点附近
                if let Some(label) = &t.label {
                    let ts = vir::text_style(
                        theme::state::TEXT,
                        12.0,
                        String::new(),
                        TextAlign::Center,
                        TextBaseline::Bottom,
                    );
                    let layout =
                        layout_text(&[RichSpan::new(label.to_string(), ts.clone())], Some(200.0));
                    let (x_off, y_off) =
                        compute_text_offset(&layout, TextAlign::Center, TextBaseline::Bottom);
                    elements.push(vir::text_node(
                        label.clone(),
                        lievisual::geometry::Point::new(ctrl_x + x_off, ctrl_y - 4.0 + y_off),
                        ts.clone()
                            .with_align(TextAlign::Left)
                            .with_baseline(TextBaseline::Top),
                        0.0,
                        Some(200.0),
                        Z_LABEL,
                    ));
                }
            } else {
                // 普通正向边：直角折线路由
                let points = if (fp.x - tp.x).abs() < 0.001 {
                    vec![lievisual::geometry::Point::new(fp.x, from_bottom), lievisual::geometry::Point::new(fp.x, to_top)]
                } else {
                    vec![
                        lievisual::geometry::Point::new(fp.x, from_bottom),
                        lievisual::geometry::Point::new(fp.x, mid_y),
                        lievisual::geometry::Point::new(tp.x, mid_y),
                        lievisual::geometry::Point::new(tp.x, to_top),
                    ]
                };

                elements.push(vir::polyline_node(points, stroke.clone(), Z_AXIS));

                // Arrow head at target
                let sz = 7.0;
                elements.push(vir::line_node(
                    lievisual::geometry::Point::new(tp.x, to_top),
                    lievisual::geometry::Point::new(tp.x - sz * 0.4, to_top - sz),
                    stroke.clone(),
                    Z_AXIS,
                ));
                elements.push(vir::line_node(
                    lievisual::geometry::Point::new(tp.x, to_top),
                    lievisual::geometry::Point::new(tp.x + sz * 0.4, to_top - sz),
                    stroke,
                    Z_AXIS,
                ));

                // Transition label centered at the horizontal segment mid-point
                if let Some(label) = &t.label {
                    let ts = vir::text_style(
                        theme::state::TEXT,
                        12.0,
                        String::new(),
                        TextAlign::Center,
                        TextBaseline::Bottom,
                    );
                    let layout =
                        layout_text(&[RichSpan::new(label.to_string(), ts.clone())], Some(200.0));
                    let (x_off, y_off) =
                        compute_text_offset(&layout, TextAlign::Center, TextBaseline::Bottom);
                    let label_cx = (fp.x + tp.x) / 2.0;
                    elements.push(vir::text_node(
                        label.clone(),
                        lievisual::geometry::Point::new(label_cx + x_off, mid_y - 4.0 + y_off),
                        ts.clone()
                            .with_align(TextAlign::Left)
                            .with_baseline(TextBaseline::Top),
                        0.0,
                        Some(200.0),
                        Z_LABEL,
                    ));
                }
            }
        }
    }

    // ---- Draw state nodes ----
    for id in &node_ids {
        let pos = &positions[id];
        let nl = &node_layouts[id];
        let node = &state_map[id];

        match node {
            StateNode::Start => {
                // 官方 mermaid: 实心黑圆 (filled circle, #333)
                let r = 10.0;
                elements.push(vir::circle_node(
                    *pos,
                    r,
                    vir::fs_fill(Color::new(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0)),
                    Z_SERIES,
                ));
            }
            StateNode::End => {
                // 官方 mermaid: 双圆环 (double circle, 同心圆描边)
                let r_outer = 12.0;
                let r_inner = 8.0;
                let stroke_color = theme::state::END_STROKE;
                elements.push(vir::circle_node(
                    *pos,
                    r_outer,
                    vir::fs_stroke(stroke_color, 2.0),
                    Z_SERIES,
                ));
                elements.push(vir::circle_node(
                    *pos,
                    r_inner,
                    vir::fs_stroke(stroke_color, 2.0),
                    Z_SERIES,
                ));
            }
            StateNode::Fork => {
                // 官方 mermaid fork: 粗横线（同步条），无三角箭头
                let w = nl.width / 2.0;
                let y = pos.y;
                elements.push(vir::line_node(
                    Point::new(pos.x - w, y),
                    Point::new(pos.x + w, y),
                    vir::Stroke::new(theme::state::STROKE, 6.0),
                    Z_SERIES,
                ));
            }
            StateNode::Join => {
                // 官方 mermaid join: 粗横线（同步条），标签在下方
                let w = nl.width / 2.0;
                let y = pos.y;
                elements.push(vir::line_node(
                    Point::new(pos.x - w, y),
                    Point::new(pos.x + w, y),
                    vir::Stroke::new(theme::state::STROKE, 6.0),
                    Z_SERIES,
                ));
                if !nl.label.is_empty() {
                    let ts = vir::text_style(
                        theme::state::TEXT,
                        FONT_SIZE,
                        theme::FONT_FAMILY.to_string(),
                        TextAlign::Center,
                        TextBaseline::Top,
                    );
                    let layout = layout_text(
                        &[RichSpan::new(nl.label.to_string(), ts.clone())],
                        None,
                    );
                    let (x_off, y_off) =
                        compute_text_offset(&layout, TextAlign::Center, TextBaseline::Top);
                    elements.push(vir::text_node(
                        nl.label.clone(),
                        Point::new(pos.x + x_off, y + 6.0 + y_off),
                        ts.with_align(TextAlign::Left).with_baseline(TextBaseline::Top),
                        0.0,
                        None,
                        Z_LABEL,
                    ));
                }
            }
            StateNode::Composite => {
                // 复合状态：外层容器矩形 + 标题 + 递归放置内部子图。
                let w = nl.width / 2.0;
                let h = nl.height / 2.0;
                let left = pos.x - w;
                let top = pos.y - h;
                elements.push(vir::rect_node(
                    Rect::from_points(
                        Point::new(left, top),
                        Point::new(pos.x + w, pos.y + h),
                    ),
                    None,
                    vir::fs_both(theme::state::FILL, theme::state::STROKE, 1.0),
                    Z_SERIES,
                ));
                // 标题
                if !nl.label.is_empty() {
                    let ts = vir::text_style(
                        theme::state::TEXT,
                        FONT_SIZE,
                        theme::FONT_FAMILY.to_string(),
                        TextAlign::Center,
                        TextBaseline::Middle,
                    );
                    let layout = layout_text(
                        &[RichSpan::new(nl.label.to_string(), ts.clone())],
                        None,
                    );
                    let (x_off, y_off) =
                        compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle);
                    elements.push(vir::text_node(
                        nl.label.clone(),
                        Point::new(pos.x + x_off, top + COMPOSITE_TITLE_H / 2.0 + y_off),
                        ts.with_align(TextAlign::Left).with_baseline(TextBaseline::Top),
                        0.0,
                        None,
                        Z_LABEL,
                    ));
                }
                // 递归渲染内部子图，并平移到容器内
                if let Some(inner) = composite_inner.get(id) {
                    let (inner_elems, _iw, _ih) = render_state_diagram(inner, &config);
                    let dx = left + COMPOSITE_PAD;
                    let dy = top + COMPOSITE_TITLE_H + COMPOSITE_PAD;
                    elements.push(vir::group_node(
                        inner_elems,
                        Some(Transform::translate(dx, dy)),
                        Z_INNER,
                    ));
                }
            }
            StateNode::Normal { description, .. } => {
                let w = nl.width / 2.0;
                let h = nl.height / 2.0;
                let r = h.min(16.0);

                let rect = Rect::new(pos.x - w, pos.y - h, pos.x + w, pos.y + h);
                elements.push(vir::rect_node(
                    rect,
                    Some(r),
                    vir::fs_both(theme::state::FILL, theme::state::STROKE, 2.0),
                    Z_SERIES,
                ));

                // State label
                let ts = vir::text_style(
                    theme::state::TEXT,
                    FONT_SIZE,
                    theme::FONT_FAMILY.to_string(),
                    TextAlign::Center,
                    TextBaseline::Middle,
                );
                let layout = layout_text(
                    &[RichSpan::new(nl.label.to_string(), ts.clone())],
                    Some(nl.width - 10.0),
                );
                let (x_off, y_off) = if description.is_some() {
                    // Shift up a bit if there's a description
                    (0.0, -8.0)
                } else {
                    compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle)
                };
                elements.push(vir::text_node(
                    nl.label.clone(),
                    Point::new(pos.x + x_off, pos.y + y_off),
                    ts.clone()
                        .with_align(TextAlign::Left)
                        .with_baseline(TextBaseline::Top),
                    0.0,
                    Some(nl.width - 10.0),
                    Z_LABEL,
                ));

                // Description text below label
                if let Some(desc) = description {
                    let dts = vir::text_style(
                        theme::state::TEXT,
                        SMALL_FONT,
                        theme::FONT_FAMILY.to_string(),
                        TextAlign::Center,
                        TextBaseline::Top,
                    );
                    let dl = layout_text(
                        &[RichSpan::new(desc.to_string(), dts.clone())],
                        Some(nl.width - 10.0),
                    );
                    let (dx_off, dy_off) =
                        compute_text_offset(&dl, TextAlign::Center, TextBaseline::Top);
                    elements.push(vir::text_node(
                        desc.clone(),
                        Point::new(pos.x + dx_off, pos.y + 4.0 + dy_off),
                        dts.clone()
                            .with_align(TextAlign::Left)
                            .with_baseline(TextBaseline::Top),
                        0.0,
                        Some(nl.width - 10.0),
                        Z_LABEL,
                    ));
                }
            }
        }
    }

    // 计算包围盒并把坐标归一化到 (0,0) 起，便于复合状态内部递归放置。
    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (id, pos) in &positions {
        if let Some(nl) = node_layouts.get(id) {
            min_x = min_x.min(pos.x - nl.width / 2.0);
            max_x = max_x.max(pos.x + nl.width / 2.0);
            min_y = min_y.min(pos.y - nl.height / 2.0);
            max_y = max_y.max(pos.y + nl.height / 2.0);
        }
    }
    if !min_x.is_finite() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 0.0;
        max_y = 0.0;
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    let norm = if min_x != 0.0 || min_y != 0.0 {
        vec![vir::group_node(elements, Some(Transform::translate(-min_x, -min_y)), 0)]
    } else {
        elements
    };
    (norm, width, height)
}

/// Simple topological sort using Kahn's algorithm
fn topological_sort(
    in_degree: &HashMap<String, usize>,
    out_edges: &HashMap<String, Vec<(String, Option<String>)>>,
) -> Vec<String> {
    use std::collections::VecDeque;

    let mut degree = in_degree.clone();
    let mut queue: VecDeque<String> = VecDeque::new();

    for (node, deg) in &degree {
        if *deg == 0 {
            queue.push_back(node.clone());
        }
    }

    let mut result = Vec::new();
    while let Some(node) = queue.pop_front() {
        result.push(node.clone());
        if let Some(edges) = out_edges.get(&node) {
            for (to, _) in edges {
                if let Some(d) = degree.get_mut(to) {
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(to.clone());
                    }
                }
            }
        }
    }

    // Append any remaining nodes (from cycles or disconnected)
    for node in in_degree.keys() {
        if !result.contains(node) {
            result.push(node.clone());
        }
    }

    result
}
