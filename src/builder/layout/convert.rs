//! AST → [`LayoutGraph`] 转换层。
//!
//! 每个图表类型实现 [`ToLayoutGraph`]，把 AST 剥离渲染语义，产出纯拓扑 + 尺寸的
//! [`LayoutGraph`]。节点 / 边 / 组的顺序**严格等于源码顺序**（确定性锚定）。
//!
//! [`Measure`] 封装节点尺寸测量（复用现有 `measure` 模块），转换器不直接依赖 theme 常量。

use std::collections::HashMap;

use lievisual::geometry::Size;

use crate::ast::{
    ArrowType, ClassDiagram, Diagram, ErDiagram, Flowchart, NodeShape, SequenceDiagram, State,
    StateDiagram, TimelineDiagram,
};
use crate::builder::layout::ir::{
    GroupChild, LEdge, LGroup, LNode, LayoutGraph, LineKind, PortHint, ShapeHint,
};
use crate::builder::layout::recognize::all_flowchart_nodes;
use crate::builder::types::OutputConfig;

use super::measure::measure_nodes;
use super::types::{NodeId, NodeMetrics};

/// 节点尺寸测量器：把 AST 节点量成 [`Size`]（kurbo）。
///
/// 复用现有 `measure_nodes`，内部按 `NodeShape` / 文本测量。
pub struct Measure<'a> {
    config: &'a OutputConfig,
}

impl<'a> Measure<'a> {
    pub fn new(config: &'a OutputConfig) -> Self {
        Self { config }
    }

    /// 测量一组 AST 节点，返回 id → 尺寸。
    fn measure_all(&self, nodes: &[crate::ast::Node]) -> HashMap<NodeId, NodeMetrics> {
        measure_nodes(nodes, self.config)
    }
}

/// 从 AST 形状推导几何形状类别。
fn shape_hint_of(shape: &Option<NodeShape>) -> ShapeHint {
    match shape {
        Some(NodeShape::Diamond) => ShapeHint::Diamond,
        Some(NodeShape::Circle) | Some(NodeShape::DoubleCircle) => ShapeHint::Circle,
        Some(NodeShape::Hexagon)
        | Some(NodeShape::Rounded)
        | Some(NodeShape::Stadium)
        | Some(NodeShape::Subroutine)
        | Some(NodeShape::Cylinder)
        | Some(NodeShape::Asymmetric)
        | Some(NodeShape::Parallelogram)
        | Some(NodeShape::ParallelogramAlt)
        | Some(NodeShape::Trapezoid)
        | Some(NodeShape::TrapezoidAlt)
        | Some(NodeShape::Rectangle) => ShapeHint::Rounded,
        None => ShapeHint::Rect,
    }
}

/// 从 AST 箭头类型推导线型类别（仅几何语义，不含颜色 / 箭头样式）。
fn line_kind_of(arrow: &ArrowType) -> LineKind {
    match arrow {
        ArrowType::Dotted => LineKind::Dashed,
        ArrowType::Both | ArrowType::MultiCircle | ArrowType::MultiCross => LineKind::Bidirectional,
        ArrowType::Invisible => LineKind::Invisible,
        ArrowType::NoArrow => LineKind::NoArrow,
        ArrowType::Thick => LineKind::Thick,
        ArrowType::Solid
        | ArrowType::Circle
        | ArrowType::Cross
        | ArrowType::Labeled(_) => LineKind::Solid,
    }
}

/// 将 AST 转换为布局图。
pub trait ToLayoutGraph {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph;
}

impl ToLayoutGraph for Flowchart {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        let nodes = all_flowchart_nodes(self);
        let metrics = measure.measure_all(&nodes);

        let mut lg = LayoutGraph::default();
        let mut id_to_idx: HashMap<String, usize> = HashMap::new();

        // 1. 节点（顶层 + subgraph 内部，顺序 = 源码顺序，去重合并空壳）
        for n in &nodes {
            let idx = lg.nodes.len();
            id_to_idx.insert(n.id.clone(), idx);
            lg.nodes.push(LNode {
                id: n.id.clone(),
                size: size_of(&metrics, &n.id),
                shape_hint: shape_hint_of(&n.shape),
            });
        }

        // 2. 组树：subgraph 作为 LGroup，children 指向组内节点索引
        for sg in &self.subgraphs {
            let children = sg
                .nodes
                .iter()
                .filter_map(|n| id_to_idx.get(&n.id).copied())
                .map(GroupChild::Node)
                .collect();
            lg.groups.push(LGroup {
                title: sg.title.clone(),
                children,
            });
        }

        // 3. 边（顶层 + 组内），映射到节点索引；跨组边单独收集
        let collect_edges = |from: &str,
                             to: &str,
                             arrow: &ArrowType,
                             out: &mut Vec<LEdge>,
                             cross: &mut Vec<LEdge>| {
            let (Some(&s), Some(&t)) = (id_to_idx.get(from), id_to_idx.get(to)) else {
                return;
            };
            let edge = LEdge {
                source: s,
                target: t,
                source_port: PortHint::Auto,
                target_port: PortHint::Auto,
                line_kind: line_kind_of(arrow),
            };
            let s_in_group = group_of_node(&lg.groups, s);
            let t_in_group = group_of_node(&lg.groups, t);
            // 跨组边：两端点所属容器不同（None 视为「根容器」）。
            // 如 A(根) → X(组0) 跨根↔组边界，X(组0)→Y(组0) 是组内边。
            if s_in_group != t_in_group {
                cross.push(edge);
            } else {
                out.push(edge);
            }
        };

        for e in &self.edges {
            collect_edges(
                &e.source,
                &e.target,
                &e.arrow_type,
                &mut lg.edges,
                &mut lg.cross_group_edges,
            );
        }
        for sg in &self.subgraphs {
            for e in &sg.edges {
                collect_edges(
                    &e.source,
                    &e.target,
                    &e.arrow_type,
                    &mut lg.edges,
                    &mut lg.cross_group_edges,
                );
            }
        }

        lg
    }
}

/// 组内节点归属（返回所属组的下标，顶层为 None）。
fn group_of_node(groups: &[crate::builder::layout::ir::LGroup], node_idx: usize) -> Option<usize> {
    groups.iter().position(|g| {
        g.children
            .iter()
            .any(|c| matches!(c, GroupChild::Node(i) if *i == node_idx))
    })
}

fn size_of(metrics: &HashMap<NodeId, NodeMetrics>, id: &str) -> Size {
    metrics
        .get(id)
        .map(|m| Size::new(m.size.width, m.size.height))
        .unwrap_or(Size::new(80.0, 40.0))
}

/// 添加 state 节点（若 id 已存在则跳过）。
fn add_state_node(
    lg: &mut LayoutGraph,
    id_to_idx: &mut HashMap<String, usize>,
    id: String,
    size: Size,
    shape: ShapeHint,
) {
    if id_to_idx.contains_key(&id) {
        return;
    }
    let idx = lg.nodes.len();
    id_to_idx.insert(id.clone(), idx);
    lg.nodes.push(LNode {
        id,
        size,
        shape_hint: shape,
    });
}

impl ToLayoutGraph for StateDiagram {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        // state 图：节点主要来自 transitions（from/to），`states` 数组可能为空。
        // 收集所有出现过的状态 id（含 [*] 映射的 start/end），保证边不因节点缺失而丢失。
        let mut lg = LayoutGraph::default();
        let _ = measure;

        // 1. 从显式 states 声明收集节点（Simple/Composite/Fork/Join/Start/End）
        let mut id_to_idx: HashMap<String, usize> = HashMap::new();
        for s in &self.states {
            match s {
                State::Simple { id, .. } | State::Composite { id, .. } => {
                    add_state_node(
                        &mut lg,
                        &mut id_to_idx,
                        id.clone(),
                        Size::new(100.0, 48.0),
                        ShapeHint::Rect,
                    );
                }
                State::Fork { id } | State::Join { id } => {
                    // fork / join：水平横条。
                    add_state_node(
                        &mut lg,
                        &mut id_to_idx,
                        id.clone(),
                        Size::new(100.0, 10.0),
                        ShapeHint::Bar,
                    );
                }
                State::Start => {
                    add_state_node(
                        &mut lg,
                        &mut id_to_idx,
                        "__start__".into(),
                        Size::new(32.0, 32.0),
                        ShapeHint::Circle,
                    );
                }
                State::End => {
                    add_state_node(
                        &mut lg,
                        &mut id_to_idx,
                        "__end__".into(),
                        Size::new(36.0, 36.0),
                        ShapeHint::Circle,
                    );
                }
            }
        }

        // 2. 从 transitions 收集出现的节点（states 为空时也覆盖；[*] 映射 start/end）
        for t in &self.transitions {
            let from = if t.from == "[*]" {
                "__start__"
            } else {
                &t.from
            };
            let to = if t.to == "[*]" { "__end__" } else { &t.to };
            let (fsize, fshape) = if from == "__start__" {
                (Size::new(32.0, 32.0), ShapeHint::Circle)
            } else {
                (Size::new(100.0, 48.0), ShapeHint::Rect)
            };
            add_state_node(&mut lg, &mut id_to_idx, from.to_string(), fsize, fshape);
            let (tsize, tshape) = if to == "__end__" {
                (Size::new(36.0, 36.0), ShapeHint::Circle)
            } else {
                (Size::new(100.0, 48.0), ShapeHint::Rect)
            };
            add_state_node(&mut lg, &mut id_to_idx, to.to_string(), tsize, tshape);
        }

        // 3. 生成边（from/to → 节点索引）
        for t in &self.transitions {
            let from = if t.from == "[*]" {
                "__start__"
            } else {
                &t.from
            };
            let to = if t.to == "[*]" { "__end__" } else { &t.to };
            if let (Some(&s), Some(&tgt)) = (id_to_idx.get(from), id_to_idx.get(to)) {
                lg.edges.push(LEdge {
                    source: s,
                    target: tgt,
                    source_port: PortHint::Auto,
                    target_port: PortHint::Auto,
                    line_kind: LineKind::Solid,
                });
            }
        }

        // 4. 复合状态作为组（递归成员），供 GroupedDirected 处理
        // （state__composite：Outer 含 Inner1/Inner2）。先收集组的成员。
        for (i, s) in self.states.iter().enumerate() {
            if let State::Composite { id, .. } = s {
                // 找出组内 transitions（from/to 在 Outer 内部）——简化：跳过组边收集，
                // 仅标记组，确保不丢失顶层节点。完整分组留给后续渲染层。
                let _ = (i, id);
            }
        }

        lg
    }
}

impl ToLayoutGraph for ClassDiagram {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        let mut lg = LayoutGraph::default();
        for c in &self.classes {
            lg.nodes.push(LNode {
                id: c.name.clone(),
                size: measure_class_box(c),
                shape_hint: ShapeHint::Rect,
            });
        }
        let id_to_idx: HashMap<String, usize> = lg
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), i))
            .collect();
        for r in &self.relations {
            if let (Some(&s), Some(&t)) = (id_to_idx.get(&r.source), id_to_idx.get(&r.target)) {
                lg.edges.push(LEdge {
                    source: s,
                    target: t,
                    source_port: PortHint::Auto,
                    target_port: PortHint::Auto,
                    line_kind: LineKind::Solid,
                });
            }
        }
        let _ = measure;
        lg
    }
}

/// 测量类框尺寸（header + attrs + methods 三栏），与渲染层 `class.rs` 保持一致。
fn measure_class_box(c: &crate::ast::Class) -> lievisual::geometry::Size {
    use lievisual::text::{RichSpan, layout_text};
    use crate::builder::theme;
    use crate::vir::{TextAlign, TextBaseline, TextStyle};

    const PAD: f64 = 12.0;
    const MIN_W: f64 = 140.0;
    const SMALL_FONT: f64 = 11.0;
    // 空栏占位高度与"一行内容"一致（行高 18 + 上下留白 8），保证三栏高度均匀。
    const SECTION_MIN_H: f64 = 18.0 + 8.0;

    let display_name = match &c.generic {
        Some(g) => format!("{}<{}>", c.name, g),
        None => c.name.clone(),
    };
    let ts = TextStyle::new(
        theme::class::TEXT,
        theme::FONT_SIZE,
        theme::FONT_FAMILY.to_string(),
    )
    .with_align(TextAlign::Left)
    .with_baseline(TextBaseline::Top);
    let name_layout = layout_text(&[RichSpan::new(display_name, ts.clone())], None);
    let name_w = name_layout.width + PAD * 2.0;

    let mut attr_lines = Vec::new();
    let mut method_lines = Vec::new();
    for m in &c.members {
        let prefix = match m.visibility {
            Some(crate::ast::Visibility::Public) => "+",
            Some(crate::ast::Visibility::Private) => "-",
            Some(crate::ast::Visibility::Protected) => "#",
            Some(crate::ast::Visibility::Package) => "~",
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
        max_w = max_w.max(l.width + PAD * 2.0);
    }
    let header_h = name_layout.height + 12.0;
    // 三栏恒定：名称栏 + 属性栏 + 方法栏，空栏保留占位高度，与渲染层 class.rs 一致。
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
    lievisual::geometry::Size::new(max_w.max(MIN_W), height)
}

impl ToLayoutGraph for ErDiagram {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        let mut lg = LayoutGraph::default();

        // 实体可能仅由关系隐式定义（如 `A ||--o{ B`），未在 entities 块中出现。
        // 补齐为节点，保证边不因节点缺失而丢失。顺序：已声明实体在前（源码序），
        // 隐式实体按关系出现顺序追加——与渲染层遍历顺序保持一致。
        let mut id_to_idx: HashMap<String, usize> = HashMap::new();
        let mut nodes: Vec<(String, lievisual::geometry::Size)> = Vec::new();
        for e in &self.entities {
            let sz = measure_er_entity(e.name.as_str(), &e.attributes);
            let idx = nodes.len();
            id_to_idx.insert(e.name.clone(), idx);
            nodes.push((e.name.clone(), sz));
        }
        for r in &self.relationships {
            for name in [&r.first_entity, &r.second_entity] {
                if !id_to_idx.contains_key(name) {
                    let sz = measure_er_entity(name, &[]);
                    let idx = nodes.len();
                    id_to_idx.insert(name.clone(), idx);
                    nodes.push((name.clone(), sz));
                }
            }
        }

        for (id, sz) in nodes {
            lg.nodes.push(LNode {
                id,
                size: sz,
                shape_hint: ShapeHint::Rect,
            });
        }

        for r in &self.relationships {
            if let (Some(&s), Some(&t)) = (
                id_to_idx.get(&r.first_entity),
                id_to_idx.get(&r.second_entity),
            ) {
                lg.edges.push(LEdge {
                    source: s,
                    target: t,
                    source_port: PortHint::Auto,
                    target_port: PortHint::Auto,
                    line_kind: LineKind::Solid,
                });
            }
        }
        let _ = measure;
        lg
    }
}

/// 测量 ER 实体框尺寸：名称行 + 属性行，最小宽高。
fn measure_er_entity(name: &str, attrs: &[crate::ast::ErAttribute]) -> lievisual::geometry::Size {
    use lievisual::text::{RichSpan, layout_text};
    use crate::builder::theme;
    use crate::vir::{TextAlign, TextBaseline, TextStyle};

    const PAD: f64 = 14.0;
    const MIN_W: f64 = 160.0;
    const SMALL_FONT: f64 = 12.0;
    const ATTR_LINE_H: f64 = 18.0;

    let name_style = TextStyle::new(
        theme::TEXT_COLOR,
        theme::FONT_SIZE,
        theme::FONT_FAMILY.to_string(),
    )
    .with_align(TextAlign::Center)
    .with_baseline(TextBaseline::Middle);
    let n = layout_text(&[RichSpan::new(name.to_string(), name_style.clone())], None);
    let name_w = n.width + PAD * 2.0;

    let mut max_w = name_w;
    for attr in attrs {
        let line = format!("{} {}", attr.type_, attr.name);
        let l = layout_text(
            &[RichSpan::new(
                line,
                TextStyle::new(
                    theme::TEXT_COLOR,
                    SMALL_FONT,
                    theme::FONT_FAMILY.to_string(),
                ),
            )],
            None,
        );
        max_w = max_w.max(l.width + PAD * 2.0);
    }
    let header_h = n.height + 10.0;
    // 无属性时 body 仍保留最小占位，与渲染层 er.rs 保持一致。
    const MIN_BODY_H: f64 = 20.0;
    let attr_h = (attrs.len() as f64 * ATTR_LINE_H).max(MIN_BODY_H);
    lievisual::geometry::Size::new(max_w.max(MIN_W), header_h + attr_h)
}

impl ToLayoutGraph for SequenceDiagram {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        // 序列图走 LinearSolver：节点 = 参与者列（按源码顺序）。
        let mut lg = LayoutGraph::default();
        for p in &self.participants {
            lg.nodes.push(LNode {
                id: p.name.clone(),
                size: Size::new(120.0, 40.0),
                shape_hint: ShapeHint::Rect,
            });
        }
        let _ = measure;
        lg
    }
}

impl ToLayoutGraph for TimelineDiagram {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        // 时间线走 LinearSolver：节点 = 每个事件。
        let mut lg = LayoutGraph::default();
        for sec in &self.sections {
            for ev in &sec.events {
                lg.nodes.push(LNode {
                    id: ev.clone(),
                    size: Size::new(80.0, 40.0),
                    shape_hint: ShapeHint::Rect,
                });
            }
        }
        let _ = measure;
        lg
    }
}

impl ToLayoutGraph for Diagram {
    fn to_layout_graph(&self, measure: &Measure) -> LayoutGraph {
        match self {
            Diagram::Flowchart(fc) => fc.to_layout_graph(measure),
            Diagram::State(sd) => sd.to_layout_graph(measure),
            Diagram::Class(c) => c.to_layout_graph(measure),
            Diagram::Er(er) => er.to_layout_graph(measure),
            Diagram::Sequence(seq) => seq.to_layout_graph(measure),
            Diagram::Timeline(t) => t.to_layout_graph(measure),
            Diagram::Pie(p) => {
                // 饼图走 SimpleSolver：无节点排布，仅标题。
                LayoutGraph {
                    title: p.title.clone(),
                    ..Default::default()
                }
            }
            Diagram::GitGraph(_g) => {
                // gitgraph 走 SimpleSolver：按分支列线性排布。
                LayoutGraph::default()
            }
        }
    }
}
