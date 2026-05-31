use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use vello_cpu::kurbo::{Point, Rect};

use crate::{
    ast::StateDiagram,
    builder::{
        layout::sugiyama::{NodeSize, SugiyamaConfig, SugiyamaLayout},
        layout::types::LayoutEngine,
        types::OutputConfig,
    },
    error::DiagramResult,
    text::{compute_text_offset, create_text_layout},
    visual::{
        theme,
        Color, FillStrokeStyle, StrokeStyle, TextAlign, TextBaseline, TextStyle,
        VisualElement, Z_AXIS, Z_LABEL, Z_SERIES,
    },
};

const STATE_PAD_X: f64 = 18.0;
const STATE_PAD_Y: f64 = 10.0;
const FONT_SIZE: f64 = theme::FONT_SIZE;
const SMALL_FONT: f64 = 11.0;

/// State diagram node kinds for layout
#[derive(Debug, Clone)]
enum StateNode {
    Start,
    End,
    Normal { id: String, description: Option<String> },
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
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<VisualElement>> {
        Ok(build_state_elements(self.diagram, config))
    }
}

pub fn build_state_elements(
    diagram: &StateDiagram,
    _config: &OutputConfig,
) -> Vec<VisualElement> {
    let mut elements = Vec::new();

    if diagram.transitions.is_empty() && diagram.states.is_empty() {
        return elements;
    }

    // ---- Collect all state nodes ----
    let mut state_map: HashMap<String, StateNode> = HashMap::new();
    let mut out_edges: HashMap<String, Vec<(String, Option<String>)>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    // Internal keys to distinguish start [*] from end [*]
    const START_KEY: &str = "__start__";
    const END_KEY: &str = "__end__";

    // Process transitions to discover all states
    for t in &diagram.transitions {
        let from_key = if t.from == "[*]" { START_KEY.to_string() } else { t.from.clone() };
        let to_key = if t.to == "[*]" { END_KEY.to_string() } else { t.to.clone() };

        // Register states
        if t.from == "[*]" {
            state_map.entry(START_KEY.to_string()).or_insert(StateNode::Start);
        } else {
            state_map
                .entry(t.from.clone())
                .or_insert_with(|| StateNode::Normal {
                    id: t.from.clone(),
                    description: None,
                });
        }

        if t.to == "[*]" {
            state_map.entry(END_KEY.to_string()).or_insert(StateNode::End);
        } else {
            state_map
                .entry(t.to.clone())
                .or_insert_with(|| StateNode::Normal {
                    id: t.to.clone(),
                    description: None,
                });
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
            StateNode::Normal { id, description } => (id.clone(), description.clone()),
        };

        // Measure text
        let ts = TextStyle {
            font_size: FONT_SIZE,
            align: TextAlign::Center,
            vertical_align: TextBaseline::Middle,
            ..Default::default()
        };
        let layout = create_text_layout(&label, &ts, None);
        let text_w = layout.width() as f64;
        let text_h = layout.height() as f64;

        let desc_text_h = if let Some(d) = &desc {
            let dl = create_text_layout(d, &TextStyle { font_size: SMALL_FONT, ..ts.clone() }, None);
            dl.height() as f64 + 4.0
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
            sugiyama_node_sizes.insert(*idx, NodeSize { width: nl.width, height: nl.height });
        }
    }

    // Run Sugiyama 4-phase layout
    let config = SugiyamaConfig {
        crossing_iterations: 10,
        ..Default::default()
    };
    let sugiyama = SugiyamaLayout::new(config, &graph);
    let result = sugiyama.layout(&sugiyama_node_sizes);

    // Convert petgraph NodeIndex positions back to string-keyed positions
    let mut positions: HashMap<String, Point> = HashMap::new();
    for (idx, pos) in &result.positions {
        let id = &graph[*idx];
        positions.insert(id.clone(), *pos);
    }

    // ---- Draw transitions with orthogonal routing ----
    for t in &diagram.transitions {
        let from_key = if t.from == "[*]" { START_KEY.to_string() } else { t.from.clone() };
        let to_key = if t.to == "[*]" { END_KEY.to_string() } else { t.to.clone() };
        let from_pos = positions.get(&from_key);
        let to_pos = positions.get(&to_key);
        if let (Some(fp), Some(tp)) = (from_pos, to_pos) {
            let from_nl = &node_layouts[&from_key];
            let to_nl = &node_layouts[&to_key];
            // Start/End 是圆形，实际边界 = 中心 + 圆半径 (r=16 + 3=19)，不是 height/2
            let from_bottom = if from_key == START_KEY {
                fp.y + 19.0
            } else {
                fp.y + from_nl.height / 2.0
            };
            let to_top = if to_key == END_KEY {
                tp.y - 19.0
            } else {
                tp.y - to_nl.height / 2.0
            };
            let mid_y = (from_bottom + to_top) / 2.0;

            let stroke = StrokeStyle { color: theme::state::EDGE, width: 1.5 };

            // Orthogonal routing:
            // - Same X: straight vertical line (2 points)
            // - Different X: vertical → horizontal → vertical (4 points)
            let points = if (fp.x - tp.x).abs() < 0.001 {
                vec![
                    Point::new(fp.x, from_bottom),
                    Point::new(fp.x, to_top),
                ]
            } else {
                vec![
                    Point::new(fp.x, from_bottom),
                    Point::new(fp.x, mid_y),
                    Point::new(tp.x, mid_y),
                    Point::new(tp.x, to_top),
                ]
            };

            elements.push(VisualElement::Polyline {
                points,
                style: stroke.clone(),
                z_index: Z_AXIS,
            });

            // Arrow head at target
            let sz = 7.0;
            elements.push(VisualElement::Line {
                start: Point::new(tp.x, to_top),
                end: Point::new(tp.x - sz * 0.4, to_top - sz),
                style: stroke.clone(),
                z_index: Z_AXIS,
            });
            elements.push(VisualElement::Line {
                start: Point::new(tp.x, to_top),
                end: Point::new(tp.x + sz * 0.4, to_top - sz),
                style: stroke,
                z_index: Z_AXIS,
            });

            // Transition label centered at the horizontal segment mid-point
            if let Some(label) = &t.label {
                let ts = TextStyle {
                    font_size: 12.0,
                    align: TextAlign::Center,
                    vertical_align: TextBaseline::Bottom,
                    color: theme::state::TEXT,
                    ..Default::default()
                };
                let layout = create_text_layout(label, &ts, Some(200.0));
                let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Bottom);
                let label_cx = (fp.x + tp.x) / 2.0;
                elements.push(VisualElement::TextRun {
                    text: label.clone(),
                    position: Point::new(label_cx + x_off, mid_y - 4.0 + y_off),
                    style: TextStyle {
                        align: TextAlign::Left,
                        vertical_align: TextBaseline::Top,
                        ..ts
                    },
                    rotation: 0.0,
                    max_width: Some(200.0),
                    layout: Some(layout),
                    z_index: Z_LABEL,
                });
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
                let r = 16.0;
                // Outer circle (white fill, dark stroke)
                elements.push(VisualElement::Circle {
                    center: *pos,
                    radius: r + 3.0,
                    style: FillStrokeStyle::new()
                        .with_fill(Color::new(255, 255, 255))
                        .with_stroke(theme::state::START_FILL, 2.0),
                    z_index: Z_SERIES,
                });
                // Inner filled circle
                elements.push(VisualElement::Circle {
                    center: *pos,
                    radius: r - 2.0,
                    style: FillStrokeStyle::new()
                        .with_fill(theme::state::START_FILL),
                    z_index: Z_SERIES,
                });
            }
            StateNode::End => {
                let r = 16.0;
                // Outer circle
                elements.push(VisualElement::Circle {
                    center: *pos,
                    radius: r + 3.0,
                    style: FillStrokeStyle::new()
                        .with_fill(Color::new(255, 255, 255))
                        .with_stroke(theme::state::END_STROKE, 2.5),
                    z_index: Z_SERIES,
                });
                // Inner ring
                elements.push(VisualElement::Circle {
                    center: *pos,
                    radius: r - 4.0,
                    style: FillStrokeStyle::new()
                        .with_fill(Color::new(255, 255, 255))
                        .with_stroke(theme::state::END_STROKE, 2.5),
                    z_index: Z_SERIES,
                });
            }
            StateNode::Normal { description, .. } => {
                let w = nl.width / 2.0;
                let h = nl.height / 2.0;
                let r = h.min(16.0);

                let rect = Rect::new(pos.x - w, pos.y - h, pos.x + w, pos.y + h);
                elements.push(VisualElement::Rect {
                    rect,
                    radius: Some(r),
                    style: FillStrokeStyle::new()
                        .with_fill(theme::state::FILL)
                        .with_stroke(theme::state::STROKE, 2.0),
                    z_index: Z_SERIES,
                });

                // State label
                let ts = TextStyle {
                    font_size: FONT_SIZE,
                    font_family: theme::FONT_FAMILY.to_string(),
                    align: TextAlign::Center,
                    vertical_align: TextBaseline::Middle,
                    color: theme::state::TEXT,
                    ..Default::default()
                };
                let layout = create_text_layout(&nl.label, &ts, Some(nl.width - 10.0));
                let (x_off, y_off) = if description.is_some() {
                    // Shift up a bit if there's a description
                    (0.0, -8.0)
                } else {
                    compute_text_offset(&layout, TextAlign::Center, TextBaseline::Middle)
                };
                elements.push(VisualElement::TextRun {
                    text: nl.label.clone(),
                    position: Point::new(pos.x + x_off, pos.y + y_off),
                    style: TextStyle {
                        align: TextAlign::Left,
                        vertical_align: TextBaseline::Top,
                        ..ts
                    },
                    rotation: 0.0,
                    max_width: Some(nl.width - 10.0),
                    layout: Some(layout),
                    z_index: Z_LABEL,
                });

                // Description text below label
                if let Some(desc) = description {
                    let dts = TextStyle {
                        font_size: SMALL_FONT,
                        font_family: theme::FONT_FAMILY.to_string(),
                        align: TextAlign::Center,
                        vertical_align: TextBaseline::Top,
                        color: theme::state::TEXT,
                        ..Default::default()
                    };
                    let dl = create_text_layout(desc, &dts, Some(nl.width - 10.0));
                    let (dx_off, dy_off) = compute_text_offset(&dl, TextAlign::Center, TextBaseline::Top);
                    elements.push(VisualElement::TextRun {
                        text: desc.clone(),
                        position: Point::new(pos.x + dx_off, pos.y + 4.0 + dy_off),
                        style: TextStyle {
                            align: TextAlign::Left,
                            vertical_align: TextBaseline::Top,
                            ..dts
                        },
                        rotation: 0.0,
                        max_width: Some(nl.width - 10.0),
                        layout: Some(dl),
                        z_index: Z_LABEL,
                    });
                }
            }
        }
    }

    elements
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
    for (node, _) in in_degree {
        if !result.contains(node) {
            result.push(node.clone());
        }
    }

    result
}
