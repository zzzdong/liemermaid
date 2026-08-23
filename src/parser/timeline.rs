//! timeline 图表的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 头部：`timeline`（可选方向 `TD`/`LR`）
//! - 标题：`title ...`
//! - 分段：`section Name`
//! - 事件行：`1950 : Event A : Event B`

use crate::ast::{TimelineDiagram, TimelineDirection, TimelineSection};
use crate::parser::common::{
    consume_line, has_input, keyword, rest_of_line, skip_ws_and_comments, PResult,
};
use winnow::{Parser, combinator::alt};

/// 顶层入口：`timeline` 图表。
pub fn timeline_diagram<'i>(input: &mut &'i str) -> PResult<'i, TimelineDiagram> {
    keyword("timeline").parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 可选方向（独立行 `TD` / `LR`）
    let mut direction = None;
    skip_ws_and_comments(input)?;
    if let Ok(dir) = alt((
        keyword("TD").map(|_| TimelineDirection::TD),
        keyword("LR").map(|_| TimelineDirection::LR),
    ))
    .parse_next(input)
    {
        direction = Some(dir);
        let _ = consume_line(input)?;
    }

    let mut title = None;
    let mut sections: Vec<TimelineSection> = Vec::new();
    let mut current_section: String = String::new();

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
            current_section = name.trim().to_string();
            if !sections.iter().any(|s| s.name == current_section) {
                sections.push(TimelineSection {
                    name: current_section.clone(),
                    events: Vec::new(),
                });
            }
        } else if trimmed.len() >= 5 && trimmed[..5].to_ascii_lowercase() == "title" {
            let t = trimmed[5..].trim();
            if !t.is_empty() {
                title = Some(t.to_string());
            }
        } else if let Some(idx) = trimmed.find(':') {
            // 事件行：`time : event1 : event2`
            let time = trimmed[..idx].trim();
            let rest = &trimmed[idx + 1..];
            let mut events: Vec<String> = rest
                .split(':')
                .map(|e| e.trim().to_string())
                .filter(|e| !e.is_empty())
                .collect();
            // 时间标记本身作为首个事件
            let mut all = vec![time.to_string()];
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
        let d = parse("timeline\n1950 : A : B\n2000 : C");
        assert_eq!(d.sections.len(), 1);
        // 时间标记作为首个事件：1950 -> [1950,A,B]，2000 -> [2000,C]
        assert_eq!(d.sections[0].events, vec!["1950", "A", "B", "2000", "C"]);
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
}
