//! timeline 图表的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 头部：`timeline`（可选方向 `TD`/`LR`）
//! - 标题：`title ...`
//! - 分段：`section Name`
//! - 事件行：`1950 : Event A : Event B`

use crate::ast::{TimelineDiagram, TimelineDirection, TimelineSection};
use crate::parser::common::{
    PResult, attempt, consume_line, has_input, keyword, rest_of_line, skip_ws_and_comments,
};
use winnow::{Parser, combinator::alt};

/// 顶层入口：`timeline` 图表。
pub fn timeline_diagram<'i>(input: &mut &'i str) -> PResult<'i, TimelineDiagram> {
    keyword("timeline").parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 可选方向（独立行 `TD` / `LR`）
    let mut direction = None;
    skip_ws_and_comments(input)?;
    if let Some(dir) = attempt(
        alt((
            keyword("TD").map(|_| TimelineDirection::TD),
            keyword("LR").map(|_| TimelineDirection::LR),
        )),
        input,
    ) {
        direction = Some(dir);
        consume_line(input)?;
    }

    let mut title = None;
    let mut sections: Vec<TimelineSection> = Vec::new();
    let mut current_section: String = String::new();
    let mut explicit_section = false;

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if !has_input(input) {
            break;
        }
        let line = rest_of_line.parse_next(input)?;
        if line.is_empty() {
            continue;
        }

        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("section ") {
            explicit_section = true;
            current_section = name.trim().to_string();
            if !sections.iter().any(|s| s.name == current_section) {
                sections.push(TimelineSection {
                    name: current_section.clone(),
                    events: Vec::new(),
                });
            }
        // `get(..5)` 保证切片不落在多字节字符中间（原 `trimmed[..5]` 遇 emoji 会 panic）。
        // 只有前 5 字节构成合法字符边界时 `get(..5)` 才返回 Some，因此 `get(5..)` 必为 Some。
        } else if trimmed
            .get(..5)
            .is_some_and(|h| h.eq_ignore_ascii_case("title"))
        {
            let t = trimmed.get(5..).map(str::trim).unwrap_or("");
            if !t.is_empty() {
                title = Some(t.to_string());
            }
        } else if let Some(idx) = trimmed.find(':') {
            // 事件行：`time : event1 : event2`
            let first = trimmed[..idx].trim();
            let rest = &trimmed[idx + 1..];
            let mut events: Vec<String> = rest
                .split(':')
                .map(|e| e.trim().to_string())
                .filter(|e| !e.is_empty())
                .collect();

            if !explicit_section {
                // 隐式 section 语法（无 `section` 关键字）：每行首 token 都是新 section 名。
                // 例：`2024 : Design : Prototype` -> section "2024"，事件 ["Design", "Prototype"]
                current_section = first.to_string();
                sections.push(TimelineSection {
                    name: current_section.clone(),
                    events,
                });
            } else {
                // 显式 section 内部的事件行：首 token 也作为首个事件。
                let mut all = vec![first.to_string()];
                all.append(&mut events);

                if let Some(sec) = sections
                    .iter_mut()
                    .find(|s: &&mut TimelineSection| s.name == current_section)
                {
                    sec.events.extend(all);
                } else {
                    sections.push(TimelineSection {
                        name: current_section.clone(),
                        events: all,
                    });
                }
            }
        }
        // 否则忽略该行
    }

    Ok(TimelineDiagram {
        title,
        direction,
        sections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> TimelineDiagram {
        let mut stream: &str = input;
        let d = timeline_diagram.parse_next(&mut stream).unwrap();
        assert!(stream.is_empty(), "trailing input: {:?}", stream);
        d
    }

    #[test]
    fn basic_timeline() {
        // 无 `section` 关键字时，每行首 token 作为隐式 section 名。
        let d = parse("timeline\n1950 : A : B\n2000 : C");
        assert_eq!(d.sections.len(), 2);
        assert_eq!(d.sections[0].name, "1950");
        assert_eq!(d.sections[0].events, vec!["A", "B"]);
        assert_eq!(d.sections[1].name, "2000");
        assert_eq!(d.sections[1].events, vec!["C"]);
    }

    #[test]
    fn title_and_sections() {
        let d = parse(
            "timeline\ntitle My History\nsection Early\n1900 : Born\nsection Later\n1950 : Retire",
        );
        assert_eq!(d.title.as_deref(), Some("My History"));
        assert_eq!(d.sections.len(), 2);
        assert_eq!(d.sections[0].name, "Early");
        assert_eq!(d.sections[0].events, vec!["1900", "Born"]);
        assert_eq!(d.sections[1].name, "Later");
    }

    #[test]
    fn direction_lr() {
        let d = parse("timeline LR\n2000 : X");
        assert_eq!(d.direction, Some(TimelineDirection::LR));
    }

    #[test]
    fn multi_byte_before_title_keyword_does_not_panic() {
        // 回归：原实现用 `trimmed[..5]` 字节切片判断 title，行首多字节字符跨过
        // 第 5 字节时 panic（byte index 5 is not a char boundary）。现改用 `get(..5)`。
        let d = parse("timeline\nab😀 title with text\n2000 : X");
        assert_eq!(d.sections.len(), 1);
        assert_eq!(d.sections[0].name, "2000");
        // 该行既非 section 也非 title，应被忽略。
    }
}
