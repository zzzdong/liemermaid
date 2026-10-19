//! Flowchart 形状与箭头：覆盖全部节点形状与连线端点类型。
//!
//! 运行：`cargo run --example flowchart_shapes`
//! 产物：`examples/out/flowchart_shapes.svg`、`examples/out/flowchart_arrows.svg`

#[path = "support/mod.rs"]
mod support;

const SHAPES: &str = r#"flowchart LR
    A[Rectangle] --- B(Rounded)
    B --- C([Stadium])
    C --- D[[Subroutine]]
    D --- E[(Cylinder)]
    E --- F((Circle))
    F --- G(((DoubleCircle)))
    G --- H{Diamond}
    H --- I{{Hexagon}}
    I --- J>Asymmetric]
    J --- K[/Parallelogram/]
    K --- L[\ParallelogramAlt\]
    L --- M[/Trapezoid\]
    M --- N[\TrapezoidAlt/]
"#;

const ARROWS: &str = r#"flowchart LR
    A --> B
    C -.-> D
    E ==> F
    G --- H
    I --o J
    K --x L
    M <--> N
    O ~~~ P
    Q o--o R
    S x--x T
"#;

fn main() {
    support::svg("flowchart_shapes", SHAPES, 1200, 500);
    support::svg("flowchart_arrows", ARROWS, 1200, 500);
}
