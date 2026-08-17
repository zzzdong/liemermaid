use lievisual::geometry::Point;

use crate::ast::Direction;

/// 逻辑坐标：所有布局计算使用主轴/交叉轴，不区分 TD/LR/BT/RL
#[derive(Debug, Clone, Copy)]
pub struct LayoutCoord {
    pub main: f64,
    pub cross: f64,
}

impl LayoutCoord {
    pub fn new(main: f64, cross: f64) -> Self {
        Self { main, cross }
    }

    /// 转换为画布绝对坐标 (x, y)
    pub fn to_canvas(&self, direction: Direction, canvas_size: (f64, f64)) -> Point {
        match direction {
            Direction::TD | Direction::TB => Point::new(self.cross, self.main),
            Direction::BT => Point::new(self.cross, canvas_size.1 - self.main),
            Direction::LR => Point::new(self.main, self.cross),
            Direction::RL => Point::new(canvas_size.0 - self.main, self.cross),
        }
    }
}

/// 方向相关的坐标操作闭包集合
#[derive(Clone)]
pub struct AxisAccess {
    pub main: fn(Point) -> f64,
    pub cross: fn(Point) -> f64,
    pub set_main: fn(&mut Point, f64),
    pub set_cross: fn(&mut Point, f64),
}

impl AxisAccess {
    pub fn from_direction(direction: Direction) -> Self {
        match direction {
            Direction::TD | Direction::TB => AxisAccess {
                main: |p: Point| p.y,
                cross: |p: Point| p.x,
                set_main: |p: &mut Point, v: f64| p.y = v,
                set_cross: |p: &mut Point, v: f64| p.x = v,
            },
            Direction::LR => AxisAccess {
                main: |p: Point| p.x,
                cross: |p: Point| p.y,
                set_main: |p: &mut Point, v: f64| p.x = v,
                set_cross: |p: &mut Point, v: f64| p.y = v,
            },
            Direction::BT => AxisAccess {
                main: |p: Point| -p.y,
                cross: |p: Point| p.x,
                set_main: |p: &mut Point, v: f64| p.y = -v,
                set_cross: |p: &mut Point, v: f64| p.x = v,
            },
            Direction::RL => AxisAccess {
                main: |p: Point| -p.x,
                cross: |p: Point| p.y,
                set_main: |p: &mut Point, v: f64| p.x = -v,
                set_cross: |p: &mut Point, v: f64| p.y = v,
            },
        }
    }
}

/// 锚点方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorDir {
    Top,
    Bottom,
    Left,
    Right,
}

/// 节点的四个锚点（相对于 center 的偏移）
#[derive(Debug, Clone)]
pub struct NodeAnchors {
    pub top: Point,
    pub bottom: Point,
    pub left: Point,
    pub right: Point,
}

impl NodeAnchors {
    pub fn new(size: (f64, f64)) -> Self {
        let (w, h) = size;
        Self {
            top: Point::new(0.0, -h / 2.0),
            bottom: Point::new(0.0, h / 2.0),
            left: Point::new(-w / 2.0, 0.0),
            right: Point::new(w / 2.0, 0.0),
        }
    }

    pub fn get(&self, dir: AnchorDir) -> Point {
        match dir {
            AnchorDir::Top => self.top,
            AnchorDir::Bottom => self.bottom,
            AnchorDir::Left => self.left,
            AnchorDir::Right => self.right,
        }
    }
}
