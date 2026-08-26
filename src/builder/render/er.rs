//! ER 渲染器：消费 `GridSolver` 产出的 `PlacedGraph` 几何，绘制实体框与关系线。
//!
//! 职责边界：坐标（节点中心、边路由）全部来自 `placed`，本模块只负责——
//! - 用 `placed.positions[i]` + 实体尺寸画实体框（header + body + 属性文本）
//! - 用 `placed.edge_routes[i]` 画关系线 + 端点基数 + 中点标签
//! 尺寸仍需从 AST 测量（`layout_text`），但位置与路由只读 `placed`。

use std::collections::HashSet;

use lievisual::geometry::{BezPath, Point, Rect};
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

use crate::{
    ast::{Cardinality, ErDiagram},
    builder::layout::ir::PlacedGraph,
    builder::types::OutputConfig,
    error::DiagramResult,
    vir::{self, Color, SceneNode, Stroke, TextAlign, TextBaseline, TextStyle, Z_AXIS, Z_LABEL, Z_SERIES, theme},
};

const FONT_SIZE: f64 = theme::FONT_SIZE;
const SMALL_FONT: f64 = 12.0;
const ENTITY_MIN_W: f64 = 160.0;
const ENTITY_PAD: f64 = 14.0;
const ATTR_LINE_H: f64 = 18.0;

/// Render an ER diagram to scene nodes, consuming `PlacedGraph` geometry.
pub fn render_er(
    placed: &PlacedGraph,
    diagram: &ErDiagram,
    _config: &OutputConfig,
) -> DiagramResult<Vec<SceneNode>> {
    Ok(build_er_elements(placed, diagram))
}

/// 与 `convert::ToLayoutGraph for ErDiagram` 保持一致的有序实体列表
/// （已声明实体在前 + 关系隐含实体按出现顺序追加）。
fn entity_order(diagram: &ErDiagram) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for e in &diagram.entities {
        if !seen.contains(&e.name) {
            order.push(e.name.clone());
            seen.insert(e.name.clone());
        }
    }
    for r in &diagram.relationships {
        for name in [&r.first_entity, &r.second_entity] {
            if !seen.contains(name) {
                order.push(name.clone());
                seen.insert(name.clone());
            }
        }
    }
    order
}

pub fn build_er_elements(placed: &PlacedGraph, diagram: &ErDiagram) -> Vec<SceneNode> {
    let mut elements: Vec<SceneNode> = Vec::new();

    let entities = entity_order(diagram);
    if entities.is_empty() {
        return elements;
    }

    // ---- 测量每个实体框尺寸 ----
    struct EntityLayout {
        width: f64,
        height: f64,
        attrs: Vec<String>,
    }
    let mut layouts: Vec<EntityLayout> = Vec::with_capacity(entities.len());
    for name in &entities {
        let ent = diagram
            .entities
            .iter()
            .find(|e| &e.name == name);
        let attrs: Vec<String> = match ent {
            Some(e) => e.attributes.iter().map(|a| format!("{} {}", a.type_, a.name)).collect(),
            None => Vec::new(),
        };
        let name_style = TextStyle::new(
            theme::TEXT_COLOR,
            FONT_SIZE,
            theme::FONT_FAMILY.to_string(),
        )
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);
        let n_layout = layout_text(&[RichSpan::new(name.clone(), name_style.clone())], None);
        let name_w = n_layout.width + ENTITY_PAD * 2.0;

        let mut max_w = name_w;
        for line in &attrs {
            let l = layout_text(
                &[RichSpan::new(
                    line.clone(),
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
        // 即使无属性，body 也保留最小占位高度（与官方一致，header 与 body 始终区分）。
        const MIN_BODY_H: f64 = 20.0;
        let attr_h = (attrs.len() as f64 * ATTR_LINE_H).max(MIN_BODY_H);
        layouts.push(EntityLayout {
            width: max_w.max(ENTITY_MIN_W),
            height: header_h + attr_h,
            attrs,
        });
    }

    // ---- 画关系线（几何来自 placed.edge_routes） ----
    for (ri, rel) in diagram.relationships.iter().enumerate() {
        let Some(route) = placed.edge_routes.get(ri) else { continue };
        if route.len() < 2 {
            continue;
        }
        let p1 = route[0];
        let p2 = *route.last().unwrap();
        let stroke = vir::stroke(theme::TEXT_COLOR, 1.5);
        // 沿边方向与垂直方向单位向量（用于基数符号朝向）
        let first_dir = Point::new(route[1].x - p1.x, route[1].y - p1.y);
        let last_dir = Point::new(p2.x - route[route.len() - 2].x, p2.y - route[route.len() - 2].y);
        let fl = (first_dir.x * first_dir.x + first_dir.y * first_dir.y).sqrt().max(1e-9);
        let ll = (last_dir.x * last_dir.x + last_dir.y * last_dir.y).sqrt().max(1e-9);
        let first_ud = Point::new(first_dir.x / fl, first_dir.y / fl);
        let last_ud = Point::new(last_dir.x / ll, last_dir.y / ll);
        let first_perp = Point::new(-first_ud.y, first_ud.x);
        let last_perp = Point::new(-last_ud.y, last_ud.x);
        // 中间段（若有绕行点）
        for w in route.windows(2) {
            elements.push(vir::line_node(w[0], w[1], stroke.clone(), Z_AXIS));
        }
        // 基数符号在端点处沿「远离节点」方向延伸
        let first_away = first_ud;  // first端：从 source 指向 target，离开 source
        let second_away = Point::new(-last_ud.x, -last_ud.y);  // second端：从 target 指向 source，离开 target
        draw_cardinality(&mut elements, &p1, &rel.cardinality_first, &first_away, &first_perp, &stroke);
        draw_cardinality(&mut elements, &p2, &rel.cardinality_second, &second_away, &last_perp, &stroke);

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

    // ---- 画实体框（位置来自 placed.positions[i]） ----
    for (i, name) in entities.iter().enumerate() {
        let layout = &layouts[i];
        let Some(&center) = placed.positions.get(i) else { continue };
        let rect = Rect::new(
            center.x - layout.width / 2.0,
            center.y - layout.height / 2.0,
            center.x + layout.width / 2.0,
            center.y + layout.height / 2.0,
        );

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
            &[RichSpan::new(name.clone(), ts.clone())],
            Some(layout.width - ENTITY_PAD),
        );
        let (x_off, y_off) = compute_text_offset(&nl, TextAlign::Center, TextBaseline::Middle);
        elements.push(vir::text_node(
            name.clone(),
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

/// 在关系端点处绘制基数符号（与官方 Mermaid 一致的图形表示）。
///
/// - `ExactlyOne` → `||`（两条平行短线）
/// - `ZeroOrOne`  → `|o`（短线 + 空心圆）
/// - `OneOrMany`  → `}|`（大括号 + 短线）
/// - `ZeroOrMany` → `}o`（大括号 + 空心圆）
///
/// `away` = 从端点远离节点方向的单位向量；符号沿 `away` 方向从端点向外排列。
/// `perp` = 垂直于连接线的单位向量；短线沿 `perp` 方向绘制，大括号顶/底也在 `perp` 方向。
fn draw_cardinality(
    elements: &mut Vec<SceneNode>,
    p: &Point,
    card: &Cardinality,
    away: &Point,
    perp: &Point,
    stroke: &Stroke,
) {
    use Cardinality::*;
    const SHORT_HALF: f64 = 4.0; // 短线沿 perp 方向的半长
    const STEP: f64 = 6.0;       // 沿 away 方向相邻符号的间距
    let c = theme::TEXT_COLOR;

    // 沿 perp 方向、中心 `center` 的短线（与连接线垂直）。
    let short_at = |elements: &mut Vec<SceneNode>, center: &Point| {
        let s = Point::new(
            center.x - perp.x * SHORT_HALF,
            center.y - perp.y * SHORT_HALF,
        );
        let e = Point::new(
            center.x + perp.x * SHORT_HALF,
            center.y + perp.y * SHORT_HALF,
        );
        elements.push(vir::line_node(s, e, stroke.clone(), Z_AXIS));
    };

    // 中心 `center`、半径 3 的空心圆。
    let circle_at = |elements: &mut Vec<SceneNode>, center: &Point| {
        elements.push(vir::circle_node(
            *center,
            3.0,
            vir::fs_both(Color::rgb(255, 255, 255), c, 1.5),
            Z_AXIS,
        ));
    };

    // 沿 away 方向偏移 idx*STEP 得到第 idx 个符号的位置。
    let at = |idx: f64| {
        Point::new(p.x + away.x * idx * STEP, p.y + away.y * idx * STEP)
    };

    match card {
        ExactlyOne => {
            // `||` 两条平行短线，沿 away 方向前后排列（与连接线同向延伸）。
            let c1 = at(0.0);
            let c2 = at(1.0);
            short_at(elements, &c1);
            short_at(elements, &c2);
        }
        ZeroOrOne => {
            // `|o` 短线在前（靠近节点），圆在后（远离节点）
            short_at(elements, &at(0.0));
            circle_at(elements, &at(1.0));
        }
        OneOrMany => {
            // `}|` 短线 (one) 靠近节点，大括号 (many) 远离节点
            short_at(elements, &at(0.0));
            draw_brace(elements, &at(1.0), away, perp, stroke);
        }
        ZeroOrMany => {
            // `}o` 圆 (zero) 靠近节点，大括号 (many) 远离节点
            circle_at(elements, &at(0.0));
            draw_brace(elements, &at(1.0), away, perp, stroke);
        }
    }
}

/// 画一个 `}` 形大括号：顶/底沿 perp 方向分置两侧，中点沿 away 方向凸出。
/// 形成 `{` 形（开口朝向 away 反方向，即端点一侧）。
fn draw_brace(
    elements: &mut Vec<SceneNode>,
    center: &Point,
    away: &Point,
    perp: &Point,
    stroke: &Stroke,
) {
    const H: f64 = 4.0; // 沿 perp 方向的半宽
    const W: f64 = 3.5; // 沿 away 方向的凸出
    let top = Point::new(
        center.x + perp.x * -H,
        center.y + perp.y * -H,
    );
    let mid = Point::new(center.x + away.x * W, center.y + away.y * W);
    let bot = Point::new(
        center.x + perp.x * H,
        center.y + perp.y * H,
    );
    let mut path = BezPath::new();
    path.move_to(Point::new(top.x, top.y));
    path.line_to(Point::new(mid.x, mid.y));
    path.line_to(Point::new(bot.x, bot.y));
    elements.push(vir::path_node(
        path,
        vir::fs_both(Color::rgb(255, 255, 255), stroke.color, stroke.width),
        Z_AXIS,
    ));
}
