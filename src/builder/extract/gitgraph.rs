//! GitGraph 的 extract：把 [`crate::ast::GitGraphDiagram`] 翻译成 [`Unigraph`]。
//!
//! family = Hierarchy：
//! - 每个提交（commit / merge / cherry-pick）→ 节点（`NodeDetail::GitCommit`，携带分支名 / 标签 / 是否合并）；
//! - 提交的父引用 → `EdgeKind::Generic` 边（commit → parent）；
//! - 每个分支 → 子图容器（`ContainerKind::GitBranch`，member_ids = 该分支提交，顺序 = 分支首见序）。
//!
//! 布局（engine Hierarchy）：提交按声明序沿 x 推进（`COMMIT_SPACING`），
//! 分支按容器序映射到 y 行（`BRANCH_SPACING`）。

use std::collections::HashMap;

use crate::{
    ast::{GitGraphDiagram, GitGraphStatement},
    builder::ir::{
        common::{
            ArrowSpec, ContainerKind, DiagramMeta, EdgePriority, LabelOrMeasured, LabelSpec,
            LineKind, NodeConstraint, NodeDetail, NodeKind, NodeRole, PortHint, PortSet,
            RoutingHint, SizeHint, StyleRef,
        },
        shape::ShapeKind,
        unigraph::{EdgeKind, GraphFamily, UGEdge, UGNode, UGSubgraph, Unigraph},
    },
};

/// 新增一个提交节点：`branch` 为当前分支，`extra_parent` 为 merge 的第二个父（可空）。
fn emit_commit(
    nodes: &mut Vec<UGNode>,
    edges: &mut Vec<UGEdge>,
    branch_heads: &mut HashMap<String, usize>,
    branch_members: &mut HashMap<String, Vec<String>>,
    edge_counter: &mut usize,
    branch: &str,
    explicit_id: Option<&str>,
    tag: Option<&str>,
    commit_type: Option<&str>,
    is_merge: bool,
    extra_parent: Option<usize>,
) {
    let idx = nodes.len();
    let id = format!("c{}", idx);
    let parent = branch_heads.get(branch).copied();
    let mut parents: Vec<usize> = Vec::new();
    if let Some(p) = parent {
        parents.push(p);
    }
    if let Some(ep) = extra_parent {
        if !parents.contains(&ep) {
            parents.push(ep);
        }
    }
    for p in parents {
        edges.push(UGEdge {
            id: format!("e{}", *edge_counter),
            source: id.clone(),
            target: format!("c{}", p),
            source_port: PortHint::Auto,
            target_port: PortHint::Auto,
            kind: EdgeKind::Generic,
            label_text: None,
            label: None,
            priority: EdgePriority::Primary,
            routing_hint: RoutingHint::Orthogonal,
            arrow: ArrowSpec::default(),
            line_kind: LineKind::Solid,
            repulsion: 1.0,
            cardinality: (None, None),
            cardinality_text: (None, None),
        });
        *edge_counter += 1;
    }
    nodes.push(UGNode {
        id: id.clone(),
        kind: NodeKind::Atom,
        role: NodeRole::Atom,
        shape: ShapeKind::Circle,
        label: LabelOrMeasured::Spec(LabelSpec {
            text: String::new(),
            spans: Vec::new(),
        }),
        ports: PortSet::default(),
        size_hint: SizeHint::ByText,
        style_ref: StyleRef::NodeDefault,
        constraint: NodeConstraint::Free,
        detail: NodeDetail::GitCommit {
            branch: branch.to_string(),
            id: explicit_id.map(str::to_string),
            tag: tag.map(str::to_string),
            commit_type: commit_type.map(str::to_string),
            is_merge,
        },
    });
    branch_heads.insert(branch.to_string(), idx);
    branch_members.entry(branch.to_string()).or_default().push(id);
}

/// 提取 gitgraph 为统一拓扑图（Hierarchy 家族）。
pub fn extract_gitgraph(graph: &GitGraphDiagram) -> Unigraph {
    let mut nodes: Vec<UGNode> = Vec::new();
    let mut edges: Vec<UGEdge> = Vec::new();
    let mut branch_heads: HashMap<String, usize> = HashMap::new();
    let mut branch_order: Vec<String> = vec!["main".to_string()];
    let mut branch_members: HashMap<String, Vec<String>> = HashMap::new();
    let mut current = "main".to_string();
    let mut edge_counter = 0usize;

    for stmt in &graph.statements {
        match stmt {
            GitGraphStatement::Branch { name } => {
                if !branch_order.contains(name) {
                    branch_order.push(name.clone());
                }
                // 新分支头指向当前分支头（分支从当前位置 fork）。
                if let Some(&head) = branch_heads.get(&current) {
                    branch_heads.insert(name.clone(), head);
                }
                current = name.clone();
            }
            GitGraphStatement::Checkout { branch } => {
                current = branch.clone();
                if !branch_order.contains(branch) {
                    branch_order.push(branch.clone());
                }
            }
            GitGraphStatement::Commit { id, tag, commit_type, .. } => {
                emit_commit(
                    &mut nodes,
                    &mut edges,
                    &mut branch_heads,
                    &mut branch_members,
                    &mut edge_counter,
                    &current,
                    id.as_deref(),
                    tag.as_deref(),
                    commit_type.as_deref(),
                    false,
                    None,
                );
            }
            GitGraphStatement::Merge { branch, id, tag, commit_type, .. } => {
                // 第二个父 = 被合并分支头；无显式标签时 merge 不显示标签（对齐官方）。
                let p2 = branch_heads.get(branch).copied();
                emit_commit(
                    &mut nodes,
                    &mut edges,
                    &mut branch_heads,
                    &mut branch_members,
                    &mut edge_counter,
                    &current,
                    id.as_deref(),
                    tag.as_deref(),
                    commit_type.as_deref(),
                    true,
                    p2,
                );
            }
            GitGraphStatement::CherryPick { id, .. } => {
                emit_commit(
                    &mut nodes,
                    &mut edges,
                    &mut branch_heads,
                    &mut branch_members,
                    &mut edge_counter,
                    &current,
                    id.as_deref(),
                    None,
                    None,
                    false,
                    None,
                );
            }
        }
    }

    // 分支容器：顺序 = 分支首见序（含 main）。
    let subgraphs: Vec<UGSubgraph> = branch_order
        .iter()
        .enumerate()
        .map(|(i, name)| UGSubgraph {
            id: format!("branch{}", i),
            title: Some(name.clone()),
            member_ids: branch_members.get(name).cloned().unwrap_or_default(),
            kind: ContainerKind::GitBranch,
        })
        .collect();

    Unigraph {
        family: GraphFamily::Hierarchy,
        direction: crate::ast::Direction::LR,
        nodes,
        edges,
        subgraphs,
        sequence_rows: None,
        meta: DiagramMeta::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ir::unigraph::GraphFamily;

    fn parse(src: &str) -> GitGraphDiagram {
        crate::MermaidParser::parse_mermaid(src)
            .map(|d| match d {
                crate::ast::Diagram::GitGraph(g) => g,
                _ => panic!("not a gitgraph"),
            })
            .expect("parse")
    }

    #[test]
    fn extract_commits_branches_and_edges() {
        let g = parse("gitGraph\n    commit\n    branch dev\n    checkout dev\n    commit id: \"d1\"\n    commit\n    checkout main\n    merge dev\n");
        let ug = extract_gitgraph(&g);
        assert_eq!(ug.family, GraphFamily::Hierarchy);
        // 提交数：main 2（commit + merge）+ dev 2 = 4。
        assert_eq!(ug.nodes.len(), 4);
        // 分支容器：main + dev。
        assert_eq!(ug.subgraphs.len(), 2);
        assert_eq!(ug.subgraphs[0].title.as_deref(), Some("main"));
        assert_eq!(ug.subgraphs[1].title.as_deref(), Some("dev"));
        // main 容器成员：c0、c3（merge）；dev 成员：c1、c2。
        assert_eq!(ug.subgraphs[0].member_ids, vec!["c0".to_string(), "c3".to_string()]);
        assert_eq!(ug.subgraphs[1].member_ids, vec!["c1".to_string(), "c2".to_string()]);
        // 边：c1→c0（fork 自 main）、c2→c1、c3→c2（merge 父1）+ c3→c0（merge 父2）。
        let pair: Vec<(String, String)> =
            ug.edges.iter().map(|e| (e.source.clone(), e.target.clone())).collect();
        assert!(pair.contains(&("c1".into(), "c0".into())));
        assert!(pair.contains(&("c2".into(), "c1".into())));
        assert!(pair.contains(&("c3".into(), "c2".into())));
        assert!(pair.contains(&("c3".into(), "c0".into())));
    }

    /// 回归：`gitGraph:` 头部不得吞掉首条 commit（parser 曾用跨行 skip_ws + consume_line）。
    #[test]
    fn extract_basic_case_structure() {
        let g = parse("gitGraph:\n    commit\n    commit\n    branch develop\n    checkout develop\n    commit\n    commit\n    checkout main\n    merge develop\n    commit\n");
        let ug = extract_gitgraph(&g);
        // 6 个提交：c0/c1 main，c2/c3 develop，c4 merge(main)，c5 main。
        assert_eq!(ug.nodes.len(), 6, "basic 用例应有 6 个提交（头部不得吞首条 commit）");
        let info: Vec<(String, &str, bool)> = ug
            .nodes
            .iter()
            .map(|n| match &n.detail {
                NodeDetail::GitCommit { branch, is_merge, .. } => {
                    (n.id.clone(), branch.as_str(), *is_merge)
                }
                _ => (n.id.clone(), "?", false),
            })
            .collect();
        assert_eq!(
            info,
            vec![
                ("c0".into(), "main", false),
                ("c1".into(), "main", false),
                ("c2".into(), "develop", false),
                ("c3".into(), "develop", false),
                ("c4".into(), "main", true),
                ("c5".into(), "main", false),
            ]
        );
        // 分支容器。
        assert_eq!(
            ug.subgraphs[0].member_ids,
            vec!["c0".to_string(), "c1".to_string(), "c4".to_string(), "c5".to_string()]
        );
        assert_eq!(
            ug.subgraphs[1].member_ids,
            vec!["c2".to_string(), "c3".to_string()]
        );
        // merge 双亲：c4 ← c3（develop 头）与 c4 ← c1（main 头）。
        let pair: Vec<(String, String)> =
            ug.edges.iter().map(|e| (e.source.clone(), e.target.clone())).collect();
        assert!(pair.contains(&("c4".into(), "c1".into())));
        assert!(pair.contains(&("c4".into(), "c3".into())));
    }

    #[test]
    fn extract_merge_is_marked() {
        let g = parse("gitGraph\n    commit\n    branch dev\n    checkout dev\n    commit\n    checkout main\n    merge dev\n");
        let ug = extract_gitgraph(&g);
        // 3 个提交：main(c0) → dev(c1) → merge(c2)。
        assert_eq!(ug.nodes.len(), 3);
        let merge = ug.nodes.iter().find(|n| n.id == "c2").unwrap();
        match &merge.detail {
            NodeDetail::GitCommit { branch, id, tag, commit_type, is_merge } => {
                assert_eq!(branch, "main");
                // 无显式标签时 merge 不应有默认 "merge dev" 标签（对齐官方）。
                assert_eq!(tag.as_deref(), None);
                assert_eq!(id.as_deref(), None);
                assert_eq!(commit_type.as_deref(), None);
                assert!(is_merge);
            }
            other => panic!("期望 GitCommit, got {other:?}"),
        }
    }
}
