//! class 迁移（新管线）回归测试：验证 class 图走 extract → measure → engine
//! 产出几何正确的 Geograph（family=Grid 的 BFS 分层 + 类框多栏尺寸）。
//!
//! 不比对像素，只断言拓扑 / 分层 / 尺寸语义。

use liemermaid::ast::{Class, ClassDiagram, ClassMember, Relation, RelationKind, Visibility};
use liemermaid::builder::extract::class::extract_class;
use liemermaid::builder::ir::scenegraph::{SceneItem, StyleIntent};
use liemermaid::builder::ir::shape::EdgeEnds;
use liemermaid::builder::{layout, materialize, measure, paint};

fn run_gg(cd: &ClassDiagram) -> liemermaid::builder::ir::geograph::Geograph {
    let ug = extract_class(cd);
    let ug = measure::measure_all(ug);
    layout::engine::run(&ug).expect("layout").0
}

fn member(visibility: Option<Visibility>, name: &str, type_: Option<&str>, is_method: bool) -> ClassMember {
    ClassMember {
        visibility,
        name: name.to_string(),
        type_: type_.map(str::to_string),
        is_method,
    }
}

#[test]
fn class_grid_layers_parent_above_child() {
    let cd = ClassDiagram {
        classes: vec![
            Class {
                name: "Animal".into(),
                generic: None,
                annotation: None,
                members: vec![member(Some(Visibility::Protected), "age", Some("int"), false)],
            },
            Class {
                name: "Dog".into(),
                generic: None,
                annotation: None,
                members: vec![member(Some(Visibility::Public), "bark", Some("void"), true)],
            },
        ],
        relations: vec![Relation {
            source: "Animal".into(),
            target: "Dog".into(),
            kind: RelationKind::Inheritance,
            cardinality_first: None,
            cardinality_second: None,
            label: None,
        }],
    };

    let gg = run_gg(&cd);
    assert_eq!(gg.nodes.len(), 2, "应有两个类节点");
    assert_eq!(gg.edges.len(), 1, "应有一条继承关系");

    let animal = gg.nodes.iter().find(|n| n.id == "Animal").unwrap();
    let dog = gg.nodes.iter().find(|n| n.id == "Dog").unwrap();
    // TB 布局：父类（入度 0，第 0 层）应在子类上方。
    assert!(
        animal.center.y < dog.center.y,
        "父类 Animal 应在子类 Dog 上方: animal.y={} dog.y={}",
        animal.center.y,
        dog.center.y
    );

    // 类框三栏尺寸：含属性/方法的类框高度应显著大于单标签节点（约 40px）。
    assert!(
        animal.size.height > 60.0,
        "类框（含属性+方法）高度应 > 60: {}",
        animal.size.height
    );
}

#[test]
fn class_detail_survives_measure_into_gg() {
    let cd = ClassDiagram {
        classes: vec![Class {
            name: "Interface".into(),
            generic: None,
            annotation: Some("Interface".into()),
            members: vec![member(Some(Visibility::Public), "run", None, true)],
        }],
        relations: vec![],
    };
    let gg = run_gg(&cd);
    let n = &gg.nodes[0];
    match &n.detail {
        liemermaid::builder::ir::common::NodeDetail::Class { annotation, attrs, methods } => {
            assert_eq!(annotation.as_deref(), Some("Interface"));
            assert!(attrs.is_empty());
            assert_eq!(methods, &["+run()".to_string()]);
        }
        _ => panic!("detail 应为 Class"),
    }
}

#[test]
fn class_box_materializes_and_paints_with_special_ends() {
    let cd = ClassDiagram {
        classes: vec![
            Class {
                name: "Animal".into(),
                generic: None,
                annotation: None,
                members: vec![member(Some(Visibility::Protected), "age", Some("int"), false)],
            },
            Class {
                name: "Dog".into(),
                generic: None,
                annotation: None,
                members: vec![member(Some(Visibility::Public), "bark", Some("void"), true)],
            },
        ],
        relations: vec![Relation {
            source: "Animal".into(),
            target: "Dog".into(),
            kind: RelationKind::Inheritance,
            cardinality_first: None,
            cardinality_second: None,
            label: None,
        }],
    };

    let gg = run_gg(&cd);
    let sg = materialize::run(&gg, &StyleIntent::default());

    // 继承关系应映射为 source 端空心三角、target 端无标记。
    let has_triangle = sg.items.iter().any(|i| {
        matches!(i, SceneItem::Edge { ends, .. } if *ends == (EdgeEnds::Triangle, EdgeEnds::None))
    });
    assert!(has_triangle, "继承关系应生成 (Triangle, None) 端点标记");

    // 类框应产出至少 2 个 Shape（header + body 背景）。
    let shape_count = sg.items.iter().filter(|i| matches!(i, SceneItem::Shape { .. })).count();
    assert!(shape_count >= 4, "两个类框应各产 header+body 共 >= 4 个 Shape: {shape_count}");

    // paint 不 panic 且产出图元。
    let scene = paint::run(&sg);
    assert!(!scene.nodes.is_empty(), "class 图 paint 应产出图元");
}

/// 真实公共 API 端到端：parse → build_diagram（走新管线）。
#[test]
fn class_via_public_api_renders() {
    let src = "classDiagram\n    class Animal\n    class Dog\n    class Cat\n    Animal <|-- Dog\n    Animal <|-- Cat\n";
    let diagram = liemermaid::MermaidParser::parse_mermaid(src).expect("parse");
    let scene = liemermaid::builder::build_diagram(&diagram).expect("build");
    assert!(!scene.nodes.is_empty(), "class 图经公共 API 应产出场景");
}
