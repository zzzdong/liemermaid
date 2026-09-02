//! 共享词法工具：标识符、文本、注释、换行等。

use winnow::{
    Parser,
    ascii::{multispace0, multispace1},
    combinator::{alt, opt, peek},
    error::InputError,
    token::{one_of, take_until, take_while},
};

use crate::ast::ParticipantKind;

/// 大小写不敏感地匹配给定关键字（仅 ASCII 字母），匹配后消耗输入。
///
/// 例如 `keyword("sequenceDiagram")` 能匹配 `sequenceDiagram` / `SequenceDiagram`
/// / `SEQUENCEDIAGRAM`。
pub fn keyword<'i>(kw: &'static str) -> impl Parser<&'i str, &'i str, InputError<&'i str>> {
    move |input: &mut &'i str| {
        let lower = kw.to_ascii_lowercase();
        // 使用 `get` 安全切片：当 input 以多字节字符（如 BOM/中文）开头时，
        // 直接返回 None 而非 panic（字节边界不安全）。
        if let Some(candidate) = input.get(..lower.len())
            && candidate.to_ascii_lowercase() == lower
        {
            // 关键字后必须是空白、行尾或常见分隔符，避免误吞前缀
            let after = input[lower.len()..].chars().next();
            if after
                .is_none_or(|c| c.is_whitespace() || c == ':' || c == '\n' || c == '\r' || c == '{')
            {
                let matched = &input[..lower.len()];
                *input = &input[lower.len()..];
                return Ok(matched);
            }
        }
        Err(InputError::at(*input))
    }
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

/// 跳过**行内**空白与 `%%` 注释（不跨行）。
///
/// 语句内部的空白必须用这个而非 [`skip_ws_and_comments`]：后者会吃掉换行，
/// 使一条语句跨行拼接（如 `autonumber` 指令的下一行消息被并进上一句）。
pub fn inline_ws_and_comments<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    loop {
        let before = input.len();
        inline_ws(input)?;
        if input.starts_with("%%") {
            let _ = take_while(0.., |c: char| c != '\n' && c != '\r').parse_next(input)?;
        }
        if input.len() == before {
            break;
        }
    }
    Ok(())
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
pub fn has_input(input: &mut &str) -> bool {
    !input.is_empty()
}

/// 跳过当前行剩余内容（直到换行或 EOF），用于丢弃无法识别的语句行。
///
/// 使用 `take_while(0..,...)` 以保证在空输入/纯空白时不报错；若未产生任何
/// 进展（输入未前进）则强制消费一个字符，防止主解析循环死锁。
/// 输入已耗尽时直接返回 `Ok`（EOF 无需跳过），否则 `take_while(1..)` 会报错
/// 并让整张图解析失败。
pub fn skip_line<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    if input.is_empty() {
        return Ok(());
    }
    let before = input.len();
    let _ = take_while(0.., |c: char| c != '\n' && c != '\r').parse_next(input)?;
    let _ = opt(("\r\n", "\n")).parse_next(input)?;
    if input.len() == before {
        // 无法前进，消耗一个字符避免死循环
        let _ = take_while(1.., |_c: char| true).parse_next(input)?;
    }
    Ok(())
}

/// 尝试运行 `parser`，**失败时回滚**已消耗的输入。
///
/// winnow 的 `Parser::parse_next` 失败时**不保证**把 `input` 恢复到调用前的位置
/// （组合子内部可能已推进游标）。各图表主循环都是
/// `if let Ok(x) = p.parse_next(input)` 的择优结构：一次失败的尝试若留下部分消费，
/// 后续的兜底 `skip_line` 就会把**已经推进到的那一行**整行丢掉，导致后随的合法
/// 语句被静默吞掉（如 `autonumber\nA->>B: hi` 把消息行吃掉），甚至在 EOF 处让
/// 整图解析失败。所有择优分支都应改用本函数。
pub fn attempt<'i, O>(
    mut parser: impl Parser<&'i str, O, InputError<&'i str>>,
    input: &mut &'i str,
) -> Option<O> {
    let start = *input;
    match parser.parse_next(input) {
        Ok(value) => Some(value),
        Err(_) => {
            *input = start;
            None
        }
    }
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

/// 标识符：字母、数字、下划线、连字符（首字母不能是数字）。
///
/// **连字符需前瞻**：mermaid 的箭头全部由 `-` 组成（`-->` / `---` / `-.->` /
/// `--o` / `--x` / `<-->` / `o--o` / `x--x` / `-.-`）。若标识符无脑吞掉 `-`，
/// `A-->B` 会被切成 `A--` + `>B`，箭头解析失败、整条边被丢弃（曾导致
/// `A-->B` 渲染出空白画布）。因此遇到 `-` 时前瞻其后字符：若是 `-` / `.` / `>`
/// （即箭头起始）就在此截断；否则保留，兼容 `node-1` 这类带连字符的 id。
pub fn identifier<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let s = *input;
    let mut end = 0usize;
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    for (i, (b, c)) in chars.iter().enumerate() {
        if c.is_alphanumeric() || *c == '_' {
            end = b + c.len_utf8();
            continue;
        }
        if *c == '-' {
            // 前瞻：构成箭头起始则截断（不在标识符内消耗该 `-`）。
            if let Some((_, next)) = chars.get(i + 1)
                && matches!(next, '-' | '.' | '>')
            {
                break;
            }
            end = b + c.len_utf8();
            continue;
        }
        break;
    }
    // 首字符必须是字母或下划线（不能是数字/连字符），且至少消耗一个字符。
    let first = s.chars().next();
    let valid_start = first.is_some_and(|c| c.is_alphabetic() || c == '_');
    if end == 0 || !valid_start {
        return Err(InputError::at(*input));
    }
    *input = &s[end..];
    Ok(s[..end].to_string())
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
