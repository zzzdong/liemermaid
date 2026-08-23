//! classDiagram 的 winnow 解析。
//!
//! 与官方 Mermaid 语义对齐（默认解析器实现）：
//! - 类声明：`class Animal` / `class Animal { ... }`，含 `<<Interface>>` 注解
//! - 成员：可见性 `+`/`-`/`#`/`~` + 名字（含 `()` 为方法）+ 可选 `: 类型`
//! - 关系：`*--`/`o--`/`-->`/`..>`/`<|--`/`--*>`/`--o` 等，带可选基数与标签

use crate::ast::{
    Class, ClassDiagram, ClassMember, Relation, RelationKind, Visibility,
};
use crate::parser2::common::{
    consume_line, has_input, identifier, inline_ws, quoted_string, rest_of_line, skip_line,
    skip_ws_and_comments, PResult,
};
use winnow::{
    Parser,
    combinator::{alt, delimited, opt},
    token::take_while,
};

/// 顶层入口：`classDiagram` 图表。
pub fn class_diagram<'i>(input: &mut &'i str) -> PResult<'i, ClassDiagram> {
    crate::parser2::common::keyword("classDiagram").parse_next(input)?;
    skip_ws_and_comments(input)?;

    let mut classes = Vec::new();
    let mut relations = Vec::new();

    while has_input(input) {
        skip_ws_and_comments(input)?;
        if input.is_empty() {
            break;
        }
        if let Ok(c) = class_decl.parse_next(input) {
            classes.push(c);
            continue;
        }
        if let Ok(r) = relation.parse_next(input) {
            relations.push(r);
            continue;
        }
        // 跳过未知行
        let _ = skip_line(input)?;
    }

    Ok(ClassDiagram { classes, relations })
}

/// `class Name` 或 `class Name <<Annotation>> { ... }`
fn class_decl<'i>(input: &mut &'i str) -> PResult<'i, Class> {
    crate::parser2::common::keyword("class").parse_next(input)?;
    skip_ws_and_comments(input)?;
    // 类名保持原始形式（如 `Animal`），泛型参数单独存储。
    // 这样关系行里引用的 `Animal` 能与类声明匹配（mermaid 中 `~T~` 是类型参数，非名字一部分）。
    let name = identifier.parse_next(input)?;

    // 泛型：`class Animal~T~`
    let generic = if input.starts_with('~') {
        let g = delimited('~', take_while(0.., |c: char| c != '~'), '~')
            .parse_next(input)?;
        Some(g.trim().to_string())
    } else {
        None
    };

    // 仅在行内跳过空白（不跨行），否则会把类声明后的换行吞掉，
    // 导致下一行 `class ...` 被本行 consume_line 一起消费。
    inline_ws(input)?;
    // 可选注解 `<<Interface>>`
    let annotation = opt(delimited("<<", take_while(0.., |c: char| c != '>'), ">>"))
        .parse_next(input)?
        .map(|s: &str| s.trim().to_string());

    // 注解之后只跳过"行内"空白（不跨行），否则会把类声明后的换行吞掉，
    // 导致下一行的 `class ...` 被本行 consume_line 一起消费。
    inline_ws(input)?;

    let mut members = Vec::new();
    // 可选成员块
    if input.starts_with('{') {
        let _ = '{'.parse_next(input)?;
        loop {
            skip_ws_and_comments(input)?;
            if input.starts_with('}') {
                break;
            }
            if !has_input(input) {
                break;
            }
            if let Ok(m) = member.parse_next(input) {
                members.push(m);
            } else {
                // 跳过无法解析的成员行
                let _ = take_while(1.., |c: char| c != '\n' && c != '}' && c != '\r')
                    .parse_next(input)?;
            }
        }
        let _ = '}'.parse_next(input)?;
    }

    // 消费行尾
    let _ = consume_line(input)?;

    Ok(Class {
        name,
        generic,
        annotation,
        members,
    })
}

/// 去掉括号部分（`eat()` -> `eat`），用于成员名。
fn strip_parens(token: &str) -> String {
    match token.find('(') {
        Some(idx) => token[..idx].to_string(),
        None => token.to_string(),
    }
}

/// 读取一个成员词（字母数字及括号/泛型符号）。
fn member_token<'i>(input: &mut &'i str) -> PResult<'i, String> {
    take_while(1.., |c: char| {
        c.is_ascii_alphanumeric() || c == '_' || c == '(' || c == ')' || c == '<' || c == '>'
    })
    .parse_next(input)
    .map(|s: &str| s.trim().to_string())
}

/// 类成员：`+name: type` / `-foo()` / `#bar: int`
fn member<'i>(input: &mut &'i str) -> PResult<'i, ClassMember> {
    // 可见性（可选）
    let visibility = opt(alt((
        '+'.map(|_| Visibility::Public),
        '-'.map(|_| Visibility::Private),
        '#'.map(|_| Visibility::Protected),
        '~'.map(|_| Visibility::Package),
    )))
    .parse_next(input)?;

    inline_ws(input)?;

    // 成员语法有两种风格：
    //   +String name    （类型在前，名字在后）
    //   +name: String   （名字在前，`:` 后为类型）
    let first = member_token(input)?;
    inline_ws(input)?;

    let (name, mut type_, is_method) = if input.starts_with(':') {
        // `+name: type` 风格
        let is_method = first.contains('(');
        let name = strip_parens(&first);
        (name, None, is_method)
    } else {
        // 读取第二个词（如有）
        let second = opt(member_token).parse_next(input)?;
        inline_ws(input)?;
        if let Some(second) = second {
            // `+type name` / `+name() type` 风格：
            // 若 first 含括号（方法名），则 first 为名、second 为返回类型；
            // 否则 first 为类型、second 为名（字段）。
            if first.contains('(') {
                let name = strip_parens(&first);
                (name, Some(second), true)
            } else {
                (second, Some(first), false)
            }
        } else {
            let is_method = first.contains('(');
            (strip_parens(&first), None, is_method)
        }
    };

    // 可选类型 `: type`（覆盖）
    if input.starts_with(':') {
        let _ = ':'.parse_next(input)?;
        skip_ws_and_comments(input)?;
        let t = rest_of_line.parse_next(input)?;
        type_ = Some(t.trim().to_string());
    }

    // 消费行尾
    let _ = consume_line(input)?;

    Ok(ClassMember {
        visibility,
        name,
        type_,
        is_method,
    })
}

/// 关系：`A "1" <|-- "*" B : label` 等。
/// mermaid 中基数是紧贴实体名的引号串（可出现在实体前/后），不带 `..` 分隔符。
fn relation<'i>(input: &mut &'i str) -> PResult<'i, Relation> {
    // 首端前可选基数
    let pre = opt(quoted_string).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let source = identifier.parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 首端后可选基数
    let post = opt(quoted_string).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let kind = relation_kind.parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 末端前可选基数
    let pre2 = opt(quoted_string).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let target = identifier.parse_next(input)?;
    skip_ws_and_comments(input)?;

    // 末端后可选基数
    let post2 = opt(quoted_string).parse_next(input)?;
    skip_ws_and_comments(input)?;

    let cardinality_first = pre.or(post);
    let cardinality_second = pre2.or(post2);

    // 仅当本行确实以 `:` 开头才算关系标签；否则视为关系行已结束，
    // 不跨行 consume_line，避免把下一行关系整行吞掉。
    let label = if input.starts_with(':') {
        let _ = ':'.parse_next(input)?;
        skip_ws_and_comments(input)?;
        let l = rest_of_line.parse_next(input)?.trim().to_string();
        let _ = consume_line(input)?;
        Some(l)
    } else {
        None
    };

    Ok(Relation {
        source,
        target,
        kind,
        cardinality_first,
        cardinality_second,
        label,
    })
}

/// 关系类型符号。
fn relation_kind<'i>(input: &mut &'i str) -> PResult<'i, RelationKind> {
    alt((
        "<|--".map(|_| RelationKind::Inheritance),
        "--|>".map(|_| RelationKind::Inheritance),
        "--*>".map(|_| RelationKind::Composition),
        "*--".map(|_| RelationKind::Composition),
        "o--".map(|_| RelationKind::Aggregation),
        "--o".map(|_| RelationKind::Aggregation),
        "-->".map(|_| RelationKind::Association),
        "..>".map(|_| RelationKind::Dependency),
        "<..".map(|_| RelationKind::Dependency),
    ))
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> ClassDiagram {
        let mut stream: &str = input;
        let d = class_diagram.parse_next(&mut stream).unwrap();
        assert!(stream.is_empty(), "trailing input: {:?}", stream);
        d
    }

    #[test]
    fn simple_class() {
        let d = parse("classDiagram\nclass Animal\nclass Dog");
        assert_eq!(d.classes.len(), 2);
        assert_eq!(d.classes[0].name, "Animal");
    }

    #[test]
    fn class_with_members() {
        let d = parse("classDiagram\nclass Animal {\n+String name\n+eat()\n#int age: 5\n}");
        assert_eq!(d.classes.len(), 1);
        let members = &d.classes[0].members;
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].visibility, Some(Visibility::Public));
        assert!(members[1].is_method); // eat() 解析：名字 eat，含括号，是方法
        assert_eq!(members[1].name, "eat");
        assert_eq!(members[2].type_.as_deref(), Some("5"));
    }

    #[test]
    fn annotated_class() {
        let d = parse("classDiagram\nclass Animal <<Interface>>");
        assert_eq!(d.classes[0].annotation.as_deref(), Some("Interface"));
    }

    #[test]
    fn relations() {
        let d = parse("classDiagram\nDog --|> Animal\nCar *-- Wheel : has");
        assert_eq!(d.relations.len(), 2);
        assert_eq!(d.relations[0].kind, RelationKind::Inheritance);
        assert_eq!(d.relations[0].source, "Dog");
        assert_eq!(d.relations[0].target, "Animal");
        assert_eq!(d.relations[1].kind, RelationKind::Composition);
        assert_eq!(d.relations[1].label.as_deref(), Some("has"));
    }
}
