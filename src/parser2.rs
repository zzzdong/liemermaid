use winnow::{
    Parser,
    ascii::{multispace0, multispace1},
    combinator::{alt, delimited, opt, peek, preceded, repeat, terminated},
    error::InputError,
    token::{one_of, take_until, take_while},
};

use crate::ast::*;

// ---------- 类型别名 ----------
type PResult<'i, O> = Result<O, InputError<&'i str>>;

// ---------- 空白与注释 ----------
fn ws<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    multispace0.parse_next(input)
}

fn ws1<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    multispace1.parse_next(input)
}

/// 标识符：字母、数字、下划线、连字符（首字母不能数字）
fn identifier<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let start = take_while(1.., |c: char| c.is_ascii_alphabetic() || c == '_');
    let rest = take_while(0.., |c: char| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-'
    });
    (start, rest)
        .map(|(s, r): (&str, &str)| format!("{}{}", s, r))
        .parse_next(input)
}

/// 带引号的字符串（双引号或单引号）
fn quoted_string<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let q = one_of(['"', '\'']).parse_next(input)?;
    let content = take_until(1, q).parse_next(input)?;
    let _ = one_of([q]).parse_next(input)?;
    Ok(content.to_string())
}

/// 普通文本（不含特殊符号）
fn unquoted_text<'i>(input: &mut &'i str) -> PResult<'i, String> {
    take_while(1.., |c: char| {
        !c.is_whitespace()
            && c != '['
            && c != ']'
            && c != '('
            && c != ')'
            && c != '{'
            && c != '}'
            && c != ';'
            && c != '|'
            && c != '<'
            && c != '>'
            && c != '='
            && c != '-'
            && c != '.'
            && c != '/'
            && c != '\\'
    })
    .map(|s: &str| s.to_string())
    .parse_next(input)
}

/// 通用文本
fn text<'i>(input: &mut &'i str) -> PResult<'i, String> {
    alt((quoted_string, unquoted_text)).parse_next(input)
}

/// 行注释：`%%` 直到行尾
fn line_comment<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    let _ = "%%".parse_next(input)?;
    let _ = take_until(1, '\n').parse_next(input)?;
    let _ = opt('\n').parse_next(input)?;
    Ok(())
}

/// 跳过空白和注释（使��循环，避免 repeat 类型推断问题）
fn skip_ws_and_comments<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    loop {
        let mut advanced = false;
        if ws1(input).is_ok() {
            advanced = true;
        }
        if line_comment(input).is_ok() {
            advanced = true;
        }
        if !advanced {
            break;
        }
    }
    Ok(())
}

/// 是否还有剩余输入（未到达 EOF）
fn has_input<'i>(input: &mut &'i str) -> bool {
    !input.is_empty()
}

/// 前瞻 `end` 关键字（不消耗）
fn peek_end<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    peek("end").parse_next(input)
}

// ---------- 方向 ----------
fn direction<'i>(input: &mut &'i str) -> PResult<'i, Direction> {
    alt((
        "TB".map(|_| Direction::TB),
        "TD".map(|_| Direction::TD),
        "BT".map(|_| Direction::BT),
        "RL".map(|_| Direction::RL),
        "LR".map(|_| Direction::LR),
    ))
    .parse_next(input)
}

// ---------- 节点形状 ----------
fn node_shape_with_text<'i>(
    input: &mut &'i str,
) -> PResult<'i, (Option<NodeShape>, Option<String>)> {
    let first = alt((
        delimited("[", text, "]").map(|t| (Some(NodeShape::Rectangle), Some(t))),
        delimited("(", text, ")").map(|t| (Some(NodeShape::Rounded), Some(t))),
        delimited("([", text, "])").map(|t| (Some(NodeShape::Stadium), Some(t))),
        delimited("[[", text, "]]").map(|t| (Some(NodeShape::Subroutine), Some(t))),
        delimited("{", text, "}").map(|t| (Some(NodeShape::Diamond), Some(t))),
        delimited("{{", text, "}}").map(|t| (Some(NodeShape::Hexagon), Some(t))),
        delimited("((", text, "))").map(|t| (Some(NodeShape::Circle), Some(t))),
        delimited("(((", text, ")))").map(|t| (Some(NodeShape::DoubleCircle), Some(t))),
        delimited("[(", text, ")]").map(|t| (Some(NodeShape::Cylinder), Some(t))),
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
    let id = identifier.parse_next(input)?;
    let (shape, text) = opt(node_shape_with_text)
        .parse_next(input)?
        .unwrap_or((None, None));
    Ok(Node { id, shape, text })
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
fn edge<'i>(input: &mut &'i str) -> PResult<'i, Edge> {
    let source = identifier.parse_next(input)?;
    skip_ws_and_comments(input)?;

    let (arrow, label) = alt((
        // A --|label|--> B
        (
            alt(("--", "-.", "==", "~~~")),
            delimited("|", text, "|"),
            arrow_type,
        )
            .map(|(_, label, arrow)| (arrow, Some(label))),
        // A --label--> B
        (alt(("--", "-.", "==", "~~~")), text, arrow_type)
            .map(|(_, label, arrow)| (arrow, Some(label))),
        // 无标签
        arrow_type.map(|arrow| (arrow, None)),
    ))
    .parse_next(input)?;

    skip_ws_and_comments(input)?;
    let target = identifier.parse_next(input)?;

    Ok(Edge {
        source,
        target,
        arrow_type: arrow,
        label,
    })
}

/// 链式链接：A --> B --> C
fn chain_edges<'i>(input: &mut &'i str) -> PResult<'i, Vec<Edge>> {
    let first = edge.parse_next(input)?;
    // 显式类型避免推断问题
    let rest: Vec<Edge> = repeat(0.., preceded(skip_ws_and_comments, edge)).parse_next(input)?;
    let mut edges = Vec::with_capacity(1 + rest.len());
    edges.push(first);
    edges.extend(rest);
    Ok(edges)
}

// ---------- 子图 ----------
fn subgraph<'i>(input: &mut &'i str) -> PResult<'i, Subgraph> {
    let _ = "subgraph".parse_next(input)?;
    skip_ws_and_comments(input)?;

    let id = opt(terminated(identifier, ws)).parse_next(input)?;
    let title = opt(text).parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 子图内部方向（可选）
    let _ = opt(preceded(("direction", ws1), direction)).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // 显式指定 any 的错误类型
    while has_input(input) {
        if peek_end(input).is_ok() {
            break;
        }
        skip_ws_and_comments(input)?;

        // 嵌套子图
        if let Ok(sub) = subgraph.parse_next(input) {
            nodes.extend(sub.nodes);
            edges.extend(sub.edges);
            continue;
        }

        // 节点
        if let Ok(node) = node_definition.parse_next(input) {
            nodes.push(node);
            continue;
        }

        // 边
        if let Ok(chain) = chain_edges.parse_next(input) {
            edges.extend(chain);
            continue;
        }

        // 跳过未知内容（直到分号或换行）
        let _ = take_while(1.., |c: char| c != ';' && c != '\n').parse_next(input)?;
    }

    let _ = "end".parse_next(input)?;
    Ok(Subgraph {
        title: title.or(id),
        nodes,
        edges,
    })
}

// ---------- 流程图主解析器 ----------
pub fn flowchart<'i>(input: &mut &'i str) -> PResult<'i, Flowchart> {
    let _ = alt(("graph", "flowchart")).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let direction = opt(direction).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut subgraphs = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;

        if let Ok(sub) = subgraph.parse_next(input) {
            subgraphs.push(sub);
            continue;
        }

        if let Ok(node) = node_definition.parse_next(input) {
            nodes.push(node);
            continue;
        }

        if let Ok(chain) = chain_edges.parse_next(input) {
            edges.extend(chain);
            continue;
        }

        let _ = take_while(1.., |c: char| c != ';' && c != '\n').parse_next(input)?;
    }

    Ok(Flowchart {
        direction,
        nodes,
        edges,
        subgraphs,
    })
}

/// 顶层解析入口
pub fn parse_diagram(input: &str) -> Result<Diagram, String> {
    let mut input = input.trim();
    match flowchart.parse_next(&mut input) {
        Ok(flow) => Ok(Diagram::Flowchart(flow)),
        Err(e) => Err(format!("Parse error: {:?}", e)),
    }
}

// ---------- 测试 ----------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple() {
        let src = r#"
        graph TD
            A[Start] --> B{Decision}
            B -->|Yes| C[Continue]
            B -->|No| D[Stop]
        "#;
        let diagram = parse_diagram(src).unwrap();
        if let Diagram::Flowchart(flow) = diagram {
            assert_eq!(flow.direction, Some(Direction::TD));
            assert_eq!(flow.nodes.len(), 4);
            assert_eq!(flow.edges.len(), 3);
        } else {
            panic!("Not flowchart");
        }
    }

    #[test]
    fn test_subgraph() {
        let src = r#"
        flowchart LR
            subgraph One
                A --> B
            end
            subgraph Two
                C --> D
            end
            A --> C
        "#;
        let diagram = parse_diagram(src).unwrap();
        if let Diagram::Flowchart(flow) = diagram {
            assert_eq!(flow.subgraphs.len(), 2);
            assert_eq!(flow.edges.len(), 1);
        }
    }
}
