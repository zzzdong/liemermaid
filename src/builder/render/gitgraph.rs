//! GitGraph 渲染器：复用既有几何算法，按新管线统一入口绘制。

use std::collections::HashMap;

use lievisual::geometry::{BezPath, Point, Rect};
use lievisual::text::{RichSpan, compute_text_offset, layout_text};
use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::{
    ast::{GitGraphDiagram, GitGraphStatement},
    builder::types::OutputConfig,
    error::DiagramResult,
    vir::{self, Color, SceneNode, TextAlign, TextBaseline, TextStyle, Z_AXIS, Z_LABEL, Z_SERIES, theme},
};

// 所有尺寸/颜色来自 theme::gitgraph
const COMMIT_RADIUS: f64 = theme::gitgraph::COMMIT_RADIUS;
const BRANCH_SPACING: f64 = theme::gitgraph::BRANCH_SPACING;
const COMMIT_SPACING: f64 = theme::gitgraph::COMMIT_SPACING;
const LEFT_MARGIN: f64 = theme::gitgraph::LEFT_MARGIN;
const TOP_MARGIN: f64 = theme::gitgraph::TOP_MARGIN;
const FONT_SIZE: f64 = theme::FONT_SIZE;

/// 把 gitGraph AST 渲染为视觉元素（复用原 `build_gitgraph_elements` 的几何算法）。
pub fn render_gitgraph(graph: &GitGraphDiagram, _config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
    let mut elements = Vec::new();

    if graph.statements.is_empty() {
        return Ok(elements);
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
        return Ok(elements);
    }

    // ===== Phase 2: Assign positions (HORIZONTAL layout) =====
    let base_y = TOP_MARGIN;

    struct CommitPos {
        branch_name: String,
        tag: Option<String>,
        position: Point,
        is_merge: bool,
    }

    let mut commit_positions: Vec<CommitPos> = Vec::new();
    let mut global_x = LEFT_MARGIN;

    let mut node_to_pos: HashMap<NodeIndex, Point> = HashMap::new();

    for &node_idx in &commit_list {
        let data = &dag[node_idx];
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
            let main_y = base_y;
            let r = BRANCH_SPACING / 3.0;

            let mut path = BezPath::new();
            path.move_to(Point::new(parent_x, main_y));
            path.line_to(Point::new(parent_x, first.y - r));
            path.quad_to(
                Point::new(parent_x, first.y),
                Point::new(first.x, first.y),
            );
            elements.push(vir::path_node(
                path,
                vir::fs_stroke(color, theme::gitgraph::LINE_WIDTH),
                Z_AXIS,
            ));
        }
    }

    // ===== Phase 4: Draw merge lines using petgraph DAG =====
    for &node_idx in &commit_list {
        let data = &dag[node_idx];
        if !data.is_merge {
            continue;
        }

        let pos = node_to_pos[&node_idx];

        for edge in dag.edges_directed(node_idx, Direction::Incoming) {
            let parent_idx = edge.source();
            let parent_data = &dag[parent_idx];

            if parent_data.branch_name != data.branch_name
                && let Some(&parent_pos) = node_to_pos.get(&parent_idx)
            {
                let merge_color = branch_colors[parent_data.branch_name.as_str()];
                let r = BRANCH_SPACING / 3.0;

                let mut path = BezPath::new();
                path.move_to(parent_pos);
                path.line_to(Point::new(parent_pos.x, pos.y - r));
                path.quad_to(Point::new(parent_pos.x, pos.y), pos);
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
            elements.push(vir::circle_node(
                cp.position,
                COMMIT_RADIUS,
                vir::fs_both(Color::rgb(255, 255, 255), color, 3.0),
                Z_SERIES,
            ));
        } else {
            elements.push(vir::circle_node(
                cp.position,
                COMMIT_RADIUS,
                vir::fs_fill(color),
                Z_SERIES,
            ));
        }

        if let Some(tag) = &cp.tag {
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

    // ===== Phase 6: Branch labels on the LEFT side =====
    for (i, branch_name) in branch_order.iter().enumerate() {
        let color = branch_colors[branch_name.as_str()];
        let y = base_y + i as f64 * BRANCH_SPACING;

        let ts_label = vir::text_style(
            Color::rgb(255, 255, 255),
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

        elements.push(vir::rect_node(
            Rect::new(lx, ly, lx + lw, ly + lh),
            Some(4.0),
            vir::fs_both(color, color, 1.0),
            Z_SERIES,
        ));

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

        let last_in_branch = commit_positions
            .iter()
            .filter(|cp| cp.branch_name == *branch_name)
            .last();
        if let Some(last) = last_in_branch {
            elements.push(vir::line_node(
                Point::new(last.position.x + COMMIT_RADIUS + 4.0, y),
                Point::new(last.position.x + COMMIT_RADIUS + 40.0, y),
                vir::stroke(Color::new(180.0 / 255.0, 180.0 / 255.0, 180.0 / 255.0, 1.0), 1.0),
                Z_AXIS,
            ));
        }
    }

    Ok(elements)
}

struct CommitData {
    branch_name: String,
    tag: Option<String>,
    is_merge: bool,
}
