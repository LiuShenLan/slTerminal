# slterm_completions（core 层）

> 独立轻量补全匹配引擎：候选值族、匹配算法与来源合同；不持产品态。

## 决策与红线

- **core 边界**：不依赖 app、不依赖渲染包；输入输出为纯值族（`Span`/`Suggestion`/`SemanticSuggestion`）。原因：匹配引擎可独立测试与复用。边界：来源缓存、会话状态与 ghost/popup 呈现归 `slterm_app`。
- **裁剪已成定局**：ANSI/LS_COLORS 颜色分支、SSH/WSL 语义源、Unix 小分支不迁；不要以「兼容」名义回填。Posix 方言（Git Bash/MSYS）保留，是产品定位的一部分。
- **匹配算法**：prefix/substring/fuzzy（nucleo）；算法与大小写折叠语义是行为合同，改动须补矩阵用例。原因：匹配结果是用户可见行为。边界：不得在引擎内硬编码产品来源。
- **来源合同**：`Completer` 是可插拔 trait；新增来源 = 实现 trait + 在应用层注册。原因：引擎与产品来源解耦。边界：来源的缓存与生命周期归应用层。

## 验收与测试

- 定向测试：`cargo test -p slterm_completions --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 匹配高亮索引、grapheme 对齐、Span 边界与方言选项是必测面。
- 测试分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 新增补全来源：在应用层实现 `Completer` 并注册；匹配引擎本身不感知来源。
- 修改匹配语义：补 prefix/substring/fuzzy 与大小写折叠矩阵用例。
