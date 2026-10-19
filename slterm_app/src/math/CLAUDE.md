# 数学引擎

> 公式排版与位图生成；同一份布局供终端覆盖层与 markdown 阅读器复用。

## 决策与红线

- **后端无关**：不得依赖窗口、GPUI、主题或终端类型；呈现接线归消费方。原因：同一布局引擎服务多个呈现面。边界：新增呈现需求只消费引擎，不改引擎的 UI 依赖。
- **单点编译**：`compile_formula` / `compile_formula_source` 是唯一排版入口；光学补偿（`OPTICAL_SCALE`）只在 `compile_formula` 生效。缓存键、fit、draw 全持名义字号，任何管线不得独立缩放或重排。
- **原始源码不重写**：`compile_formula_source` 不做实体解码/CRLF 归一/行断猜测；只有传输路径 `normalize_formula_source` 允许有限修复。
- **预算不可放宽**：`DEFAULT_LIMITS` 的 16 KiB、64、8192、4096、1024、1024、8192 等限制、glyph 512px 维度上限、coverage gamma 0.75、位图 8192px/24 MiB、`LAYOUT_CACHE_BUDGET` 4 MiB 均为契约值。原因：不可信公式不得拖垮渲染。边界：改动契约数值须走设计评审。
- **黑名单先于宏展开**：`FORBIDDEN_COMMANDS` 在 pulldown-latex 之前线性扫描；未知命令保持解析失败，不得静默替换。
- **字体资产单点**：`FONT_BYTES` 是唯一内嵌字体；换字体只改 `font.rs` 与 SHA-256 基线。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：排版单点、预算超限、黑名单优先、原始源码不改写、字体资产哈希、缓存复用。
- 上屏后的像素级观感由构建产物人工目测，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 消费面：`compile_formula`、`pixels_per_point`、`MathLayoutCache`、`MathFont`、`MIN_READABLE_MATH_PX`。
- 换字体：只改 `font.rs` 与 SHA-256 基线，并补资产校验用例。
- 调整预算：属契约数值变更，须评审并补超限用例。
