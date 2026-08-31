# Changelog

本项目遵循 [Semantic Versioning](https://semver.org/)。

## [0.2.0-beta.1] - 2026-08-31

首个 0.2 版本。主版本号提升对应一次**非向后兼容**的序列块数据模型调整
（`SequenceBlock.items` → `branches`），以及许可 / CLI / 站点等工程面变更。

### Added

- **sequenceDiagram：`alt` / `par` / `critical` 多分支**。此前 `else` / `and` /
  `option` 分隔出的后续分支被静默丢弃，只渲染首分支。现在：
  - parser 识别 `else` / `and` / `option` 分隔行并拆分为多个分支段（`SequenceBranch`）；
  - 渲染在分支交界处画出横贯块宽的虚线分隔线 + 块内居中的分支标题
    （`[else 条件]`，与官方 `<text class="sectionTitle">` 对齐）。
  - `loop` / `opt` / `break` / `rect` 内不识别分隔符，维持单分支旧行为。
- 新增 golden 用例 `sequence__alt_branches`（与官方 mermaid-cli 对拍）。

### Fixed

- **无标签分组块吞掉下一行语句**：`loop` / `alt` 等关键字后若直接换行，下一行
  消息会被并进块标签（渲染出 `[A->>B: x]` 这种伪标签）。块标签现只取本行，
  与语句内空白语义一致。

### Changed

- **CLI 二进制更名为 `liemermaid-cli`**，与 `liemermaid` 库名区分（此前两者同名，
  `cargo install` 后二进制与库易混淆）。clap 的命令名同步更新。
- **许可改为 MIT OR Apache-2.0 双许可**：`LICENSE` 重命名为 `LICENSE-APACHE` 并
  填写版权行，新增 `LICENSE-MIT`；`Cargo.toml` 的 `license` 字段更新为 SPDX 表达式。
- README 重写为英文精简版，补充在线演示入口。
- 升级依赖 `lievisual` 至 `=0.2.0-beta.2`（API 兼容，golden 基线测试全部通过）。
- **破坏性变更**：`ast::SequenceBlock.items` 替换为 `branches`（分支段列表），
  `serde` 反序列化使用 `#[serde(default)]` 兼容旧 JSON。

### Fixed

- **site: 补齐缺失的 `index.html`**（GitHub Pages 部署工作流会拷贝该文件，此前缺失
  导致部署必失败）。新增品牌页头、8 种图表下拉、Docs/GitHub 导航；`.gitignore`
  放行 `!site/index.html`。
- site: 编辑器防抖实时预览（停止输入 700ms 后自动渲染）、PNG 预览 Blob URL 泄漏、
  Monaco 与 WASM 初始化竞态兜底。

## [0.1.0] - 2026-08-30

首个发布版本。支持 flowchart / sequence / class / state / er / pie / gitgraph /
timeline 八种 Mermaid 图表，输出统一走 lievisual 的 `Scene` IR 与
SVG / vello_cpu PNG 双后端。

**注意**：`0.1.0` 属于初期版本，与官方 mermaid 的输出尚未做到像素级一致
（见下「已知限制」），后续 minor 版本会在保持公开 API 稳定的前提下持续收敛。

### Fixed

- **parser: 标识符不再吞掉 `-`**（`src/parser/common.rs`）
  此前 `identifier` 把 `A-->B` 切成 `A--` + `>B`，箭头匹配失败、整行被丢弃，
  导致 `A-->B` 这类无空格写法渲染出**空白画布**。现在遇到 `-` 会前瞻其后字符，
  若构成箭头起始（`--` / `-.` / `->`）即截断；`node-1` 这类带连字符的 id 不受影响。

- **parser: 择优分支失败时回滚输入**（新增 `common::attempt`）
  winnow 的 `parse_next` 失败时不保证回滚 `input`，一次失败的尝试会留下部分消费，
  随后兜底的 `skip_line` 把**已推进到的那一行**整行丢掉。后果包括：
  `sequenceDiagram` 里的 `autonumber` 会吞掉下一行的消息；
  某些输入在 EOF 处让整图解析直接失败（如 `critical ... end`）。
  全部图表的主循环改用 `attempt`，失败即完整回滚。

- **parser: `skip_line` 在 EOF 处不再报错**，避免语句行在文件末尾触发整图解析失败。

- **sequence: 消息语句内部不再跨行**
  语句内空白从 `skip_ws_and_comments` 改为新增的 `inline_ws_and_comments`，
  杜绝一条语句吞噬下一行。

- **flowchart: 支持链式链接与端点形状**
  `A --> B --> C` 曾只解析出第一条边；`A[Start] --> B[End]` 的端点形状会丢失标签。
  现在边解析器在节点声明之前尝试，端点可自带形状并登记为节点。

- **flowchart: 支持 `&` 多端点**
  `A & B --> C` / `A --> B & C` 展开为多条边（此前整行被丢弃）。

- **class: 补齐关系符号**
  新增 `..|>` / `<|..`（实现）、`..`（虚线连接）、`--`（实线连接）。
  对应 `ast::RelationKind` 增加 `Realization` / `Link` / `Dashed` 三个变体。

- **sequence: 补齐块类型与箭头**
  新增 `critical` / `break` / `rect` 分组块（`SequenceBlockKind` 新增对应变体），
  以及 `-)` / `--)` 开放箭头。`end` 关键字的判定改为要求后随空白/分号/EOF，
  避免 `endpoint` 这类参与者名被误判为块结束。

- **er: 支持 `as` 别名**
  `CUSTOMER as C ||--o{ ORDER as O : places` 现在以别名作为实体名。

- **构建: `serde` 可选依赖缺少 feature 门控**
  `ast.rs` 无条件 `use serde` 并在 42 处类型上派生 `Serialize`/`Deserialize`，
  而 `serde` 声明为 `optional`。因此 `default-features = false` 时**编译直接失败**
  （4 个 `unresolved import`）。已全部改为 `#[cfg_attr(feature = "serde", ...)]`，
  并验证 `default` / `--no-default-features` / `--all-features` 三种组合均可编译。

- **builder: 画布尺寸不再下溢**
  配置尺寸小于 2×边距时 `scale` 会变负，产出 `viewBox="0 0 0 -20.41"` 这类负尺寸画布。
  现在可用区域下限取 1pt、`scale` 下限取最小正数，并对 NaN / Inf / 负数配置回退到默认画布。

### Added

- **parser: 标识符支持 Unicode**
  `identifier` 从「仅 ASCII」放宽为「Unicode 字母/数字/下划线」，中文等标识符
  （如 `开始[开始] --> 结束[结束]`）现在可正常解析。
  sequence 的 `seq_id` 同步放宽但**仍不含连字符**，以免吞掉 `-x` 箭头。

- **测试: 语义层对比重新接入**
  `tests/golden/semantics.rs` 此前是孤儿模块（没有任何测试引用它），
  `official_compare_test.rs` 的注释却声称会做语义比对。现已接回并参与报告，
  其中 4 个此前从未执行的单元测试也一并启用。

- `tests/syntax_coverage_test.rs`：语法覆盖与回归测试（19 个用例），
  锁定上述每一个缺陷，并覆盖病态输入不 panic、退化画布尺寸为正。

### Changed

- `cargo fmt` 全仓库格式化（此前仓库并非 rustfmt 干净状态，纯排版变更、无语义影响）。
- 补齐 crate 级文档（管线与画布语义）并重写 README 的架构与限制章节。
- 清理全部 `cargo clippy --all-targets` 与 `cargo doc` 警告。
