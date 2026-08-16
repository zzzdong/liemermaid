use std::collections::{HashMap, VecDeque};

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use vello_cpu::kurbo::{BezPath, Point, Rect};

use crate::{
    ast::{ClassDiagram, RelationKind, Visibility},
    builder::{layout::types::LayoutEngine, types::OutputConfig},
    text::{compute_text_offset, create_text_layout},
    visual::{
        Color, FillStrokeStyle, StrokeStyle, TextAlign, TextBaseline, TextStyle, VisualElement,
        Z_AXIS, Z_LABEL, Z_SERIES, theme,
    },
};

use crate::error::DiagramResult;

const FONT_SIZE: f64 = theme::FONT_SIZE;
const SMALL_FONT: f64 = 11.0;
const CLASS_MIN_W: f64 = 140.0;
const CLASS_PAD: f64 = 12.0;
const CLASS_MARGIN_X: f64 = 80.0;

pub struct ClassEngine<'a> {
    diagram: &'a ClassDiagram,
}

impl<'a> ClassEngine<'a> {
    pub fn new(diagram: &'a ClassDiagram) -> Self {
        Self { diagram }
    }
}

impl<'a> LayoutEngine for ClassEngine<'a> {
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<VisualElement>> {
        Ok(build_class_elements(self.diagram, config))
    }
}

pub fn build_class_elements(diagram: &ClassDiagram, _config: &OutputConfig) -> Vec<VisualElement> {
    let mut elements = Vec::new();

    if diagram.classes.is_empty() {
        return elements;
    }

    // ---- Measure each class ----
    struct ClassLayout {
        name: String,
        width: f64,
        height: f64,
        header_h: f64,
        attrs: Vec<String>,
        methods: Vec<String>,
    }

    let mut layouts: HashMap<String, ClassLayout> = HashMap::new();
    for cls in &diagram.classes {
        let ts = TextStyle {
            font_size: FONT_SIZE,
            font_family: theme::FONT_FAMILY.to_string(),
            align: TextAlign::Left,
            vertical_align: TextBaseline::Top,
            ..Default::default()
        };
        let name_layout = create_text_layout(&cls.name, &ts, None);
        let name_w = name_layout.width() as f64 + CLASS_PAD * 2.0;

        let mut attr_lines = Vec::new();
        let mut method_lines = Vec::new();
        for m in &cls.members {
            let prefix = match m.visibility {
                Some(Visibility::Public) => "+ ",
                Some(Visibility::Private) => "- ",
                Some(Visibility::Protected) => "# ",
                Some(Visibility::Package) => "~ ",
                None => "",
            };
            let line = if m.is_method {
                format!("{}{}()", prefix, m.name)
            } else if let Some(t) = &m.type_ {
                format!("{}{}: {}", prefix, m.name, t)
            } else {
                format!("{}{}", prefix, m.name)
            };
            if m.is_method {
                method_lines.push(line);
            } else {
                attr_lines.push(line);
            }
        }

        let col_w = name_w;
        let mut max_w = col_w;
        for line in attr_lines.iter().chain(method_lines.iter()) {
            let l = create_text_layout(
                line,
                &TextStyle {
                    font_size: SMALL_FONT,
                    ..ts.clone()
                },
                None,
            );
            max_w = max_w.max(l.width() as f64 + CLASS_PAD * 2.0);
        }

        let header_h = name_layout.height() as f64 + 12.0;
        let attr_h = if attr_lines.is_empty() {
            0.0
        } else {
            attr_lines.len() as f64 * 18.0 + 8.0
        };
        let method_h = if method_lines.is_empty() {
            0.0
        } else {
            method_lines.len() as f64 * 18.0 + 8.0
        };
        let height = header_h + attr_h + method_h;

        layouts.insert(
            cls.name.clone(),
            ClassLayout {
                name: cls.name.clone(),
                width: max_w.max(CLASS_MIN_W),
                height,
                header_h,
                attrs: attr_lines,
                methods: method_lines,
            },
        );
    }

    // ---- Compute positions via petgraph layered layout ----
    let mut positions: HashMap<String, Point> = HashMap::new();
    let mut class_rects: HashMap<String, Rect> = HashMap::new();

    let class_names: Vec<String> = diagram.classes.iter().map(|c| c.name.clone()).collect();

    // Build inheritance graph using petgraph
    let mut graph = DiGraph::new();
    let mut node_indices: HashMap<String, NodeIndex> = HashMap::new();
    for name in &class_names {
        let idx = graph.add_node(name.clone());
        node_indices.insert(name.clone(), idx);
    }
    for rel in &diagram.relations {
        if let (Some(&from), Some(&to)) =
            (node_indices.get(&rel.source), node_indices.get(&rel.target))
        {
            graph.add_edge(from, to, ());
        }
    }

    // BFS from roots (nodes with no incoming edges) to assign layers
    let roots: Vec<NodeIndex> = graph
        .node_indices()
        .filter(|&idx| graph.neighbors_directed(idx, Direction::Incoming).count() == 0)
        .collect();
    let roots = if roots.is_empty() {
        vec![graph.node_indices().next().unwrap()]
    } else {
        roots
    };

    let mut layers: HashMap<NodeIndex, usize> = HashMap::new();
    let mut queue = VecDeque::new();
    for root in &roots {
        layers.insert(*root, 0);
        queue.push_back(*root);
    }
    while let Some(node) = queue.pop_front() {
        let cur_layer = layers[&node];
        for target in graph.neighbors_directed(node, Direction::Outgoing) {
            let new_layer = cur_layer + 1;
            let existing = layers.get(&target).copied().unwrap_or(usize::MAX);
            if new_layer < existing {
                layers.insert(target, new_layer);
                queue.push_back(target);
            }
        }
    }

    // Ensure all class nodes have a layer (disconnected nodes go to layer 0)
    let mut next_layer = layers.values().copied().max().unwrap_or(0) + 1;
    for name in &class_names {
        if let std::collections::hash_map::Entry::Vacant(e) = layers.entry(node_indices[name]) {
            e.insert(0);
            next_layer = next_layer.max(1);
        }
    }

    // Group nodes by layer, sort by name for deterministic layout
    let mut max_layer = 0usize;
    let mut layer_nodes: Vec<Vec<String>> = Vec::new();
    for (idx, &layer) in &layers {
        let name = graph.node_weight(*idx).unwrap().clone();
        while layer_nodes.len() <= layer {
            layer_nodes.push(Vec::new());
        }
        layer_nodes[layer].push(name);
        max_layer = max_layer.max(layer);
    }
    for nodes in &mut layer_nodes {
        nodes.sort();
    }

    // 计算每层总宽度，用于跨层居中
    let mut layer_total_width: HashMap<usize, f64> = HashMap::new();
    for (layer, nodes) in layer_nodes.iter().enumerate() {
        let total: f64 = nodes.iter().map(|n| layouts[n].width).sum::<f64>()
            + (nodes.len().saturating_sub(1)) as f64 * CLASS_MARGIN_X;
        layer_total_width.insert(layer, total);
    }
    let max_layer_width = layer_total_width
        .values()
        .cloned()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    // Compute positions per layer
    let start_x = 40.0;
    let start_y = 40.0;
    let mut current_y = start_y;

    for layer in 0..=max_layer {
        let nodes = &layer_nodes[layer];
        if nodes.is_empty() {
            continue;
        }
        let mut max_h: f64 = 0.0;
        for name in nodes {
            max_h = max_h.max(layouts[name].height);
        }
        // 居中：该层总宽度相对于最宽层的偏移
        let layer_width = layer_total_width[&layer];
        let offset = ((max_layer_width - layer_width) / 2.0).max(0.0);
        let mut cur_x = start_x + offset;
        for name in nodes {
            let layout = &layouts[name];
            let cx = cur_x + layout.width / 2.0;
            let cy = current_y + layout.height / 2.0;
            positions.insert(name.clone(), Point::new(cx, cy));
            class_rects.insert(
                name.clone(),
                Rect::new(
                    cur_x,
                    current_y,
                    cur_x + layout.width,
                    current_y + layout.height,
                ),
            );
            cur_x += layout.width + CLASS_MARGIN_X;
        }
        current_y += max_h + 30.0;
    }

    // ---- Draw relations ----
    for rel in &diagram.relations {
        let from_pos = positions.get(&rel.source);
        let to_pos = positions.get(&rel.target);
        if let (Some(fp), Some(tp)) = (from_pos, to_pos) {
            let from_r = class_rects[&rel.source];
            let to_r = class_rects[&rel.target];
            let is_from_left = from_r.x0 < to_r.x0;
            let is_from_top = from_r.y0 < to_r.y0;

            let start = if (from_r.y0 - to_r.y0).abs() < 10.0 {
                if is_from_left {
                    Point::new(from_r.x1, fp.y)
                } else {
                    Point::new(from_r.x0, fp.y)
                }
            } else if is_from_top {
                Point::new(fp.x, from_r.y1)
            } else {
                Point::new(fp.x, from_r.y0)
            };

            let end = if (from_r.y0 - to_r.y0).abs() < 10.0 {
                if is_from_left {
                    Point::new(to_r.x0, tp.y)
                } else {
                    Point::new(to_r.x1, tp.y)
                }
            } else {
                Point::new(tp.x, to_r.y0)
            };

            let stroke = StrokeStyle {
                color: theme::class::EDGE,
                width: 1.5,
            };

            // 检查同层水平边是否有中间节点需要绕行
            let is_same_row_horizontal = is_from_left && (from_r.y0 - to_r.y0).abs() < 10.0;
            let has_intermediate = if is_same_row_horizontal {
                class_rects.iter().any(|(n, r)| {
                    n.as_str() != rel.source
                        && n.as_str() != rel.target
                        && (r.y0 - from_r.y0).abs() < 10.0
                        && r.x0 > from_r.x1
                        && r.x0 < to_r.x0
                })
            } else {
                false
            };

            if has_intermediate {
                // 三段正交绕行：从源右侧向上 → 水平越过中间节点 → 向下到目标左侧
                let route_y = from_r.y0 - 14.0;
                let segs: Vec<(Point, Point)> = vec![
                    (start, Point::new(start.x, route_y)),
                    (Point::new(start.x, route_y), Point::new(end.x, route_y)),
                    (Point::new(end.x, route_y), end),
                ];

                let is_dashed = rel.kind == RelationKind::Dependency;
                for (seg_start, seg_end) in &segs {
                    if is_dashed {
                        draw_dashed_line(&mut elements, seg_start, seg_end, &stroke);
                    } else {
                        elements.push(VisualElement::Line {
                            start: *seg_start,
                            end: *seg_end,
                            style: stroke.clone(),
                            z_index: Z_AXIS,
                        });
                    }
                }

                // 箭头/菱形头：分别处理起点端和终点端
                let first_seg_dir =
                    Point::new(segs[0].1.x - segs[0].0.x, segs[0].1.y - segs[0].0.y);
                let first_len =
                    (first_seg_dir.x * first_seg_dir.x + first_seg_dir.y * first_seg_dir.y).sqrt();
                let first_ud = Point::new(first_seg_dir.x / first_len, first_seg_dir.y / first_len);
                let last_seg_dir = Point::new(segs[2].1.x - segs[2].0.x, segs[2].1.y - segs[2].0.y);
                let last_len =
                    (last_seg_dir.x * last_seg_dir.x + last_seg_dir.y * last_seg_dir.y).sqrt();
                let last_ud = Point::new(last_seg_dir.x / last_len, last_seg_dir.y / last_len);

                match rel.kind {
                    RelationKind::Inheritance => {
                        draw_triangle_head(&mut elements, &end, &last_ud, false, &stroke);
                    }
                    RelationKind::Composition => {
                        draw_diamond_head(&mut elements, &start, &first_ud, true, &stroke);
                    }
                    RelationKind::Aggregation => {
                        draw_diamond_head(&mut elements, &start, &first_ud, false, &stroke);
                    }
                    RelationKind::Association => {
                        draw_triangle_head(&mut elements, &end, &last_ud, true, &stroke);
                    }
                    RelationKind::Dependency => {
                        draw_triangle_head(&mut elements, &end, &last_ud, true, &stroke);
                    }
                }
            } else {
                // 直接连接
                let is_dashed = rel.kind == RelationKind::Dependency;
                if is_dashed {
                    draw_dashed_line(&mut elements, &start, &end, &stroke);
                } else {
                    elements.push(VisualElement::Line {
                        start,
                        end,
                        style: stroke.clone(),
                        z_index: Z_AXIS,
                    });
                }

                let dir = Point::new(end.x - start.x, end.y - start.y);
                let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
                let ud = Point::new(dir.x / len, dir.y / len);

                match rel.kind {
                    RelationKind::Inheritance => {
                        draw_triangle_head(&mut elements, &end, &ud, false, &stroke);
                    }
                    RelationKind::Composition => {
                        draw_diamond_head(&mut elements, &start, &ud, true, &stroke);
                    }
                    RelationKind::Aggregation => {
                        draw_diamond_head(&mut elements, &start, &ud, false, &stroke);
                    }
                    RelationKind::Association => {
                        draw_triangle_head(&mut elements, &end, &ud, true, &stroke);
                    }
                    RelationKind::Dependency => {
                        draw_triangle_head(&mut elements, &end, &ud, true, &stroke);
                    }
                }
            }
        }
    }

    // ---- Draw class boxes ----
    for name in &class_names {
        let layout = &layouts[name];
        let rect = class_rects[name];

        // Background
        elements.push(VisualElement::Rect {
            rect,
            radius: None,
            style: FillStrokeStyle::new()
                .with_fill(theme::class::FILL)
                .with_stroke(theme::class::STROKE, 2.0),
            z_index: Z_SERIES,
        });

        // Header background
        let header_rect = Rect::new(rect.x0, rect.y0, rect.x1, rect.y0 + layout.header_h);
        elements.push(VisualElement::Rect {
            rect: header_rect,
            radius: None,
            style: FillStrokeStyle::new()
                .with_fill(theme::class::HEADER_FILL)
                .with_stroke(theme::class::STROKE, 2.0),
            z_index: Z_SERIES,
        });

        // Class name text (bold via slightly bigger size)
        let ts = TextStyle {
            font_size: FONT_SIZE,
            font_family: theme::FONT_FAMILY.to_string(),
            align: TextAlign::Center,
            vertical_align: TextBaseline::Middle,
            color: theme::class::TEXT,
            ..Default::default()
        };
        let name_layout = create_text_layout(&layout.name, &ts, Some(layout.width - 8.0));
        let (x_off, y_off) =
            compute_text_offset(&name_layout, TextAlign::Center, TextBaseline::Middle);
        elements.push(VisualElement::TextRun {
            text: layout.name.clone(),
            position: Point::new(
                rect.x0 + layout.width / 2.0 + x_off,
                rect.y0 + layout.header_h / 2.0 + y_off,
            ),
            style: TextStyle {
                align: TextAlign::Left,
                vertical_align: TextBaseline::Top,
                ..ts
            },
            rotation: 0.0,
            max_width: Some(layout.width - 8.0),
            layout: Some(Box::new(name_layout)),
            z_index: Z_LABEL,
        });

        // Separator under header
        elements.push(VisualElement::Line {
            start: Point::new(rect.x0, rect.y0 + layout.header_h),
            end: Point::new(rect.x1, rect.y0 + layout.header_h),
            style: StrokeStyle {
                color: theme::class::STROKE,
                width: 1.5,
            },
            z_index: Z_AXIS,
        });

        // Attributes
        let mut line_y = rect.y0 + layout.header_h + 4.0;
        for attr in &layout.attrs {
            let ts = TextStyle {
                font_size: SMALL_FONT,
                font_family: theme::FONT_FAMILY.to_string(),
                align: TextAlign::Left,
                vertical_align: TextBaseline::Top,
                color: theme::class::TEXT,
                ..Default::default()
            };
            let l = create_text_layout(attr, &ts, Some(layout.width - CLASS_PAD));
            let (x_off, y_off) = compute_text_offset(&l, TextAlign::Left, TextBaseline::Top);
            elements.push(VisualElement::TextRun {
                text: attr.to_string(),
                position: Point::new(rect.x0 + CLASS_PAD + x_off, line_y + y_off),
                style: TextStyle {
                    align: TextAlign::Left,
                    vertical_align: TextBaseline::Top,
                    ..ts
                },
                rotation: 0.0,
                max_width: Some(layout.width - CLASS_PAD),
                layout: Some(Box::new(l)),
                z_index: Z_LABEL,
            });
            line_y += 18.0;
        }

        // Separator before methods
        if !layout.attrs.is_empty() && !layout.methods.is_empty() {
            elements.push(VisualElement::Line {
                start: Point::new(rect.x0 + 4.0, line_y),
                end: Point::new(rect.x1 - 4.0, line_y),
                style: StrokeStyle {
                    color: theme::class::SEPARATOR,
                    width: 1.0,
                },
                z_index: Z_AXIS,
            });
        }

        // Methods
        for method in &layout.methods {
            let ts = TextStyle {
                font_size: SMALL_FONT,
                font_family: theme::FONT_FAMILY.to_string(),
                align: TextAlign::Left,
                vertical_align: TextBaseline::Top,
                color: theme::class::TEXT,
                ..Default::default()
            };
            let l = create_text_layout(method, &ts, Some(layout.width - CLASS_PAD));
            let (x_off, y_off) = compute_text_offset(&l, TextAlign::Left, TextBaseline::Top);
            elements.push(VisualElement::TextRun {
                text: method.to_string(),
                position: Point::new(rect.x0 + CLASS_PAD + x_off, line_y + y_off),
                style: TextStyle {
                    align: TextAlign::Left,
                    vertical_align: TextBaseline::Top,
                    ..ts
                },
                rotation: 0.0,
                max_width: Some(layout.width - CLASS_PAD),
                layout: Some(Box::new(l)),
                z_index: Z_LABEL,
            });
            line_y += 18.0;
        }
    }

    elements
}

fn draw_triangle_head(
    elements: &mut Vec<VisualElement>,
    tip: &Point,
    dir: &Point,
    filled: bool,
    style: &StrokeStyle,
) {
    let sz = 10.0;
    let perp_x = -dir.y;
    let perp_y = dir.x;
    let base = Point::new(tip.x - dir.x * sz, tip.y - dir.y * sz);
    let p1 = Point::new(base.x + perp_x * sz * 0.5, base.y + perp_y * sz * 0.5);
    let p2 = Point::new(base.x - perp_x * sz * 0.5, base.y - perp_y * sz * 0.5);

    let mut path = BezPath::new();
    path.move_to(*tip);
    path.line_to(p1);
    path.line_to(p2);
    path.close_path();

    let fill = if filled {
        Some(theme::class::EDGE)
    } else {
        Some(Color::new(255, 255, 255))
    };
    elements.push(VisualElement::Path {
        path,
        style: FillStrokeStyle::new()
            .with_fill(fill.unwrap())
            .with_stroke(style.color, style.width),
        z_index: Z_AXIS,
    });
}

fn draw_diamond_head(
    elements: &mut Vec<VisualElement>,
    center: &Point,
    dir: &Point,
    filled: bool,
    style: &StrokeStyle,
) {
    let sz = 8.0;
    let perp_x = -dir.y;
    let perp_y = dir.x;
    let front = Point::new(center.x + dir.x * sz, center.y + dir.y * sz);
    let back = Point::new(center.x - dir.x * sz, center.y - dir.y * sz);
    let p1 = Point::new(center.x + perp_x * sz * 0.6, center.y + perp_y * sz * 0.6);
    let p2 = Point::new(center.x - perp_x * sz * 0.6, center.y - perp_y * sz * 0.6);

    let mut path = BezPath::new();
    path.move_to(front);
    path.line_to(p1);
    path.line_to(back);
    path.line_to(p2);
    path.close_path();

    let fill = if filled {
        Some(theme::class::EDGE)
    } else {
        Some(Color::new(255, 255, 255))
    };
    elements.push(VisualElement::Path {
        path,
        style: FillStrokeStyle::new()
            .with_fill(fill.unwrap())
            .with_stroke(style.color, style.width),
        z_index: Z_AXIS,
    });
}

fn draw_dashed_line(
    elements: &mut Vec<VisualElement>,
    start: &Point,
    end: &Point,
    style: &StrokeStyle,
) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return;
    }
    let udx = dx / len;
    let udy = dy / len;
    let dash = 6.0;
    let gap = 4.0;
    let mut cur = 0.0;
    while cur < len {
        let seg_end = (cur + dash).min(len);
        let s = Point::new(start.x + udx * cur, start.y + udy * cur);
        let e = Point::new(start.x + udx * seg_end, start.y + udy * seg_end);
        elements.push(VisualElement::Line {
            start: s,
            end: e,
            style: style.clone(),
            z_index: Z_AXIS,
        });
        cur = seg_end + gap;
    }
}
