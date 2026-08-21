//! 共享词法工具：标识符、文本、注释、换行等。

use winnow::{
    Parser,
    ascii::{multispace0, multispace1},
    combinator::{alt, opt, peek},
    error::InputError,
    token::{one_of, take_until, take_while},
};

/// 解析结果类型别名
pub type PResult<'i, O> = Result<O, InputError<&'i str>>;

// ---------- 空白与注释 ----------

pub fn ws<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    multispace0.parse_next(input)
}

pub fn ws1<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    multispace1.parse_next(input)
}

/// 行注释：`%%` 直到行尾
pub fn line_comment<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    let _ = "%%".parse_next(input)?;
    let _ = take_until(1.., '\n').parse_next(input)?;
    let _ = opt('\n').parse_next(input)?;
    Ok(())
}

/// 跳过空白（含换行）和注释
pub fn skip_ws_and_comments<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    loop {
        let mut advanced = false;
        if ws1(input).is_ok() {
            advanced = true;
        }
        if input.starts_with('\n') || input.starts_with("\r\n") {
            if input.starts_with("\r\n") {
                *input = &input[2..];
            } else {
                *input = &input[1..];
            }
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
pub fn has_input<'i>(input: &mut &'i str) -> bool {
    !input.is_empty()
}

/// 前瞻 `end` 关键字（不消耗）
pub fn peek_end<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    peek("end").parse_next(input)
}

// ---------- 标识符与文本 ----------

/// 标识符：字母、数字、下划线、连字符（首字母不能数字）
pub fn identifier<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let start = take_while(1.., |c: char| c.is_ascii_alphabetic() || c == '_');
    let rest =
        take_while(0.., |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    (start, rest)
        .map(|(s, r): (&str, &str)| format!("{}{}", s, r))
        .parse_next(input)
}

/// 带引号的字符串（双引号或单引号）
pub fn quoted_string<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let q = one_of(['"', '\'']).parse_next(input)?;
    let content = take_until(1.., q).parse_next(input)?;
    let _ = one_of([q]).parse_next(input)?;
    Ok(content.to_string())
}

/// 普通文本（不含特殊符号）
pub fn unquoted_text<'i>(input: &mut &'i str) -> PResult<'i, String> {
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
pub fn text<'i>(input: &mut &'i str) -> PResult<'i, String> {
    alt((quoted_string, unquoted_text)).parse_next(input)
}

// ---------- 方向 ----------

pub fn direction<'i>(input: &mut &'i str) -> PResult<'i, crate::ast::Direction> {
    use crate::ast::Direction;
    alt((
        "TB".map(|_| Direction::TB),
        "TD".map(|_| Direction::TD),
        "BT".map(|_| Direction::BT),
        "RL".map(|_| Direction::RL),
        "LR".map(|_| Direction::LR),
    ))
    .parse_next(input)
}
