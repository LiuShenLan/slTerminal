# 跨模块安全与敏感材料审查

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支），只审当前工作树；范围覆盖凭据与审计、路径授权、命令执行、HTML/SVG、更新链、加密备份、Hooks、unsafe/FFI、依赖审计与许可。

## 覆盖与局限

- 已读：根 `CLAUDE.md`、`slterm_app` / `slterm_app/src/local_control` / `slterm_app/src/encrypted_backup` / `slterm_app/src/platform` / `slterm_app/src/ai_hook` / `slterm_app/src/gpui_terminal` / `slterm_app/src/persistence` / `slterm_app/src/app_shell/markdown` / `slterm_path` / `slterm_document` / `slterm_hook` 的路径级 `CLAUDE.md`；`.claude/docs/architecture/overview.md`、`.claude/docs/testing/exemptions.md`；`docs/devPlan/design` 的 settings-recovery、runtime、markdown、hooks、files-git、windows、terminal；`docs/featureSpec` 的 path-policy、runtime、backup-and-recovery；`docs/devPlan/progress.md` 与 `traceability.md`。
- 已读代码：`secrets.rs`、`platform/credentials`、`local_control`（credentials/discovery/server/client/service/reads/git）、`encrypted_backup` 与 recovery/maintenance、`diagnostics.rs`、`logging.rs`、`audit.rs`、`atomic_file.rs`、`platform/dpapi.rs`、`update_check`、`update_download` 与 handoff、`ai_hook` 的 payload/protocol/ordering/windows/editor/draft/local、`slterm_hook`、`git` 与 `app_root` 的 git_changes/runtime_snapshot、`documents.rs`、`file_editor/document.rs`、`markdown_html.rs`、`markdown_svg.rs`、markdown 资源与解码、`theme_library`、`scripts/prepare-windows-runtime.ps1`、`licenses` 与 `check_licenses.py`、CI 工作流、`Cargo.toml`/`Cargo.lock`、`gates/cargo-audit.log` 与 `gates/cargo-tree-duplicates.log`。
- 未做：按合同不运行 cargo/测试/网络；未使用真实 Windows 凭据、真实 GUI、真实安装器或真实发布包；`SLTERM_TEST_CREDENTIAL_STORE` 真凭据管理器往返用例未验证；依赖审计由作业门禁实际执行，`cargo audit` 以非零退出并报告安全公告（见作业临时目录的 `gates/cargo-audit.log`），仓库没有 audit 例外/忽略配置，`.github/workflows/ci.yml` 也没有 audit 步骤；生产依赖的影响、传播路径与可达性见下，但未做 TLS 中间人、计时侧信道或崩溃复现实测；全仓凭据扫描是基于模式的（仅出现 `sk-test` 形态假值），不能证明不存在语义性秘密。
- 全仓 tracked 扫描未发现真实令牌、私钥、`.env`、`.pem`、`.pfx` 或云厂商 key 形态；命中的敏感形态均为测试假值。报告不复制任何命中值。

## 问题

### 阻断｜加密备份没有用户可达的导出/恢复入口，工作区类别也不承载真实工作区

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs`（模块文档、`collect_from`、`restore`、`restore_selected`、`undo`）、`slterm_app/src/app_shell/mod.rs`（仅注册 `BackupRestoreRecoverer`）、`slterm_app/src/session.rs`（`session_path`、`load`、`save_final`、`quarantine`）、`docs/devPlan/progress.md` 的备份验收行、`docs/devPlan/traceability.md` 的备份章节。
- 现象：生产代码没有调用 `collect`/`seal` 触发导出，也没有调用 `restore`/`restore_selected`/`undo` 或 `stage_restore` 触发恢复；设置页没有备份分区，`app_shell` 只注册了启动维护期恢复器。`BackupCategory::Session` 固定读写 `session.json`，而该文件的生产入口被显式标注为不调用；真实工作区位于 SQLite 与 `ProjectSnapshot`，按设计不进入备份。进度与追溯却把加密备份、恢复点、撤回和跨机重定位登记为已验收。
- 根因：备份功能停留在模块层，设置页入口、关闭预检、受控暂存到维护请求的用户链没有接线；工作区类别沿用了旧会话文件而不是规范逻辑对象投影。
- 影响：featureSpec 备份与恢复的导出、预览、整包恢复、撤回在产品中不可达，相关验收不成立；`backup_workspace_category_exclusions` 只证明“某文件没入包”，不能证明“工作区逻辑对象被正确导出”；恢复点回滚、关闭取消和跨机重定位都没有真实用户流程证据。按合同判据这属于核心验收不成立。
- 证据：`encrypted_backup.rs` 顶部同时写着“设置页安全分区的加密备份导出已接”和“恢复与撤回 UI 尚未接线”，二者自相矛盾；`git grep "encrypted_backup::"` 在 `slterm_app/src` 中只有 `app_shell/mod.rs` 的恢复器注册；`session.rs` 的 `load`/`save_final`/`quarantine` 带 `dead_code` 注记“生产 AppRoot 不调用”；`BackupCategory::Session` 的收集与恢复目标仅为 `session.json`；追溯备份行引用的是模块级例名，其中 `backup_cross_machine_requires_relocation` 只在于测试中预先写入 `RelocationRequired` 状态。
- 优化方向：要么把导出、预览、关闭预检、受控暂存、维护期应用与撤回接到设置页和工作区规范快照，使工作区类别来自 `ProjectSnapshot`/ViewState/侧栏/终端元数据而不是 legacy 文件；要么在进度与追溯中把该能力降级为未验收并登记不可自动化面。两种做法按设计真值都成立，唯独不能保留“已验收”叙述。

### 重要｜加密备份用小型敏感键黑名单替代字段白名单

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的 `collect_from`、`add_file`、`filter_settings`、`strip_sensitive_keys`、`SENSITIVE_KEYS`，以及测试例 `collection_uses_allowlist_and_strips_sensitive_keys`、`backup_restore_does_not_restore_credentials`；对照 `slterm_app/src/settings_transfer.rs` 的 `export_settings`。
- 现象：设置类别直接把完整 `settings.json` 读入，只按键名黑名单剥除若干“看起来敏感”的键；未知命名空间、未注册段和任意嵌套值都原样进入包。测试明确断言未知段 `handmade` 原样保留，和“未知命名空间本地保留但不导出”的普通设置导出行为相反。
- 根因：备份收集没有复用 `SettingsRegistry` 的 `exportable && !sensitive` 白名单投影，而是采用“已知敏感键列表 + 其余放行”的黑名单；该表无法证明“非敏感”，只能覆盖有限拼写。
- 影响：用户或历史版本写进 `settings.json` 未知命名空间的认证材料、CLI 派生配置或其他秘密可以进入加密包；这与 design 的“未注册或无法证明非敏感的字段不入包”、根 `CLAUDE.md` 的“备份白名单即安全边界”以及 featureSpec 的“未注册字段不按应用设置导出”冲突。导入侧复验同样只看同一张表，不能拒绝含秘密值的非敏感键名。
- 证据：`SENSITIVE_KEYS` 命中逻辑只做去空白后 ASCII 小写精确匹配，因此 `env` 对象下的 `ANTHROPIC_AUTH_TOKEN`、`credential_blob` 之外的别名或任意手写键都不会被剥除；`filter_sensitive_document` 只递归按键删除；同仓 `settings_transfer::export_settings` 已提供正面白名单，备份没有复用它。
- 优化方向：设置类别应从 SettingsRegistry 的导出白名单投影后入包，未注册或不可证明非敏感的字段一律拒绝并计数提示；导入复验使用同一白名单。黑名单可保留为第二道防线，但不能作为唯一边界。

### 重要｜加密备份缺少明文/报文大小上限，恢复任意文件可被内存放大

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的 `open`、`decrypt_bytes`、`seal`、`collect_from`；`slterm_app/src/encrypted_backup/maintenance.rs` 的 `stage_restore`。
- 现象：`decrypt_bytes` 只检查魔数和最小头部长度，没有报文上限；`open` 会把整段密文解密并整体反序列化；`seal` 与 `collect_from` 也不检查明文上限。design 明确要求“明文上限 64 MiB”，并要求密码学测试覆盖“大小上限”，实现与测试都没有对应边界。
- 根因：格式实现只固定了魔数、盐、nonce、KDF 参数与认证方式，遗漏了长度这一资源边界；收集侧在读取设置/会话/主题时也未做累计大小限制。
- 影响：用户选择恢复一个超大或恶意构造的备份文件时，Argon2 派生之后还要完整分配密文、明文、JSON 结构和大块主题/设置字节，导致长时间占用后台线程或进程内存耗尽；这不是“校验失败零写入”，而是在分配阶段就可能崩溃。收集侧同样可能把超大设置/主题整体读入内存。
- 证据：`decrypt_bytes` 只有 `packet.len() <= header_len` 与魔数判断；`seal` 只校验版本与归档结构；`collect_from` 的 `add_file`/`add_themes` 直接 `fs::read`；design 的“明文上限 64 MiB，旧格式拒绝”没有落地，测试也没有超限用例。
- 优化方向：在 KDF/解密之前按头部与报文长度拒绝超限，并在 `seal`/`collect_from`/`stage_restore` 统一施加明文与累计类别预算；补边界值、超限和“超限零写入”测试。

### 重要｜Runtime `git.diff` 的工作区侧按词法路径直接打开文件，可经符号链接读取授权根之外的内容

- 置信度：确认
- 位置：`slterm_app/src/local_control/git.rs` 的 `git_diff_snapshot`（工作区读取路径）；入口校验在 `slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `runtime_git_diff_target` 与 `valid_repo_relative_path`。
- 现象：Runtime 只校验仓库相对路径非空、不以分隔符开头、不含 `.`/`..` 或反斜杠，随后在后台把该路径拼到仓库工作树根上，用 `std::fs::File::open` 打开并读取最多 10 MiB 返回 diff 文本。如果仓库内该路径是符号链接或目录联接，读取会跟随到工作树之外，而 PathPolicy 与文件身份复核没有被调用。
- 根因：Runtime diff 读取只复用 HEAD 侧的 `head_file` 与 core 差分，没有复用 `DocumentOpenService`/`DocumentPathResolver` 的规范路径、文件身份与授权链；词法路径校验被误当成真实路径授权。
- 影响：持有 Runtime token 的本机进程可以让用户打开一个含链接的仓库，再通过 `git.diff` 读取任意当前用户可读的文本文件并取回内容；这违反 path-policy 的“敏感操作前重新解析真实目标”“读取不返回未授权文件内容”，也绕开了 RepoRoot 只读范围的真实边界。若目标文件是凭据、CLI 配置或 AI 会话正文，还可能造成敏感材料外流。
- 证据：`git_diff_snapshot` 中 `worktree_root.as_path().join(&target.path)` 后直接 `File::open` 与 `read_prefix`，没有任何 `symlink_metadata`、reparse 拒绝、canonicalize、`PathPolicy::authorize` 或 `file_identity` 比对；同一仓的编辑器/Diff 面板工作区侧走共享 Document 与授权身份链，Runtime 这条是旁路。
- 优化方向：Runtime diff 的目标必须经 `DocumentPathResolver` 解析为规范路径和稳定文件身份，并按 `PathOperation::Read` 的 RepoRoot/ProjectRoot 能力重新授权；对 reparse point 或身份不符直接拒绝。若目标本应只来自已打开 Document，则改为只接受已授权 DocumentId。

### 重要｜Git 读取未统一遵守有界执行、禁网与凭据提示约束

- 置信度：确认
- 位置：`slterm_app/src/git/mod.rs` 的 `command`、`head_file`、`verify_head`、`file_hunks`；对照 `slterm_app/src/git/repository.rs` 的 `repository_command` 与 `run_repository_with_limit`。
- 现象：`repository_command` 会加 `--literal-pathspecs`、`GIT_TERMINAL_PROMPT=0`、`GIT_NO_LAZY_FETCH=1`、`GIT_ALLOW_PROTOCOL=` 并有超时/输出上限；但 `head_file`、`verify_head`、`file_hunks` 使用裸 `command`，没有这些环境变量，也没有统一超时与输出上限。`head_file` 的 `git show` 在部分克隆中可能触发 lazy fetch，且等待子进程没有 deadline；`file_hunks` 的 `.output()` 没有任何输出预算。
- 根因：早期读取函数保留了独立的子进程构造，没有收敛到仓库执行器；read-only 路径被默认为“不会联网/不会卡住”。
- 影响：设计明确要求“所有调用使用直接 argv、`--literal-pathspecs`、`GIT_TERMINAL_PROMPT=0` 和有界 stdout/stderr”“不访问网络”。当前实现可能在 partial clone 下发起网络取对象、产生不可见凭据提示，或在 git 卡住时无限占用 UI/Runtime 的后台线程；diff 输出也可能无界占用内存。`head_file` 同时服务 Diff 面板与 Runtime `git.diff`，影响面不止视图。
- 证据：`head_file` 用 `command(&root).args(["cat-file"/"show", &spec])`，`file_hunks` 用 `command(root)` 后 `.output()`；`repository_command` 的约束与 `run_cancellable` 都不在这条路径上；`head_file` 只在截断时 kill，未截断时会一直阻塞到子进程结束。
- 优化方向：所有 Git 子进程统一经 `repository_command` 与有界执行器，设置固定超时、stderr/stdout 预算和懒取对象禁止；将“不允许网络与凭据提示”做成构造口的不变量而非调用方纪律。

### 重要｜套餐凭据配置、诊断导出与清理服务未接入生产，追溯矩阵仍标记自动化

- 置信度：确认
- 位置：`slterm_app/src/plan_balance`（`PlanBalanceClient`、`PlanBalanceService`、`PlanBalanceSource::credential_target`）、`slterm_app/src/background_tasks.rs`（仅注册 `PLAN_BALANCE` 元数据）、`slterm_app/src/diagnostics.rs`、`slterm_app/src/cleanup.rs`；`docs/devPlan/progress.md` 的 Windows 服务验收行；`docs/devPlan/traceability.md` 的套餐与 Runtime 诊断行。
- 现象：套餐查询与调度只有模块和单元测试，没有生产订阅者、没有设置页 token 输入、没有状态展示，也没有真实调度；诊断报告与清理服务没有生产调用点；`runtime_token_absent_from_materials` 仅存在于诊断模块测试。进度文档却声明套餐调度、诊断白名单和清理切片全绿。
- 根因：Windows 服务层切片以模块级用例替代了真实接线验证；测试例只锁定内部函数和假 DTO，没有覆盖“设置页输入 token 后不回显、不进入持久材料”和“诊断导出真实 DTO 不含秘密”的用户链。
- 影响：凭据红线中的“设置页输入只用于本次凭据写入、保存后清空且不回显”没有实现入口可验证；套餐凭据无法配置，`settings_plan_token_not_cli_credentials` 只证明“没有读取 CLI 凭据”，不能证明“专用 token 不回显”；诊断/清理的敏感材料边界只有模块测试证据。追溯矩阵因此给出高于实际覆盖的结论。
- 证据：`git grep "PlanBalanceClient|PlanBalanceService|PlanBalanceCredentials"` 在 `plan_balance` 目录外无命中；`background_tasks::subscribe` 的生产调用只出现在更新检查；`DiagnosticReport` 只在验收与清理测试中出现；`CleanupService` 在 `cleanup.rs` 外无调用；i18n 已存在套餐/备份/诊断文案但没有对应控件消费。
- 优化方向：接线设置页与后台调度，或把相应进度/追溯状态改为未验收并登记人工面；凭据类 UI 必须补“写入后清空、不回显、不进入持久材料”的视图级测试，诊断必须用真实 DTO 做白名单断言。

### 重要｜生产 TLS 栈命中 emSign 弃用中间证书公告，更新链的哈希信任锚与传输层同源

- 置信度：确认（依赖版本与生产路径）；端到端中间人利用未实测
- 位置：`slterm_app/Cargo.toml` 的 `ureq` 依赖；`slterm_app/src/update_proxy.rs` 的 `agent`、`slterm_app/src/update_check.rs` 的 `check_from`/`fetch_release_with_agent`、`slterm_app/src/update_download.rs` 的 `download_with_job`、`slterm_app/src/plan_balance/client.rs` 的 `fetch`、`slterm_app/src/app_shell/markdown/remote_resource.rs` 的 `UreqRemoteImageClient`；`gates/cargo-audit.log` 的 RUSTSEC-2026-0382 与 RUSTSEC-2026-0381。
- 现象：`ureq` 未关闭默认特性，默认 `rustls` 链路解析到 `rustls 0.23.46`、`rustls-webpki 0.103.15` 与 `webpki-roots 1.0.9`。前者可能接受包含已弃用 emSign 中间证书的证书路径；后者的捆绑根列表仍含 `emSign ECC Root CA - G3`，而公告明确说明这些中间证书的私钥已公开。ureq 默认信任锚来自 `webpki-roots`，因此两条件在同一客户端栈中同时具备。更新元数据、资产下载、套餐请求和远程 Markdown 图片共用这套 TLS 栈。
- 根因：依赖版本停在修复线之前，且更新链没有独立于 TLS 的发布签名或公钥钉扎；SHA-256 只锚定在同一 TLS 通道取回的 release 元数据上，一旦传输层被伪造，哈希校验只能证明“与攻击者提供的元数据一致”。
- 影响：若公告所述路径被接受，具备网络中间人位置的攻击者可用已公开私钥伪造 GitHub 或套餐端点证书链，替换更新元数据与安装包；`update_download` 只校验长度、`MZ` 头和 release 中的 SHA-256，未发现 Authenticode/WinVerifyTrust 或钉扎公钥校验，自动安装形态会把恶意安装包当作校验通过。当前更新检查、下载与套餐服务没有生产订阅者（见跨域观察），用户可达的 TLS 出网面主要是点击加载远程 Markdown 图片；该路径无凭据、无 Referer、零重定向，影响限于内容伪造与预算内解码。更新链一旦接线，这将构成更新供应链的完整信任失效。
- 证据：`slterm_app/Cargo.toml` 的 `ureq = { version = "3", features = ["json", "socks-proxy"] }` 未禁用默认特性；ureq 的 `default = ["rustls", "gzip"]`，`rustls` 特性启用 `rustls-webpki-roots`；`Cargo.lock` 反向边为 `rustls-webpki ← rustls ← ureq` 与 `webpki-roots ← ureq`；`target/debug/.fingerprint` 中二者有构建指纹；`cargo-audit.log` 列出 RUSTSEC-2026-0382（`rustls-webpki 0.103.15`，修复 >=0.103.16）与 RUSTSEC-2026-0381（`webpki-roots 1.0.9`，修复 >=1.0.10）；`verify_download` 的校验面只有长度、`MZ` 与 SHA-256。
- 优化方向：把 TLS 依赖升到修复版本并锁定；更新链增加独立于传输层的发布签名或公钥钉扎，安装前验证 Authenticode/清单签名；在依赖审计门禁中把这两条公告列为必须修复项，而不是默认放行的告警。

### 一般｜Hooks 草稿写盘绕过审计 facade 与 PathPolicy 的 hooks 能力

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/editor.rs` 的 `write_editable_at`/`save_editable`、`slterm_app/src/ai_hook/draft.rs` 的 `save_hooks_draft`、`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `save_hooks_draft_for`、`slterm_app/src/audit.rs` 的测试例 `settings_writes_are_centralized_in_the_audit_wrapper`、`slterm_path/src/policy.rs` 的 `PathGrant::hooks_read_target`/`hooks_write_target`。
- 现象：生产 Hooks 页保存直接 `atomic_file::write` 到用户 `~/.claude/settings.json`、项目 `.claude/settings.json` 与 `.claude/settings.local.json`，没有经过 `audit::write_user_settings`；审计模块的“唯一写路径”扫描只检查 `ai_hook/local.rs` 的安装器两个函数，扫描不到 `editor.rs`/`draft.rs`。`PathOperation::HooksRead`/`HooksWrite` 及对应 `PathGrant` 只在 policy 测试中使用，生产 hooks 写从未做能力判定。
- 根因：安装器写入与 GUI/JSON 草稿写入走了两套路径，审计与能力位只在安装器一侧收敛；扫描测试以函数名单代替了“所有设置文件写点”的全局约束。
- 影响：对 CLI 安全配置的实际编辑没有审计留痕；设计要求的“所有 Hooks 写入经受限 PathPolicy 能力”不成立；未来若新增写路径，现有扫描测试仍会通过，形成假保障。
- 证据：`write_editable_at` 的写盘语句是 `crate::atomic_file::write(path, ...)`；审计扫描只对 `ensure_claude_hooks_in`/`remove_claude_hooks_at` 做函数体断言；`hooks_read_target`/`hooks_write_target` 在 `slterm_path` 外无调用。
- 优化方向：把 Hooks 草稿、安装器、卸载器统一到一个经审计与能力校验的写入口，或明确把 draft 写入排除并同步审计文档与测试，使“唯一写路径”与实际一致；删除未使用的能力位或真正接入。

### 一般｜恢复点、暂存材料与 Hooks 配置写盘未统一使用当前用户 ACL 与受控原子写

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/recovery.rs` 的 `write_point`、`slterm_app/src/encrypted_backup/maintenance.rs` 的 `stage_restore`、`slterm_app/src/ai_hook/local.rs` 的 `write_atomic`。
- 现象：恢复点与 `staged.restore` 使用 `atomic_file::write`（复制目标 ACL 或继承父目录），没有使用同仓已有的 `write_current_user`；design 要求恢复点“写入本地根原子文件并限制当前用户 ACL”。Hooks 安装器使用的 `write_atomic` 用 `std::fs::write` 写可预测临时名后再 rename，不保留目标 ACL，也与 hooks 设计的“目标原子写并保留 ACL”不一致。
- 根因：敏感材料写盘没有统一到“当前用户 + 原子替换 + 保留必要 ACL”的单一原语；`atomic_file` 中已有正确原语但调用方未选用。
- 影响：如果 `SLTERM_LOCAL_DIR` 指向共享目录、父目录 ACL 被放宽，或恢复点/暂存文件落在继承宽 ACL 的位置，其他用户可读取 DPAPI 密文或暂存明文（settings/session/theme 内容）；Hooks 配置重写会丢掉用户显式设置的 ACE。同用户预置临时名文件也可能干扰写入。
- 证据：文档快照走 `atomic_file::write_current_user`，而恢复点与暂存没有；`write_atomic` 使用 `path.with_extension("slterm-tmp-<pid>")` + `std::fs::write` + `fs::rename`，没有 `create_new` 或 ACL 复制；design 与模块 CLAUDE 的 ACL 要求均可对照。
- 优化方向：DPAPI 恢复点、暂存材料和其它安全材料统一使用当前用户 DACL 的原子写原语；Hooks 配置统一走 `atomic_file` 的 ACL 保留路径，临时文件用不可预测且 `create_new` 的名字。

### 一般｜进程内恢复入口绕过维护期应用、世代轮换与关闭预检，且简单恢复路径无回滚

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的 `restore`/`restore_at`/`restore_to`、`slterm_app/src/encrypted_backup/recovery.rs` 的 `restore_selected_at`/`undo_at`、`slterm_app/src/encrypted_backup/maintenance.rs` 的 `apply`/`roll_back`。
- 现象：`restore_to` 逐条 `atomic_file::write`，任一写入失败直接返回错误，前面已写文件不回滚；`restore_at` 被模块文档保留为生产“恢复”接缝；`restore_selected_at` 和 `undo_at` 在运行期直接改写 settings/session/themes，只额外创建一个恢复点，不登记维护请求、不作关闭预检、不更换 `store_epoch`。设计明确要求恢复/撤回收敛到启动维护期，运行期只做暂存与预览。
- 根因：维护期实现与早期直接写实现并存，早期接缝没有删除或降级为测试专用；两条路径的失败语义和世代语义不同。
- 影响：若未来 UI 直接调用这些接缝，用户可能在写入中途失败后得到部分恢复（`restore_at`），或者在应用运行中替换设置/会话而旧 Runtime Operation 与 Journal 世代仍然有效，导致旧写复活；这与“恢复或撤回实现不得直接替换运行中的文件”“任一写入失败使用恢复点回滚”冲突。当前无人调用只降低发生概率，不消除误接线风险。
- 证据：`restore_to` 的循环中没有回滚分支；`restore_selected_at` 有进程内回滚但不产生维护请求、不轮换世代；`maintenance.rs` 的 `apply` 才具备恢复点、`Applying` 状态与 `rotate_store_epoch`；模块文档自述恢复/撤回 UI 未接线。
- 优化方向：只保留“运行期暂存 → 启动维护期应用/回滚”的单一通道，把直接写恢复入口移出生产可见面或明确标注测试专用；用编译或源码扫描阻止第二通道被接线。

### 一般｜端点发现记录未与存活进程身份绑定，客户端未验证 RunMarker

- 置信度：疑似
- 位置：`slterm_app/src/local_control/discovery.rs` 的 `read_discovery_at`/`publish_at`、`slterm_app/src/local_control/client.rs` 的 `handover`、`slterm_app/src/local_control/startup.rs` 的 `handover_existing`。
- 现象：发现记录包含 `process_id` 与 `process_created_at_ms`，但客户端只校验 loopback、版本、端口和 `instance.handover` 能力，从不核对记录声称的进程是否仍是当前监听者；服务器也不在握手中证明自己就是记录对应的 `run_id`/进程。`DiscoveryRecord::validate` 要求进程身份非零，但没有使用它。
- 根因：发现记录被当作可信端点目录，而不是带生命周期的运行事实；设计要求的 `RunMarker` 核对只落在记录字段与握手元数据上，没有形成验证链。
- 影响：在首次实例崩溃留下过期端点记录、或同用户进程抢占同一 loopback 端口的场景下，第二实例客户端可能把 handover 凭据发送给冒充服务；CLI 的 `runtime discover` 若在过期记录上合成输出，Runtime token 也可能被发往错误端点。当前记录文件受当前用户 ACL 保护，且 Runtime token 需要调用方主动使用，风险受限于同机同用户或端口复用窗口。
- 证据：`handover` 只检查 `hello.capabilities`；`HelloResponse` 不含可核对的进程创建时间或可验证的端点身份；`record.process_id`/`record.process_created_at_ms` 在客户端路径中未被读取。还缺运行时证据：需要构造“崩溃遗留记录 + 端口被另一进程监听”的实测，才能确认凭据确实送达冒充者。
- 优化方向：让端点身份可验证：服务器在握手中返回与记录一致的 `run_id`、PID、创建时间并证明对凭据的持有；客户端在发送凭据前核对记录与握手事实，或在锁所有权下用独占端口/句柄防止端口复用；过期记录应在取得实例锁后优先作废。

### 一般｜追溯矩阵多个安全场景的断言范围小于例名与文档声明

- 置信度：确认
- 位置：`slterm_app/src/diagnostics.rs` 的 `runtime_token_absent_from_materials`、`slterm_app/src/local_control/boundary_tests.rs` 的 `integration_runtime_credential_boundaries`、`slterm_app/src/app_shell/app_root/tests/terminal_restore.rs` 的 `backup_cross_machine_requires_relocation`、`slterm_app/src/plan_balance/mod.rs` 的 `settings_plan_token_not_cli_credentials`；`docs/devPlan/traceability.md` 的对应行。
- 现象：追溯行声称“端点、设置、备份和诊断不含 Runtime token”，但锚只检查端点文件与诊断 DTO，没有扫描设置或备份包；跨机恢复锚由测试直接预置 `RelocationRequired`，恢复导入路径本身没有任何设置该状态的代码；套餐凭据锚只检查模块不读取 CLI 环境/配置文件，没有 token 写入、清空、不回显或备份排除断言。
- 根因：测试以“能证明一个点”替代“例名覆盖整个场景”；追溯矩阵按例名登记而不是按断言内容核销。
- 影响：这些场景被标记为已自动化，但真实安全行为没有被验证；后续改动破坏设置/备份中的 token 排除或跨机拒绝逻辑时，回归门禁不会红。
- 证据：`runtime_token_absent_from_materials` 的输入只有 `DiagnosticReport`；`integration_runtime_credential_boundaries` 写端点文件和 discover 输出，不创建备份包或设置文件；`backup_cross_machine_requires_relocation` 通过 `set_root_status` 构造状态，没有任何 backup 导入调用；`settings_plan_token_not_cli_credentials` 只断言若干字符串不在生产源码中。
- 优化方向：让锚与声明一致：用假 token 生成真实备份包/设置文件并断言不含 token；在恢复导入后用真实路径触发 RelocationRequired 并断言不读正文、不启动 PTY；为套餐凭据补写入-清空-回显-备份排除的完整链。若某面当前不可达，应拆分为未验收并登记，而不是沿用宽例名。

### 一般｜备份加密与哈希栈命中 ctutils 非常量时间公告，未发现受影响 API 调用

- 置信度：疑似（未找到受影响的 `Choice` mask/u128 调用；未证明所有编译路径与内联结果）
- 位置：`slterm_app/Cargo.toml` 的 `aes-gcm`/`argon2`/`sha2`；`slterm_app/src/encrypted_backup.rs` 的 `seal`/`open`/`encrypt_bytes`/`decrypt_bytes`；`gates/cargo-audit.log` 的 RUSTSEC-2026-0385。
- 现象：`aes-gcm 0.11.1`、`argon2 0.6.0`、`sha2 0.11.1` 与 `digest`/`universal-hash`/`phc` 共享 `ctutils 0.4.3`。公告称 `Choice` 的整数 mask 生成可能产生与秘密相关的分支，且 `u128` 比较在 32 位目标上生成分支。产品是 x86_64 Windows，32 位分支项不适用；备份 GCM tag 比较走 `ct_eq` 的 `cmov` 切片实现，受影响的 `Choice::to_*_mask`/`from_u128_*` 未在已审源码中出现。
- 根因：加密依赖链仍锁定在公告修复线之前；备份加密代码信任下游 crate 的常量时间实现，没有对 tag 比较与口令派生路径建立“只用常量时间原语”的独立约束。
- 影响：若后续代码或下游 crate 版本改用 `Choice::to_*_mask`/`select`，秘密相关分支可能通过本地计时侧信道泄漏备份口令或认证标签；当前未发现可触发路径，也没有实测时间差，故不认定为已可利用。
- 证据：`Cargo.lock` 反向边为 `ctutils ← aes-gcm`、`ctutils ← digest`、`ctutils ← phc`、`ctutils ← universal-hash`；`aes-gcm` 的 `decrypt` 使用 `expected_tag.ct_eq(tag)`；`CtEqSlice` 的整数实现宏使用 `cmov::CmovEq`；相关 crate 源码中未发现 `to_u128_mask`/`from_u128_*` 的调用；`cargo-audit.log` 列出 RUSTSEC-2026-0385（`ctutils 0.4.3`，修复 >=0.4.4）。
- 优化方向：升级并锁定 `ctutils`；为备份 GCM tag 比较与口令派生增加常量时间原语的代码级约束与测试锚，避免未来替换实现时静默退化。

### 提示｜凭据明文副本没有全程清零：握手帧、请求缓冲与套餐 header 仍用普通 String/Vec

- 置信度：确认
- 位置：`slterm_app/src/local_control/server.rs` 的 `read_frame`/`handle_connection`、`slterm_app/src/local_control/client.rs` 的 `handover`/`read_response`、`slterm_app/src/plan_balance/client.rs` 的 `fetch`、`slterm_runtime_protocol` 的 `Credential::clear`。
- 现象：`Credential` 会在 Drop 时清零自身，但包含 token 的原始 JSON 帧与读取缓冲是普通 `Vec<u8>`，解析后未擦除；套餐请求用 `format!("Bearer {}", token.as_str())` 生成普通 `String` 再交给 ureq header，`into_string` 返回的 `Zeroizing<String>` 之外又产生未清零副本。
- 根因：秘密类型只覆盖了凭据值本身，没有覆盖线缆帧和请求构造中间态；请求头组装点没有使用可清零载体。
- 影响：64 位 hex 的 Runtime/handover token 会以普通堆内存形式残留到分配器复用；与“有界秘密类型并尽快清零”的凭据纪律有差距。威胁需要本机进程读取进程内存，风险不高，但属于红线执行不彻底。
- 证据：`read_frame` 返回 `Vec<u8>` 并直接传给 `parse_client_request`；`handle_connection` 的 `buffer` 在会话期间保留未解析字节；`PlanBalanceClient::fetch` 的 header 值来自 `format!`；`SecretBytes::into_string` 只保证返回值是 `Zeroizing`。
- 优化方向：线缆帧与读取缓冲使用可清零类型并在解析后立即擦除；请求头直接在可清零缓冲中组装；保持长度上限先于分配。

### 提示｜AI hook 解析在 debug 日志输出整条事件，含会话文本与 attention 消息

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/windows.rs` 的 `dispatch`、`slterm_app/src/ai_hook/event.rs` 的 `AiHookEvent`/`AttentionContext`。
- 现象：`dispatch` 对每条已解析事件执行 `log::debug!("ai_hook: {event:?}")`。`AiHookEvent` 的 Debug 含 `message`（CLI 最后回答或通知文案）、`session_file`；`AttentionContext` 的自定义 Debug 已隐藏 selection/raw 正文、但仍输出 `message`。当前 `logging::initialize` 固定 Warn 过滤掉 Debug，因此生产默认不落盘；但该闸门是级别配置而非类型或字段级保障。
- 根因：事件类型实现了包含正文的 Debug，并把 Debug 直接接到日志；没有为诊断提供独立的脱敏摘要类型。
- 影响：未来接线 verbosity、提高 max_level、或通过其它日志后端采集 debug 时，AI 会话正文与权限提示文本会进入日志/诊断材料，违反“hook 正文、命令和可疑内容不进入日志”的设计。
- 证据：`AiHookEvent` 派生 Debug 且包含 `message` 字段；`AttentionContext` 的 Debug 显式包含 `message`；`dispatch` 的日志语句直接插值 `{event:?}`；`logging` 的 `is_allowed_target` 允许 `slterm` target 在 Trace 档位输出 Debug。
- 优化方向：为事件提供只含 source/kind/pane/长度/散列的 Debug 或摘要 DTO，日志只消费摘要；用测试锁定事件 Debug 不含 message/selection/raw_context。

### 提示｜Hook 管道事件未校验客户端属于目标 pane 的进程树

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/windows.rs` 的 `dispatch`、`slterm_app/src/ai_hook/ordering.rs` 的 `verdict`、`slterm_app/src/agent_host.rs` 的 `handle_hook`。
- 现象：`dispatch` 从内核取得 `client_pid` 并计算 `agent_pid`，注释称这是“路由的第二因子：校验它是否真的跑在声明的 pane 进程树内”；但 `verdict` 与 `handle_hook` 都没有把这两个 pid 与目标 pane 的 PTY 进程身份比较，`agent_pid` 只进入流键。任何能打开命名管道的同用户进程都可以声明任意 `source`/`pane`，投递伪造的完成、等待输入或权限事件，并可把伪造的 `session_id`/`session_file` 写入终端记录。
- 根因：进程身份只被采集，没有被消费；管道 DACL 只保证同用户，不保证调用者属于目标 pane。
- 影响：一旦命名管道服务接入生产，伪造事件会改变 Agent 状态、通知与恢复线索；最坏情况是重启后注入一个形状合法但不属于该 pane 的会话 ID。它不直接给凭据能力，因此按提示登记。当前生产未接线管道服务，风险处于潜在状态。
- 证据：`AiHookEvent` 的 `client_pid`/`agent_pid` 字段在 `ai_hook` 目录外没有消费点；`stream_key` 仅把 `agent_pid` 用作去重维度；没有 `TerminalRegistry`/PTY 祖先链比对代码。
- 优化方向：在 pane 路由前用 Registry 的 PTY 进程身份与祖先链验证 `client_pid`/`agent_pid`；无法证明归属的事件只计数不改变状态，并补端到端伪造用例。

### 提示｜更新下载 `.part` 打开跟随预置链接，未使用 `create_new` 或 reparse 防护

- 置信度：疑似
- 位置：`slterm_app/src/update_download.rs` 的 `download_with_job`、`download_paths_at`。
- 现象：`.part` 文件以 `create(true).truncate(true).open` 打开，若同用户或共享数据根中有人预置了指向其他文件的符号链接/目录联接，写入会跟随链接并截断目标；最终 `replace` 会替换链接本身，而不是写穿目标。`updates` 目录只由数据根派生，没有当前用户 DACL 加固。
- 根因：下载临时文件沿用普通文件打开语义，没有把“临时文件名由本进程首次创建”作为不变量；威胁模型默认数据根不可被他人写。
- 影响：在数据根被覆盖为共享目录或被同用户恶意进程预置链接时，可能截断任意当前用户可写文件；正常单用户安装不受影响。需要运行时构造预置链接才能证实完整利用链。
- 证据：`OpenOptions` 没有 `create_new` 或 `FILE_FLAG_OPEN_REPARSE_POINT`，也没有 `symlink_metadata` 检查；下载路径来自 `root.join("updates").join(asset.name)`，`asset.name` 虽经白名单校验，但目录内容与链接状态未校验。
- 优化方向：`.part` 使用 `create_new` 或打开时拒绝 reparse point，最终发布走已校验的原子替换；更新目录按当前用户 ACL 创建。

### 提示｜捆绑 ConPTY 提取只按文件大小复用，运行时未校验来源哈希

- 置信度：确认（缺失哈希校验）；利用前提为同用户写权限
- 位置：`slterm_terminal/src/tty/windows/conpty.rs` 的 `ensure_extracted`、`write_if_size_differs`、`load_sideloaded`；`scripts/prepare-windows-runtime.ps1` 与 `slterm_terminal/vendor/conpty/README.md`。
- 现象：构建期 vendor 目录的文件哈希与脚本预期一致，但运行时从 `%LOCALAPPDATA%\slterm\conpty` 提取/复用时只比较长度；同尺寸替换不会触发重写，随后按绝对路径 `LoadLibraryW`/启动 `OpenConsole.exe`。`LOCALAPPDATA` 环境变量参与提取目录选择。
- 根因：运行时把“大小相同”当作“内容相同”，把编译期信任错误地延续到可被同用户写入的目录。
- 影响：同一用户可预置同尺寸 DLL/EXE，应用下次启动侧载执行；这不是跨权限提升（同用户本已可执行代码），但破坏了捆绑二进制的防篡改与来源可核验性，也使“哈希／来源”只在构建期成立。
- 证据：`write_if_size_differs` 只用 `metadata.len()`；`CONPTY_DLL_BYTES`/`OPENCONSOLE_EXE_BYTES` 是 `include_bytes!` 的可信字节，却没有在提取后比对摘要；`prepare-windows-runtime.ps1` 的 SHA-256 钉扎没有在运行时复现。
- 优化方向：复用前校验内嵌字节的 SHA-256（不匹配则覆盖或拒绝），或每次以当前用户 ACL 覆盖写入；提取根从受信 Known Folder 解析，不信任可改写的环境变量。

### 提示｜合同冲突：备份 AAD 与类别清单在 design、模块文档与代码之间不一致

- 置信度：确认
- 位置：`docs/devPlan/design/settings-recovery.md` 的加密格式与类别段、`slterm_app/src/encrypted_backup/CLAUDE.md`、`slterm_app/src/encrypted_backup.rs` 的 `encrypt_bytes`/`decrypt_bytes`/`BackupCategory`。
- 现象：design 要求“完整头部作为 AAD”“类别只有 settings 与 workspace”，且自定义主题属于设置类别；模块 CLAUDE 与代码只把魔数作 AAD，并把类别拆成 Settings/Session/Themes；模块顶部注释还声明“设置/会话/自定义主题/命令历史/目录历史五类”，而代码并不收集历史，featureSpec 又把历史列为永久排除。三处口径互不一致。
- 根因：格式与类别在演进中只改了实现和部分注释，没有回写 design；AAD 的“完整头部”也没有在实现中体现。
- 影响：salt 与 nonce 已通过 KDF 与 GCM nonce 隐式认证，AAD 差异不构成直接漏洞；但类别与格式是外部可见恢复契约，文档冲突会让后续实现和测试对“哪些内容应入包、哪些必须拒绝”产生分歧，也使追溯矩阵无法判定验收真值。
- 证据：design 的格式句与类别句；模块 CLAUDE 的“魔数同时作 AAD”；`encrypt_bytes`/`decrypt_bytes` 的 `Payload { aad: MAGIC }`；`BackupCategory` 只有三个变体；顶部注释的五类声明。
- 优化方向：按 featureSpec 与 design 重新确定唯一格式与类别真值，同步代码、模块文档、schema 与测试；AAD 若保持魔数，应明确写明“头部其余字段经 KDF/nonce 隐式认证”并撤销 design 的相反表述。

### 提示｜诊断脱敏词表与备份敏感键表不一致，覆盖不到非 `sk-` 形态的键值

- 置信度：确认
- 位置：`slterm_app/src/diagnostics.rs` 的 `redact`/`consume_secret`；对照 `slterm_app/src/encrypted_backup.rs` 的 `SENSITIVE_KEYS`。
- 现象：诊断脱敏只匹配 `sk-`、`Bearer`、`token=`、`token: `、`token":`、`secret=`、`secret: `、`password=`、`password: ` 等有限形态；`api_key=`、`api-key:`、`authorization:`、`cookie=`、`private_key=`、`refresh_token=` 等不会命中；64 位 hex 或非 `sk-` 前缀的 token 若以裸值出现也不会被识别。
- 根因：诊断与备份各自维护一份敏感词表，诊断侧没有复用备份的键名归一逻辑，也没有统一测试语料。
- 影响：如果应用日志中以 `api_key=...` 之类的形态出现认证材料，复制/导出的诊断包会原样携带。白名单材料来源仍是硬边界，设计也声明“不承诺识别所有秘密”，但两个安全输出通道的口径不一致会让用户误以为诊断与备份同等严格。
- 证据：`redact` 的模式表不含 `api_key`/`authorization`/`cookie`；`SENSITIVE_KEYS` 含这些键；诊断测试只覆盖 `sk-`、`Bearer`、`token`、`secret`、`password` 样例。
- 优化方向：把敏感键归一表收敛为单源，诊断按键名边界与值形态同时识别；补 `api_key`、`authorization`、`cookie`、`private_key` 及裸长 hex 的负向/正向语料。

### 提示｜凭据 Win32 FFI 调用缺少逐块 SAFETY 不变量

- 置信度：确认
- 位置：`slterm_app/src/platform/credentials/windows.rs` 的 `load_secret`、`save_secret`、`delete_secret`（`CredReadW`/`CredWriteW`/`CredDeleteW`/`CredFree` 调用点）。
- 现象：这些 unsafe 调用依赖指针有效期、`CREDENTIALW` 内多个指针指向本地缓冲、`CredReadW` 输出由 `CredFree` 释放、错误路径不重复释放等不变量，但调用点没有 SAFETY 注释；同仓其它 FFI（如 `copy_credential_blob`、`remove_if_matches`、DPAPI）都有明确的 SAFETY 说明。模块文档解释了清零纪律，但没有替代逐调用的安全契约。
- 根因：凭据后端实现较早，安全说明集中在模块顶部与调用后的行内注释，没有按项目其它处的 FFI 注释规范落到每个 unsafe 块。
- 影响：凭据代码是最高敏感度的 FFI 面；缺少局部不变量说明会让后续改动（缓冲区扩容、错误分支、重复释放）缺少审查锚点。当前未发现实际 UB，但这是可维护的安全缺口。
- 证据：`load_secret` 的 `CredReadW`、`save_secret` 的 `CredWriteW`、`delete_secret` 的 `CredDeleteW` 均无 SAFETY 块；`CREDENTIALW` 构造依赖 `target`/`user`/`blob` 三个本地 Vec 的生存期，释放与清零顺序只在注释中说明。
- 优化方向：为每个 unsafe 调用写明缓冲区所有权、指针有效期、系统调用失败时的释放责任与清零顺序，并让 reviewer 能按块核对。

### 提示｜url 命中 `Url::origin()` 嵌套 blob 栈溢出公告，生产闭包未发现调用点

- 置信度：疑似（未做完整依赖闭包调用图）
- 位置：`Cargo.lock` 的 `url 2.5.8` 及反向路径 `url ← gpui ← slterm`、`url ← http_client ← gpui`；`gates/cargo-audit.log` 的 RUSTSEC-2026-0592；对照 `slterm_app/src/app_shell/markdown/link.rs` 的 `dispatch` 与 `slterm_app/src/app_shell/markdown/remote_resource.rs` 的 `validate_remote_url`。
- 现象：`url` 作为 gpui/http_client 的传递依赖进入生产闭包；公告要求对不可信输入调用 `Url::origin()` 且 URL 含嵌套 `blob:` 前缀才触发，栈溢出会中止整个进程。仓库源码、gpui/http_client 与 gpui-component 的已审源码中没有 `.origin()` 调用；Markdown 链接分派只接受 http/https/mailto/tel，blob 在解析前被拒。
- 根因：`url` 版本未达修复线，且项目没有把“不可信 URL 不进入 `Url::origin()`”作为可检查约束；当前不可达依赖调用图，而不是类型或边界。
- 影响：若未来接线代码在渲染、下载或链接预览中对用户可控 URL 调用 `Url::origin()`，一个嵌套 `blob:` 链接即可让应用进程崩溃；当前未发现生产调用路径，不能证明所有传递依赖都未调用。
- 证据：`Cargo.lock` 的反向边；`cargo-audit.log` 列出 RUSTSEC-2026-0592（`url 2.5.8`，修复 >=2.5.9）；`git grep` 在仓库源码中无 `Url::origin`；gpui/http_client 源码中无该调用；`link.rs` 的 `dispatch` 对非 http/https/mailto/tel 协议直接 `Rejected`。
- 优化方向：升级 `url` 到修复版本；对不可信 URL 的解析入口保持 scheme 白名单，并补“嵌套 blob 不进入 origin”的回归锚；若无法升级，用封装类型限制 `origin()` 的调用面。

### 提示｜cookie 公告在 Windows 与不 fork 的应用上不适用，且当前构建未启用 cookies

- 置信度：确认
- 位置：`slterm_app/Cargo.toml` 的 `ureq` 依赖与 `features = ["json", "socks-proxy"]`；`Cargo.lock` 的 `cookie 0.18.2 ← cookie_store ← ureq`；`gates/cargo-audit.log` 的 RUSTSEC-2026-0594。
- 现象：公告只影响 Unix-like 系统中 `fork()` 后在父子进程同一线程添加私有 cookie 的应用，并明确写明 Windows 应用与从不 fork 的应用不受影响。当前 `ureq` 未启用 `cookies` 特性，`cookie_store`/`cookie` 是可选依赖；`target/debug/.fingerprint` 中没有二者构建指纹。
- 根因：`Cargo.lock` 按全特性与全目标解析保留可选依赖，审计工具按锁文件报告；产品构建没有启用该特性，平台也不在受影响列表。
- 影响：当前 Windows 构建没有 cookie 使用面，nonce 复用不可达；风险只存在于未来启用 `cookies` 且移植到 Unix 并自行 fork 的场景，属于锁文件噪音而非产品漏洞。
- 证据：advisory 的受影响系统与条件段；`ureq` 的 `cookies` 特性未在依赖声明中启用；构建指纹缺失；`cargo-audit.log` 列出 RUSTSEC-2026-0594（`cookie 0.18.2`，修复 >=0.18.3）。
- 优化方向：保持 `cookies` 关闭；若未来启用，必须升级到修复版本并在审计例外中写明平台边界，而不是长期允许告警。

### 提示｜未维护的生产传递依赖进入 Windows 构建

- 置信度：确认（路径与编译面）；当前无对应已知漏洞
- 位置：`Cargo.lock` 与 `gates/cargo-audit.log`；`bincode 1.3.3 ← syntect ← comrak ← slterm_document`、`chrono 0.4.45 ← gpui/scheduler/zlog/gpui-component`、`instant 0.1.13 ← gpui-component/notify-types`、`paste 1.0.15 ← gpui-component`、`rustybuzz 0.20.1 ← usvg ← gpui`、`ttf-parser 0.25.1 ← slterm`（直接依赖）。
- 现象：这些 crate 被公告标记 unmaintained，但仍是 Windows 生产闭包的一部分。对照之下，`yaml-rust 0.4.5` 只经 `syntect` 的可选 `yaml-load` 特性进入，当前未启用且无构建指纹；`rustls-pemfile 2.2.0` 只经 `gpui-component-assets` 的 wasm 目标引入；`zbus_names`/`zvariant`/`zvariant_derive` 只经 Linux 的 `gpui_linux`/`zbus` 引入，均不在 Windows 生产编译面。
- 根因：上游冻结基线（gpui、gpui-kit、syntect、comrak）带来未维护依赖；项目没有依赖健康门禁，unmaintained 只作为 cargo audit 的默认允许告警。
- 影响：这些 crate 不再接收安全修复；字体解析链（`ttf-parser`/`rustybuzz`）属于复杂二进制解析面，语法高亮资产（`bincode`）与时间/宏依赖影响面较低。当前没有对应漏洞公告，但“无公告”不能当作长期安全保证。
- 证据：`Cargo.lock` 反向边；`target/debug/.fingerprint` 的构建指纹（`yaml-rust` 无指纹）；`syntect` 的默认特性集合不含 `yaml-load`；`gpui-component-assets` 的 `zed-reqwest` 依赖位于 wasm 目标；`gpui_linux`/`zbus` 不在 Windows 目标；`cargo-audit.log` 的 unmaintained 列表。
- 优化方向：把未维护依赖纳入依赖健康门禁与升级路线，至少为字体解析与语法高亮资产建立版本升级或替换方向；对确认不进入生产编译的锁文件条目建立按目标的例外清单，避免审计结果长期不可读。

### 提示｜unsound 与 yanked 依赖出现在生产闭包，受影响 API 未触达

- 置信度：确认（调用面）；疑似（未做全依赖调用图与运行时验证）
- 位置：`Cargo.lock` 与 `gates/cargo-audit.log`；`heapless 0.9.3 ← gpui/sum_tree`、`rustix 1.1.5`（Unix 目标经 `tempfile`/`polling`/`async-io`/`zbus` 引入，Windows 目标不编译 xattr 模块）、`async-executor 1.14.0 ← smol ← gpui-component`。
- 现象：`heapless` 的多条 unsound 公告覆盖 `Deque`、`IndexMap`、`Spsc`、`FindMutView`、`SortedLinkedList` 与 Treiber stack 等 API；生产代码只使用 `heapless::Vec`，未触达这些结构。`rustix` 的公告只影响 `getxattr`/`listxattr` 等 xattr 函数，而这些函数在 `fs/mod.rs` 中只对 apple/linux_kernel/hurd 编译，Windows 目标不包含。`async-executor` 是 yanked 版本而非已知漏洞，经 `smol` 进入 Windows 生产构建。
- 根因：上游依赖锁定与 yanked 版本未清理；unsound 公告的风险取决于 API 使用面，而门禁只按 crate 版本报告，无法区分“可达”与“仅传递”。
- 影响：当前未发现可达 UB 或已知漏洞；但 `heapless` 的 unsound 面较广，未来若在窗口或树结构中使用 `Deque`/`IndexMap` 会直接引入 UB 风险；yanked 版本没有修复通道，后续依赖解析可能被动漂移。
- 证据：`sum_tree/src/cursor.rs` 与 `sum_tree/src/sum_tree.rs` 只 `use heapless::Vec as ArrayVec`；gpui profiler 只使用 `heapless::Vec<ActionTiming, 5>`；`rustix` 的 `mod xattr` 与 `getxattr` 实现均有 `cfg(any(apple, linux_kernel, target_os = "hurd"))`；`Cargo.lock` 反向边；`cargo-audit.log` 的 RUSTSEC-2026-0574 至 RUSTSEC-2026-0582、RUSTSEC-2026-0486 与 async-executor yanked 行。
- 优化方向：把 unsound 公告纳入“API 可达性”审查而不是按 crate 放行；升级 `rustix` 到修复版本，跟踪 `heapless` 上游修复并在无修复版本时限制受影响 API 的使用面，换用未 yanked 的 `async-executor` 解析。

### 提示｜依赖审计未进入 CI 常驻门禁，门禁失败没有修复闭环

- 置信度：确认
- 位置：`.github/workflows/ci.yml` 的步骤集合；作业临时目录的 `gates/cargo-audit.log`；仓库根缺失的 `audit.toml`/`deny.toml` 例外登记。
- 现象：作业门禁实际运行了 `cargo audit`，结果以漏洞非零退出并列出安全公告与告警；仓库 CI 没有 audit 步骤，根 `CLAUDE.md` 的“门禁常挂”清单也没有依赖审计；仓库没有任何 audit 例外/忽略配置，告警只能靠工具默认策略放行。
- 根因：依赖审计只存在于一次性本地门禁脚本，结果没有变成可重复的 CI 证据，也没有“漏洞必须修复、告警按可达性分类”的固定策略。
- 影响：已知有修复版本的漏洞可以随依赖树长期留在产品中；新增漏洞不会在提交或发布时阻断；发布证据缺少依赖审计结论。更新链的哈希信任锚又依赖同一 TLS 通道，使该门禁缺口与上面的 TLS 公告叠加。
- 证据：CI 步骤集合以 fmt/clippy/test/架构/文档/品牌/脚本/打包/资产为主，没有 audit；`cargo-audit.log` 以漏洞非零退出结束，并标出安全公告与多条允许告警；`git ls-files` 中没有 audit/deny 配置。
- 优化方向：把依赖审计纳入 CI 常驻门禁，固定 advisory 数据库更新策略与例外登记格式，区分“漏洞必须修”与“告警按可达性分类”，并保存每次结果作为发布证据。

## 跨域观察

- AI hook 命名管道服务在生产未接线：`spawn_gpui_server` 仅在测试特性下暴露，`AgentHub::dispatch`/`handle_hook` 没有生产调用者。结果是 `slterm-hook` 安装后转发的事件不会被宿主消费，AI CLI 状态与通知链路在真实构建中不成立。这属于功能接线问题，但直接放大上面的 Hook 信任与日志问题：一旦接线，必须同时落地进程归属校验与脱敏摘要。
- 更新检查与更新弹窗没有生产订阅者：`update_check::start_gpui` 未被调用，`begin_update_install`/`cancel_update_install` 标为 `dead_code`；`update_download` 的安装移交只有启动恢复对账，而 `apply_scheduled_at` 又要求 `session.json` 中存在可恢复工作区，生产路径却不写该文件，预约安装会在提交前失败。更新资产与 handoff 的安全校验本身可读，但没有真实用户链证据。同一 `ureq`/`rustls` 栈还承载套餐查询与远程 Markdown 图片；套餐查询未接线，远程图片是当前用户可达的 TLS 出网面。依赖审计发现的 TLS 公告因此在当前构建中主要威胁未来更新链接线，而不是已发生的更新替换。
- `PathGrant::hooks_read_target`/`hooks_write_target` 与 `PathOperation::HooksRead`/`HooksWrite` 只在 `slterm_path` 的单元测试中使用，未进入 Hooks 安装或编辑执行链；`history_*` 能力位同样没有生产调用。能力位存在但未接线，容易被误读为已强制。
- 加密备份的 i18n 文案已经存在，但没有控件或命令消费；这类“文案先行、接线缺失”的状态会让追溯矩阵的“已自动化”看起来比实际更强。本报告只在安全相关处展开，其余功能完整性应交由对应范围审查。

## 存疑未证

- GitShow 的“系统打开”保留原扩展名并经 `ShellExecuteW` 打开：对 `.bat`/`.cmd` 等可执行关联，系统可能直接运行 blob 内容。设计明确要求保留原扩展名，因此这可能是被接受的产品风险，需要真实桌面关联与点击实测才能确认是否构成执行面；报告不把它提升为确认缺陷。
- 端点发现记录的冒充利用需要运行时证据：构造“首实例崩溃遗留端点记录 + 另一进程占用同一 loopback 端口”，验证第二实例是否会把 handover 凭据发送给冒充者，以及 `slterm runtime discover --json` 是否会在过期记录上合成 Runtime token。当前只确认客户端不核对 `process_id`/`process_created_at_ms`。
- 更新 `.part` 的符号链接截断需要共享数据根或同用户预置链接的实测；当前只确认打开语义没有 `create_new`/reparse 防护。
- 捆绑 ConPTY 同尺寸替换需要同用户写 `%LOCALAPPDATA%\slterm\conpty` 的实测；当前只确认提取路径不做哈希校验，且 `LOCALAPPDATA` 可被环境覆盖。
- 诊断脱敏的实际漏出取决于真实日志内容；未发现生产代码以 `api_key=` 形态记录认证材料，因此只登记为口径不一致与潜在泄漏面，而不是已发生泄漏。
- emSign 证书链公告的实际利用未实测：需要在受控 DNS/代理下，用公告所称已公开私钥的中间证书构造 GitHub 或套餐端点的伪造链，验证 `rustls-webpki 0.103.15` 与 `webpki-roots 1.0.9` 的组合确实接受该路径；报告只确认版本、信任锚与生产依赖路径，不宣称已完成攻击。
- `ctutils` 与 `url` 的可达性结论依赖调用图检查，不是运行时证据：当前只在仓库源码、gpui/http_client、gpui-component 与相关加密 crate 中未发现受影响 API 或 `Url::origin()` 调用；完整依赖闭包、宏展开与编译器内联后的调用面仍需机械核对。
- `heapless` 的 unsound 公告未做运行时或 Miri 验证；当前只确认生产代码使用 `heapless::Vec`，未使用公告列出的 `Deque`/`IndexMap`/`Spsc` 等结构。
