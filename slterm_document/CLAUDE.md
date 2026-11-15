# slterm_document（core 层）

> 文本、编码、差分、语法与 Markdown 语义核心，不持 GPUI 或 I/O。

## 决策与红线

- **纯文档边界**：只承载文本、编码、差分、语法与 Markdown 语义。原因：文档语义须可独立于界面和外部系统验证。边界：不得持有 GPUI 类型或执行 I/O。
- **文本状态身份**：`TextStateId` 使用 application 注入的 UUIDv7；新编辑产生新值，撤销／重做恢复原值，不以正文散列代替身份。Session 记录全部曾用值，redo 清空或历史淘汰后仍禁止新编辑复用。原因：同一正文经不同编辑路径到达时仍需精确区分保存与撤销状态。边界：本 crate 只校验和保存值，不生成随机数。
- **编辑事务单点**：所有 Rope 修改只经 `DocumentRegistry::apply` 消费 `EditTransaction`；校验 Document、revision、只读状态、UTF-8 字节边界、区间顺序与后台原文后，在候选 Rope 上按降序应用，成功才推进单调 revision 和当前状态。差量历史保存实际应用顺序的逆操作及前后 TextStateId；普通相邻输入按 600 毫秒合并，替换全部／重读／行尾变化不合并，撤销预算固定 8 MiB 并先淘汰最旧组。原因：文本、撤销和清脏必须共享一个原子事实。边界：undo／redo 只恢复文本与状态身份，不恢复视图瞬态；`mark_saved` 只更新保存状态并切断合并，保存期间的新编辑仍保持脏；任何失败不改正文、revision 或历史。
- **编码与内容路由**：`DocumentContent` 封闭为 Editable／Paged／Unavailable；前 8192 字节按解码后的字符单元检查 NUL，UTF-16 不得因零字节误判二进制。自动识别仅接受 BOM 或合法 UTF-8，GBK／GB18030 必须显式指定，所有解码与回写均严格失败而不替换字符。文本统一为 LF，保存按首个磁盘换行恢复，BOM、原始编码与末尾换行事实独立保留；新文件默认 UTF-8、无 BOM、CRLF。原因：未知编码、二进制与分页内容不得共享一个可保存正文基线。边界：真实字节数超过 10,000,000 关闭文档内补全，超过 50,000,000 进入 Paged，超过 200,000,000 关闭语法；Paged 的分页读取与 LRU 由后续切片实现，本模块不把有限前缀伪装成完整可编辑正文。
- **Document 聚合单点**：`DocumentRegistry` 以 `DocumentId` 持有全部存活 `DocumentSession`，正文使用 Rope，另以文件身份与规范路径建立唯一索引。打开先命中真实文件身份；无法取得身份时按规范路径去重，后续取得身份只补充原聚合索引。路径已绑定旧身份而本次解析为另一身份时必须创建新聚合，不继承旧正文。原因：路径、别名与文件替换都不能复制或错绑 Document。边界：本 crate 不执行解析或 I/O；真实身份由 application 在读取句柄上取得，编辑事务与保存／丢弃决定由后续切片扩展同一 Session；失去全部布局引用或打开候选失败时经同一 Registry 回收聚合及全部别名索引。
- **Git 固定版本值**：`CommitId` 只接受完整 40 或 64 位小写十六进制对象 ID；`RepoPath` 只接受正斜杠规范相对文件路径。原因：短哈希、引用名与多义词法路径不能固定版本对象。边界：对象存在性由 GitFacade 查询，本 crate 不执行 Git。
- **仓库路径事实单点**：`RepositoryBinding` 直接复用 `slterm_path` 的规范路径与文件身份，不另建路径 DTO。原因：绑定复核必须与 PathPolicy 使用同一身份事实。边界：本 crate 只比较纯值；真实路径解析、HEAD 存在性与对象读取由 application 提供。
- **grammar 单一运行时**：`tree-sitter-json5 0.1.0` 与 `tree-sitter-dockerfile 0.2.0` 使用仓内锁定源码快照，Rust binding 统一为 `LanguageFn`，随包 highlights query 直接导出。原因：全部 grammar 必须由 tree-sitter 0.27.1 执行 ABI 与真实 query 审计，旧 binding 不得带入第二套原生运行时或重复 query 副本。边界：C grammar、scanner 与 query 只按各自快照锚升级；测试不开放手写 C 入口适配，生产库保持 `forbid(unsafe_code)`。
- **SCSS 固定 MSVC 修复 rev**：crates.io 的 `tree-sitter-scss 1.0.0` 会向 MSVC 直传 GCC 警告参数，固定上游 `9ab738d22bb248771d42e8678d1a307d14990df1`、版本仍为 1.0.0。原因：该 rev 相对发布基线不改 grammar、query 或 Rust 入口，只补平台构建分支及 README。边界：不得移动 rev；回到 crates.io 或升级前须重新审计并实测 Windows x64。

## 验收与测试

- 定向测试：`cargo test -p slterm_document --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- grammar 审计：`cargo test -p slterm_document --test lib_tests grammar_audit -- --test-threads=1`；每种 grammar 必须实际验证 ABI 13–15、真实 highlights query 编译和最小合法样例无错误，禁止 skip、空 query 或占位 query。
