# slterm_completions（core 层）

> 独立轻量补全匹配引擎：候选值族、匹配算法与字面路径遍历；不持产品态、不读 I/O。

## 决策与红线

- **core 边界**：不依赖 app、不依赖渲染包；输入输出为纯值族（`Span`/`Suggestion`/`SemanticSuggestion`）。目录枚举与 home 解析归 application，经 `file::DirectoryReader` 注入条目；本 crate 不读文件系统或进程环境。原因：匹配引擎可独立测试与复用，OS 读取受 application 授权与审计统一管理。边界：来源缓存、会话状态与 ghost/popup 呈现归 `slterm_app`；路径转义与 shell 拼写归 `command_context`。
- **裁剪已成定局**：ANSI/LS_COLORS 颜色分支、SSH/WSL 语义源、Unix 小分支不迁；不要以「兼容」名义回填。Posix 方言（Git Bash/MSYS）保留，是产品定位的一部分。
- **可复核编辑**：`CompletionEdit` 绑定预期输入 revision、合法 UTF-8 `Span`、整行 SHA-256 与插入文本；`apply` 只有在当前原文和 revision 均匹配时才返回新行。原因：候选计算与应用之间可能发生 ABA，单凭文本余量不能证明编辑仍合法。边界：core 不读终端状态、不写 PTY；调用方必须在接受候选前调用 `apply`，失配不得产生输入副作用。
- **匹配算法**：prefix/substring/fuzzy（nucleo）；算法与大小写折叠语义是行为合同，改动须补矩阵用例。原因：匹配结果是用户可见行为。边界：不得在引擎内硬编码产品来源。
- **来源编排归应用层**：本 crate 不保留补全来源 trait 或来源注册表；候选来源、缓存与生命周期全部归 `slterm_app`。原因：没有生产消费者的来源抽象会与真实编排形成第二套合同。边界：新增来源在应用层实现并注册，不得回填来源 trait。
- **转义与拼写单点**：shell 引用与转义只归 `command_context::CommandContext`；`file` 只产出未引用的字面路径。路径候选以 `-` 开头时必须强制引用，未知方言直接不产出；普通选项参数仍保持裸写。原因：两套引用规则会让候选绕过方言安全。边界：路径来源需要引用时经 `CommandContext::path_candidate` 拼写。

## 验收与测试

- 定向测试：`cargo test -p slterm_completions --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 匹配高亮索引、grapheme 对齐、Span 边界与方言选项是必测面。
- 测试分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 新增补全来源：在应用层实现来源并注册；匹配引擎本身不感知来源。
- 新增文件系统来源：在 application 实现 `DirectoryReader` 并注入，不在本 crate 增加目录或环境读取。
- 修改匹配语义：补 prefix/substring/fuzzy 与大小写折叠矩阵用例。
