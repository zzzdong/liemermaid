//! 布局全局配置。

use crate::ast::Direction;

/// 布局配置（全局参数）。
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// 布局方向（TB/TD/BT/RL/LR）。
    pub direction: Direction,
    /// 层内节点水平间距。
    pub node_gap: f64,
    /// 层间垂直间距。
    pub layer_gap: f64,
    /// 子图容器内边距。
    pub group_padding: f64,
    /// Sugiyama 交叉减少迭代次数。
    pub crossing_iterations: usize,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            direction: Direction::TD,
            node_gap: 50.0,
            layer_gap: 60.0,
            group_padding: 16.0,
            crossing_iterations: 12,
        }
    }
}
