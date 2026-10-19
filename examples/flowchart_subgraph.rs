//! Flowchart 子图：嵌套容器与子图内 `direction`。
//!
//! 运行：`cargo run --example flowchart_subgraph`
//! 产物：`examples/out/flowchart_subgraph.svg`

#[path = "support/mod.rs"]
mod support;

const DIAGRAM: &str = r#"flowchart TB
    Client[Client] --> API[API Gateway]

    subgraph backend
        direction LR
        API --> Service[Order Service]

        subgraph storage
            direction TD
            Primary[(Primary DB)] --> Replica[(Replica DB)]
        end

        Service --> Primary
    end

    Service --> Queue[[Message Queue]]
"#;

fn main() {
    support::svg("flowchart_subgraph", DIAGRAM, 1100, 700);
}
