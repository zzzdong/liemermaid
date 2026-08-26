//! 共享词法工具：标识符、文本、注释、换行等。

use winnow::{
    Parser,
    ascii::{multispace0, multispace1},
    combinator::{alt, opt, peek},
    error::InputError,
    token::{one_of, take_until, take_while},
};

use crate::ast::{Direction, ParticipantKind};

/// 大小写不敏感地匹配给定关键字（仅 ASCII 字母），匹配后消耗输入。
///
/// 例如 `keyword("sequenceDiagram")` 能匹配 `sequenceDiagram` / `SequenceDiagram`
/// / `SEQUENCEDIAGRAM`。
pub fn keyword<'i>(kw: &'static str) -> impl Parser<&'i str, &'i str, InputError<&'i str>> {
    move |input: &mut &'i str| {
        let lower = kw.to_ascii_lowercase();
        // 使用 `get` 安全切片：当 input 以多字节字符（如 BOM/中文）开头时，
        // 直接返回 None 而非 panic（字节边界不安全）。
        if let Some(candidate) = input.get(..lower.len()) {
            if candidate.to_ascii_lowercase() == lower {
                // 关键字后必须是空白、行尾或常见分隔符，避免误吞前缀
                let after = input[lower.len()..].chars().next();
                if after.map_or(true, |c| {
                    c.is_whitespace() || c == ':' || c == '\n' || c == '\r' || c == '{'
                }) {
                    let matched = &input[..lower.len()];
                    *input = &input[lower.len()..];
                    return Ok(matched);
                }
            }
        }
        Err(InputError::at(*input))
    }
}

/// 解析方向关键字（大小写不敏感），返回 [`Direction`]。
pub fn direction_ci<'i>(input: &mut &'i str) -> PResult<'i, Direction> {
    alt((
        keyword("TB").map(|_| Direction::TB),
        keyword("TD").map(|_| Direction::TD),
        keyword("BT").map(|_| Direction::BT),
        keyword("RL").map(|_| Direction::RL),
        keyword("LR").map(|_| Direction::LR),
    ))
    .parse_next(input)
}

/// 解析 sequence 图的参与者种类（大小写不敏感）。
pub fn participant_kind<'i>(input: &mut &'i str) -> PResult<'i, ParticipantKind> {
    use ParticipantKind::*;
    alt((
        keyword("actor").map(|_| Actor),
        keyword("boundary").map(|_| Boundary),
        keyword("control").map(|_| Control),
        keyword("entity").map(|_| Entity),
        keyword("database").map(|_| Database),
        keyword("collections").map(|_| Collections),
        keyword("queue").map(|_| Queue),
    ))
    .parse_next(input)
}

/// 解析直到行尾的文本（用于备注、标题等自由文本字段）。
///
/// 不含结尾的换行符；行内 `%%` 注释不会在此被截断（调用方按需处理）。
pub fn rest_of_line<'i>(input: &mut &'i str) -> PResult<'i, String> {
    take_while(1.., |c: char| c != '\n' && c != '\r')
        .map(|s: &str| s.trim().to_string())
        .parse_next(input)
}

/// 解析结果类型别名
pub type PResult<'i, O> = Result<O, InputError<&'i str>>;

// ---------- 空白与注释 ----------

pub fn ws<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    multispace0.parse_next(input)
}

/// 仅跳过行内空白（空格/Tab），不跨行，用于解析同类声明时避免吞掉换行。
pub fn inline_ws<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    take_while(0.., |c: char| c == ' ' || c == '\t').parse_next(input)
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

/// 跳过当前行剩余内容（直到换行或 EOF），用于丢弃无法识别的语句行。
///
/// 使用 `take_while(0..,...)` 以保证在空输入/纯空白时不报错；若未产生任何
/// 进展（输入未前进）则强制消费一个字符，防止主解析循环死锁。
pub fn skip_line<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    let before = input.len();
    let _ = take_while(0.., |c: char| c != '\n' && c != '\r').parse_next(input)?;
    let _ = opt(("\r\n", "\n")).parse_next(input)?;
    if input.len() == before {
        // 无法前进，消耗一个字符避免死循环
        let _ = take_while(1.., |_c: char| true).parse_next(input)?;
    }
    Ok(())
}

/// 消费当前行剩余内容（直到换行或 EOF），用于语句行解析后的清理。
///
/// 使用 `take_while(0..,...)`，因此在文件末尾（EOF）不会因无更多字符而失败。
pub fn consume_line<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    let _ = take_while(0.., |c: char| c != '\n' && c != '\r').parse_next(input)?;
    let _ = opt(alt(("\r\n", "\n"))).parse_next(input)?;
    Ok(())
}

/// 前瞻 `end` 关键字（不消耗）
pub fn peek_end<'i>(input: &mut &'i str) -> PResult<'i, &'i str> {
    peek("end").parse_next(input)
}

// ---------- 标识符与文本 ----------

/// 标识符：字母、数字、下划线、连字符（首字母不能数字）
pub fn identifier<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let start = take_while(1.., |c: char| c.is_ascii_alphabetic() || c == '_');
    let rest = take_while(0.., |c: char| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-'
    });
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
