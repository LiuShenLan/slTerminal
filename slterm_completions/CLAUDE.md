# slterm_completions（core 层）

## 存在理由与边界

独立轻量补全匹配引擎：候选值族、匹配算法与来源合同。应用层的来源缓存、会话状态与 ghost/popup 呈现归 `slterm_app`，本 crate 不持产品态。

- **core 边界**：不依赖 app、不依赖渲染包；输入输出为纯值族（`Span`/`Suggestion`/`SemanticSuggestion`）。
- **裁剪已成定局**：ANSI/LS_COLORS 颜色分支、SSH/WSL 语义源、Unix 小分支不迁；不要以「兼容」名义回填。Posix 方言（Git Bash/MSYS）保留，是产品定位的一部分。
- **匹配算法三态**：prefix/substring/fuzzy（nucleo）；算法与大小写折叠语义是行为合同，改动须补矩阵用例。
- **来源合同**：`Completer` 是可插拔 trait；新增来源 = 实现 trait + 在应用层注册，不在引擎内硬编码产品来源。

## 测试

- 定向测试：`cargo test -p slterm_completions --test lib_tests <filter> -- --test-threads=1`；禁 `--lib`。
- 匹配高亮索引、grapheme 对齐、Span 边界与方言选项是必测面。
