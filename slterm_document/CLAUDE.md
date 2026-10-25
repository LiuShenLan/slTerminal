# slterm_document（core 层）

> 文本、编码、差分、语法与 Markdown 语义核心，不持 GPUI 或 I/O。

## 决策与红线

- **纯文档边界**：只承载文本、编码、差分、语法与 Markdown 语义。原因：文档语义须可独立于界面和外部系统验证。边界：不得持有 GPUI 类型或执行 I/O。
- **旧 grammar 适配只留测试**：`tree-sitter-json5 0.1.0` 与 `tree-sitter-dockerfile 0.2.0` 的 Rust binding 仍使用 tree-sitter 0.20 类型，且未导出随包 highlights query。原因：审计须以 tree-sitter 0.27.1 和真实 query 验证 ABI，不得空 query 或改用其他语言资产。边界：C 入口适配仅在 `cfg(test)` 开放，生产库保持 `forbid(unsafe_code)`；本地 query 必须与锁定包原件逐字一致，升级时重新核对。
- **SCSS 固定 MSVC 修复 rev**：crates.io 的 `tree-sitter-scss 1.0.0` 会向 MSVC 直传 GCC 警告参数，固定上游 `9ab738d22bb248771d42e8678d1a307d14990df1`、版本仍为 1.0.0。原因：该 rev 相对发布基线不改 grammar、query 或 Rust 入口，只补平台构建分支及 README。边界：不得移动 rev；回到 crates.io 或升级前须重新审计并实测 Windows x64。

## 验收与测试

- 定向测试：`cargo test -p slterm_document --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- grammar 审计：`cargo test -p slterm_document --test lib_tests grammar_audit -- --test-threads=1`；每种 grammar 必须实际验证 ABI 13–15、真实 highlights query 编译和最小合法样例无错误，禁止 skip、空 query 或占位 query。
