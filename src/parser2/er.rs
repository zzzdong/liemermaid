//! erDiagram 的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 实体：`ENTITY { type name [PK|FK|...] ... }`
//! - 关系：`E1 ||--o{ E2 : label`，两端基数映射为 [`Cardinality`]

use crate::ast::{
    Cardinality, ErAttribute, ErDiagram, ErEntity, ErRelationship,
};
use crate::parser2::common::{
    consume_line, has_input, identifier, quoted_string, rest_of_line, skip_line,
    skip_ws_and_comments, ws, PResult,
};
use winnow::{
    Parser,
    combinator::{alt, opt, peek, preceded},
    token::take_while,
};

/// 顶层入口：`erDiagram` 图表。
pub fn er_diagram<'i>(input: &mut &'i str) -> PResult<'i, ErDiagram> {
    crate::parser2::common::keyword("erDiagram").parse_next(input)?;
    skip_ws_and_comments(input)?;

    let mut entities = Vec::new();
    let mut relationships = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if input.is_empty() {
            break;
        }
        if let Ok(e) = entity.parse_next(input) {
            entities.push(e);
            continue;
        }
        if let Ok(r) = relationship.parse_next(input) {
            relationships.push(r);
            continue;
        }
        // 跳过未知行
        let _ = skip_line(input)?;
    }

    Ok(ErDiagram {
        entities,
        relationships,
    })
}

/// 实体：`NAME { type name [PK] ... }`
fn entity<'i>(input: &mut &'i str) -> PResult<'i, ErEntity> {
    // 仅当后面紧跟 `{` 时才是实体声明，避免误吞关系行（peek 会回滚）。
    let _ = peek((identifier, ws, '{')).parse_next(input)?;

    let name = identifier.parse_next(input)?;
    skip_ws_and_comments(input)?;

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
            // 消费行尾（PK/FK/UK/NN 等行内标记一并随行尾跳过；注意不能跨行）
            let _ = take_while(0.., |c: char| c != '\n' && c != '}' && c != '\r')
                .parse_next(input)?;
            attributes.push(ErAttribute {
                type_: type_,
                name: attr_name,
            });
        }
        let _ = '}'.parse_next(input)?;
    }

    let _ = consume_line(input)?;

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
        .map(|s| s.trim().to_string());
    let _ = consume_line(input)?;

    Ok(ErRelationship {
        first_entity,
        second_entity,
        cardinality_first,
        cardinality_second,
        label,
    })
}

/// 实体引用（关系行中）：标识符或引号串。
fn entity_ref<'i>(input: &mut &'i str) -> PResult<'i, String> {
    alt((quoted_string, identifier)).parse_next(input)
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
        assert_eq!(d.relationships[1].cardinality_second, Cardinality::ZeroOrOne);
        assert_eq!(d.relationships[3].cardinality_second, Cardinality::ZeroOrOne);
    }
}
