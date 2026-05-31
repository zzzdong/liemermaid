use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::graph::UnGraph;
use petgraph::visit::EdgeRef;
use vello_cpu::kurbo::{Point, Rect};

use crate::{
    ast::{Cardinality, ErDiagram},
    builder::{
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

const FONT_SIZE: f64 = theme::FONT_SIZE;
const SMALL_FONT: f64 = 11.0;
const ENTITY_MIN_W: f64 = 120.0;
const ENTITY_PAD: f64 = 14.0;
const ENTITY_GAP_X: f64 = 100.0;

pub struct ErEngine<'a> {
    diagram: &'a ErDiagram,
}

impl<'a> ErEngine<'a> {
    pub fn new(diagram: &'a ErDiagram) -> Self {
        Self { diagram }
    }
}

impl<'a> LayoutEngine for ErEngine<'a> {
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<VisualElement>> {
        Ok(build_er_elements(self.diagram, config))
    }
}

pub fn build_er_elements(
    diagram: &ErDiagram,
    _config: &OutputConfig,
) -> Vec<VisualElement> {
    let mut elements = Vec::new();

    if diagram.entities.is_empty() && diagram.relationships.is_empty() {
        return elements;
    }

    // ---- Measure each entity ----
    struct EntityLayout {
        name: String,
        width: f64,
        height: f64,
        attr_lines: Vec<String>,
    }

    let mut entity_layouts: HashMap<String, EntityLayout> = HashMap::new();
    for ent in &diagram.entities {
        let ts = TextStyle { font_size: FONT_SIZE, font_family: theme::FONT_FAMILY.to_string(), align: TextAlign::Center, vertical_align: TextBaseline::Middle, ..Default::default() };
        let name_layout = create_text_layout(&ent.name, &ts, None);
        let name_w = name_layout.width() as f64 + ENTITY_PAD * 2.0;

        let mut attr_lines = Vec::new();
        for attr in &ent.attributes {
            let line = format!("{} : {}", attr.type_, attr.name);
            attr_lines.push(line);
        }

        let mut max_w = name_w;
        for line in &attr_lines {
            let small_ts = TextStyle { font_size: SMALL_FONT, align: TextAlign::Left, vertical_align: TextBaseline::Top, ..Default::default() };
            let l = create_text_layout(line, &small_ts, None);
            max_w = max_w.max(l.width() as f64 + ENTITY_PAD * 2.0);
        }

        let header_h = name_layout.height() as f64 + 12.0;
        let attr_h = if attr_lines.is_empty() { 0.0 } else { attr_lines.len() as f64 * 18.0 + 8.0 };
        let height = header_h + attr_h;

        entity_layouts.insert(ent.name.clone(), EntityLayout {
            name: ent.name.clone(),
            width: max_w.max(ENTITY_MIN_W),
            height,
            attr_lines,
        });
    }

    // ---- Discover all entities involved in relationships ----
    let mut all_entity_names: Vec<String> = diagram.entities.iter().map(|e| e.name.clone()).collect();
    for rel in &diagram.relationships {
        if !all_entity_names.contains(&rel.first_entity) {
            all_entity_names.push(rel.first_entity.clone());
        }
        if !all_entity_names.contains(&rel.second_entity) {
            all_entity_names.push(rel.second_entity.clone());
        }
    }

    // Ensure all entities have a layout (create minimal for auto-discovered)
    let ts = TextStyle { font_size: FONT_SIZE, font_family: theme::FONT_FAMILY.to_string(), ..Default::default() };
    for name in &all_entity_names {
        if !entity_layouts.contains_key(name) {
            let name_layout = create_text_layout(name, &ts, None);
            let h = name_layout.height() as f64 + 12.0;
            entity_layouts.insert(name.clone(), EntityLayout {
                name: name.clone(),
                width: name_layout.width() as f64 + ENTITY_PAD * 2.0 + ENTITY_MIN_W,
                height: h,
                attr_lines: vec![],
            });
        }
    }

    // ---- Build petgraph undirected graph from relationships ----
    let mut graph = UnGraph::<String, ()>::new_undirected();
    let mut node_indices: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();

    for name in &all_entity_names {
        let idx = graph.add_node(name.clone());
        node_indices.insert(name.clone(), idx);
    }
    for rel in &diagram.relationships {
        if let (Some(&a), Some(&b)) = (node_indices.get(&rel.first_entity), node_indices.get(&rel.second_entity)) {
            if !graph.contains_edge(a, b) {
                graph.add_edge(a, b, ());
            }
        }
    }

    // ---- Find connected components via BFS ----
    let mut visited: HashSet<String> = HashSet::new();
    let mut components: Vec<Vec<String>> = Vec::new();

    for name in &all_entity_names {
        if visited.contains(name) {
            continue;
        }
        // BFS from this entity
        let mut component: Vec<String> = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(name.clone());
        visited.insert(name.clone());

        while let Some(n) = queue.pop_front() {
            component.push(n.clone());
            if let Some(&idx) = node_indices.get(&n) {
                for edge in graph.edges(idx) {
                    let neighbor = if edge.target() == idx { edge.source() } else { edge.target() };
                    let nbr_name = &graph[neighbor];
                    if !visited.contains(nbr_name) {
                        visited.insert(nbr_name.clone());
                        queue.push_back(nbr_name.clone());
                    }
                }
            }
        }
        components.push(component);
    }

    // ---- Layout each component horizontally, stack vertically ----
    let start_x = 40.0;
    let start_y = 40.0;

    let mut entity_centers: HashMap<String, Point> = HashMap::new();
    let mut entity_rects: HashMap<String, Rect> = HashMap::new();

    let mut cur_y = start_y;

    for component in &components {
        let mut cur_x = start_x;

        // Use BFS order from the first entity in this component
        let first = component[0].clone();
        let mut bfs_order: Vec<String> = Vec::new();
        let mut bfs_visited: HashSet<String> = HashSet::new();
        let mut bfs_queue = VecDeque::new();
        bfs_queue.push_back(first);
        bfs_visited.insert(component[0].clone());

        while let Some(n) = bfs_queue.pop_front() {
            bfs_order.push(n.clone());
            if let Some(&idx) = node_indices.get(&n) {
                for edge in graph.edges(idx) {
                    let neighbor = if edge.target() == idx { edge.source() } else { edge.target() };
                    let nbr_name = &graph[neighbor];
                    if !bfs_visited.contains(nbr_name) {
                        bfs_visited.insert(nbr_name.clone());
                        bfs_queue.push_back(nbr_name.clone());
                    }
                }
            }
        }

        // Position entities in BFS order
        for name in &bfs_order {
            let layout = &entity_layouts[name];
            let cx = cur_x + layout.width / 2.0;
            let cy = cur_y + layout.height / 2.0;
            entity_centers.insert(name.clone(), Point::new(cx, cy));
            entity_rects.insert(name.clone(), Rect::new(
                cur_x, cur_y, cur_x + layout.width, cur_y + layout.height,
            ));
            cur_x += layout.width + ENTITY_GAP_X;
        }

        cur_y += component.iter().map(|name| entity_layouts[name].height).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0) + ENTITY_GAP_X;
    }

    // ---- Draw relationships ----
    for rel in &diagram.relationships {
        let fp = entity_centers.get(&rel.first_entity);
        let tp = entity_centers.get(&rel.second_entity);
        if let (Some(fc), Some(tc)) = (fp, tp) {
            let fr = entity_rects[&rel.first_entity];
            let tr = entity_rects[&rel.second_entity];

            let is_left = fc.x < tc.x;
            let start = Point::new(if is_left { fr.x1 } else { fr.x0 }, fc.y);
            let end = Point::new(if is_left { tr.x0 } else { tr.x1 }, tc.y);

            let stroke = StrokeStyle { color: theme::er::EDGE, width: 1.5 };

            // Main line
            elements.push(VisualElement::Line { start, end, style: stroke.clone(), z_index: Z_AXIS });

            // Cardinality markers
            let sz = 8.0;
            draw_cardinality(&mut elements, &start, &rel.cardinality_first, is_left, &stroke, sz);
            draw_cardinality(&mut elements, &end, &rel.cardinality_second, !is_left, &stroke, sz);

            // Label
            if let Some(label) = &rel.label {
                let mid = Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0 - 10.0);
                let ts = TextStyle { font_size: SMALL_FONT, align: TextAlign::Center, vertical_align: TextBaseline::Bottom, color: theme::er::TEXT, ..Default::default() };
                let l = create_text_layout(label, &ts, Some(200.0));
                let (x_off, y_off) = compute_text_offset(&l, TextAlign::Center, TextBaseline::Bottom);
                elements.push(VisualElement::TextRun {
                    text: label.clone(),
                    position: Point::new(mid.x + x_off, mid.y + y_off),
                    style: TextStyle { align: TextAlign::Left, vertical_align: TextBaseline::Top, ..ts },
                    rotation: 0.0,
                    max_width: Some(200.0),
                    layout: Some(l),
                    z_index: Z_LABEL,
                });
            }
        }
    }

    // ---- Draw entity boxes ----
    for name in &all_entity_names {
        let layout = &entity_layouts[name];
        let rect = entity_rects[name];

        // Full box background
        elements.push(VisualElement::Rect {
            rect,
            radius: Some(theme::NODE_RADIUS),
            style: FillStrokeStyle::new()
                .with_fill(theme::er::FILL)
                .with_stroke(theme::er::STROKE, 2.0),
            z_index: Z_SERIES,
        });

        // Header background
        let header_h = layout.height - if layout.attr_lines.is_empty() { 0.0 } else { layout.attr_lines.len() as f64 * 18.0 + 8.0 };
        let header_rect = Rect::new(rect.x0, rect.y0, rect.x1, rect.y0 + header_h);
        elements.push(VisualElement::Rect {
            rect: header_rect,
            radius: Some(theme::NODE_RADIUS),
            style: FillStrokeStyle::new()
                .with_fill(theme::er::HEADER_FILL)
                .with_stroke(theme::er::STROKE, 2.0),
            z_index: Z_SERIES,
        });

        // Entity name
        let ts = TextStyle { font_size: FONT_SIZE, font_family: theme::FONT_FAMILY.to_string(), align: TextAlign::Center, vertical_align: TextBaseline::Middle, color: theme::er::TEXT, ..Default::default() };
        let name_layout = create_text_layout(&layout.name, &ts, Some(layout.width - 8.0));
        let (x_off, y_off) = compute_text_offset(&name_layout, TextAlign::Center, TextBaseline::Middle);
        elements.push(VisualElement::TextRun {
            text: layout.name.clone(),
            position: Point::new(rect.x0 + layout.width / 2.0 + x_off, rect.y0 + header_h / 2.0 + y_off),
            style: TextStyle { align: TextAlign::Left, vertical_align: TextBaseline::Top, ..ts },
            rotation: 0.0,
            max_width: Some(layout.width - 8.0),
            layout: Some(name_layout),
            z_index: Z_LABEL,
        });

        // Separator
        elements.push(VisualElement::Line {
            start: Point::new(rect.x0, rect.y0 + header_h),
            end: Point::new(rect.x1, rect.y0 + header_h),
            style: StrokeStyle { color: theme::er::STROKE, width: 1.5 },
            z_index: Z_AXIS,
        });

        // Attributes
        let mut line_y = rect.y0 + header_h + 4.0;
        for attr_line in &layout.attr_lines {
            let ts = TextStyle { font_size: SMALL_FONT, font_family: theme::FONT_FAMILY.to_string(), align: TextAlign::Left, vertical_align: TextBaseline::Top, color: theme::er::TEXT, ..Default::default() };
            let l = create_text_layout(attr_line, &ts, Some(layout.width - ENTITY_PAD));
            let (x_off, y_off) = compute_text_offset(&l, TextAlign::Left, TextBaseline::Top);
            elements.push(VisualElement::TextRun {
                text: attr_line.clone(),
                position: Point::new(rect.x0 + ENTITY_PAD + x_off, line_y + y_off),
                style: TextStyle { align: TextAlign::Left, vertical_align: TextBaseline::Top, ..ts },
                rotation: 0.0,
                max_width: Some(layout.width - ENTITY_PAD),
                layout: Some(l),
                z_index: Z_LABEL,
            });
            line_y += 18.0;
        }
    }

    elements
}

fn draw_cardinality(elements: &mut Vec<VisualElement>, pos: &Point, card: &Cardinality, on_right: bool, _style: &StrokeStyle, sz: f64) {
    let dir: f64 = if on_right { 1.0 } else { -1.0 };
    let stroke = StrokeStyle { color: theme::er::EDGE, width: 1.5 };

    match card {
        Cardinality::ZeroOrOne => {
            let line_end = Point::new(pos.x + dir * sz * 0.3, pos.y);
            elements.push(VisualElement::Line { start: *pos, end: line_end, style: stroke, z_index: Z_AXIS });
            let circle_cx = pos.x + dir * sz * 0.8;
            elements.push(VisualElement::Circle {
                center: Point::new(circle_cx, pos.y),
                radius: sz * 0.35,
                style: FillStrokeStyle::new().with_fill(Color::new(255, 255, 255)).with_stroke(theme::er::EDGE, 1.5),
                z_index: Z_AXIS,
            });
        }
        Cardinality::ExactlyOne => {
            elements.push(VisualElement::Line { start: *pos, end: Point::new(pos.x + dir * sz * 0.3, pos.y), style: stroke, z_index: Z_AXIS });
            elements.push(VisualElement::Line {
                start: Point::new(pos.x + dir * sz * 0.3, pos.y - sz * 0.4),
                end: Point::new(pos.x + dir * sz * 0.3, pos.y + sz * 0.4),
                style: StrokeStyle { color: theme::er::EDGE, width: 2.0 },
                z_index: Z_AXIS,
            });
            elements.push(VisualElement::Line {
                start: Point::new(pos.x + dir * sz * 0.6, pos.y - sz * 0.4),
                end: Point::new(pos.x + dir * sz * 0.6, pos.y + sz * 0.4),
                style: StrokeStyle { color: theme::er::EDGE, width: 2.0 },
                z_index: Z_AXIS,
            });
        }
        Cardinality::ZeroOrMany => {
            let circle_cx = pos.x + dir * sz * 0.35;
            elements.push(VisualElement::Circle {
                center: Point::new(circle_cx, pos.y),
                radius: sz * 0.35,
                style: FillStrokeStyle::new().with_fill(Color::new(255, 255, 255)).with_stroke(theme::er::EDGE, 1.5),
                z_index: Z_AXIS,
            });
            let fork_x = pos.x + dir * sz;
            elements.push(VisualElement::Line {
                start: Point::new(circle_cx + dir * sz * 0.35, pos.y),
                end: Point::new(fork_x, pos.y),
                style: stroke,
                z_index: Z_AXIS,
            });
            for i in -1..=1 {
                elements.push(VisualElement::Line {
                    start: Point::new(fork_x, pos.y),
                    end: Point::new(fork_x + dir * sz * 0.2, pos.y + i as f64 * sz * 0.4),
                    style: StrokeStyle { color: theme::er::EDGE, width: 1.5 },
                    z_index: Z_AXIS,
                });
            }
        }
        Cardinality::OneOrMany => {
            let x1 = pos.x + dir * sz * 0.15;
            elements.push(VisualElement::Line {
                start: Point::new(x1, pos.y - sz * 0.4),
                end: Point::new(x1, pos.y + sz * 0.4),
                style: StrokeStyle { color: theme::er::EDGE, width: 2.0 },
                z_index: Z_AXIS,
            });
            let x2 = pos.x + dir * sz * 0.45;
            elements.push(VisualElement::Line {
                start: Point::new(x2, pos.y - sz * 0.4),
                end: Point::new(x2, pos.y + sz * 0.4),
                style: StrokeStyle { color: theme::er::EDGE, width: 2.0 },
                z_index: Z_AXIS,
            });
            let fork_x = pos.x + dir * sz;
            elements.push(VisualElement::Line {
                start: Point::new(x2, pos.y),
                end: Point::new(fork_x, pos.y),
                style: stroke,
                z_index: Z_AXIS,
            });
            for i in -1..=1 {
                elements.push(VisualElement::Line {
                    start: Point::new(fork_x, pos.y),
                    end: Point::new(fork_x + dir * sz * 0.2, pos.y + i as f64 * sz * 0.4),
                    style: StrokeStyle { color: theme::er::EDGE, width: 1.5 },
                    z_index: Z_AXIS,
                });
            }
        }
    }
}