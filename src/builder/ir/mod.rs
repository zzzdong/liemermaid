//! 布局与渲染管线的三层中间表示（IR）。
//!
//! 三层 IR 各司其职、单向流动，是重构后的中枢：
//!
//! - [`Unigraph`] (UG)：语义拓扑，[`crate::ast`] 的唯一出口，不含颜色。
//! - [`Geograph`] (GG)：布局几何，solver 产物，坐标已定、尺寸已测、边已路由，不含颜色。
//! - [`SceneGraph`] (SG)：视觉自足 IR，materialize 产物，颜色/线型/字体已解析，与 AST 解耦。
//!
//! 设计详见 `docs/redesign-from-scratch.md` 与 `docs/redesign-task-plan.md`。

pub mod common;
pub mod geograph;
pub mod scenegraph;
pub mod shape;
pub mod unigraph;

pub use common::*;
pub use geograph::*;
pub use scenegraph::*;
pub use shape::*;
pub use unigraph::*;
