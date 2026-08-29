//! gitGraph 图表的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 头部：`gitGraph`（可选 `: "main"` 指定主分支）
//! - 语句：`commit` / `branch name` / `checkout name` / `merge name` / `cherry-pick`
//!   其中 `commit`/`merge`/`cherry-pick` 通过 `id:`/`type:`/`tag:`/`parent:` 携带属性

use crate::ast::{GitGraphDiagram, GitGraphStatement};
use crate::parser::common::{
    PResult, consume_line, has_input, identifier, inline_ws, keyword, quoted_string, skip_line,
    skip_ws_and_comments,
};
use winnow::{
    Parser,
    combinator::{alt, opt, peek, separated},
    token::take_while,
};

/// 顶层入口：`gitGraph` 图表。
pub fn gitgraph_diagram<'i>(input: &mut &'i str) -> PResult<'i, GitGraphDiagram> {
    keyword("gitGraph").parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 可选 `: "main"`（仅限本行，不可跨行吞掉下一行语句）
    if input.starts_with(':') {
        let _ = ':'.parse_next(input)?;
        inline_ws(input)?;
        let _ = opt(quoted_string).parse_next(input)?;
        consume_line(input)?;
    }

    let mut statements = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if !has_input(input) {
            break;
        }
        if let Ok(stmt) = commit_stmt.parse_next(input) {
            statements.push(stmt);
            continue;
        }
        if let Ok(stmt) = branch_stmt.parse_next(input) {
            statements.push(stmt);
            continue;
        }
        if let Ok(stmt) = checkout_stmt.parse_next(input) {
            statements.push(stmt);
            continue;
        }
        if let Ok(stmt) = merge_stmt.parse_next(input) {
            statements.push(stmt);
            continue;
        }
        if let Ok(stmt) = cherry_pick_stmt.parse_next(input) {
            statements.push(stmt);
            continue;
        }
        // 跳过未知行
        skip_line(input)?;
    }

    Ok(GitGraphDiagram { statements })
}

/// 解析一组 `key: value` 属性对，返回映射。
fn attr_map<'i>(input: &mut &'i str) -> PResult<'i, std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    separated(0.., attr_pair, take_while(1.., |c: char| c.is_whitespace()))
        .map(|pairs: Vec<(String, String)>| {
            for (k, v) in pairs {
                map.insert(k, v);
            }
        })
        .parse_next(input)?;
    Ok(map)
}

fn attr_pair<'i>(input: &mut &'i str) -> PResult<'i, (String, String)> {
    // 用 peek 守卫：只有当形如 `identifier : ...` 时才消费，避免部分消费导致不回滚丢字符。
    let _ = peek((identifier, skip_ws_and_comments, ':')).parse_next(input)?;
    let key = identifier.parse_next(input)?;
    skip_ws_and_comments(input)?;
    let _ = ':'.parse_next(input)?;
    skip_ws_and_comments(input)?;
    let value = alt((quoted_string, identifier)).parse_next(input)?;
    Ok((key, value))
}

fn commit_stmt<'i>(input: &mut &'i str) -> PResult<'i, GitGraphStatement> {
    keyword("commit").parse_next(input)?;
    inline_ws(input)?;
    let map = attr_map(input)?;
    consume_line.parse_next(input)?;
    Ok(GitGraphStatement::Commit {
        id: map.get("id").cloned(),
        commit_type: map.get("type").cloned(),
        tag: map.get("tag").cloned(),
    })
}

fn branch_stmt<'i>(input: &mut &'i str) -> PResult<'i, GitGraphStatement> {
    keyword("branch").parse_next(input)?;
    inline_ws(input)?;
    let name = alt((quoted_string, identifier)).parse_next(input)?;
    consume_line.parse_next(input)?;
    Ok(GitGraphStatement::Branch { name })
}

fn checkout_stmt<'i>(input: &mut &'i str) -> PResult<'i, GitGraphStatement> {
    keyword("checkout").parse_next(input)?;
    inline_ws(input)?;
    let branch = alt((quoted_string, identifier)).parse_next(input)?;
    consume_line.parse_next(input)?;
    Ok(GitGraphStatement::Checkout { branch })
}

fn merge_stmt<'i>(input: &mut &'i str) -> PResult<'i, GitGraphStatement> {
    keyword("merge").parse_next(input)?;
    inline_ws(input)?;
    let branch = alt((quoted_string, identifier)).parse_next(input)?;
    inline_ws(input)?;
    let map = attr_map(input)?;
    consume_line.parse_next(input)?;
    Ok(GitGraphStatement::Merge {
        branch,
        id: map.get("id").cloned(),
        tag: map.get("tag").cloned(),
        commit_type: map.get("type").cloned(),
    })
}

fn cherry_pick_stmt<'i>(input: &mut &'i str) -> PResult<'i, GitGraphStatement> {
    keyword("cherry-pick").parse_next(input)?;
    inline_ws(input)?;
    let map = attr_map(input)?;
    consume_line.parse_next(input)?;
    Ok(GitGraphStatement::CherryPick {
        id: map.get("id").cloned(),
        parent: map.get("parent").cloned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> GitGraphDiagram {
        let mut stream: &str = input;
        let d = gitgraph_diagram.parse_next(&mut stream).unwrap();
        assert!(stream.is_empty(), "trailing input: {:?}", stream);
        d
    }

    #[test]
    fn basic_commits_and_branch() {
        let d = parse("gitGraph\ncommit\ncommit\nbranch develop\ncheckout develop");
        assert_eq!(d.statements.len(), 4);
        assert!(matches!(d.statements[0], GitGraphStatement::Commit { .. }));
        assert!(matches!(d.statements[2], GitGraphStatement::Branch { .. }));
        assert!(matches!(
            d.statements[3],
            GitGraphStatement::Checkout { .. }
        ));
    }

    #[test]
    fn commit_with_attrs() {
        let d = parse("gitGraph\ncommit id: \"a1\" type: HIGHLIGHT tag: \"v1\"");
        match &d.statements[0] {
            GitGraphStatement::Commit {
                id,
                tag,
                commit_type,
            } => {
                assert_eq!(id.as_deref(), Some("a1"));
                assert_eq!(tag.as_deref(), Some("v1"));
                assert_eq!(commit_type.as_deref(), Some("HIGHLIGHT"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn merge_and_cherry_pick() {
        let d = parse("gitGraph\nmerge develop id: \"m1\"\ncherry-pick id: \"c1\" parent: \"p1\"");
        assert_eq!(d.statements.len(), 2);
        match &d.statements[0] {
            GitGraphStatement::Merge { branch, id, .. } => {
                assert_eq!(branch, "develop");
                assert_eq!(id.as_deref(), Some("m1"));
            }
            _ => panic!(),
        }
        match &d.statements[1] {
            GitGraphStatement::CherryPick { id, parent } => {
                assert_eq!(id.as_deref(), Some("c1"));
                assert_eq!(parent.as_deref(), Some("p1"));
            }
            _ => panic!(),
        }
    }
}
