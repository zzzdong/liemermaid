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
///
/// 实现使用 `peekable` 线性扫描：此前先把「剩余全文」收集成 `Vec<(usize, char)>`，
/// 每个标识符都要遍历一遍全文，整体退化为 O(n²)（4000 条边的图解析近 1 秒）。
pub fn identifier<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let s = *input;
    let mut end = 0usize;
    let mut chars = s.char_indices().peekable();
    let mut first = true;
    while let Some((i, c)) = chars.next() {
        if first {
            // 首字符必须是字母或下划线（不能是数字/连字符）。
            if !(c.is_alphabetic() || c == '_') {
                break;
            }
            first = false;
            end = i + c.len_utf8();
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            end = i + c.len_utf8();
            continue;
        }
        if c == '-' {
            // 前瞻：构成箭头起始则截断（不在标识符内消耗该 `-`）。
            match chars.peek() {
                Some((_, '-' | '.' | '>')) => break,
                _ => {
                    end = i + c.len_utf8();
                    continue;
                }
            }
        }
        break;
    }
    if end == 0 {
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

// ---------- 标签文本（节点 / 边） ----------
//
// Mermaid 的标签几乎总是含空格与标点：`A[Hello World]`、`A[HTTP/2]`、
// `A -->|Get money| B`、`A[Start-Here]`。旧 `unquoted_text` 以「遇到空白或
// `.`/`/`/`-` 即停」的方式解析，导致这些语句整行解析失败并被主循环静默跳过，
// 常见表现为**空白画布**。这里改为「按闭合定界符截取原始文本」：只要不在引号内，
// 空格与标点都算标签内容。

/// 去掉最外层成对的引号（`"..."` / `'...'`），并 trim。
///
/// 仅当**配对引号恰好是最后一个字符**时才剥离：`"Hello"` → `Hello`，
/// 但 `"a" and "b"` 保持原样（内部引号不构成包裹）。反斜杠转义会被跳过。
fn strip_wrapping_quotes(s: &str) -> String {
    let t = s.trim();
    let Some(first) = t.chars().next() else {
        return String::new();
    };
    if first != '"' && first != '\'' {
        return t.to_string();
    }
    let mut escaped = false;
    let mut close = None;
    for (i, c) in t.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == first {
            close = Some(i);
            break;
        }
    }
    match close {
        Some(i) if i + first.len_utf8() == t.len() => t[first.len_utf8()..i].to_string(),
        _ => t.to_string(),
    }
}

/// 读取直到 `close`（不在引号内）为止的标签文本；**不消费** `close`。
///
/// 换行视为非法（标签不跨行）；引号内的 `close` 不算终止符，因此
/// `A["a]b"]` 能正确取到 `a]b`。
fn label_until<'i>(close: &'static str) -> impl Parser<&'i str, String, InputError<&'i str>> {
    move |input: &mut &'i str| {
        let s = *input;
        let mut quote: Option<char> = None;
        let mut escaped = false;
        let mut seen_content = false;
        let mut end = None;
        for (i, c) in s.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if let Some(q) = quote {
                if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
                continue;
            }
            match c {
                // 只有标签**开头**的引号才是包裹引号；`A[Don't]` 里的撇号是普通字符。
                '"' | '\'' if !seen_content => {
                    quote = Some(c);
                    seen_content = true;
                }
                '\n' | '\r' => break,
                _ if s[i..].starts_with(close) => {
                    end = Some(i);
                    break;
                }
                _ => {
                    if !c.is_whitespace() {
                        seen_content = true;
                    }
                }
            }
        }
        let idx = end.ok_or_else(|| InputError::at(s))?;
        let content = strip_wrapping_quotes(&s[..idx]);
        *input = &s[idx..];
        Ok(content)
    }
}

/// 带定界符的标签：`open ... close`，返回去引号后的文本。
pub fn shape_label<'i>(
    open: &'static str,
    close: &'static str,
) -> impl Parser<&'i str, String, InputError<&'i str>> {
    move |input: &mut &'i str| {
        let mut open = open;
        open.parse_next(input)?;
        let content = label_until(close).parse_next(input)?;
        let mut close = close;
        close.parse_next(input)?;
        Ok(content)
    }
}

/// 裸标签（`A -- text --> B`）的终止序列：箭头起始。
const BARE_LABEL_STOPS: &[&str] = &[
    "-->", "---", "--o", "--x", "-.->", ".->", "==>", "~~~", "--|",
];

/// 解析箭头中间的裸标签：读到箭头起始 / 行尾 / `;` 为止，允许空格与标点。
///
/// 空标签视为失败（`A --- B` 应走无标签分支，而不是产生空标签）。
pub fn bare_label<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let s = *input;
    // 供 `link_arrow` 的 `--label-->` 分支使用：此前的 `--` 可能其实是箭头
    // `-->` / `--o` / `--x` / `---` 的前半段。若紧邻字符表明是箭头，立即失败，
    // 让 `alt` 回退到无标签箭头分支（否则 `A --> B` 会被读成 `A --(">B")-->`）。
    let mut rest = s.chars();
    match (rest.next(), rest.next()) {
        (Some('>'), _) | (Some('|'), _) => return Err(InputError::at(s)),
        (Some('o' | 'x'), None) => return Err(InputError::at(s)),
        (Some('o' | 'x'), Some(c)) if c.is_whitespace() => return Err(InputError::at(s)),
        (Some('-'), Some('-' | '.' | '>')) => return Err(InputError::at(s)),
        (Some('='), Some('=' | '>')) => return Err(InputError::at(s)),
        _ => {}
    }
    let mut end = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut seen_content = false;
    for (i, c) in s.char_indices() {
        if escaped {
            escaped = false;
            seen_content = true;
            end = i + c.len_utf8();
            continue;
        }
        if let Some(q) = quote {
            if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            end = i + c.len_utf8();
            continue;
        }
        match c {
            // 只有标签开头的引号才是包裹引号；`-- Don't stop -->` 的撇号是普通字符。
            '"' | '\'' if !seen_content => {
                quote = Some(c);
                seen_content = true;
                end = i + c.len_utf8();
            }
            '\n' | '\r' | ';' => break,
            _ if BARE_LABEL_STOPS.iter().any(|t| s[i..].starts_with(t)) => break,
            _ => {
                if !c.is_whitespace() {
                    seen_content = true;
                }
                end = i + c.len_utf8();
            }
        }
    }
    let content = strip_wrapping_quotes(&s[..end]);
    if content.is_empty() {
        return Err(InputError::at(s));
    }
    *input = &s[end..];
    Ok(content)
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
