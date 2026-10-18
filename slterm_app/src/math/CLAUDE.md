# 数学引擎

## 决策与红线

- **后端无关**:十模块不得依赖窗口、GPUI、主题或终端类型;同一份布局供
  终端覆盖层与 markdown 阅读器复用。呈现接线归 M9.5。
- **单点编译**:`compile_formula` / `compile_formula_source` 是唯一排版入口;
  光学补偿(`OPTICAL_SCALE`)只在 `compile_formula` 生效。缓存键、fit、draw
  全持名义字号,任何管线不得独立缩放或重排。
- **原始源码不重写**:`compile_formula_source` 不做实体解码/CRLF 归一/
  行断猜测;只有传输路径 `normalize_formula_source` 允许有限修复。
- **预算不可放宽**:`DEFAULT_LIMITS` 八项(16KB/64/8192/4096/1024/1024/
  8192)、glyph 512px 维度上限、coverage gamma 0.75、位图 8192px/24MB、
  `LAYOUT_CACHE_BUDGET` 4MB。改动契约数值须走设计评审。
- **黑名单先于宏展开**:`FORBIDDEN_COMMANDS` 在 pulldown-latex 之前线性扫描;
  未知命令保持解析失败,不得静默替换。
- **字体资产单点**:`FONT_BYTES` 是唯一内嵌字体;换字体只改 `font.rs` 与
  SHA-256 基线。

## 步骤接缝

- M9.5 消费面:`compile_formula`、`pixels_per_point`、`MathLayoutCache`、
  `MathFont`、`MIN_READABLE_MATH_PX`(终端覆盖层 / markdown 数学共用)。
