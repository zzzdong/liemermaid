//! erDiagram 的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 实体：`ENTITY { type name [PK|FK|...] ... }`
//! - 关系：`E1 ||--o{ E2 : label`，两端基数映射为 [`Cardinality`]

use crate::ast::{Cardinality, ErAttribute, ErDiagram, ErEntity, ErRelationship};
use crate::parser::common::{
    PResult, attempt, consume_line, has_input, identifier, inline_ws_and_comments, keyword,
    quoted_string, rest_of_line, skip_line, skip_ws_and_comments,
};
use winnow::{
    Parser,
    combinator::{alt, opt, preceded},
    token::take_while,
};

/// 顶层入口：`erDiagram` 图表。
pub fn er_diagram<'i>(input: &mut &'i str) -> PResult<'i, ErDiagram> {
    crate::parser::common::keyword("erDiagram").parse_next(input)?;
    skip_ws_and_comments(input)?;

    let mut entities = Vec::new();
    let mut relationships = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if input.is_empty() {
            break;
        }
        if let Some(e) = attempt(entity, input) {
            entities.push(e);
            continue;
        }
        if let Some(r) = attempt(relationship, input) {
            relationships.push(r);
            continue;
        }
        // 跳过未知行
        skip_line(input)?;
    }

    Ok(ErDiagram {
        entities,
        relationships,
    })
}

/// 实体：`NAME { type name [PK] ... }`，名字可带 `as` 别名。
fn entity<'i>(input: &mut &'i str) -> PResult<'i, ErEntity> {
    let name = entity_name.parse_next(input)?;
    skip_ws_and_comments(input)?;
    // 仅当后面紧跟 `{` 时才是实体声明，避免误吞关系行。
    if !input.starts_with('{') {
        return Err(winnow::error::InputError::at(*input));
    }

    let mut attributes = Vec::new();
    if input.starts_with('{') {
        let _ = '{'.parse_next(input)?;
        loop {
            skip_ws_and_comments(input)?;
            if input.starts_with('}') || !has_input(input) {
                break;
            }
            // 一个属性：type name [PK|FK|...]
            let type_ = alt((quoted_string, identifier)).parse_next(input)?;
            skip_ws_and_comments(input)?;
            let attr_name = alt((quoted_string, identifier)).parse_next(input)?;
            // 消费当前行剩余空白（不跳过换行），避免把下一行 type 误读为约束。
            let _ = take_while(0.., |c: char| c == ' ' || c == '\t').parse_next(input)?;
            // 可选约束标记（PK/FK/UK/NN 等）。仅当同一行还有非换行内容时读取。
            let constraint = if has_input(input)
                && !input.starts_with('\n')
                && !input.starts_with('\r')
                && !input.starts_with('}')
            {
                let tok = identifier.parse_next(input)?;
                Some(tok)
            } else {
                None
            };
            // 跳过本行剩余内容（注释 / 多余空白）。
            let _ =
                take_while(0.., |c: char| c != '\n' && c != '}' && c != '\r').parse_next(input)?;
            attributes.push(ErAttribute {
                type_,
                name: attr_name,
                constraint,
            });
        }
        let _ = '}'.parse_next(input)?;
    }

    consume_line(input)?;

    Ok(ErEntity { name, attributes })
}

/// 完整基数连接器（左连接符右），返回 `(card_first, card_second)`。
fn cardinality_pair<'i>(input: &mut &'i str) -> PResult<'i, (Cardinality, Cardinality)> {
    use Cardinality::*;
    let card_first = alt((
        "||".map(|_| ExactlyOne),
        "|o".map(|_| ZeroOrOne),
        "}o".map(|_| ZeroOrMany),
        "}|".map(|_| OneOrMany),
    ))
    .parse_next(input)?;

    // 连接符（标识关系类型，此处仅消耗）
    let _ = alt(("--", "..")).parse_next(input)?;

    let card_second = alt((
        "||".map(|_| ExactlyOne),
        "|o".map(|_| ZeroOrOne),
        "}o".map(|_| ZeroOrMany),
        "}|".map(|_| OneOrMany),
        "o{".map(|_| OneOrMany),
        "|{".map(|_| OneOrMany),
        "o|".map(|_| ZeroOrOne),
    ))
    .parse_next(input)?;

    Ok((card_first, card_second))
}

/// 关系：`E1 ||--o{ E2 : label`
fn relationship<'i>(input: &mut &'i str) -> PResult<'i, ErRelationship> {
    let first_entity = entity_ref.parse_next(input)?;
    skip_ws_and_comments(input)?;

    let (cardinality_first, cardinality_second) = cardinality_pair.parse_next(input)?;
    skip_ws_and_comments(input)?;

    let second_entity = entity_ref.parse_next(input)?;
    skip_ws_and_comments(input)?;

    let label = opt(preceded(':', rest_of_line))
        .parse_next(input)?
        .map(|s| {
            let t = s.trim();
            // 关系名可带引号（`: "places"`），去掉外层引号以对齐官方渲染。
            if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
                t[1..t.len() - 1].to_string()
            } else {
                t.to_string()
            }
        });
    consume_line(input)?;

    Ok(ErRelationship {
        first_entity,
        second_entity,
        cardinality_first,
        cardinality_second,
        label,
    })
}

/// 实体引用（关系行中）：引号串，或标识符（可带 `as` 别名）。
fn entity_ref<'i>(input: &mut &'i str) -> PResult<'i, String> {
    alt((quoted_string, entity_name)).parse_next(input)
}

/// 实体名（可带 `as` 别名）：`CUSTOMER as C`。
///
/// mermaid 以别名作为实体显示名，因此这里直接返回别名。
fn entity_name<'i>(input: &mut &'i str) -> PResult<'i, String> {
    let name = identifier.parse_next(input)?;
    let cp = *input;
    inline_ws_and_comments(input)?;
    if let Some(alias) = attempt(
        preceded((keyword("as"), inline_ws_and_comments), identifier),
        input,
    ) {
        return Ok(alias);
    }
    *input = cp;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> ErDiagram {
        let mut stream: &str = input;
        let d = er_diagram.parse_next(&mut stream).unwrap();
        assert!(stream.is_empty(), "trailing input: {:?}", stream);
        d
    }

    #[test]
    fn entity_with_attributes() {
        let d = parse("erDiagram\nCUSTOMER {\nint id PK\nstring name\n}");
        assert_eq!(d.entities.len(), 1);
        assert_eq!(d.entities[0].name, "CUSTOMER");
        assert_eq!(d.entities[0].attributes.len(), 2);
        assert_eq!(d.entities[0].attributes[0].type_, "int");
        assert_eq!(d.entities[0].attributes[0].name, "id");
    }

    #[test]
    fn relationship_cardinality() {
        let d = parse("erDiagram\nCUSTOMER ||--o{ ORDER : places");
        assert_eq!(d.relationships.len(), 1);
        let r = &d.relationships[0];
        assert_eq!(r.first_entity, "CUSTOMER");
        assert_eq!(r.second_entity, "ORDER");
        assert_eq!(r.cardinality_first, Cardinality::ExactlyOne);
        assert_eq!(r.cardinality_second, Cardinality::OneOrMany);
        assert_eq!(r.label.as_deref(), Some("places"));
    }

    #[test]
    fn relationship_cardinality_all_symbols() {
        // 覆盖所有 6 种基数符号组合（card_second 需含 |o、}o 等）
        let d = parse(
            "erDiagram\nA ||--|| B : ExactlyOne\nC |o--|o D : ZeroOrOne\nE }|--|{ F : OneOrMany\nG }o--o| H : ZeroOrMany",
        );
        assert_eq!(d.relationships.len(), 4);
        assert_eq!(
            d.relationships[1].cardinality_second,
            Cardinality::ZeroOrOne
        );
        assert_eq!(
            d.relationships[3].cardinality_second,
            Cardinality::ZeroOrOne
        );
    }
}
