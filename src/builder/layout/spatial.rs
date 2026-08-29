//! 碰撞检测原语：线段-线段 / 线段-矩形相交判定。
//!
//! 被 `layout/route.rs` 的边路由用作「边-节点/边-容器」避让的几何判断。
//! （原 `SpatialGrid` 网格哈希空间索引已被移除 —— 仅被自身测试引用，属死代码。）

use lievisual::geometry::{Point, Rect};

/// 线段是否与矩形相交（含端点在外、线段穿越矩形）。
pub fn segment_intersects_rect(a: Point, b: Point, rect: &Rect) -> bool {
    // 端点在内
    if rect.contains(a) || rect.contains(b) {
        return true;
    }
    // 四条边
    let corners = [
        Point::new(rect.min_x(), rect.min_y()),
        Point::new(rect.max_x(), rect.min_y()),
        Point::new(rect.max_x(), rect.max_y()),
        Point::new(rect.min_x(), rect.max_y()),
    ];
    for i in 0..4 {
        let c0 = corners[i];
        let c1 = corners[(i + 1) % 4];
        if segment_intersects_segment(a, b, c0, c1) {
            return true;
        }
    }
    false
}

/// 共线 / 端点接触判定的数值容差（与 `route.rs` 等处的 `1e-9` 对齐）。
const EPS: f64 = 1e-9;

/// 两线段是否相交（含端点接触）。
///
/// 标准「跨立实验 + 共线重叠」：
/// - 严格异号 → 内部相交；
/// - 某端点叉积为 0（共线 / 端点接触）→ 用 bbox 重叠判断是否真正接触，
///   避免「共线但不重叠」的假阳性（旧实现 `d1*d2<=0 && d3*d4<=0` 会把
///   共线且分离的两线段误判为相交）。
pub fn segment_intersects_segment(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    let d1 = cross(a0, a1, b0);
    let d2 = cross(a0, a1, b1);
    let d3 = cross(b0, b1, a0);
    let d4 = cross(b0, b1, a1);

    // 严格异号 = 真正内部相交。
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    // 端点接触 / 共线重叠：点在对方线段上才成立。
    (d1.abs() < EPS && point_on_segment(a0, a1, b0))
        || (d2.abs() < EPS && point_on_segment(a0, a1, b1))
        || (d3.abs() < EPS && point_on_segment(b0, b1, a0))
        || (d4.abs() < EPS && point_on_segment(b0, b1, a1))
}

/// 点 `p` 是否落在以 `a`/`b` 为端点的线段（含 bbox 容差）上。
fn point_on_segment(a: Point, b: Point, p: Point) -> bool {
    p.x >= a.x.min(b.x) - EPS
        && p.x <= a.x.max(b.x) + EPS
        && p.y >= a.y.min(b.y) - EPS
        && p.y <= a.y.max(b.y) + EPS
}

/// 叉积 (b-a) × (c-a) 的 z 分量（2D 标量）。
#[inline]
fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_hits_rect_when_crossing() {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(segment_intersects_rect(Point::new(-5.0, 5.0), Point::new(15.0, 5.0), &rect));
        assert!(segment_intersects_rect(Point::new(5.0, -5.0), Point::new(5.0, 15.0), &rect));
        assert!(!segment_intersects_rect(Point::new(-5.0, -5.0), Point::new(-1.0, -1.0), &rect));
    }

    #[test]
    fn segments_intersect_basic() {
        let a0 = Point::new(0.0, 0.0);
        let a1 = Point::new(10.0, 10.0);
        let b0 = Point::new(0.0, 10.0);
        let b1 = Point::new(10.0, 0.0);
        assert!(segment_intersects_segment(a0, a1, b0, b1));
        // 真正分离的两条线段（不共享端点、不交叉）
        let c0 = Point::new(20.0, 20.0);
        let c1 = Point::new(20.0, 30.0);
        assert!(!segment_intersects_segment(a0, a1, c0, c1));
    }

    #[test]
    fn collinear_segments_only_intersect_when_overlapping() {
        // 共线但分离：旧实现会误判为相交（假阳性）。
        let a0 = Point::new(0.0, 0.0);
        let a1 = Point::new(10.0, 0.0);
        let b0 = Point::new(20.0, 0.0);
        let b1 = Point::new(30.0, 0.0);
        assert!(!segment_intersects_segment(a0, a1, b0, b1));

        // 共线且重叠：应判为相交。
        let c0 = Point::new(5.0, 0.0);
        let c1 = Point::new(15.0, 0.0);
        assert!(segment_intersects_segment(a0, a1, c0, c1));

        // 端点接触（T 形）：应判为相交。
        let d0 = Point::new(10.0, 0.0);
        let d1 = Point::new(10.0, 10.0);
        assert!(segment_intersects_segment(a0, a1, d0, d1));
    }
}
