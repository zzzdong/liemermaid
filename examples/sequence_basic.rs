//! Sequence 基础：参与者、别名、消息、备注与自动编号。
//!
//! 运行：`cargo run --example sequence_basic`
//! 产物：`examples/out/sequence_basic.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"sequenceDiagram
    autonumber
    participant C as Client
    participant S as Server
    participant D as Database

    Note over C,S: TLS 握手完成
    C->>S: GET /api/users
    S->>D: SELECT * FROM users
    D-->>S: rows
    Note right of S: 组装响应
    S-->>C: 200 OK
"#;

fn main() {
    support::svg("sequence_basic", DIAGRAM, 900, 600);
}
