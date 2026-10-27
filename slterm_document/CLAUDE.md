# slterm_document（core 层）

> 文本、编码、差分、语法与 Markdown 语义核心，不持 GPUI 或 I/O。

## 决策与红线

- **纯文档边界**：只承载文本、编码、差分、语法与 Markdown 语义。原因：文档语义须可独立于界面和外部系统验证。边界：不得持有 GPUI 类型或执行 I/O。
- **文本状态身份**：`TextStateId` 使用 application 注入的 UUIDv7；新编辑产生新值，撤销／重做恢复原值，不以正文散列代替身份。原因：同一正文经不同编辑路径到达时仍需精确区分保存与撤销状态。边界：本 crate 只校验和保存值，不生成随机数。
- **Git 固定版本值**：`CommitId` 只接受完整 40 或 64 位小写十六进制对象 ID；`RepoPath` 只接受正斜杠规范相对文件路径。原因：短哈希、引用名与多义词法路径不能固定版本对象。边界：对象存在性由 GitFacade 查询，本 crate 不执行 Git。
- **仓库路径事实单点**：`RepositoryBinding` 直接复用 `slterm_path` 的规范路径与文件身份，不另建路径 DTO。原因：绑定复核必须与 PathPolicy 使用同一身份事实。边界：本 crate 只比较纯值；真实路径解析、HEAD 存在性与对象读取由 application 提供。
- **grammar 单一运行时**：`tree-sitter-json5 0.1.0` 与 `tree-sitter-dockerfile 0.2.0` 使用仓内锁定源码快照，Rust binding 统一为 `LanguageFn`，随包 highlights query 直接导出。原因：全部 grammar 必须由 tree-sitter 0.27.1 执行 ABI 与真实 query 审计，旧 binding 不得带入第二套原生运行时或重复 query 副本。边界：C grammar、scanner 与 query 只按各自快照锚升级；测试不开放手写 C 入口适配，生产库保持 `forbid(unsafe_code)`。
- **SCSS 固定 MSVC 修复 rev**：crates.io 的 `tree-sitter-scss 1.0.0` 会向 MSVC 直传 GCC 警告参数，固定上游 `9ab738d22bb248771d42e8678d1a307d14990df1`、版本仍为 1.0.0。原因：该 rev 相对发布基线不改 grammar、query 或 Rust 入口，只补平台构建分支及 README。边界：不得移动 rev；回到 crates.io 或升级前须重新审计并实测 Windows x64。

## 验收与测试

- 定向测试：`cargo test -p slterm_document --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- grammar 审计：`cargo test -p slterm_document --test lib_tests grammar_audit -- --test-threads=1`；每种 grammar 必须实际验证 ABI 13–15、真实 highlights query 编译和最小合法样例无错误，禁止 skip、空 query 或占位 query。
