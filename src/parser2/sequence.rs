//! sequenceDiagram 的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 参与者声明（`participant`/`actor`/... 及 `as` 别名）
//! - 消息（`->>`/`-->>`/`->`/`-->`/`-x`/`--x`/`x->`/`x-->`/`<<->>`/`<<-->>`），
//!   支持箭头后的 `+`/`-` 激活符号
//! - 备注（`left of`/`right of`/`over`）
//! - 分组块（`loop`/`alt`/`opt`/`par`），支持嵌套，以 `end` 结束

use crate::ast::{
    Message, MessageActivation, MessageArrow, Note, NotePlacement, Participant, ParticipantKind,
    SequenceBlock, SequenceBlockKind, SequenceDiagram, SequenceItem, SequenceStatement,
};
use crate::parser2::common::{
    PResult, consume_line, has_input, inline_ws, keyword, participant_kind, quoted_string, rest_of_line, skip_line, skip_ws_and_comments,
};
use winnow::{
    Parser,
    combinator::{alt, opt, preceded},
    token::take_while,
};

/// 顶层入口：`sequenceDiagram` 图表。
pub fn sequence_diagram<'i>(input: &mut &'i str) -> PResult<'i, SequenceDiagram> {
    keyword("sequenceDiagram").parse_next(input)?;
    skip_ws_and_comments(input)?;

    let mut participants = Vec::new();
    let mut statements = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if input.is_empty() {
            break;
        }
        // 参与者声明
        if let Ok(p) = participant_decl.parse_next(input) {
            participants.push(p);
            continue;
        }
        // 分组块
        if let Ok(stmt) = block_statement.parse_next(input) {
            statements.push(stmt);
            continue;
        }
        // 备注
        if let Ok(stmt) = note_statement.parse_next(input) {
            statements.push(stmt);
            continue;
        }
        // 消息
        if let Ok(stmt) = message_statement.parse_next(input) {
            statements.push(stmt);
            continue;
        }
        // 跳过无法识别的行
        let _ = skip_line(input)?;
    }

    Ok(SequenceDiagram {
        participants,
        statements,
    })
}

/// `participant` / `actor` / ... 声明。
///
/// 必须以显式关键字（`participant`/`actor`/...）开头——否则消息行（如
/// `Alice->>Bob`）会被误判为参与者。声明后必须紧跟行尾。
fn participant_decl<'i>(input: &mut &'i str) -> PResult<'i, Participant> {
    let kind = alt((
        keyword("participant").map(|_| ParticipantKind::Participant),
        participant_kind,
    ))
    .parse_next(input)?;

    // 参与者声明严格单行：只用行内空白，避免 `ws`（multispace0）吞掉换行跨行。
    inline_ws(input)?;
    let name = p_name.parse_next(input)?;

    // `as` 别名
    inline_ws(input)?;
    let alias = opt(preceded((keyword("as"), inline_ws), p_name))
        .parse_next(input)?;

    // 必须到达行尾（否则这不是一个合法的参与者声明）；仅跳过行内空白，不跨行
    inline_ws(input)?;
    if !(input.is_empty() || input.starts_with('\n') || input.starts_with('\r')) {
        return Err(winnow::error::InputError::at(*input));
    }
    let _ = opt(("\r\n", "\n")).parse_next(input)?;

    Ok(Participant {
        name,
        alias,
        kind,
    })
}

/// 参与者名字：引号串或标识符。
/// 参与者名字：引号串或严格标识符（不含连字符，避免与箭头 `-` 冲突）。
fn p_name<'i>(input: &mut &'i str) -> PResult<'i, String> {
    alt((quoted_string, seq_id)).parse_next(input)
}

/// 严格标识符：字母 / 数字 / 下划线（首字符非数字）。用于 sequence 参与者名。
fn seq_id<'i>(input: &mut &'i str) -> PResult<'i, String> {
    take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
        .map(|s: &str| s.to_string())
        .parse_next(input)
}

/// 消息语句：`A->>B: text`
fn message_statement<'i>(input: &mut &'i str) -> PResult<'i, SequenceStatement> {
    let from = p_name.parse_next(input)?;
    skip_ws_and_comments(input)?;

    let (arrow, activation) = arrow_with_activation.parse_next(input)?;
    skip_ws_and_comments(input)?;

    let to = p_name.parse_next(input)?;

    // 可选的 `: text`
    skip_ws_and_comments(input)?;
    let text = opt(preceded(':', rest_of_line)).parse_next(input)?;

    Ok(SequenceStatement::Message(Message {
        from,
        to,
        arrow,
        activation,
        text,
    }))
}

/// 箭头 + 激活符号：`->>+`、`-->>-`、`x->` 等。
fn arrow_with_activation<'i>(
    input: &mut &'i str,
) -> PResult<'i, (MessageArrow, Option<MessageActivation>)> {
    let base = alt((
        "<<->>".map(|_| MessageArrow::Both),
        "<<-->>".map(|_| MessageArrow::Both),
        "->>".map(|_| MessageArrow::SolidTip),
        "-->>".map(|_| MessageArrow::DashedTip),
        "->x".map(|_| MessageArrow::Cross),
        "--x".map(|_| MessageArrow::Cross),
        "->".map(|_| MessageArrow::Solid),
        alt((
            "-->".map(|_| MessageArrow::Dashed),
            "-x".map(|_| MessageArrow::Cross),
            alt(("x->".map(|_| MessageArrow::Open), "x-->".map(|_| MessageArrow::Open))),
        )),
    ))
    .parse_next(input)?;

    // 激活符号
    let activation = opt(alt((
        '+'.map(|_| MessageActivation::Activate),
        '-'.map(|_| MessageActivation::Deactivate),
    )))
    .parse_next(input)?;

    Ok((base, activation))
}

/// 备注语句：`Note left of A: text` / `Note over A,B: text`
fn note_statement<'i>(input: &mut &'i str) -> PResult<'i, SequenceStatement> {
    keyword("Note").parse_next(input)?;
    skip_ws_and_comments(input)?;

    let placement = alt((
        keyword("left of").map(|_| NotePlacement::LeftOf),
        keyword("right of").map(|_| NotePlacement::RightOf),
        keyword("over").map(|_| NotePlacement::Over),
    ))
    .parse_next(input)?;

    skip_ws_and_comments(input)?;
    let targets = if placement == NotePlacement::Over {
        // `over A, B` 或 `over A`
        let first = p_name.parse_next(input)?;
        let mut targets = vec![first];
        loop {
            skip_ws_and_comments(input)?;
            if let Ok(t) = preceded(',', preceded(skip_ws_and_comments, p_name)).parse_next(input) {
                targets.push(t);
            } else {
                break;
            }
        }
        targets
    } else {
        // `left of A`
        vec![p_name.parse_next(input)?]
    };

    skip_ws_and_comments(input)?;
    let text = opt(preceded(':', rest_of_line)).parse_next(input)?.unwrap_or_default();

    Ok(SequenceStatement::Note(Note {
        placement,
        targets,
        text,
    }))
}

/// 分组块：`loop label ... end`
fn block_statement<'i>(input: &mut &'i str) -> PResult<'i, SequenceStatement> {
    let kind = alt((
        keyword("loop").map(|_| SequenceBlockKind::Loop),
        keyword("alt").map(|_| SequenceBlockKind::Alt),
        keyword("opt").map(|_| SequenceBlockKind::Opt),
        keyword("par").map(|_| SequenceBlockKind::Par),
    ))
    .parse_next(input)?;

    skip_ws_and_comments(input)?;
    let label = opt(rest_of_line).parse_next(input)?;

    let mut items = Vec::new();
    // 解析块内语句，直到 `end`
    loop {
        skip_ws_and_comments(input)?;
        if !has_input(input) {
            break;
        }
        if input.starts_with("end") {
            break;
        }
        // 嵌套块
        if let Ok(SequenceStatement::Block(b)) = block_statement.parse_next(input) {
            items.push(SequenceItem::Block(b));
            continue;
        }
        // 备注
        if let Ok(stmt) = note_statement.parse_next(input) {
            if let SequenceStatement::Note(n) = stmt {
                items.push(SequenceItem::Note(n));
            }
            continue;
        }
        // 消息
        if let Ok(stmt) = message_statement.parse_next(input) {
            if let SequenceStatement::Message(m) = stmt {
                items.push(SequenceItem::Message(m));
            }
            continue;
        }
        // 跳过未知行
        let _ = skip_line(input)?;
    }

    // 消费 `end`
    keyword("end").parse_next(input)?;
    let _ = consume_line(input)?;

    Ok(SequenceStatement::Block(SequenceBlock {
        kind,
        label,
        items,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> SequenceDiagram {
        let mut stream: &str = input;
        let d = sequence_diagram.parse_next(&mut stream).unwrap();
        assert!(stream.is_empty(), "trailing input: {:?}", stream);
        d
    }

    #[test]
    fn basic_messages() {
        let d = parse("sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: Hi");
        assert_eq!(d.participants.len(), 0);
        assert_eq!(d.statements.len(), 2);
        match &d.statements[0] {
            SequenceStatement::Message(m) => {
                assert_eq!(m.from, "Alice");
                assert_eq!(m.to, "Bob");
                assert_eq!(m.arrow, MessageArrow::SolidTip);
                assert_eq!(m.text.as_deref(), Some("Hello"));
            }
            _ => panic!("expected message"),
        }
    }

    #[test]
    fn participants_and_alias() {
        let d = parse("sequenceDiagram\nparticipant Alice as A\nactor Bob\nAlice->>Bob: x");
        assert_eq!(d.participants.len(), 2);
        assert_eq!(d.participants[0].name, "Alice");
        assert_eq!(d.participants[0].alias.as_deref(), Some("A"));
        assert_eq!(d.participants[1].kind, ParticipantKind::Actor);
    }

    #[test]
    fn activation_symbols() {
        let d = parse("sequenceDiagram\nAlice->>+Bob: req\nBob-->>-Alice: resp");
        match &d.statements[0] {
            SequenceStatement::Message(m) => {
                assert_eq!(m.activation, Some(MessageActivation::Activate));
            }
            _ => panic!(),
        }
        match &d.statements[1] {
            SequenceStatement::Message(m) => {
                assert_eq!(m.activation, Some(MessageActivation::Deactivate));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn loop_block() {
        let d = parse("sequenceDiagram\nloop every 5s\nAlice->>Bob: ping\nend");
        match &d.statements[0] {
            SequenceStatement::Block(b) => {
                assert_eq!(b.kind, SequenceBlockKind::Loop);
                assert_eq!(b.label.as_deref(), Some("every 5s"));
                assert_eq!(b.items.len(), 1);
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn note_over() {
        let d = parse("sequenceDiagram\nNote over Alice,Bob: discussion");
        match &d.statements[0] {
            SequenceStatement::Note(n) => {
                assert_eq!(n.placement, NotePlacement::Over);
                assert_eq!(n.targets, vec!["Alice", "Bob"]);
                assert_eq!(n.text, "discussion");
            }
            _ => panic!("expected note"),
        }
    }
}
