use std::collections::HashMap;

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use lievisual::geometry::{Point};

use crate::{
    ast::{GitGraphDiagram, GitGraphStatement},
    builder::{layout::types::LayoutEngine, types::OutputConfig},
    error::DiagramResult,
    vir::{self,
        Color,
        SceneNode,
        TextAlign,
        TextBaseline,
        TextStyle,
        Z_AXIS,
        Z_LABEL,
        Z_SERIES,
        theme,
    },
};
use lievisual::text::{compute_text_offset, layout_text, RichSpan};


const COMMIT_RADIUS: f64 = 8.0;
const BRANCH_SPACING: f64 = 40.0;
const COMMIT_SPACING: f64 = 40.0;
const LABEL_OFFSET: f64 = 20.0;
const LEFT_MARGIN: f64 = 60.0;
const TOP_MARGIN: f64 = 40.0;
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
    /// Is this a merge commit (has 2+ parents)?
    is_merge: bool,
}

pub fn build_gitgraph_elements(
    graph: &GitGraphDiagram,
    _config: &OutputConfig,
) -> Vec<SceneNode> {
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
            GitGraphStatement::Commit { tag } => {
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
            GitGraphStatement::Merge { branch } => {
                let parent1 = branch_heads.get(&current_branch).copied();
                let parent2 = branch_heads.get(branch).copied();
                let idx = dag.add_node(CommitData {
                    branch_name: current_branch.clone(),
                    tag: Some(format!("merge {}", branch)),
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
        }
    }

    if commit_list.is_empty() {
        return elements;
    }

    // ===== Phase 2: Assign positions =====
    let base_x = LEFT_MARGIN;

    struct CommitPos {
        branch_name: String,
        tag: Option<String>,
        position: Point,
    }

    let mut commit_positions: Vec<CommitPos> = Vec::new();
    let mut global_y = TOP_MARGIN;

    // Build node_idx → position mapping for edge drawing
    let mut node_to_pos: HashMap<NodeIndex, Point> = HashMap::new();

    for &node_idx in &commit_list {
        let data = &dag[node_idx];
        let col_idx = branch_order
            .iter()
            .position(|b| b == &data.branch_name)
            .unwrap_or(0);
        let x = base_x + col_idx as f64 * BRANCH_SPACING;
        let y = global_y;
        global_y += COMMIT_SPACING;

        let pos = Point::new(x, y);
        node_to_pos.insert(node_idx, pos);

        commit_positions.push(CommitPos {
            branch_name: data.branch_name.clone(),
            tag: data.tag.clone(),
            position: pos,
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

    // ===== Phase 3: Draw same-branch lines =====
    for branch_name in &branch_order {
        let branch_commits: Vec<&CommitPos> = commit_positions
            .iter()
            .filter(|cp| cp.branch_name == *branch_name)
            .collect();

        let color = branch_colors[branch_name.as_str()];

        if branch_commits.len() >= 2 {
            for i in 0..branch_commits.len() - 1 {
                elements.push(vir::line_node(branch_commits[i].position, branch_commits[i + 1].position, vir::stroke(color, 2.5), Z_AXIS));
            }
        }

        // 非 main 分支：从父 commit 的 Y 高度画竖线到第一个 commit（分支分叉线段）
        if *branch_name != "main"
            && !branch_commits.is_empty()
            && let Some(parent_y) = commit_list
                .iter()
                .copied()
                .find(|&n| dag[n].branch_name == *branch_name)
                .and_then(|n| {
                    dag.edges_directed(n, Direction::Incoming)
                        .filter_map(|e| node_to_pos.get(&e.source()).map(|p| p.y))
                        .next()
                })
        {
            let first = branch_commits[0].position;
            elements.push(vir::line_node(Point::new(first.x, parent_y), first, vir::stroke(color, 2.5), Z_AXIS));
        }
    }

    // ===== Phase 4: Draw merge lines using petgraph DAG =====
    for &node_idx in &commit_list {
        let data = &dag[node_idx];
        if !data.is_merge {
            continue;
        }

        let pos = node_to_pos[&node_idx];
        let color = branch_colors[data.branch_name.as_str()];

        for edge in dag.edges_directed(node_idx, Direction::Incoming) {
            let parent_idx = edge.source();
            let parent_data = &dag[parent_idx];

            if parent_data.branch_name != data.branch_name
                && let Some(&parent_pos) = node_to_pos.get(&parent_idx)
            {
                let mid_x = (pos.x + parent_pos.x) / 2.0;
                elements.push(vir::polyline_node(
                    vec![
                        parent_pos,
                        Point::new(mid_x, parent_pos.y),
                        Point::new(mid_x, pos.y),
                        pos,
                    ],
                    vir::stroke(color, 1.5),
                    Z_AXIS,
                ));
            }
        }
    }

    // ===== Phase 5: Draw commit circles and labels =====
    for cp in &commit_positions {
        let color = branch_colors[cp.branch_name.as_str()];

        elements.push(vir::circle_node(cp.position, COMMIT_RADIUS, vir::fs_both(Color::rgb(255, 255, 255), color, 2.5), Z_SERIES));

        elements.push(vir::circle_node(cp.position, 3.0, vir::fs_fill(color), Z_SERIES));

        if let Some(tag) = &cp.tag {
            let ts = TextStyle::new(color, FONT_SIZE, theme::FONT_FAMILY.to_string())
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Middle);
            let layout = layout_text(&[RichSpan::new(tag.to_string(), ts.clone())], Some(200.0));
            let (x_off, y_off) =
                compute_text_offset(&layout, TextAlign::Left, TextBaseline::Middle);
            elements.push(vir::text_node(
                tag.clone(),
                Point::new(cp.position.x + LABEL_OFFSET + x_off, cp.position.y + y_off),
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

    // ===== Phase 6: Branch labels at the top =====
    for (i, branch_name) in branch_order.iter().enumerate() {
        let color = branch_colors[branch_name.as_str()];
        let ts = TextStyle::new(color, FONT_SIZE, theme::FONT_FAMILY.to_string())
            .with_align(TextAlign::Right)
            .with_baseline(TextBaseline::Middle);
        let layout = layout_text(&[RichSpan::new(branch_name.to_string(), ts.clone())], Some(120.0));
        let x = base_x + i as f64 * BRANCH_SPACING;
        let y = TOP_MARGIN + 12.0;
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Right, TextBaseline::Middle);

        elements.push(vir::text_node(
            branch_name.clone(),
            Point::new(x - LABEL_OFFSET + x_off, y + y_off),
            vir::text_style(
                color,
                FONT_SIZE,
                theme::FONT_FAMILY,
                TextAlign::Left,
                TextBaseline::Top,
            ),
            0.0,
            Some(120.0),
            Z_LABEL,
        ));
    }

    elements
}
