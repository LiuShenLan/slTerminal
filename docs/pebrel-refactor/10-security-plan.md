# pebrel 重构优化 spec · 分片 10：安全与套餐（凭据安全面 / 加密备份 / 凭据管理器 / plan_balance / 安全审计）

baseline commit：`e537d528c508e8607d0f5f9fd25e5902f40d661e`（pebrel，Rust + GPUI 模块化单体）

## 优化面

本分片覆盖六件事：

1. **AI provider 凭据安全面**——`ai_providers.rs` 的凭据侧纪律：`pebrel_providers.json` 独立存放绝不与普通设置同文件序列化、凭据引用字段与元数据分离、`credential_target` 命名、`api_key_hint` 尾四掩码、`store_provider_api_key` 写即清稿、`remove_provider` 先删凭据删除序、`test_provider` 凭据内聚查找 + Zeroizing 全程（provider 体系本体——normalize/preset/连通测试——归分片 08，本片只取凭据安全面）。
2. **加密备份**——`encrypted_backup.rs` + `encrypted_backup/recovery.rs`：AES-256-GCM 口令保护备份（Argon2id 派生 + `MAGIC` PEBRBAK1 + AAD 绑定）、白名单即安全边界（AI 会话库/环境变量/SSH 密钥材料/凭据库永不进出备份）、`validate_archive` 恢复前全量校验先验后写、选择性恢复的加密恢复点与撤回。
3. **Windows 凭据管理器集成**——`platform/credentials.rs` 通用封装（CredentialIdentity + load/store/delete）、`ssh_credentials.rs` 的 `store_generic_secret`/`load_generic_secret`/`delete_generic_secret` 三原语门面、`ssh_credentials.rs::windows_store` 的 Win32 原语（CredReadW/CredWriteW/CredDeleteW + 缓冲区清零）。SSH 门面砍、凭据原语保留落 platform 层。
4. **隐私闸门边界登记**——`ai_assistant.rs::redact_secrets`（分片 08 已取）、`ai_hook` 作用域闸门（分片 03 已取）、`runtime_api` loopback token（分片 04 已取）。本片只登记边界，不重复裁定。
5. **slTerminal 侧现状对照**——`src-tauri/src/plan_balance/`（套餐余额：读 user 层 `~/.claude/settings.json` token → 查询代理端点 → 5h/7d 用量快照）的单进程移植形态与「token 不出后端」红线的单进程等价物；安全审计清单（SEC-12 statusline 审查 / SEC-13 Outdated 防篡改 / SEC-17 user 层写入审计三件套，SEC-05 校验前置、SEC-11 白名单等既有 SEC 机制）的单进程归宿；`src-tauri/src/state.rs::validate_path_within_root` 沙箱交叉确认（分片 07 已裁消亡）；SEC-18 凭据红线在 fork 后的延续。
6. **凭据存储共享裁定**——plan_balance 的 settings.json 自读通道与 ai_providers 凭据库（Windows 凭据管理器）是否共享存储（分片 08 登记归本片裁定）。

已定全局决策：安全三件套照抄（Windows 凭据管理器集成 / provider 凭据独立文件 + Zeroizing / AES-256-GCM 加密备份——白名单即安全边界）；plan_balance 与安全审计是 slTerminal 自有功能，pebrel 无对应物，保留并按单进程模块化风格移植；SSH 全家砍（凭据管理器的 SSH 门面、SSH target 命名、askpass 随之砍，通用凭据封装形态保留）；SEC-18 红线延续；`nebula_*`/`pebrel_*` → `slterm_*`。

## 采纳点

### AI provider 凭据安全面（与分片 08 交界：08 取 provider 体系本身，本片取凭据安全面）

1. **照抄：凭据独立文件 + 绝不与普通设置同文件序列化**（`ai_providers.rs` 模块头注释纪律 + `STORE_FILE`/`store_path`/`load`/`save`）。模块头注释明列理由：普通设置是用户可编辑运行态文件，凭据需要稳定身份、绝不能与之同序列化——这条「文件边界即凭据边界」的根纪律本片登记为安全面裁定；provider 体系本体（`normalize`/preset/`test_provider` 连通测试）归分片 08 采纳点，本片不重复。
2. **照抄：凭据引用与元数据分离的字段形态**（`ai_providers.rs::AiProvider` 的 `api_key_set`/`api_key_hint`，字段注释 "A credential reference only. The actual key is held by the OS store."）。明文永不入 store 是凭据面根纪律，store 文件可整份展示/备份/ diff 而无泄露。
3. **照抄：`api_key_hint` 尾四掩码**（`ai_providers.rs::api_key_hint` 的 `••••` + 末四位；测试 `key_hint_never_contains_the_full_secret` 随迁）。用户可辨识指纹与全量绝不见面是同一份代码的两面。
4. **照抄：`credential_target` 命名空间归调用方**（`ai_providers.rs::credential_target` 的 `Pebrel/AI/<id>` → `Slterm/AI/<id>`；改名分片 08 已登记）。target 前缀按域划分（AI/SSH/Backup 各自独立命名空间），凭据库因此可多域复用。
5. **照抄：`store_provider_api_key` 写即清稿纪律**（`ai_providers.rs::store_provider_api_key` doc comment "The plaintext never enters `ProviderStore` and callers must clear their write-only draft immediately after this returns" + `save_api_key` 入 OS 库即返回 hint）。凭据明文单行生命周期：过手即入库即弃，不落地任何中间态。
6. **照抄：`remove_provider` 先删凭据再删元数据的删除序**（`ai_providers.rs::remove_provider`：先 `delete_api_key` 成功再动 store，注释 "avoiding an unreachable orphaned secret"）。失败停在删凭据前，元数据不动——绝不产生指不进删不掉的孤儿密钥。
7. **照抄：`test_provider` 凭据内聚查找 + Zeroizing 全程**（`ai_providers.rs::test_provider`：凭据查找内聚在函数体内，key 与 `Bearer ` 拼接串均 `Zeroizing` 包裹；注释 "no UI receives plaintext from the OS credential store"）。凭据明文生命周期闭在后台执行体内部，任何 UI 层零接触。

### 加密备份与恢复

8. **照抄：AES-256-GCM + Argon2id 口令派生管线**（`encrypted_backup.rs::derive_key`/`encrypt_bytes`/`decrypt_bytes`：Argon2id v0x13、m=19456 KiB/t=2/p=1、盐 16 字节/nonce 12 字节/密钥 32 字节、派生密钥与解密明文皆 `Zeroizing`、`getrandom` 取盐与 nonce）。口令保护备份的完整算法栈，参数组合作契约数值随迁。
9. **照抄：`MAGIC` 魔数 + AAD 绑定**（`encrypted_backup.rs::MAGIC` 的 `PEBRBAK1` 作 `Payload::aad`，解密侧先验魔数再派生）。格式身份与密文认证绑死——魔数不符即「非本应用备份或版本不支持」，一票拒解；fork 后重设计为 slterm 形态魔数。
10. **照抄：白名单即安全边界**（`encrypted_backup.rs` 模块头注释 "The allowlist below is the security boundary for both export and import"；不遍历用户 home 目录，`collect_from` 只认 data_dir 固定文件清单）。导入导出同一道边界，枚举即授权，清单外文件不存在「顺手带上」的通道。
11. **照抄：敏感面绝不进出备份的双闸**（`encrypted_backup.rs` 模块头注释明列 AI 会话库/环境变量/SSH 密钥材料/凭据库永不读写；`filter_settings` 导出侧剥 `ssh_proxy_*`/`pinned_hosts`/`saved_hosts`/`hidden_hosts`，`add_sanitized_ssh` 剥 `private_keys`/`proxy`，`ssh_is_sanitized` 导入侧对同字段复验）。导出清洗 + 导入复验双闸形态照抄；SSH 类别具象随 SSH 砍（见不采纳点 5），双闸抽象保留给 slterm 自有敏感文件。
12. **照抄：`validate_archive` 恢复前全量校验**（`encrypted_backup.rs::validate_archive`：版本号、类别集合无重复、逐条 `safe_path` 拒绝对路径与 `..` 分量、按类别精确文件名白名单、`terminal_profiles.json` 须可解析 JSON、重复条目名拒）。任何一条不过整单拒写，先于任何落盘动作。
13. **照抄：恢复先鉴权后落盘 + 原子写 + 符号链接防线**（`encrypted_backup.rs::restore` 先 `open` 完整鉴权再 `restore_to`；`atomic_file::write` 原子落盘；`restore_path` 逐分量核查父链——命中符号链接或非目录即拒）。半份不可信报文不产生任何副作用，写不进半个文件。
14. **照抄：`recovery.rs` 选择性恢复 + 加密恢复点撤回**（`encrypted_backup/recovery.rs::RestorePoint` 只持句柄、UI 不碰写入规则；`restore_selected_at` 先采原字节与「不存在」状态 → 本机加密落盘恢复点 → 恢复失败自动回滚 → `undo` 凭口令精确撤回；`undo_at` 目标名集合必须与该次已验证恢复一致——外部导入文件不能借撤回扩展白名单）。恢复是用户可逆操作而非单向覆盖；恢复点只在本机加密落盘、不经会过滤凭据字段的可分享收集器。
15. **参考：`BackupSelection`/`BackupCategory` 分类选择机制**（`encrypted_backup.rs::BackupSelection` 的 `set`/`categories`/`is_empty`/`from_categories` + `validate_archive` 的按类别校验机制）。机制照抄；类别枚举与文件名清单按 slterm 自有数据文件重定（pebrel 各类别指向 nebula 系文件与 SSH 面，清单设计属实现期），「默认仅外观一项」的保守默认思想保留。

### Windows 凭据管理器集成

16. **照抄：通用凭据封装落 platform 层**（`platform/credentials.rs::load`/`store`/`store_with_username`/`delete` + `CredentialIdentity` 的 service/target 身份形态；Windows 分支转 `ssh_credentials::windows_store`）。封装层与具体 Win32 调用分离，调用方只给 target 与明文，不碰平台 API。
17. **照抄：`windows_store` Win32 原语**（`ssh_credentials.rs::windows_store::{load_secret,save_secret,delete_secret}`：`CredReadW`/`CredWriteW`/`CredDeleteW`、`CRED_TYPE_GENERIC` + `CRED_PERSIST_LOCAL_MACHINE`、`ERROR_NOT_FOUND` 归一为 `Ok(None)`、`CredFree` 释放、写后 `blob`/`target`/`user` 缓冲区全 `fill(0)`）。从 ssh_credentials 模块解绑、改名落 platform/credentials 下——SSH 砍但凭据原语保留，模块注释 "reusable without duplicating Windows API code or ever exposing the value to UI state" 即其存在理由。
18. **照抄：`store_generic_secret`/`load_generic_secret`/`delete_generic_secret` 三原语门面**（`ssh_credentials.rs::{store_generic_secret,load_generic_secret,delete_generic_secret}`；doc comment "Keeping the target namespace at the caller makes the secret store reusable"）。非 SSH 功能消费凭据管理器的唯一入口——本片 provider 凭据面与任何未来凭据域共用此门面，禁止旁路直调 Win32。

### 隐私闸门边界登记（只登记，不重复裁定）

19. **登记：出境/本机暴露面三闸门归各分片**——`ai_assistant.rs::redact_secrets`（AI 出境打码）归分片 08 采纳点；`ai_hook` 作用域闸门归分片 03;`runtime_api` loopback token 归分片 04。本片取的是凭据「存储面」（落哪、怎么落、怎么擦），三闸门取的是「出境面」（出了应用边界前打码/闸控），两面互补不重叠，本片不重复裁定。

### slTerminal 自有：plan_balance 移植（pebrel 无对应物，保留移植）

20. **照抄（自有保留）：plan_balance 模块域整体**（`src-tauri/src/plan_balance/` 的 `source.rs::PlanSource`/`query.rs::PlanQuery` 双静态切片注册表、`mod.rs::merge_slot` 快照合并语义、`mod.rs::poll_once_with` 参数化编排、`SNAPSHOT` 模块级静态存储、`apply_snapshot` 含 updated_at 的变化才 emit、`refresh_plan_balance` 恒返回 Ok）。deepseek/kimi 响应解析、全有或全无、冻结判定、已用百分比口径、kimi 实证红线（2026-08 实测修正规格）全部原样随迁。
21. **照抄（自有保留）：「token 不出后端」红线的单进程等价物——红线不变、防线升格**（`src-tauri/src/plan_balance/CLAUDE.md` 外部坑节「token 不出后端」+ 根 CLAUDE.md 硬约束 #14）。单进程后「后端」边界消亡，等价物 = token 生命周期闭在凭据域内（resolve 产出 → fetch 消费 → 即焚），永不进快照/DTO/渲染/日志/持久化；原 serde 键集精确匹配测试（`mod.rs::plan_balance_info_serde_key_set`）的等价物从「序列化层测试」升格为「类型层保证」——`FetchOutcome`/`PlanBalanceInfo` 无 token 字段即编译期拒；tracing/Err 消息禁止插值 token 与 Authorization 头的纪律原样。
22. **照抄（自有补强）：Zeroizing 内存纪律回补 plan_balance 面**（现状对照：`kimi.rs::fetch` 的 `format!("Bearer {token}")` 临时串、`source.rs::resolve_env` 返回的 `String` token 均不擦除，Cargo.lock 已有 zeroize 传递依赖但 src-tauri 源码零使用——grep 实证）。fork 后借 pebrel 凭据面纪律（采纳点 7）将 token 全链路 Zeroizing 化、即用即焚——这是对自有模块的安全补强，非行为变更。

### slTerminal 自有：安全机制单进程归宿（既有 SEC 清单）

23. **照抄（自有保留）：hooks 域审计三件套**——SEC-12 statusline 原命令审查（`src-tauri/src/hooks/claude/inject.rs::audit_suspicious_statusline`：命中可疑模式即暂停注入 + `suspiciousCommand` 原文回显 + `agent_hooks_confirm_inject` 确认二次调用流；启动重注入路径命中即跳过 + 审计，不改用户配置）、SEC-13 Outdated 防篡改审计路径（磁盘脚本 vs 内嵌模板 SHA-256 内容哈希比对；启动对账只补缺失不覆盖，「磁盘脚本被替换 → Outdated 提示」路径保留）、SEC-17 user 层写入审计（`src-tauri/src/hooks/claude/config.rs` 的 `tracing::warn!(target: "audit")` 通道 + TQ-COV-05 tracing-test 断言兜底可观测）。单进程归宿：三者本质都是 Rust 域逻辑，原样存活；「审计通道」等价物 = tracing target 保留，前端确认对话框变 GPUI 原生模态交互。
24. **照抄（自有保留）：SEC-05 校验前置**（`src-tauri/src/agent_history/` 的 `validate_session_id` trait 强制契约：delete/read_title 必须先校验后定位，不信托前端任何路径参数，UUID 形态 + 遍历定位）。单进程虽无 IPC 注入面，「不信托调用方输入」的校验语义仍是文件系统面防线，随模块原样存活。
25. **照抄（自有保留）：SEC-11 持久化入口校验**（`src-tauri/src/settings.rs` 顶层键白名单 + `app_dir.rs::MAX_PERSIST_BYTES` 1MB 上限；`src-tauri/src/projects.rs` 1MB + 必须 JSON 对象）。单进程后 settings 形态重定（分片 06 边界），但「持久化入口一律过校验」的语义保留，白名单键集随新 settings 形态重定。
26. **交叉确认：Tauri 沙箱/IPC 面 SEC 机制随架构消亡，OS 面 SEC 机制随模块存活**（分片 07 已裁「沙箱校验消亡」，本片交叉确认并二分归档）。随消亡：`state.rs::validate_path_within_root` 沙箱与 SEC-02 路径沙箱面、SEC-03/04 预览 nonce/校验链（ADR-0019/0021 防护对象随 WebView 消亡）、SEC-06 剪贴板插件权限（Tauri 权限面）、SEC-07 命令三处注册、SEC-08 pty 归属校验（IPC 调用方归属概念消亡）、SEC-14/16 project_root 链。随存活：SEC-01 shell 白名单与 SEC-15 Win32 句柄级文件身份比对（OS 面逻辑，无 IPC 依存）、SEC-08 的 notify 符号链接过滤（OS 面）、SEC-05/11/12/13/17（如上）。消亡项的防护对象不复存在，不做对位重建。
27. **照抄（自有保留）：SEC-18 红线与假值占位符纪律**（定义于 `src-tauri/src/plan_balance/CLAUDE.md` 外部坑/红线节 + 根 CLAUDE.md 硬约束 #14：真实凭据值禁止写入任何 git 追踪文件，测试/文档/夹具一律 `sk-test` 形态假值，真实凭据只存 user 层 `~/.claude/settings.json` 仓库外）。fork 后纪律原样延续，并扩展适用到本片新增凭据面（provider 凭据库、加密备份口令、恢复点）——凡 git 追踪面，恒为假值。

### 凭据存储共享裁定（分片 08 登记归本片）

28. **裁定：plan_balance 与 ai_providers 凭据基建不共享存储**——plan_balance 的 token 所有权属 Claude Code user 层配置（SEC-18 指定唯一真源，即 claude CLI 自身消费的那份），其只读通道的存续理由就是「与 CLI 同 token、零拷贝、零漂移」;ai_providers 凭据库管的是 slTerminal 自管 provider key（assistant/test_provider 的写通道，落 Windows 凭据管理器）。若共享（plan_balance 改读凭据库）= 要求用户把 user 层真实凭据复制出第二份进 slTerminal 管理域——副本漂移、泄露面扩大、且违背 SEC-18 最小化原则。终态双通道并存：plan_balance 只读 user 层（不落盘、不展示、内存即焚），ai_providers 库走凭据管理器；两者的共享面仅采纳点 16–18 的凭据原语封装与 Zeroizing 纪律，不共享凭据本体。

## 不采纳点

1. **SSH 凭据全家，不采纳（已定 SSH 砍）**（`ssh_credentials.rs` 的 `credential_target` `Pebrel/SSH/<destination>`、`username_hint`、`store_password`/`load_stored_password`/`forget_password`、`prompt_password`、`store_private_key_passphrase`/`load_private_key_passphrase`/`forget_private_key_passphrase`、askpass 面 `is_askpass_env`/`run_askpass_from_env` 与 env 双前缀兼容、`windows_store` 的 SSH 门面 `run_askpass`/`prompt_password`/`save_password`/`delete_password`/`load_password`）。`windows_store::{load_secret,save_secret,delete_secret}` 原语保留（采纳点 17），其余 SSH 语义与 SSH target 命名空间全砍。
2. **多平台凭据后端，不采纳（已定 Windows 单平台）**（`platform/credentials.rs` 的 macOS `security_framework` 分支、Linux `secret-tool` 的 `invoke`/`secret_tool`/`valid_secret`、`can_store` 平台分叉）。仅保留 Windows 分支，平台分支代码与 secret-tool 超时轮询等坑一并不迁移。
3. **Nebula→Pebrel 遗留凭据迁移链，不采纳（无历史凭据包袱）**（`platform/credentials.rs::CredentialIdentity::legacy` + `load_with` 双身份回退 + `delete_with` 先删 legacy 再删 current 的删除序；`ssh_credentials.rs` askpass env `NEBULA_*`/`PEBREL_*` 双前缀兼容测试）。slTerminal fork 后凭据域从零起步，无 Nebula/Pebrel 存量可迁；`CredentialIdentity` 收敛为单一 `Slterm` service 单态，迁移链机制不成立。
4. **`prompt_generic_secret` OS 凭据提示框，不采纳**（`ssh_credentials.rs::prompt_generic_secret` + `windows_store::prompt_generic_secret` 的 `CredUIPromptForCredentialsW` + `CREDUI_FLAGS_DO_NOT_PERSIST` 形态）。pebrel 仓内除定义与 SSH 面外无消费者；GPUI 自绘写输入框 + 采纳点 5 的「即收即存即清」纪律可实现等价 DO_NOT_PERSIST 语义，不引入第二套录入面与其测试面。
5. **备份的 SSH 类别与脱敏具象，不采纳（SSH 砍）**（`encrypted_backup.rs::BackupCategory::Ssh`、`add_sanitized_ssh`、`ssh_is_sanitized`、`filter_settings` 的 `ssh_proxy_*` 字面量）。SSH 相关类别与字段字面量随 SSH 砍；「导出清洗 + 导入复验」双闸形态抽象保留（采纳点 11），套到 slterm 自有敏感文件清单。
6. **远端备份推送全家，不采纳**（`backup_remote.rs` 多协议远端推送：本地/UNC 目录、WebDAV Basic 认证、S3 SigV4、SFTP 复用 SSH 会话栈；`KEEP_ARCHIVES` 远端清理序；`pebrel_backup.txt` 远端配置与 WebDAV/S3 凭据 target）。已定安全三件套 = 本地加密备份；SFTP 随 SSH 砍，WebDAV/S3/OAuth 回环不在三件套范围。加密归档格式（采纳点 8/9）与远端推送解耦照抄，远端通道不建；未来若需另行立项。
7. **备份清单的 pebrel 文件名具象，不采纳（清单内容重定）**（`encrypted_backup.rs::collect_from` 的 `nebula_settings.txt`/nebula.lua 系/`terminal_profiles.json`/`session.json`/`directory_history.json` 字面量与 `nebula_history::history_file_names` 引用；`BackupManifest.device` 设备名字段）。机制照抄、清单按 slterm 自有数据文件（settings/history/provider 库等）重定（采纳点 15）；设备名标识随远端推送砍、无本地消费方。

## 优化方向

凭据基建方向：建成「platform 凭据原语 + 域凭据面」两层——platform/credentials 承载 Windows 凭据管理器唯一封装（load/store/delete 三原语 + Win32 原语，target 命名空间归调用方，全门面禁旁路），provider 凭据面在其上叠独立文件 + 引用字段 + 尾四掩码 + 写即清稿 + Zeroizing 纪律；plan_balance 只读通道独立不并库，内存纪律借 pebrel Zeroizing 回补。SEC-18 红线贯穿全程：真实凭据全应用仅两处落点（user 层 settings.json、Windows 凭据管理器）,git 追踪面恒为假值。

加密备份方向：本地加密备份作为 data_dir 自有文件的保护机制重建——AES-256-GCM + Argon2id 管线、魔数 AAD、白名单边界、先验后写、恢复点撤回全链路照抄，slterm 魔数与类别清单重设计；凭据库/token 来源文件/AI 会话库等敏感面永不进备份清单为设计红线（导入导出同一道白名单）；原子写与符号链接防线随 atomic_file 等价物保留；远端推送不建。

plan_balance 方向：查询语义原样（来源判定、双注册表、快照合并、emit 口径、冻结与已用百分比口径、外部 API 实证红线全部保留），执行面换单进程形态——后台轮询走 GPUI 后台执行器、事件推送改订阅模型、配置段随分片 06 settings 形态重定;「token 不出后端」升格为「token 不出凭据域」：类型层（DTO 无 token 字段即编译期拒）+ 内存擦除（Zeroizing 即用即焚）+ 日志不插值三重防线替代原 IPC 边界 + serde 键集测试。

安全审计方向：hooks 域审计三件套（statusline 审查 / Outdated 防篡改 / user 层写入审计）作为 Rust 域逻辑原样存活，审计可观测面保留 tracing target 通道，用户确认交互变 GPUI 原生模态；既有 SEC 清单按「OS 面逻辑保留 / IPC-webview 面消亡」二分归档（采纳点 26)，消亡项的防护对象随载体不复存在，不做对位重建、不残留僵尸校验。

测试方向：凭据面测试随迁——掩码不含全量、删除序不留孤儿密钥、备份 round-trip/错口令失败/路径穿越拒写/重复名拒写/恢复点精确撤回、Win32 原语 ERROR_NOT_FOUND 归一；plan_balance 全量语义测试随模块移植，serde 键集测试升格为类型层断言 + Zeroizing 用例；审计三件套 L1 用例原样保留（tracing-test 捕获 target "audit"、哈希比对检出 Outdated、可疑命令暂停流）。测试体系总体归分片 11。
