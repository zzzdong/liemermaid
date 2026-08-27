//! 形状与连线端点的枚举定义 —— **全项目唯一真相源**。
//!
//! 三处共用：
//! - `extract`：AST 形状 → [`ShapeKind`]
//! - `layout`：根据 [`ShapeKind`] 算端口 / 尺寸
//! - `paint`：[`ShapeKind`] → [`lievisual::scene::Element`] 变体
//!
//! state 的 `__start__/__end__` 特判在此消失，变为 `StartDot` / `EndDot`。

use lievisual::geometry::{Point, Size};

/// 节点几何形状类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShapeKind {
    #[default]
    Rectangle,
    Rounded,
    Stadium,
    Subroutine,
    Diamond,
    Hexagon,
    Circle,
    DoubleCircle,
    Cylinder,
    Asymmetric,
    Parallelogram,
    Trapezoid,
    Bar,
    /// state 起始节点（实心圆点）。
    StartDot,
    /// state 终止节点（双环圆）。
    EndDot,
    /// pie 扇区。
    PieSlice,
    /// quadrant 单元格。
    QuadrantCell,
}

/// 形状几何描述（布局求解后写入 GG，paint 直接消费）。
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeGeometry {
    Rect { at: Point, size: Size },
    RoundedRect { at: Point, size: Size, radius: f64 },
    Stadium { at: Point, size: Size },
    Diamond { center: Point, size: Size },
    Ellipse { center: Point, rx: f64, ry: f64 },
    Polygon { points: Vec<Point> },
    Path { ops: Vec<PathOp> },
    Pie { center: Point, radius: f64, start_angle: f64, end_angle: f64 },
}

/// 简单路径操作（圆柱等复杂形状的几何描述）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathOp {
    MoveTo(Point),
    LineTo(Point),
    ArcTo(Point, f64),
    CurveTo(Point, Point, Point),
}

/// 连线两端标记（已解析的枚举），paint 查表生成起止标记原语。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeEnds {
    #[default]
    None,
    Arrow,
    Circle,
    Cross,
    Both,
    MultiCircle,
    MultiCross,
}
