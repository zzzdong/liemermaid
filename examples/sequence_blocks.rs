//! Sequence 分组块：激活条、alt 分支、loop 与 par 并行。
//!
//! 运行：`cargo run --example sequence_blocks`
//! 产物：`examples/out/sequence_blocks.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"sequenceDiagram
    participant A as Alice
    participant B as Order Service
    participant C as SMS
    participant D as Email

    A->>+B: 下单
    alt 库存充足
        B-->>-A: 创建成功
        par 并行通知
            B->>C: 发送短信
        and
            B->>D: 发送邮件
        end
    else 库存不足
        B-->>-A: 缺货
    end

    loop 每 30 秒
        A->>B: 查询订单状态
    end
"#;

fn main() {
    support::svg("sequence_blocks", DIAGRAM, 1000, 750);
}
