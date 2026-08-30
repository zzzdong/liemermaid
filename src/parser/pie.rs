//! pie 图表的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 头部：`pie`（可选 `showData`）
//! - 标题：`title ...`
//! - 数据：`"label" : value` 或 `label : value`

use crate::ast::{PieData, PieDiagram};
use crate::parser::common::{
    PResult, attempt, consume_line, has_input, identifier, keyword, quoted_string, rest_of_line,
    skip_line, skip_ws_and_comments,
};
use winnow::{
    Parser,
    combinator::{alt, opt, preceded},
    token::take_while,
};

/// 顶层入口：`pie` 图表。
pub fn pie_diagram<'i>(input: &mut &'i str) -> PResult<'i, PieDiagram> {
    keyword("pie").parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 可选 `showData`
    let show_data = opt(keyword("showData")).parse_next(input)?.is_some();
    skip_ws_and_comments(input)?;

    let mut title = None;
    let mut data = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if !has_input(input) {
            break;
        }
        // 标题
        if let Some(t) = attempt(preceded(keyword("title"), rest_of_line), input) {
            title = Some(t);
            continue;
        }
        // 数据行
        if let Some(d) = attempt(data_row, input) {
            data.push(d);
            continue;
        }
        // 跳过未知行
        skip_line(input)?;
    }

    Ok(PieDiagram {
        title,
        show_data,
        data,
    })
}

/// 数据行：`label : value`
fn data_row<'i>(input: &mut &'i str) -> PResult<'i, PieData> {
    let label = alt((quoted_string, identifier)).parse_next(input)?;
    skip_ws_and_comments(input)?;
    let _ = ':'.parse_next(input)?;
    skip_ws_and_comments(input)?;

    // value 可以是数字、百分比或带引号的串
    let value = alt((
        quoted_string,
        take_while(1.., |c: char| c != '\n' && c != '\r' && c != '"')
            .map(|s: &str| s.trim().to_string()),
    ))
    .parse_next(input)?;

    consume_line(input)?;

    Ok(PieData { label, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> PieDiagram {
        let mut stream: &str = input;
        let d = pie_diagram.parse_next(&mut stream).unwrap();
        assert!(stream.is_empty(), "trailing input: {:?}", stream);
        d
    }

    #[test]
    fn basic_pie() {
        let d = parse("pie\ntitle My Pie\n\"A\" : 30\n\"B\" : 70");
        assert_eq!(d.title.as_deref(), Some("My Pie"));
        assert!(!d.show_data);
        assert_eq!(d.data.len(), 2);
        assert_eq!(d.data[0].label, "A");
        assert_eq!(d.data[0].value, "30");
        assert_eq!(d.data[1].value, "70");
    }

    #[test]
    fn show_data_flag() {
        let d = parse("pie showData\nC : 1\nD : 2");
        assert!(d.show_data);
        assert_eq!(d.data.len(), 2);
    }
}
