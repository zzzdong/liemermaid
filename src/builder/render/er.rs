use std::collections::{HashMap, HashSet};

use lievisual::geometry::{Point, Rect};

use crate::{
    ast::{Cardinality, ErDiagram, ErRelationship},
    builder::types::OutputConfig,
    error::DiagramResult,
    vir::{
        self, Color, Element, SceneNode, Stroke, TextAlign, TextBaseline, TextStyle, Z_AXIS,
        Z_LABEL, Z_SERIES, theme,
    },
};
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

const FONT_SIZE: f64 = theme::FONT_SIZE;
const SMALL_FONT: f64 = 12.0;
const ENTITY_MIN_W: f64 = 160.0;
const ENTITY_PAD: f64 = 14.0;
const ENTITY_MARGIN_X: f64 = 110.0;
const ATTR_LINE_H: f64 = 18.0;

/// Render an ER diagram to scene nodes.
///
/// This is part of the **Grid family** (class + er) and is invoked by
/// `GridRenderer`. The layout/geometry (BFS entity grouping + relationship
/// routing) is domain-specific, so it lives here rather than in the generic
/// grid solver.
pub fn render_er(diagram: &ErDiagram, _config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
    Ok(build_er_elements(diagram, _config))
}

pub fn build_er_elements(diagram: &ErDiagram, _config: &OutputConfig) -> Vec<SceneNode> {
    let mut elements: Vec<SceneNode> = Vec::new();

    // 实体可能仅由关系隐式定义（如 `A ||--o{ B : r`，无显式 `A {...}` 块）。
    // 始终从关系两端补齐未在 `entities` 中出现过的实体（属性为空），保证渲染出实体框。
    let mut entities = diagram.entities.clone();
    let mut existing: std::collections::HashSet<String> =
        entities.iter().map(|e| e.name.clone()).collect();
    for rel in &diagram.relationships {
        for name in [&rel.first_entity, &rel.second_entity] {
            if !existing.contains(name) {
                entities.push(crate::ast::ErEntity {
                    name: name.clone(),
                    attributes: Vec::new(),
                });
                existing.insert(name.clone());
            }
        }
    }

    if entities.is_empty() {
        return elements;
    }

    // ---- Measure each entity ----
    struct EntityLayout {
        name: String,
        width: f64,
        height: f64,
        attrs: Vec<String>,
    }

    let mut layouts: HashMap<String, EntityLayout> = HashMap::new();
    for ent in &diagram.entities {
        let name_style = TextStyle::new(
            theme::TEXT_COLOR,
            FONT_SIZE,
            theme::FONT_FAMILY.to_string(),
        )
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);
        let n_layout = layout_text(&[RichSpan::new(ent.name.clone(), name_style.clone())], None);
        let name_w = n_layout.width + ENTITY_PAD * 2.0;

        let mut attr_lines = Vec::new();
        for attr in &ent.attributes {
            let line = format!("{} {}", attr.type_, attr.name);
            attr_lines.push(line);
        }

        let mut max_w = name_w;
        for line in &attr_lines {
            let l = layout_text(
                &[RichSpan::new(
                    line.to_string(),
                    TextStyle::new(
                        theme::TEXT_COLOR,
                        SMALL_FONT,
                        theme::FONT_FAMILY.to_string(),
                    ),
                )],
                None,
            );
            max_w = max_w.max(l.width + ENTITY_PAD * 2.0);
        }

        let header_h = n_layout.height + 10.0;
        let attr_h = attr_lines.len() as f64 * ATTR_LINE_H;
        let height = header_h + attr_h;

        layouts.insert(
            ent.name.clone(),
            EntityLayout {
                name: ent.name.clone(),
                width: max_w.max(ENTITY_MIN_W),
                height,
                attrs: attr_lines,
            },
        );
    }

    // ---- Group entities by connected components ----
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for ent in &entities {
        adj.entry(ent.name.clone()).or_default();
    }
    for rel in &diagram.relationships {
        adj.entry(rel.first_entity.clone())
            .or_default()
            .push(rel.second_entity.clone());
        adj.entry(rel.second_entity.clone())
            .or_default()
            .push(rel.first_entity.clone());
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut groups: Vec<Vec<String>> = Vec::new();
    for ent in &entities {
        if visited.contains(&ent.name) {
            continue;
        }
        let mut group = Vec::new();
        let mut stack = vec![ent.name.clone()];
        while let Some(n) = stack.pop() {
            if visited.contains(&n) {
                continue;
            }
            visited.insert(n.clone());
            group.push(n.clone());
            if let Some(ns) = adj.get(&n) {
                for m in ns {
                    if !visited.contains(m) {
                        stack.push(m.clone());
                    }
                }
            }
        }
        group.sort();
        groups.push(group);
    }

    // ---- Compute positions per group (horizontal line, centered) ----
    let start_x = 40.0;
    let start_y = 40.0;
    let mut positions: HashMap<String, Point> = HashMap::new();
    let mut entity_rects: HashMap<String, Rect> = HashMap::new();

    let mut cur_x = start_x;
    for (gi, group) in groups.iter().enumerate() {
        // 只考虑已在 layouts 中完成测量的实体，跳过名称不匹配（异常输入）的节点
        let group: Vec<&String> = group.iter().filter(|n| layouts.contains_key(*n)).collect();
        let group_w: f64 = group
            .iter()
            .map(|n| layouts[*n].width)
            .sum::<f64>()
            + (group.len().saturating_sub(1)) as f64 * ENTITY_MARGIN_X;
        let group_left = cur_x;
        let group_center_x = group_left + group_w / 2.0;
        let mut cx = group_left;
        for name in group {
            let layout = &layouts[name];
            let cy = start_y;
            let x = cx;
            positions.insert(name.clone(), Point::new(x + layout.width / 2.0, cy + layout.height / 2.0));
            entity_rects.insert(
                name.clone(),
                Rect::new(x, cy, x + layout.width, cy + layout.height),
            );
            cx += layout.width + ENTITY_MARGIN_X;
        }
        cur_x = group_left + group_w + 150.0; // gap between groups
        if gi < groups.len() - 1 {
            cur_x = group_center_x + group_w / 2.0 + 150.0;
        }
    }

    // ---- Draw relationships ----
    for rel in &diagram.relationships {
        let r1 = match entity_rects.get(&rel.first_entity) {
            Some(r) => r,
            None => continue, // 实体未参与布局（异常输入），跳过该关系
        };
        let r2 = match entity_rects.get(&rel.second_entity) {
            Some(r) => r,
            None => continue,
        };

        // Connection points (left/right middle)
        let p1 = Point::new(r1.max_x(), r1.min_y() + r1.height() / 2.0);
        let p2 = Point::new(r2.min_x(), r2.min_y() + r2.height() / 2.0);

        let stroke = vir::stroke(theme::TEXT_COLOR, 1.5);

        elements.push(vir::line_node(p1, p2, stroke.clone(), Z_AXIS));

        // Cardinality markers at both ends
        draw_cardinality(&mut elements, &p1, &rel.cardinality_first, EndSide::Right);
        draw_cardinality(&mut elements, &p2, &rel.cardinality_second, EndSide::Left);

        // Relationship label (middle)
        if let Some(lbl) = &rel.label {
            if !lbl.is_empty() {
                let mid = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0 - 8.0);
                let tl = layout_text(
                    &[RichSpan::new(
                        lbl.clone(),
                        TextStyle::new(
                            theme::TEXT_COLOR,
                            SMALL_FONT,
                            theme::FONT_FAMILY.to_string(),
                        )
                        .with_align(TextAlign::Center)
                        .with_baseline(TextBaseline::Middle),
                    )],
                    None,
                );
                let (ox, oy) = compute_text_offset(&tl, TextAlign::Center, TextBaseline::Middle);
                elements.push(vir::text_node(
                    lbl.clone(),
                    Point::new(mid.x + ox, mid.y + oy),
                    vir::text_style(
                        theme::TEXT_COLOR,
                        SMALL_FONT,
                        theme::FONT_FAMILY,
                        TextAlign::Left,
                        TextBaseline::Top,
                    ),
                    0.0,
                    None,
                    Z_LABEL,
                ));
            }
        }
    }

    // ---- Draw entity boxes ----
    for ent in &entities {
        let layout = match layouts.get(&ent.name) {
            Some(l) => l,
            None => continue,
        };
        let rect = match entity_rects.get(&ent.name) {
            Some(r) => *r,
            None => continue,
        };

        // Header
        let header_rect = Rect::new(rect.min_x(), rect.min_y(), rect.max_x(), rect.min_y() + 30.0);
        elements.push(vir::rect_node(
            header_rect,
            None,
            vir::fs_both(theme::class::HEADER_FILL, theme::TEXT_COLOR, 1.5),
            Z_SERIES,
        ));
        // Body
        let body_rect = Rect::new(
            rect.min_x(),
            rect.min_y() + 30.0,
            rect.max_x(),
            rect.max_y(),
        );
        elements.push(vir::rect_node(
            body_rect,
            None,
            vir::fs_both(Color::rgb(255, 255, 255), theme::TEXT_COLOR, 1.5),
            Z_SERIES,
        ));

        // Name
        let ts = TextStyle::new(
            theme::TEXT_COLOR,
            FONT_SIZE,
            theme::FONT_FAMILY.to_string(),
        )
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);
        let nl = layout_text(
            &[RichSpan::new(ent.name.clone(), ts.clone())],
            Some(layout.width - ENTITY_PAD),
        );
        let (x_off, y_off) = compute_text_offset(&nl, TextAlign::Center, TextBaseline::Middle);
        elements.push(vir::text_node(
            ent.name.clone(),
            Point::new(rect.min_x() + layout.width / 2.0 + x_off, rect.min_y() + 15.0 + y_off),
            vir::text_style(
                theme::TEXT_COLOR,
                FONT_SIZE,
                theme::FONT_FAMILY,
                TextAlign::Left,
                TextBaseline::Top,
            ),
            0.0,
            Some(layout.width - ENTITY_PAD),
            Z_LABEL,
        ));

        // Attributes
        let mut line_y = rect.min_y() + 30.0 + 6.0;
        for attr in &layout.attrs {
            let ts = TextStyle::new(
                theme::TEXT_COLOR,
                SMALL_FONT,
                theme::FONT_FAMILY.to_string(),
            )
            .with_align(TextAlign::Left)
            .with_baseline(TextBaseline::Top);
            let l = layout_text(
                &[RichSpan::new(attr.to_string(), ts.clone())],
                Some(layout.width - ENTITY_PAD),
            );
            let (x_off, y_off) = compute_text_offset(&l, TextAlign::Left, TextBaseline::Top);
            elements.push(vir::text_node(
                attr.to_string(),
                Point::new(rect.min_x() + ENTITY_PAD + x_off, line_y + y_off),
                vir::text_style(
                    theme::TEXT_COLOR,
                    SMALL_FONT,
                    theme::FONT_FAMILY,
                    TextAlign::Left,
                    TextBaseline::Top,
                ),
                0.0,
                Some(layout.width - ENTITY_PAD),
                Z_LABEL,
            ));
            line_y += ATTR_LINE_H;
        }
    }

    elements
}

#[derive(Clone, Copy)]
enum EndSide {
    Left,
    Right,
}

fn draw_cardinality(elements: &mut Vec<SceneNode>, p: &Point, card: &Cardinality, side: EndSide) {
    let text = format_cardinality(card);
    if text.is_empty() {
        return;
    }
    let ts = TextStyle::new(
        theme::TEXT_COLOR,
        SMALL_FONT,
        theme::FONT_FAMILY.to_string(),
    )
    .with_align(TextAlign::Center)
    .with_baseline(TextBaseline::Middle);
    let tl = layout_text(&[RichSpan::new(text.clone(), ts.clone())], None);
    let (ox, oy) = compute_text_offset(&tl, TextAlign::Center, TextBaseline::Middle);

    let dx = match side {
        EndSide::Left => -24.0,
        EndSide::Right => 24.0,
    };
    elements.push(vir::text_node(
        text,
        Point::new(p.x + dx + ox, p.y + oy),
        vir::text_style(
            theme::TEXT_COLOR,
            SMALL_FONT,
            theme::FONT_FAMILY,
            TextAlign::Left,
            TextBaseline::Top,
        ),
        0.0,
        None,
        Z_LABEL,
    ));
}

fn format_cardinality(card: &Cardinality) -> String {
    match card {
        Cardinality::ExactlyOne => "1".to_string(),
        Cardinality::ZeroOrOne => "0..1".to_string(),
        Cardinality::OneOrMany => "1..N".to_string(),
        Cardinality::ZeroOrMany => "0..N".to_string(),
    }
}
