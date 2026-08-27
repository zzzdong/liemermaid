//! 空间索引（网格哈希），用于边路由时的「节点/边」邻近查询，
//! 将朴素的 O(E²) 全量相交检测降到接近 O(E)（仅查相邻网格）。
//!
//! 评审建议：原 `RouteOptimizer` 风格的边-边排斥若用 O(E²) 两两检测，图规模大时退化；
//! 以均匀网格哈希（cell 边长 = 典型节点尺寸量级）做空间分桶，插入/查询均为近似常数。

use std::collections::HashMap;

use lievisual::geometry::{Point, Rect};

/// 网格哈希：把线段 / 矩形按其所覆盖的 cell 登记，查询时只回传同 cell 及相邻 cell 内的候选。
pub struct SpatialGrid {
    cell: f64,
    /// key = "cx,cy"（cell 坐标） -> 该 cell 内的元素 id 列表
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl SpatialGrid {
    /// 新建网格，cell 边长建议取典型节点尺寸（如 80.0）。
    pub fn new(cell: f64) -> Self {
        Self {
            cell: cell.max(1.0),
            cells: HashMap::new(),
        }
    }

    fn cell_coords(&self, x: f64, y: f64) -> (i64, i64) {
        (
            (x / self.cell).floor() as i64,
            (y / self.cell).floor() as i64,
        )
    }

    /// 插入一个元素（id），登记其覆盖的矩形所横跨的所有 cell。
    pub fn insert_rect(&mut self, id: usize, rect: &Rect) {
        let (x0, y0) = (rect.min_x(), rect.min_y());
        let (x1, y1) = (rect.max_x(), rect.max_y());
        let (cx0, cy0) = self.cell_coords(x0, y0);
        let (cx1, cy1) = self.cell_coords(x1, y1);
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                self.cells.entry((cx, cy)).or_default().push(id);
            }
        }
    }

    /// 插入一个元素（id），登记其线段两端点横跨的所有 cell。
    pub fn insert_segment(&mut self, id: usize, a: Point, b: Point) {
        let (ax, ay) = (a.x, a.y);
        let (bx, by) = (b.x, b.y);
        let (cx0, cy0) = self.cell_coords(ax.min(bx), ay.min(by));
        let (cx1, cy1) = self.cell_coords(ax.max(bx), ay.max(by));
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                self.cells.entry((cx, cy)).or_default().push(id);
            }
        }
    }

    /// 查询与给定矩形可能相交的候选元素 id（去重）。
    pub fn query_rect(&self, rect: &Rect) -> Vec<usize> {
        let (x0, y0) = (rect.min_x(), rect.min_y());
        let (x1, y1) = (rect.max_x(), rect.max_y());
        let (cx0, cy0) = self.cell_coords(x0, y0);
        let (cx1, cy1) = self.cell_coords(x1, y1);
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                if let Some(ids) = self.cells.get(&(cx, cy)) {
                    for &id in ids {
                        if seen.insert(id) {
                            out.push(id);
                        }
                    }
                }
            }
        }
        out
    }

    /// 查询与给定线段可能相交的候选元素 id（去重）。
    pub fn query_segment(&self, a: Point, b: Point) -> Vec<usize> {
        let (ax, ay) = (a.x, a.y);
        let (bx, by) = (b.x, b.y);
        let (cx0, cy0) = self.cell_coords(ax.min(bx), ay.min(by));
        let (cx1, cy1) = self.cell_coords(ax.max(bx), ay.max(by));
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                if let Some(ids) = self.cells.get(&(cx, cy)) {
                    for &id in ids {
                        if seen.insert(id) {
                            out.push(id);
                        }
                    }
                }
            }
        }
        out
    }
}

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

/// 两线段是否相交（含端点接触）。标准跨立实验。
pub fn segment_intersects_segment(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    let d1 = cross(a0, a1, b0);
    let d2 = cross(a0, a1, b1);
    let d3 = cross(b0, b1, a0);
    let d4 = cross(b0, b1, a1);
    // 跨立：两端点在另一侧（异号或零接触）
    (d1 * d2 <= 0.0) && (d3 * d4 <= 0.0)
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
    fn grid_query_returns_only_near_cells() {
        let mut g = SpatialGrid::new(100.0);
        // 元素 0 覆盖 (10,10)-(30,30)，元素 1 覆盖 (500,500)-(520,520)
        g.insert_rect(0, &Rect::new(10.0, 10.0, 30.0, 30.0));
        g.insert_rect(1, &Rect::new(500.0, 500.0, 520.0, 520.0));
        // 查询靠近元素0的区域，不应含元素1
        let near = g.query_rect(&Rect::new(0.0, 0.0, 50.0, 50.0));
        assert!(near.contains(&0));
        assert!(!near.contains(&1));
        // 查询靠近元素1的区域，不应含元素0
        let near1 = g.query_rect(&Rect::new(480.0, 480.0, 540.0, 540.0));
        assert!(near1.contains(&1));
        assert!(!near1.contains(&0));
    }

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
}
