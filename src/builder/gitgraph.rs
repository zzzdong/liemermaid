use std::collections::HashMap;

use lievisual::geometry::{BezPath, Point, Rect};
use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::{
    ast::{GitGraphDiagram, GitGraphStatement},
    builder::{layout::types::LayoutEngine, types::OutputConfig},
    error::DiagramResult,
    vir::{
        self, Color, SceneNode, TextAlign, TextBaseline, TextStyle, Z_AXIS, Z_LABEL, Z_SERIES,
        theme,
    },
};
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

// 所有尺寸/颜色来自 theme::gitgraph
const COMMIT_RADIUS: f64 = theme::gitgraph::COMMIT_RADIUS;
const BRANCH_SPACING: f64 = theme::gitgraph::BRANCH_SPACING;
const COMMIT_SPACING: f64 = theme::gitgraph::COMMIT_SPACING;
const LEFT_MARGIN: f64 = theme::gitgraph::LEFT_MARGIN;
const TOP_MARGIN: f64 = theme::gitgraph::TOP_MARGIN;
const FONT_SIZE: f64 = theme::FONT_SIZE;

pub struct GitGraphEngine<'a> {
    diagram: &'a GitGraphDiagram,
}

impl<'a> GitGraphEngine<'a> {
    pub fn new(diagram: &'a GitGraphDiagram) -> Self {
        Self { diagram }
    }
}

impl<'a> LayoutEngine for GitGraphEngine<'a> {
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
        Ok(build_gitgraph_elements(self.diagram, config))
    }
}

/// Commit node metadata stored in petgraph DAG
struct CommitData {
    branch_name: String,
    tag: Option<String>,
    is_merge: bool,
}

pub fn build_gitgraph_elements(graph: &GitGraphDiagram, _config: &OutputConfig) -> Vec<SceneNode> {
    let mut elements = Vec::new();

    if graph.statements.is_empty() {
        return elements;
    }

    // ===== Phase 1: Build commit DAG using petgraph =====
    let mut dag = DiGraph::new();
    let mut branch_heads: HashMap<String, NodeIndex> = HashMap::new();
    let mut branch_order: Vec<String> = Vec::new();
    let mut current_branch: String = "main".to_string();
    let mut commit_list: Vec<NodeIndex> = Vec::new();

    branch_order.push("main".to_string());

    for stmt in &graph.statements {
        match stmt {
            GitGraphStatement::Branch { name } => {
                if !branch_order.contains(name) {
                    branch_order.push(name.clone());
                }
                if let Some(&head) = branch_heads.get(&current_branch) {
                    branch_heads.insert(name.clone(), head);
                }
                current_branch = name.clone();
            }
            GitGraphStatement::Checkout { branch } => {
                current_branch = branch.clone();
                if !branch_order.contains(branch) {
                    branch_order.push(branch.clone());
                }
            }
            GitGraphStatement::Commit { tag, .. } => {
                let parent = branch_heads.get(&current_branch).copied();
                let idx = dag.add_node(CommitData {
                    branch_name: current_branch.clone(),
                    tag: tag.clone(),
                    is_merge: false,
                });
                if let Some(p) = parent {
                    dag.add_edge(p, idx, ());
                }
                branch_heads.insert(current_branch.clone(), idx);
                commit_list.push(idx);
            }
            GitGraphStatement::Merge { branch, tag, .. } => {
                let parent1 = branch_heads.get(&current_branch).copied();
                let parent2 = branch_heads.get(branch).copied();
                let idx = dag.add_node(CommitData {
                    branch_name: current_branch.clone(),
                    tag: tag.clone().or_else(|| Some(format!("merge {}", branch))),
                    is_merge: true,
                });
                if let Some(p1) = parent1 {
                    dag.add_edge(p1, idx, ());
                }
                if let Some(p2) = parent2 {
                    dag.add_edge(p2, idx, ());
                }
                branch_heads.insert(current_branch.clone(), idx);
                commit_list.push(idx);
            }
            GitGraphStatement::CherryPick { .. } => {
                let parent = branch_heads.get(&current_branch).copied();
                let idx = dag.add_node(CommitData {
                    branch_name: current_branch.clone(),
                    tag: None,
                    is_merge: false,
                });
                if let Some(p) = parent {
                    dag.add_edge(p, idx, ());
                }
                branch_heads.insert(current_branch.clone(), idx);
                commit_list.push(idx);
            }
        }
    }

    if commit_list.is_empty() {
        return elements;
    }

    // ===== Phase 2: Assign positions (HORIZONTAL layout) =====
    // X = 时间轴（从左到右），Y = 分支行（上到下）
    let base_y = TOP_MARGIN;

    struct CommitPos {
        branch_name: String,
        tag: Option<String>,
        position: Point,
        is_merge: bool,
    }

    let mut commit_positions: Vec<CommitPos> = Vec::new();
    let mut global_x = LEFT_MARGIN; // 横向：X 随每个 commit 递增

    // Build node_idx → position mapping for edge drawing
    let mut node_to_pos: HashMap<NodeIndex, Point> = HashMap::new();

    for &node_idx in &commit_list {
        let data = &dag[node_idx];
        // Y 由分支行决定（main=0, develop=1, ...）
        let row_idx = branch_order
            .iter()
            .position(|b| b == &data.branch_name)
            .unwrap_or(0);
        let x = global_x;
        let y = base_y + row_idx as f64 * BRANCH_SPACING;
        global_x += COMMIT_SPACING;

        let pos = Point::new(x, y);
        node_to_pos.insert(node_idx, pos);

        commit_positions.push(CommitPos {
            branch_name: data.branch_name.clone(),
            tag: data.tag.clone(),
            position: pos,
            is_merge: data.is_merge,
        });
    }

    let branch_colors: HashMap<&str, Color> = branch_order
        .iter()
        .enumerate()
        .map(|(i, name)| {
            (
                name.as_str(),
                theme::gitgraph::BRANCH_COLORS[i % theme::gitgraph::BRANCH_COLORS.len()],
            )
        })
        .collect();

    // ===== Phase 3: Draw same-branch horizontal lines =====
    for branch_name in &branch_order {
        let branch_commits: Vec<&CommitPos> = commit_positions
            .iter()
            .filter(|cp| cp.branch_name == *branch_name)
            .collect();

        let color = branch_colors[branch_name.as_str()];

        if branch_commits.len() >= 2 {
            for i in 0..branch_commits.len() - 1 {
                elements.push(vir::line_node(
                    branch_commits[i].position,
                    branch_commits[i + 1].position,
                    vir::stroke(color, theme::gitgraph::LINE_WIDTH),
                    Z_AXIS,
                ));
            }
        }

        // 非 main 分支：从父 commit 的 X 位置画圆角弯折线到第一个 commit
        // 横向布局下，从 main 行向下弯折到 develop 行（U 形曲线）
        if *branch_name != "main"
            && !branch_commits.is_empty()
            && let Some(parent_x) = commit_list
                .iter()
                .copied()
                .find(|&n| dag[n].branch_name == *branch_name)
                .and_then(|n| {
                    dag.edges_directed(n, Direction::Incoming)
                        .filter_map(|e| node_to_pos.get(&e.source()).map(|p| p.x))
                        .next()
                })
        {
            let first = branch_commits[0].position;
            let main_y = base_y; // main 分支在第 0 行
            let r = BRANCH_SPACING / 3.0; // 圆角半径

            // 用贝塞尔二次曲线画平滑 U 形弯折
            let mut path = BezPath::new();
            path.move_to(Point::new(parent_x, main_y));
            // 竖直向下到弯折起点
            path.line_to(Point::new(parent_x, first.y - r));
            // 二次贝塞尔弧线弯折到目标 X
            path.quad_to(
                Point::new(parent_x, first.y), // 控制点（形成圆角）
                Point::new(first.x, first.y),
            );
            elements.push(vir::path_node(
                path,
                vir::fs_stroke(color, theme::gitgraph::LINE_WIDTH),
                Z_AXIS,
            ));
        }
    }

    // ===== Phase 4: Draw merge lines using petgraph DAG (平滑贝塞尔曲线向上合并) =====
    for &node_idx in &commit_list {
        let data = &dag[node_idx];
        if !data.is_merge {
            continue;
        }

        let pos = node_to_pos[&node_idx];

        for edge in dag.edges_directed(node_idx, Direction::Incoming) {
            let parent_idx = edge.source();
            let parent_data = &dag[parent_idx];

            // 只画跨分支的合并线（同分支的已在 Phase 3 画过）
            if parent_data.branch_name != data.branch_name
                && let Some(&parent_pos) = node_to_pos.get(&parent_idx)
            {
                // 用**被合并分支**的颜色（不是目标分支）
                let merge_color = branch_colors[parent_data.branch_name.as_str()];
                let r = BRANCH_SPACING / 3.0;

                // 从被合并分支（下方）平滑弯折到目标分支（上方）
                let mut path = BezPath::new();
                path.move_to(parent_pos);
                // 竖直向上到弯折起点
                path.line_to(Point::new(parent_pos.x, pos.y - r));
                // 二次贝塞尔弧线弯折到目标位置
                path.quad_to(
                    Point::new(parent_pos.x, pos.y), // 控制点
                    pos,
                );
                elements.push(vir::path_node(
                    path,
                    vir::fs_stroke(merge_color, theme::gitgraph::LINE_WIDTH),
                    Z_AXIS,
                ));
            }
        }
    }

    // ===== Phase 5: Draw commit circles and labels =====
    for cp in &commit_positions {
        let color = branch_colors[cp.branch_name.as_str()];

        if cp.is_merge {
            // Merge commit: 空心圆（白底+彩色描边）
            elements.push(vir::circle_node(
                cp.position,
                COMMIT_RADIUS,
                vir::fs_both(Color::rgb(255, 255, 255), color, 3.0),
                Z_SERIES,
            ));
        } else {
            // Normal commit: 实心圆
            elements.push(vir::circle_node(
                cp.position,
                COMMIT_RADIUS,
                vir::fs_fill(color),
                Z_SERIES,
            ));
        }

        // Commit label below the circle (rotated-like, just text)
        if let Some(tag) = &cp.tag {
            let ts = TextStyle::new(color, FONT_SIZE, theme::FONT_FAMILY.to_string())
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Top);
            let _layout = layout_text(&[RichSpan::new(tag.to_string(), ts.clone())], Some(200.0));
            elements.push(vir::text_node(
                tag.clone(),
                Point::new(cp.position.x - 20.0, cp.position.y + COMMIT_RADIUS + 4.0),
                vir::text_style(
                    color,
                    FONT_SIZE,
                    theme::FONT_FAMILY,
                    TextAlign::Left,
                    TextBaseline::Top,
                ),
                0.0,
                Some(200.0),
                Z_LABEL,
            ));
        }
    }

    // ===== Phase 6: Branch labels on the LEFT side with colored background =====
    for (i, branch_name) in branch_order.iter().enumerate() {
        let color = branch_colors[branch_name.as_str()];
        let y = base_y + i as f64 * BRANCH_SPACING;

        // 彩色背景矩形标签（类似官方）
        let ts_label = vir::text_style(
            Color::rgb(255, 255, 255), // 白色文字
            FONT_SIZE,
            theme::FONT_FAMILY.to_string(),
            TextAlign::Center,
            TextBaseline::Middle,
        );
        let label_layout = layout_text(
            &[RichSpan::new(branch_name.to_string(), ts_label.clone())],
            None,
        );
        let pad_x = 12.0;
        let pad_y = 6.0;
        let lw = label_layout.width + pad_x * 2.0;
        let lh = label_layout.height + pad_y * 2.0;
        let lx = LEFT_MARGIN - lw - 16.0;
        let ly = y - lh / 2.0;

        // 背景矩形
        elements.push(vir::rect_node(
            Rect::new(lx, ly, lx + lw, ly + lh),
            Some(4.0),
            vir::fs_both(color, color, 1.0),
            Z_SERIES,
        ));

        // 文字居中
        let (tx_off, ty_off) =
            compute_text_offset(&label_layout, TextAlign::Center, TextBaseline::Middle);
        elements.push(vir::text_node(
            branch_name.clone(),
            Point::new(lx + lw / 2.0 + tx_off, ly + lh / 2.0 + ty_off),
            ts_label
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Top),
            0.0,
            None,
            Z_LABEL,
        ));

        // 虚线延长线（从最后一个 commit 向右延伸）
        let last_in_branch = commit_positions
            .iter()
            .filter(|cp| cp.branch_name == *branch_name)
            .last();
        if let Some(last) = last_in_branch {
            elements.push(vir::line_node(
                Point::new(last.position.x + COMMIT_RADIUS + 4.0, y),
                Point::new(last.position.x + COMMIT_RADIUS + 40.0, y),
                vir::stroke(
                    Color::new(180.0 / 255.0, 180.0 / 255.0, 180.0 / 255.0, 1.0),
                    1.0,
                ),
                Z_AXIS,
            ));
        }
    }

    elements
}
