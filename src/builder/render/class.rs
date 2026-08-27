//! Class 渲染器：消费 `GridSolver` 产出的 `PlacedGraph` 几何，绘制类框与关系线。
//!
//! 职责边界：坐标（节点中心、边路由）全部来自 `placed`，本模块只负责——
//! - 用 `placed.positions[i]` + 类框尺寸画类框（header / attrs / methods / 注解）
//! - 用 `placed.edge_routes[i]` 画关系线 + 箭头 + 标签 + 基数
//! 尺寸从 AST 测量（`layout_text`），位置与路由只读 `placed`。

use lievisual::geometry::{BezPath, Point, Rect};
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

use crate::{
    ast::{ClassDiagram, RelationKind, Visibility},
    builder::layout::ir::PlacedGraph,
    builder::types::OutputConfig,
    error::DiagramResult,
    vir::{
        self, Color, SceneNode, Stroke, TextAlign, TextBaseline, TextStyle, Z_AXIS, Z_LABEL,
        Z_SERIES, theme,
    },
};

const FONT_SIZE: f64 = theme::FONT_SIZE;
const SMALL_FONT: f64 = 11.0;
const CLASS_MIN_W: f64 = 140.0;
const CLASS_PAD: f64 = 12.0;

/// Render a class diagram to scene nodes, consuming `PlacedGraph` geometry.
pub fn render_class(
    placed: &PlacedGraph,
    diagram: &ClassDiagram,
    _config: &OutputConfig,
) -> DiagramResult<Vec<SceneNode>> {
    Ok(build_class_elements(placed, diagram))
}

pub fn build_class_elements(placed: &PlacedGraph, diagram: &ClassDiagram) -> Vec<SceneNode> {
    let mut elements = Vec::new();

    if diagram.classes.is_empty() {
        return elements;
    }

    // ---- 测量每个类框（顺序 = diagram.classes 源码序，与 convert 一致）----
    struct ClassLayout {
        name: String,
        width: f64,
        height: f64,
        header_h: f64,
        attrs: Vec<String>,
        methods: Vec<String>,
    }

    let mut layouts: Vec<ClassLayout> = Vec::with_capacity(diagram.classes.len());
    for cls in &diagram.classes {
        let display_name = match &cls.generic {
            Some(g) => format!("{}<{}>", cls.name, g),
            None => cls.name.clone(),
        };
        let ts = TextStyle::new(
            theme::class::TEXT,
            FONT_SIZE,
            theme::FONT_FAMILY.to_string(),
        )
        .with_align(TextAlign::Left)
        .with_baseline(TextBaseline::Top);
        let name_layout = layout_text(&[RichSpan::new(display_name.clone(), ts.clone())], None);
        let name_w = name_layout.width + CLASS_PAD * 2.0;

        let mut attr_lines = Vec::new();
        let mut method_lines = Vec::new();
        for m in &cls.members {
            let prefix = match m.visibility {
                Some(Visibility::Public) => "+",
                Some(Visibility::Private) => "-",
                Some(Visibility::Protected) => "#",
                Some(Visibility::Package) => "~",
                None => "",
            };
            let line = if m.is_method {
                if let Some(ret) = &m.type_ {
                    format!("{}{}() : {}", prefix, m.name, ret)
                } else {
                    format!("{}{}()", prefix, m.name)
                }
            } else if let Some(t) = &m.type_ {
                format!("{}{} {}", prefix, t, m.name)
            } else {
                format!("{}{}", prefix, m.name)
            };
            if m.is_method {
                method_lines.push(line);
            } else {
                attr_lines.push(line);
            }
        }

        let mut max_w = name_w;
        for line in attr_lines.iter().chain(method_lines.iter()) {
            let l = layout_text(
                &[RichSpan::new(
                    line.to_string(),
                    TextStyle::new(
                        theme::class::TEXT,
                        SMALL_FONT,
                        theme::FONT_FAMILY.to_string(),
                    ),
                )],
                None,
            );
            max_w = max_w.max(l.width + CLASS_PAD * 2.0);
        }

        let header_h = name_layout.height + 12.0;
        // 三栏：名称栏（header）+ 属性栏 + 方法栏。按官方行为恒定三栏，
        // 即使某栏为空也保留占位高度与分隔线。
        // 空栏占位高度与"一行内容"一致（行高 18 + 上下留白 8），保证三栏高度均匀。
        const SECTION_MIN_H: f64 = 18.0 + 8.0;
        let attr_h = if attr_lines.is_empty() {
            SECTION_MIN_H
        } else {
            attr_lines.len() as f64 * 18.0 + 8.0
        };
        let method_h = if method_lines.is_empty() {
            SECTION_MIN_H
        } else {
            method_lines.len() as f64 * 18.0 + 8.0
        };
        let height = header_h + attr_h + method_h;

        layouts.push(ClassLayout {
            name: display_name.clone(),
            width: max_w.max(CLASS_MIN_W),
            height,
            header_h,
            attrs: attr_lines,
            methods: method_lines,
        });
    }

    // ---- 画关系线（几何来自 placed.edge_routes）----
    for (ri, rel) in diagram.relations.iter().enumerate() {
        let Some(route) = placed.edge_routes.get(ri) else { continue };
        if route.len() < 2 {
            continue;
        }
        let start = route[0];
        let end = *route.last().unwrap();
        let stroke = vir::stroke(theme::class::EDGE, 1.5);

        // 路由中点之间画线段（若只有一个直段）
        for w in route.windows(2) {
            let is_dashed = rel.kind == RelationKind::Dependency;
            if is_dashed {
                draw_dashed_line(&mut elements, &w[0], &w[1], &stroke);
            } else {
                elements.push(vir::line_node(w[0], w[1], stroke.clone(), Z_AXIS));
            }
        }

        // 箭头方向：起点端取首段方向，终点端取末段方向
        let last_dir = Point::new(
            route[route.len() - 1].x - route[route.len() - 2].x,
            route[route.len() - 1].y - route[route.len() - 2].y,
        );
        let last_len = (last_dir.x * last_dir.x + last_dir.y * last_dir.y).sqrt();
        let last_ud = if last_len > 1e-9 {
            Point::new(last_dir.x / last_len, last_dir.y / last_len)
        } else {
            Point::new(0.0, 0.0)
        };
        let first_dir = Point::new(route[1].x - route[0].x, route[1].y - route[0].y);
        let first_len = (first_dir.x * first_dir.x + first_dir.y * first_dir.y).sqrt();
        let first_ud = if first_len > 1e-9 {
            Point::new(first_dir.x / first_len, first_dir.y / first_len)
        } else {
            Point::new(0.0, 0.0)
        };

        match rel.kind {
            // 官方：`A <|-- B` 空心三角在 A（source）端，表示 B 继承 A，指向父类。
            // 三角尖端朝向 source 内部（沿 first_ud 反向）。
            RelationKind::Inheritance => {
                let src_dir = Point::new(-first_ud.x, -first_ud.y);
                draw_triangle_head(&mut elements, &start, &src_dir, false, &stroke);
            }
            // 官方：`A *-- B` 组合 / `A o-- B` 聚合的菱形在 A（source）端。
            RelationKind::Composition => {
                draw_diamond_head(&mut elements, &start, &first_ud, true, &stroke);
            }
            RelationKind::Aggregation => {
                draw_diamond_head(&mut elements, &start, &first_ud, false, &stroke);
            }
            // 官方：`A --> B` 关联 / `A ..> B` 依赖的箭头在 B（target）端。
            RelationKind::Association | RelationKind::Dependency => {
                draw_triangle_head(&mut elements, &end, &last_ud, true, &stroke);
            }
        }

        // 关系标签（边中点上方）
        if let Some(lbl) = &rel.label
            && !lbl.is_empty() {
                let mid = Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0 - 6.0);
                let tl = layout_text(
                    &[RichSpan::new(
                        lbl.clone(),
                        TextStyle::new(
                            theme::class::EDGE,
                            SMALL_FONT,
                            theme::FONT_FAMILY.to_string(),
                        )
                        .with_align(TextAlign::Center)
                        .with_baseline(TextBaseline::Middle),
                    )],
                    None,
                );
                let (ox, oy) =
                    compute_text_offset(&tl, TextAlign::Center, TextBaseline::Middle);
                elements.push(vir::text_node(
                    lbl.clone(),
                    Point::new(mid.x + ox, mid.y + oy),
                    vir::text_style(
                        theme::class::EDGE,
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
        // 基数（端点附近）
        if let Some(cf) = &rel.cardinality_first {
            let p = Point::new(start.x + 3.0, start.y - 2.0);
            let tl = layout_text(
                &[RichSpan::new(
                    cf.clone(),
                    TextStyle::new(
                        theme::class::EDGE,
                        SMALL_FONT,
                        theme::FONT_FAMILY.to_string(),
                    )
                    .with_align(TextAlign::Left)
                    .with_baseline(TextBaseline::Middle),
                )],
                None,
            );
            let (ox, oy) = compute_text_offset(&tl, TextAlign::Left, TextBaseline::Middle);
            elements.push(vir::text_node(
                cf.clone(),
                Point::new(p.x + ox, p.y + oy),
                vir::text_style(
                    theme::class::EDGE,
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
        if let Some(cs) = &rel.cardinality_second {
            let p = Point::new(end.x - 3.0, end.y - 2.0);
            let tl = layout_text(
                &[RichSpan::new(
                    cs.clone(),
                    TextStyle::new(
                        theme::class::EDGE,
                        SMALL_FONT,
                        theme::FONT_FAMILY.to_string(),
                    )
                    .with_align(TextAlign::Right)
                    .with_baseline(TextBaseline::Middle),
                )],
                None,
            );
            let (ox, oy) = compute_text_offset(&tl, TextAlign::Right, TextBaseline::Middle);
            elements.push(vir::text_node(
                cs.clone(),
                Point::new(p.x + ox, p.y + oy),
                vir::text_style(
                    theme::class::EDGE,
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

    // ---- 画类框（位置来自 placed.positions[i]）----
    for (i, cls) in diagram.classes.iter().enumerate() {
        let layout = &layouts[i];
        let Some(&center) = placed.positions.get(i) else { continue };
        let rect = Rect::new(
            center.x - layout.width / 2.0,
            center.y - layout.height / 2.0,
            center.x + layout.width / 2.0,
            center.y + layout.height / 2.0,
        );
        let ann = cls.annotation.clone();

        // Background
        elements.push(vir::rect_node(
            rect,
            None,
            vir::fs_both(theme::class::FILL, theme::class::STROKE, 2.0),
            Z_SERIES,
        ));

        // Header background
        let header_rect = Rect::new(
            rect.min_x(),
            rect.min_y(),
            rect.max_x(),
            rect.min_y() + layout.header_h,
        );
        elements.push(vir::rect_node(
            header_rect,
            None,
            vir::fs_both(theme::class::HEADER_FILL, theme::class::STROKE, 2.0),
            Z_SERIES,
        ));

        // 注解（如 «Interface»）显示在 header 顶部
        if let Some(ann) = &ann {
            let ann_text = format!("«{}»", ann);
            let ts = TextStyle::new(
                theme::class::TEXT,
                SMALL_FONT,
                theme::FONT_FAMILY.to_string(),
            )
            .with_align(TextAlign::Center)
            .with_baseline(TextBaseline::Middle);
            let al = layout_text(
                &[RichSpan::new(ann_text.clone(), ts)],
                Some(layout.width - 8.0),
            );
            let (aox, aoy) = compute_text_offset(&al, TextAlign::Center, TextBaseline::Middle);
            elements.push(vir::text_node(
                ann_text,
                Point::new(
                    rect.min_x() + layout.width / 2.0 + aox,
                    rect.min_y() + 9.0 + aoy,
                ),
                vir::text_style(
                    theme::class::TEXT,
                    SMALL_FONT,
                    theme::FONT_FAMILY,
                    TextAlign::Left,
                    TextBaseline::Top,
                ),
                0.0,
                Some(layout.width - 8.0),
                Z_LABEL,
            ));
        }

        // Class name text
        let name_y = if ann.is_some() {
            rect.min_y() + layout.header_h / 2.0 + 7.0
        } else {
            rect.min_y() + layout.header_h / 2.0
        };
        let ts = TextStyle::new(
            theme::class::TEXT,
            FONT_SIZE,
            theme::FONT_FAMILY.to_string(),
        )
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);
        let name_layout = layout_text(
            &[RichSpan::new(layout.name.to_string(), ts.clone())],
            Some(layout.width - 8.0),
        );
        let (x_off, y_off) =
            compute_text_offset(&name_layout, TextAlign::Center, TextBaseline::Middle);
        elements.push(vir::text_node(
            layout.name.clone(),
            Point::new(rect.min_x() + layout.width / 2.0 + x_off, name_y + y_off),
            vir::text_style(
                theme::class::TEXT,
                FONT_SIZE,
                theme::FONT_FAMILY,
                TextAlign::Left,
                TextBaseline::Top,
            ),
            0.0,
            Some(layout.width - 8.0),
            Z_LABEL,
        ));

        // Separator under header（名称栏与内容栏的分界，始终画）
        elements.push(vir::line_node(
            Point::new(rect.min_x(), rect.min_y() + layout.header_h),
            Point::new(rect.max_x(), rect.min_y() + layout.header_h),
            vir::stroke(theme::class::STROKE, 1.5),
            Z_AXIS,
        ));

        // 三栏恒定显示：属性栏、方法栏即使为空也保留占位与分隔线。
        // 空栏占位高度与"一行内容"一致（行高 18 + 上下留白 8），保证三栏高度均匀。
        const SECTION_MIN_H: f64 = 18.0 + 8.0;
        let mut line_y = rect.min_y() + layout.header_h;

        // 属性栏
        line_y += 4.0;
        for attr in &layout.attrs {
            let ts = TextStyle::new(
                theme::class::TEXT,
                SMALL_FONT,
                theme::FONT_FAMILY.to_string(),
            )
            .with_align(TextAlign::Left)
            .with_baseline(TextBaseline::Top);
            let l = layout_text(
                &[RichSpan::new(attr.to_string(), ts.clone())],
                Some(layout.width - CLASS_PAD),
            );
            let (x_off, y_off) = compute_text_offset(&l, TextAlign::Left, TextBaseline::Top);
            elements.push(vir::text_node(
                attr.to_string(),
                Point::new(rect.min_x() + CLASS_PAD + x_off, line_y + y_off),
                vir::text_style(
                    theme::class::TEXT,
                    SMALL_FONT,
                    theme::FONT_FAMILY,
                    TextAlign::Left,
                    TextBaseline::Top,
                ),
                0.0,
                Some(layout.width - CLASS_PAD),
                Z_LABEL,
            ));
            line_y += 18.0;
        }
        if layout.attrs.is_empty() {
            line_y += SECTION_MIN_H;
        }

        // 属性栏与方法栏的分隔线（恒画）
        elements.push(vir::line_node(
            Point::new(rect.min_x() + 4.0, line_y),
            Point::new(rect.max_x() - 4.0, line_y),
            vir::stroke(theme::class::SEPARATOR, 1.0),
            Z_AXIS,
        ));

        // 方法栏
        line_y += 4.0;
        for method in &layout.methods {
            let ts = TextStyle::new(
                theme::class::TEXT,
                SMALL_FONT,
                theme::FONT_FAMILY.to_string(),
            )
            .with_align(TextAlign::Left)
            .with_baseline(TextBaseline::Top);
            let l = layout_text(
                &[RichSpan::new(method.to_string(), ts.clone())],
                Some(layout.width - CLASS_PAD),
            );
            let (x_off, y_off) = compute_text_offset(&l, TextAlign::Left, TextBaseline::Top);
            elements.push(vir::text_node(
                method.to_string(),
                Point::new(rect.min_x() + CLASS_PAD + x_off, line_y + y_off),
                vir::text_style(
                    theme::class::TEXT,
                    SMALL_FONT,
                    theme::FONT_FAMILY,
                    TextAlign::Left,
                    TextBaseline::Top,
                ),
                0.0,
                Some(layout.width - CLASS_PAD),
                Z_LABEL,
            ));
            line_y += 18.0;
        }
        if layout.methods.is_empty() {
            line_y += SECTION_MIN_H;
        }
    }

    elements
}

fn draw_triangle_head(
    elements: &mut Vec<SceneNode>,
    tip: &Point,
    dir: &Point,
    filled: bool,
    style: &Stroke,
) {
    let sz = 10.0;
    let perp_x = -dir.y;
    let perp_y = dir.x;
    let base = Point::new(tip.x - dir.x * sz, tip.y - dir.y * sz);
    let p1 = Point::new(base.x + perp_x * sz * 0.5, base.y + perp_y * sz * 0.5);
    let p2 = Point::new(base.x - perp_x * sz * 0.5, base.y - perp_y * sz * 0.5);

    let mut path = BezPath::new();
    path.move_to(Point::new(tip.x, tip.y));
    path.line_to(Point::new(p1.x, p1.y));
    path.line_to(Point::new(p2.x, p2.y));
    path.close_path();

    let fill = if filled {
        Some(theme::class::EDGE)
    } else {
        Some(Color::rgb(255, 255, 255))
    };
    elements.push(vir::path_node(
        path,
        vir::fs_both(fill.unwrap(), style.color, style.width),
        Z_AXIS,
    ));
}

fn draw_diamond_head(
    elements: &mut Vec<SceneNode>,
    center: &Point,
    dir: &Point,
    filled: bool,
    style: &Stroke,
) {
    let sz = 8.0;
    let perp_x = -dir.y;
    let perp_y = dir.x;
    let front = Point::new(center.x + dir.x * sz, center.y + dir.y * sz);
    let back = Point::new(center.x - dir.x * sz, center.y - dir.y * sz);
    let p1 = Point::new(center.x + perp_x * sz * 0.6, center.y + perp_y * sz * 0.6);
    let p2 = Point::new(center.x - perp_x * sz * 0.6, center.y - perp_y * sz * 0.6);

    let mut path = BezPath::new();
    path.move_to(Point::new(front.x, front.y));
    path.line_to(Point::new(p1.x, p1.y));
    path.line_to(Point::new(back.x, back.y));
    path.line_to(Point::new(p2.x, p2.y));
    path.close_path();

    let fill = if filled {
        Some(theme::class::EDGE)
    } else {
        Some(Color::rgb(255, 255, 255))
    };
    elements.push(vir::path_node(
        path,
        vir::fs_both(fill.unwrap(), style.color, style.width),
        Z_AXIS,
    ));
}

fn draw_dashed_line(elements: &mut Vec<SceneNode>, start: &Point, end: &Point, style: &Stroke) {
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
        elements.push(vir::line_node(s, e, style.clone(), Z_AXIS));
        cur = seg_end + gap;
    }
}
