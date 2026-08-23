//! stateDiagram / stateDiagram-v2 的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - `[*]` 表示起始/终止状态
//! - 状态声明：`state A` / `state "desc" as A` / `state C { ... }`（复合状态，可嵌套）
//! - 转移：`A --> B : label`，from/to 可为 `[*]`

use crate::ast::{State, StateDiagram, Transition};
use crate::parser::common::{
    consume_line, has_input, identifier, keyword, quoted_string, rest_of_line, skip_line,
    skip_ws_and_comments, inline_ws, PResult,
};
use winnow::{
    Parser,
    combinator::{alt, delimited, fail, opt, preceded},
    token::take_while,
};

/// 顶层入口：`stateDiagram` / `stateDiagram-v2` 图表。
pub fn state_diagram<'i>(input: &mut &'i str) -> PResult<'i, StateDiagram> {
    let _ = alt((
        keyword("stateDiagram-v2"),
        keyword("stateDiagram"),
    ))
    .parse_next(input)?;
    skip_ws_and_comments(input)?;

    parse_body(input)
}

/// 解析状态图主体（供复合状态嵌套复用）。
fn parse_body<'i>(input: &mut &'i str) -> PResult<'i, StateDiagram> {
    let mut states = Vec::new();
    let mut transitions = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if !has_input(input) {
            break;
        }
        // 复合状态块在 `}` 处结束
        if input.starts_with('}') {
            break;
        }
        // 每次尝试前保存检查点：winnow 组合子在 Cut 失败时不会回滚输入，
        // 手动恢复以确保后续分支（声明/转移/裸声明）都能从头解析本行。
        let cp = *input;
        // 状态声明（简单/复合/fork/join）
        if let Ok(s) = state_decl.parse_next(input) {
            states.push(s);
            continue;
        }
        *input = cp;
        // 转移
        if let Ok(t) = transition.parse_next(input) {
            transitions.push(t);
            continue;
        }
        *input = cp;
        // 裸状态声明：`s3` 或 `s2 : 描述`（不以 `state` 关键字开头的声明）
        if let Ok(s) = bare_state_decl.parse_next(input) {
            states.push(s);
            continue;
        }
        *input = cp;
        // 跳过未知行
        let _ = skip_line(input)?;
    }

    Ok(StateDiagram { states, transitions })
}

/// 状态引用（字符串）：`[*]` 或普通标识符/引号串。
fn state_ref<'i>(input: &mut &'i str) -> PResult<'i, String> {
    alt((
        "[*]".map(|s: &str| s.to_string()),
        quoted_string,
        identifier,
    ))
    .parse_next(input)
}

/// 状态声明：`state id`、`state id "desc"`、`state "desc" as id`、
/// `state id <<fork|join>>`、`state id { ... }`
fn state_decl<'i>(input: &mut &'i str) -> PResult<'i, State> {
    keyword("state").parse_next(input)?;
    inline_ws(input)?;

    // 形如 `state "desc"` 或 `state "desc" as A`
    let description = opt(delimited(
        '"',
        take_while(0.., |c: char| c != '"'),
        '"',
    ))
    .parse_next(input)?
    .map(|s: &str| s.trim().to_string());

    inline_ws(input)?;

    let id = if description.is_some() {
        // `state "desc" as A`
        keyword("as").parse_next(input)?;
        inline_ws(input)?;
        identifier.parse_next(input)?
    } else {
        identifier.parse_next(input)?
    };
    inline_ws(input)?;

    // 可选注解：`<<fork>>` / `<<join>>`
    let annotation = opt(delimited(
        "<<",
        take_while(0.., |c: char| c != '>'),
        ">>",
    ))
    .parse_next(input)?;
    inline_ws(input)?;

    if let Some(anno) = annotation {
        return match anno.trim() {
            "fork" => Ok(State::Fork { id }),
            "join" => Ok(State::Join { id }),
            _ => {
                consume_line(input)?;
                Ok(State::Simple { id, description })
            }
        };
    }

    if input.starts_with('{') {
        let _ = '{'.parse_next(input)?;
        let inner = parse_body(input)?;
        let _ = '}'.parse_next(input)?;
        consume_line(input)?;
        return Ok(State::Composite {
            id,
            inner: Box::new(inner),
        });
    }

    consume_line(input)?;
    Ok(State::Simple { id, description })
}

/// 裸状态声明（不以 `state` 关键字开头）：`s3` 或 `s2 : 描述`。
/// 仅在 `id` 后紧跟空白+冒号/换行/分号/EOF 时成立，避免把 `A --> B` 中的
/// `A` 误当成裸状态声明（转移解析优先级更高，不会到此处）。
fn bare_state_decl<'i>(input: &mut &'i str) -> PResult<'i, State> {
    let start = *input;
    let id = identifier.parse_next(input)?;
    inline_ws(input)?;
    let is_bare = input.is_empty()
        || input.starts_with(':')
        || input.starts_with('\n')
        || input.starts_with("\r\n")
        || input.starts_with(';');
    if !is_bare {
        *input = start;
        return fail.parse_next(input);
    }
    if input.starts_with(':') {
        let _ = ':'.parse_next(input)?;
        inline_ws(input)?;
        let desc = rest_of_line.parse_next(input)?.trim().to_string();
        consume_line(input)?;
        Ok(State::Simple {
            id,
            description: Some(desc),
        })
    } else {
        consume_line(input)?;
        Ok(State::Simple { id, description: None })
    }
}

/// 转移：`A --> B : label`，端可为 `[*]`。
fn transition<'i>(input: &mut &'i str) -> PResult<'i, Transition> {
    let from = state_ref.parse_next(input)?;
    inline_ws(input)?;

    let _ = "-->".parse_next(input)?;
    inline_ws(input)?;

    let to = state_ref.parse_next(input)?;
    inline_ws(input)?;

    let label = opt(preceded(':', rest_of_line))
        .parse_next(input)?
        .map(|s| s.trim().to_string());
    let _ = consume_line(input)?;

    Ok(Transition { from, to, label })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> StateDiagram {
        let mut stream: &str = input;
        let d = state_diagram.parse_next(&mut stream).unwrap();
        assert!(stream.is_empty(), "trailing input: {:?}", stream);
        d
    }

    #[test]
    fn start_end_transitions() {
        let d = parse("stateDiagram-v2\n[*] --> Idle\nIdle --> [*]");
        assert_eq!(d.transitions.len(), 2);
        assert_eq!(d.transitions[0].from, "[*]");
        assert_eq!(d.transitions[0].to, "Idle");
        assert_eq!(d.transitions[1].to, "[*]");
    }

    #[test]
    fn labeled_transition() {
        let d = parse("stateDiagram-v2\nA --> B : start");
        assert_eq!(d.transitions[0].label.as_deref(), Some("start"));
    }

    #[test]
    fn composite_state() {
        let d = parse("stateDiagram-v2\nstate Foo {\nA --> B\n}");
        assert_eq!(d.states.len(), 1);
        match &d.states[0] {
            State::Composite { id, inner } => {
                assert_eq!(id, "Foo");
                assert_eq!(inner.transitions.len(), 1);
            }
            _ => panic!("expected composite"),
        }
    }

    #[test]
    fn described_state() {
        let d = parse("stateDiagram-v2\nstate \"Active\" as Act");
        assert_eq!(d.states.len(), 1);
        match &d.states[0] {
            State::Simple { id, description } => {
                assert_eq!(id, "Act");
                assert_eq!(description.as_deref(), Some("Active"));
            }
            _ => panic!(),
        }
    }
}
