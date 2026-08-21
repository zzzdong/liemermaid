# Winnow 1.0.4 API 参考

本文档总结本项目中使用的 winnow 1.0.4 核心 API，避免反复查阅源码。

## 核心概念

- **输入流**：`&str`（complete stream），`Partial<&str>`（partial stream）
- **错误类型**：`ContextError`（默认）、`InputError<I>`（测试用）
- **结果类型**：`ModalResult<O, E>` = `Result<O, ErrMode<E>>`
- **Parser trait**：`fn parse_next(&mut self, input: &mut I) -> Result<O, E>`

## 基本用法

```rust
use winnow::{Parser, prelude::*};
use winnow::error::InputError;

type PResult<'i, O> = Result<O, InputError<&'i str>>;

// 字面量匹配（消耗输入）
let s: &str = "hello".parse_next(input)?;

// peek：不消耗输入
let s: &str = peek("hello").parse_next(input)?;
```

## Token 模块 (`winnow::token`)

### `take_while(range, predicate)`

```rust
// 匹配 1 到多个满足条件的字符
take_while(1.., |c: char| c.is_alphanumeric())
// 匹配 0 到多个
take_while(0.., |c: char| c == ' ')
```

- `range` 实现 `Into<Range>`，支持：`usize`, `Range<usize>`, `RangeFrom<usize>`, `RangeTo<usize>`, `RangeInclusive<usize>`, `RangeFull`
- 返回 `&str`（Slice）

### `take_until(range, literal)`

```rust
// 匹配直到遇到 literal（不含），至少匹配 1 个字符
take_until(1.., '\n')
take_until(1.., '"')
```

- `literal` 可以是 `char`、`&str`、`[char; N]` 等（实现 `FindSlice`）
- 返回 `&str`（不含终止符的部分）

### `one_of(set)`

```rust
one_of(['a', 'b', 'c'])  // 匹配集合中任一字符
one_of([b'a', b'b'])     // 字节
```

- 返回 `char`（对 `&str` 输入）

### `any`

```rust
any.parse_next(input)?  // 匹配任意一个字符，返回 char
```

## Combinator 模块 (`winnow::combinator`)

### `alt((a, b, c))`

按顺序尝试，返回第一个成功的结果。

```rust
alt(("hello", "world")).parse_next(input)?
```

### `opt(parser)`

```rust
opt("optional").parse_next(input)?  // Option<&str>
```

### `delimited(before, parser, after)`

```rust
delimited("[", text, "]").parse_next(input)?
```

### `terminated(parser, after)`

```rust
terminated(identifier, ws).parse_next(input)?
```

### `preceded(before, parser)`

```rust
preceded("fn ", identifier).parse_next(input)?
```

### `repeat(range, parser)`

```rust
repeat(0.., parser)       // Vec<T>
repeat(1.., parser)       // Vec<T>（至少一个）
repeat(3..5, parser)      // Vec<T>（3到5个）
```

### `peek(parser)`

```rust
peek("end").parse_next(input)?  // 不消耗输入
```

### `many0` / `many1`（已废弃，用 `repeat` 替代）

## ASCII 模块 (`winnow::ascii`)

### `multispace0` / `multispace1`

```rust
multispace0.parse_next(input)?  // 匹配 0+ 空白字符，返回 &str
multispace1.parse_next(input)?  // 匹配 1+ 空白字符，返回 &str
```

## 错误处理

### `InputError<I>`

```rust
use winnow::error::InputError;
type PResult<'i, O> = Result<O, InputError<&'i str>>;
```

- 仅记录输入位置（用于测试）
- 实现 `ParserError<I>`

### `ContextError`

```rust
use winnow::error::ContextError;
// 默认错误类型，包含上下文信息
```

### `ErrMode`

- `ErrMode::Backtrack(e)`：可回退的错误（alt 中使用）
- `ErrMode::Cut(e)`：不可回退的错误（`cut_err()` 标记）

## 字面量作为 Parser

`&str`、`char`、`[u8; N]` 等实现了 `Parser` trait：

```rust
"hello".parse_next(input)?       // 匹配 "hello"，返回 &str
'a'.parse_next(input)?           // 匹配 'a'，返回 char
(b"hi",).parse_next(input)?      // 元组匹配
```

## Map 和 Verify

```rust
// map：转换输出
"hello".map(|s: &str| s.to_string())

// verify：验证条件（失败则回退）
take_while(1.., is_alpha).verify(|s: &str| s.len() > 2)
```

## 本项目中的模式

### 标准 PResult 类型

```rust
use winnow::error::InputError;
pub type PResult<'i, O> = Result<O, InputError<&'i str>>;
```

### 手写循环（替代 repeat 的类型推断问题）

```rust
fn skip_ws<'i>(input: &mut &'i str) -> PResult<'i, ()> {
    loop {
        let mut advanced = false;
        if ws1(input).is_ok() { advanced = true; }
        if comment(input).is_ok() { advanced = true; }
        if !advanced { break; }
    }
    Ok(())
}
```

### 条件解析（peek + 分支）

```rust
fn statement<'i>(input: &mut &'i str) -> PResult<'i, Stmt> {
    if peek("subgraph").parse_next(input).is_ok() {
        subgraph(input)
    } else if peek("end").parse_next(input).is_ok() {
        // handle end
    } else {
        // default
    }
}
```

## 注意事项

1. **`take_until` 的 range 参数**：`1..` 表示至少匹配 1 个字符（不含终止符）；`0..` 允许空匹配
2. **`alt` 中的错误**：自动使用 `Backtrack` 模式，失败会回退输入位置
3. **字面量 `&str`**：匹配成功返回 `&str`（与输入相同切片）
4. **`char` 作为 literal**：匹配成功返回 `char`
5. **`one_of` 返回 `char`**：对 `&str` 输入
6. **生命周期**：所有解析器函数签名 `fn parse<'i>(input: &mut &'i str) -> PResult<'i, T>`
