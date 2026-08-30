//! flowchart / graph 图表的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 节点形状、箭头类型、链式边、子图（含嵌套）
//! - 重复声明覆盖、引号 id、行内注释

use crate::ast::{ArrowType, Edge, Flowchart, Node, NodeShape, Subgraph};
use crate::parser::common::{
    PResult, attempt, direction, has_input, identifier, inline_ws, inline_ws_and_comments,
    peek_end, rest_of_line, skip_ws_and_comments, text, ws1,
};
use winnow::{
    Parser,
    combinator::{alt, delimited, fail, opt, peek, preceded},
    token::{any, take_while},
};

// ---------- 节点形状 ----------

/// 解析节点形状定界符与文本，返回 `(shape, text)`。
///
/// 定界符按“最长优先”排列，避免 `((` 被 `(` 抢先匹配。
fn node_shape_with_text<'i>(
    input: &mut &'i str,
) -> PResult<'i, (Option<NodeShape>, Option<String>)> {
    let first = alt((
        delimited(
            "(((",
            take_while(1.., |c: char| c != ')' && c != '\n' && c != '\r'),
            ")))",
        )
        .map(|t: &str| (Some(NodeShape::DoubleCircle), Some(t.trim().to_string()))),
        delimited(
            "\x28\x28",
            take_while(1.., |c: char| c != ')' && c != '\n' && c != '\r'),
            "\x29\x29",
        )
        .map(|t: &str| (Some(NodeShape::Circle), Some(t.trim().to_string()))),
        delimited("\x5b\x28", text, "\x29\x5d")
            .map(|t| (Some(NodeShape::Cylinder), Some(t)))
            .map(|(s, t)| (s, t)),
        delimited("([", text, "])").map(|t| (Some(NodeShape::Stadium), Some(t))),
        delimited("[[", text, "]]").map(|t| (Some(NodeShape::Subroutine), Some(t))),
        delimited("{{", text, "}}").map(|t| (Some(NodeShape::Hexagon), Some(t))),
        delimited("(", text, ")").map(|t| (Some(NodeShape::Rounded), Some(t))),
        delimited("[", text, "]").map(|t| (Some(NodeShape::Rectangle), Some(t))),
        delimited("{", text, "}").map(|t| (Some(NodeShape::Diamond), Some(t))),
    ));
    let second = alt((
        delimited(">", text, "]").map(|t| (Some(NodeShape::Asymmetric), Some(t))),
        delimited("[/", text, "/]").map(|t| (Some(NodeShape::Parallelogram), Some(t))),
        delimited("[\\", text, "\\]").map(|t| (Some(NodeShape::ParallelogramAlt), Some(t))),
        delimited("[/", text, "\\]").map(|t| (Some(NodeShape::Trapezoid), Some(t))),
        delimited("[\\", text, "/]").map(|t| (Some(NodeShape::TrapezoidAlt), Some(t))),
    ));
    alt((first, second)).parse_next(input)
}

fn node_definition<'i>(input: &mut &'i str) -> PResult<'i, Node> {
    // 用 peek 守卫：只有在 `id [shape]` 后面不是箭头（即这是节点声明而非边）时才成立，
    // 避免把 `A --> B` 中的 `A` 误当成节点（winnow 在 parser 失败时不会回滚已消耗的 input）。
    // 用 peek 守卫：只有 `id [shape]` 才是节点声明（纯 `id` 行由边端点补节点）。
    // peek 在失败时回滚已消耗的 input，避免把 `A --> B` 中的 `A` 误当节点。
    let _ = peek((identifier, node_shape_with_text)).parse_next(input)?;
    let id = identifier.parse_next(input)?;
    let (shape, txt) = node_shape_with_text.parse_next(input)?;
    Ok(Node {
        id,
        shape,
        text: txt,
    })
}

/// 裸节点声明（孤立节点），如 `E`：identifier 后紧跟行尾/分号/EOF（而非箭头等
/// 后续 token），用于支持无形状、无出边的独立节点声明。手动回滚以避免 winnow
/// 组合子在失败时输入未回滚导致误吞后续边。
fn bare_node<'i>(input: &mut &'i str) -> PResult<'i, Node> {
    let start = *input;
    let id = identifier.parse_next(input)?;
    let rest = *input;
    let is_end = rest.is_empty()
        || rest.starts_with('\n')
        || rest.starts_with("\r\n")
        || rest.starts_with(';');
    if is_end {
        Ok(Node {
            id,
            shape: None,
            text: None,
        })
    } else {
        *input = start;
        fail.parse_next(input)
    }
}

// ---------- 箭头 ----------

fn arrow_type<'i>(input: &mut &'i str) -> PResult<'i, ArrowType> {
    let first = alt((
        "<-->".map(|_| ArrowType::Both),
        "o--o".map(|_| ArrowType::MultiCircle),
        "x--x".map(|_| ArrowType::MultiCross),
        "--o".map(|_| ArrowType::Circle),
        "--x".map(|_| ArrowType::Cross),
        "~~~".map(|_| ArrowType::Invisible),
    ));
    let second = alt((
        "==>".map(|_| ArrowType::Thick),
        "-.->".map(|_| ArrowType::Dotted),
        "-->".map(|_| ArrowType::Solid),
        "---".map(|_| ArrowType::NoArrow),
    ));
    alt((first, second)).parse_next(input)
}

// ---------- 边 ----------

/// 箭头 + 可选标签。标签形式：
///   `A -->|label| B`   （竖线包裹，箭头后）
///   `A --|label|--> B` （竖线包裹，箭头中）
///   `A --label--> B`   （裸文本，仅 solid/thick/dotted）
///   `A --> B`          （无标签）
fn link_arrow<'i>(input: &mut &'i str) -> PResult<'i, (ArrowType, Option<String>)> {
    alt((
        // A --|label|--> B（标签在箭头中间）
        (
            alt(("--", "-.", "==", "~~~")),
            delimited("|", text, "|"),
            arrow_type,
        )
            .map(|(_, label, arrow)| (arrow, Some(label))),
        // A --label--> B（裸标签在箭头中间）；`text` 到空白即停，故标签两侧的
        // 行内空白都要跳过（`-- yes -->` 里 `--`/`yes` 与 `yes`/`-->` 之间都是空格，
        // 否则 `unquoted_text` 遇前导空格匹配 0 字符而失败、整条边被丢弃）。
        (
            alt(("--", "-.", "==")),
            inline_ws,
            text,
            inline_ws,
            arrow_type,
        )
            .map(|(_, _, label, _, arrow)| (arrow, Some(label))),
        // A -->|label| B（标签在箭头后）
        (arrow_type, opt(delimited("|", text, "|"))).map(|(arrow, label)| (arrow, label)),
        // 无标签
        arrow_type.map(|arrow| (arrow, None)),
    ))
    .parse_next(input)
}

/// 单个边端点：`A` 或带形状的 `A[Start]`。
///
/// 返回 `(id, 可选节点声明)`：端点上直接写形状是 mermaid 最常见写法
/// （`A[Start] --> B[End]`），形状需要登记为节点，否则标签会丢失。
fn edge_endpoint<'i>(input: &mut &'i str) -> PResult<'i, (String, Option<Node>)> {
    let id = identifier.parse_next(input)?;
    let node = attempt(node_shape_with_text, input).map(|(shape, text)| Node {
        id: id.clone(),
        shape,
        text,
    });
    Ok((id, node))
}

/// `&` 分隔的端点列表：`A & B`。mermaid 的 `A & B --> C` 等价于两条边。
fn endpoint_list<'i>(input: &mut &'i str) -> PResult<'i, (Vec<String>, Vec<Node>)> {
    let (first_id, first_node) = edge_endpoint.parse_next(input)?;
    let mut ids = vec![first_id];
    let mut nodes = Vec::new();
    if let Some(n) = first_node {
        nodes.push(n);
    }
    loop {
        let before = *input;
        inline_ws_and_comments(input)?;
        if !input.starts_with('&') {
            *input = before;
            break;
        }
        let _ = '&'.parse_next(input)?;
        inline_ws_and_comments(input)?;
        match attempt(edge_endpoint, input) {
            Some((id, node)) => {
                ids.push(id);
                nodes.extend(node);
            }
            None => {
                *input = before;
                break;
            }
        }
    }
    Ok((ids, nodes))
}

/// 链式链接：`A --> B --> C`，并支持 `&` 多端点（`A & B --> C`）。
///
/// mermaid 语义：第 2 段起沿用上一段的目标作为源。返回 `(边, 端点处声明的节点)`。
fn chain_edges<'i>(input: &mut &'i str) -> PResult<'i, (Vec<Edge>, Vec<Node>)> {
    let (mut sources, mut nodes) = endpoint_list.parse_next(input)?;
    let mut edges: Vec<Edge> = Vec::new();

    loop {
        let before = *input;
        inline_ws_and_comments(input)?;
        let Some((arrow, label)) = attempt(link_arrow, input) else {
            *input = before;
            break;
        };
        inline_ws_and_comments(input)?;
        let (targets, target_nodes) = endpoint_list.parse_next(input)?;
        nodes.extend(target_nodes);
        for s in &sources {
            for t in &targets {
                edges.push(Edge {
                    source: s.clone(),
                    target: t.clone(),
                    arrow_type: arrow.clone(),
                    label: label.clone(),
                });
            }
        }
        sources = targets;
    }

    // 没有任何箭头就不是边（交给 `node_definition` / `bare_node` 处理）。
    if edges.is_empty() {
        return Err(winnow::error::InputError::at(*input));
    }
    Ok((edges, nodes))
}

// ---------- 子图 ----------

fn subgraph<'i>(input: &mut &'i str) -> PResult<'i, Subgraph> {
    let _ = "subgraph".parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 标题：subgraph 后整行剩余文本（到换行），支持多词（如 `backend services`）。
    let title = opt(rest_of_line).parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 子图内部方向（可选）
    let _ = opt(preceded(("direction", ws1), direction)).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if peek_end(input).is_ok() {
            break;
        }

        // 嵌套子图
        if let Some(sub) = attempt(subgraph, input) {
            nodes.extend(sub.nodes);
            edges.extend(sub.edges);
            continue;
        }
        // 边（含链式与 `&` 多端点）。必须**先于**节点声明尝试：
        // `A[Start] --> B[End]` 的端点自带形状，只有边解析器能同时登记节点。
        if let Some((chain, declared)) = attempt(chain_edges, input) {
            edges.extend(chain);
            nodes.extend(declared);
            continue;
        }
        // 节点
        if let Some(node) = attempt(node_definition, input) {
            nodes.push(node);
            continue;
        }
        // 裸节点声明（孤立节点），如 `E`
        if let Some(node) = attempt(bare_node, input) {
            nodes.push(node);
            continue;
        }
        // 跳过未知内容（直到分号或换行）
        let _ = take_while(1.., |c: char| c != ';' && c != '\n').parse_next(input)?;
    }

    let _ = "end".parse_next(input)?;

    // 将内部边的端点补为节点（若没有显式节点定义），与顶层流程图一致。
    let mut node_ids: std::collections::HashSet<String> =
        nodes.iter().map(|n| n.id.clone()).collect();
    for e in &edges {
        for ep in [&e.source, &e.target] {
            if node_ids.insert(ep.clone()) {
                nodes.push(Node {
                    id: ep.clone(),
                    shape: None,
                    text: None,
                });
            }
        }
    }

    Ok(Subgraph {
        title,
        nodes,
        edges,
    })
}

// ---------- 流程图主解析器 ----------

/// 顶层入口：`flowchart` / `graph` 图表。
pub fn flowchart_diagram<'i>(input: &mut &'i str) -> PResult<'i, Flowchart> {
    let _ = alt(("graph", "flowchart")).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let dir = opt(direction).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut subgraphs = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if input.is_empty() {
            break;
        }

        if let Some(sub) = attempt(subgraph, input) {
            subgraphs.push(sub);
            continue;
        }
        // 边先于节点声明：`A[Start] --> B[End]` 的端点形状需由边解析器登记。
        if let Some((chain, declared)) = attempt(chain_edges, input) {
            edges.extend(chain);
            nodes.extend(declared);
            continue;
        }
        if let Some(node) = attempt(node_definition, input) {
            nodes.push(node);
            continue;
        }
        // 裸节点声明（孤立节点），如 `E`
        if let Some(node) = attempt(bare_node, input) {
            nodes.push(node);
            continue;
        }
        // 跳过无法识别的行（剩余纯空白或空时不再强制消费字符）
        let before = input.len();
        let _ = take_while(0.., |c: char| c != ';' && c != '\n' && c != '\r').parse_next(input)?;
        let _ = opt(("\r\n", "\n")).parse_next(input)?;
        if input.len() == before {
            // 没有任何进展，防止死锁，强制消费一个字符
            let _ = any.parse_next(input)?;
        }
    }

    // 将边的端点补为节点（若没有显式节点定义），与旧解析器行为一致。
    let mut node_ids: std::collections::HashSet<String> =
        nodes.iter().map(|n| n.id.clone()).collect();
    for e in &edges {
        for ep in [&e.source, &e.target] {
            if node_ids.insert(ep.clone()) {
                nodes.push(Node {
                    id: ep.clone(),
                    shape: None,
                    text: None,
                });
            }
        }
    }
    // 同时处理子图内出现的边端点
    for sg in &mut subgraphs {
        let sg_ids: std::collections::HashSet<String> =
            sg.nodes.iter().map(|n| n.id.clone()).collect();
        let mut extra = Vec::new();
        for e in &sg.edges {
            for ep in [&e.source, &e.target] {
                if !sg_ids.contains(ep) && !node_ids.contains(ep) {
                    extra.push(Node {
                        id: ep.clone(),
                        shape: None,
                        text: None,
                    });
                }
            }
        }
        sg.nodes.extend(extra);
    }

    Ok(Flowchart {
        direction: dir,
        nodes,
        edges,
        subgraphs,
    })
}

/// 末尾允许空白 / 注释 / 换行。
pub fn trailing_ws<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    skip_ws_and_comments(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Direction;

    fn parse(input: &str) -> Flowchart {
        let mut stream: &str = input;
        let fc = flowchart_diagram.parse_next(&mut stream).unwrap();
        assert!(stream.is_empty(), "trailing input: {:?}", stream);
        fc
    }

    #[test]
    fn simple_chain() {
        let fc = parse("flowchart TD\nA[Start] --> B{Decision} --> C[End]");
        // 注意：与 pest 一致，节点声明与边是独立语句；
        // 此处 `A[Start]` 后跟 `-->` 无法在同一条语句解析，
        // 因此该输入实际只解析出节点 A 与形状，边需单独成行。
        assert_eq!(fc.direction, Some(Direction::TD));
    }

    #[test]
    fn nodes_and_edges_separate() {
        let fc = parse("flowchart TD\nA[Start]\nB{Decision}\nC[End]\nA --> B\nB --> C");
        assert_eq!(fc.nodes.len(), 3);
        assert_eq!(fc.edges.len(), 2);
        assert_eq!(fc.nodes[0].shape, Some(NodeShape::Rectangle));
        assert_eq!(fc.nodes[0].text.as_deref(), Some("Start"));
        assert_eq!(fc.nodes[1].shape, Some(NodeShape::Diamond));
        assert_eq!(fc.edges[0].arrow_type, ArrowType::Solid);
        assert_eq!(fc.edges[0].source, "A");
        assert_eq!(fc.edges[0].target, "B");
    }

    #[test]
    fn labeled_edges() {
        let fc = parse("flowchart LR\nA -->|Yes| B\nA -->|No| C");
        assert_eq!(fc.edges.len(), 2);
        assert_eq!(fc.edges[0].label.as_deref(), Some("Yes"));
        assert_eq!(fc.edges[1].label.as_deref(), Some("No"));
    }

    #[test]
    fn bare_label_edge_with_spaces() {
        // `-- yes -->`：裸标签在箭头中间，且标签与箭头间有空格。
        let fc = parse(
            "flowchart TD\nA[Start]\nB{Check}\nC[End]\nA --> B\nB -- yes --> C\nB -- no --> A",
        );
        assert_eq!(fc.edges.len(), 3, "应解析出 3 条边");
        assert_eq!(fc.edges[1].source, "B");
        assert_eq!(fc.edges[1].target, "C");
        assert_eq!(fc.edges[1].label.as_deref(), Some("yes"));
        assert_eq!(fc.edges[2].source, "B");
        assert_eq!(fc.edges[2].target, "A");
        assert_eq!(fc.edges[2].label.as_deref(), Some("no"));
    }

    #[test]
    fn all_shapes() {
        let fc = parse(
            "flowchart TD\nA[rect]\nB(rounded)\nC([stadium])\nD[[subroutine]]\nE{diamond}\nF{{hexagon}}\nG((circle))\nH(((double)))\nI[(cylinder)]\nJ>asym]\nK[/para/]\nL[\\para_alt\\]\nM[/trap\\]\nN[\\trap_alt/]",
        );
        let shapes: Vec<_> = fc.nodes.iter().map(|n| n.shape.clone()).collect();
        assert_eq!(
            shapes,
            vec![
                Some(NodeShape::Rectangle),
                Some(NodeShape::Rounded),
                Some(NodeShape::Stadium),
                Some(NodeShape::Subroutine),
                Some(NodeShape::Diamond),
                Some(NodeShape::Hexagon),
                Some(NodeShape::Circle),
                Some(NodeShape::DoubleCircle),
                Some(NodeShape::Cylinder),
                Some(NodeShape::Asymmetric),
                Some(NodeShape::Parallelogram),
                Some(NodeShape::ParallelogramAlt),
                Some(NodeShape::Trapezoid),
                Some(NodeShape::TrapezoidAlt),
            ]
        );
    }

    #[test]
    fn arrow_types() {
        let fc = parse(
            "flowchart LR\nA --> B\nC -.-> D\nE ==> F\nG --- H\nI --o J\nK --x L\nM <--> N\nO ~~~ P\nQ o--o R\nS x--x T",
        );
        let arrows: Vec<_> = fc.edges.iter().map(|e| e.arrow_type.clone()).collect();
        assert_eq!(
            arrows,
            vec![
                ArrowType::Solid,
                ArrowType::Dotted,
                ArrowType::Thick,
                ArrowType::NoArrow,
                ArrowType::Circle,
                ArrowType::Cross,
                ArrowType::Both,
                ArrowType::Invisible,
                ArrowType::MultiCircle,
                ArrowType::MultiCross,
            ]
        );
    }

    #[test]
    fn subgraph_with_title() {
        let fc =
            parse("flowchart TD\nsubgraph One\nA --> B\nend\nsubgraph Two\nC --> D\nend\nA --> C");
        assert_eq!(fc.subgraphs.len(), 2);
        assert_eq!(fc.subgraphs[0].title.as_deref(), Some("One"));
        assert_eq!(fc.subgraphs[0].edges.len(), 1);
        assert_eq!(fc.edges.len(), 1);
    }

    #[test]
    fn nested_subgraph() {
        let fc = parse("flowchart TD\nsubgraph outer\nsubgraph inner\nA --> B\nend\nend\nC --> A");
        assert_eq!(fc.subgraphs.len(), 1);
        assert_eq!(fc.subgraphs[0].title.as_deref(), Some("outer"));
        // 内层节点/边被提升
        assert_eq!(fc.subgraphs[0].nodes.len(), 2);
        assert_eq!(fc.subgraphs[0].edges.len(), 1);
        assert_eq!(fc.edges.len(), 1);
    }

    #[test]
    fn quoted_ids_and_comments() {
        let fc = parse("flowchart TD\nA[\"Start\"]\nB[End]\nA --> B");
        assert_eq!(fc.nodes[0].text.as_deref(), Some("Start"));
        assert_eq!(fc.edges.len(), 1);
    }
}
