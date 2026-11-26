# 持久化、崩溃恢复与加密备份审查

> 审查基线：当前工作树 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060` 的当前工作树；范围限于持久化、崩溃恢复、Document 快照与加密备份，只做静态审查。

## 覆盖与局限

- 已读模块：`slterm_app/src/persistence/`（数据库、schema、元数据、Operation、Journal、关闭意图、项目聚合、ViewState、侧栏、窗口几何、维护请求、启动恢复与维护协调、学习库与写线程、Document 快照信封／存储／写线程／恢复器）、`atomic_file.rs`、`audit.rs`、`logging.rs`、`panic.rs`、`error.rs`、`encrypted_backup.rs`、`encrypted_backup/recovery.rs`、`encrypted_backup/maintenance.rs`，以及 `app_shell` 中启动、恢复、关闭与备份维护接线、`platform/dirs.rs`、`settings_store.rs`、`settings_transfer.rs`。
- 已读文档：根 `CLAUDE.md`、`slterm_app/CLAUDE.md`、`persistence/CLAUDE.md`、`document_snapshot/CLAUDE.md`、`startup/CLAUDE.md`、`encrypted_backup/CLAUDE.md`、`docs/devPlan/design/persistence.md`、`lifecycle.md`、`settings-recovery.md`、`docs/featureSpec/settings-and-persistence.md`、`backup-and-recovery.md`、`docs/devPlan/traceability.md`、`plan.md`、`progress.md` 相关条目。
- 未执行：`cargo`、构建、测试、网络、真实用户数据目录与真实 GUI。涉及 SQLite WAL 行为、Windows junction 判定、DPAPI 跨账户行为与真实崩溃窗口的结论均标注为“疑似”并写明缺口。
- 未审：设置页 UI、更新链、Runtime 协议、终端与编辑器的完整行为；仅在发现持久化/备份副作用时旁及。

## 问题

### 阻断｜Document 崩溃快照只有恢复器，没有生产写入与删除链路

- 置信度：确认
- 位置：`slterm_app/src/persistence/document_snapshot/writer.rs` 的 `DocumentSnapshotWriter`、`DocumentSnapshotClient`；`store.rs` 的 `DocumentSnapshotStore`；`slterm_app/src/app_shell/app_root/side_view.rs` 的 `AppRootPersistence::load`；`docs/devPlan/plan.md` 的「崩溃快照写入与配额」「快照启动恢复器」；`docs/devPlan/traceability.md` 的「快照配额」「异常启动先布局后对账」。
- 现象：生产代码只实例化 `DocumentSnapshotStore` 用于启动扫描与恢复；`DocumentSnapshotWriter`、`DocumentSnapshotClient`、`DocumentSnapshotStore::write`、`reconcile`、`delete_closed` 在 `slterm_app/src` 的非测试代码中没有调用点。编辑器编辑、保存、撤销回干净基线、局部／应用关闭、退出冲刷均不会产生、刷新或删除快照。
- 根因：快照写入与关闭删除原本应由 AppRoot 的编辑事务、保存回执、关闭协调器与退出链接线；当前只有恢复侧的 `SnapshotRecoverer` 被接入 `StartupRecovery`，其余仍是未接线的库代码（`persistence` 整体带 `#[allow(dead_code)]`）。
- 影响：崩溃、强杀或断电时所有未保存正文都不会有恢复材料；配额驱逐、脏文档保护、删除提交边界、无主快照与 7 天保留期在真实产品中不存在。`snapshot_quota_does_not_evict_dirty`、`integration_snapshot_quota`、`snapshot_delete_commit_boundary_crash`、`orphan_snapshot_requires_explicit_target` 等锚只证明未接线代码自身行为，不能证明验收场景成立。
- 证据：全仓检索 `DocumentSnapshotWriter`／`DocumentSnapshotClient` 仅命中模块自身与测试；`AppRootPersistence::load` 只创建恢复器并调用 `StartupRecovery::recover`；`commands.rs` 的 `restore_recovery_panel` 只处理恢复占位；测试 `startup_reconciles_layout_before_snapshot` 由测试直接调用 `DocumentSnapshotStore::write` 造出快照，再交给恢复器。
- 优化方向：把快照写入所有权真正接到唯一编辑事务与关闭主干上，并让配额保护集来自“当前打开且脏”的 Document 集合；恢复器与写入器共用同一 DocumentId 与 TextStateId 事实，才能让现有验收锚变成产品级证据。

### 阻断｜加密备份没有工作区类别，跨机重定位与逻辑工作区导出不存在

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的 `BackupCategory`、`BackupSelection`、`collect_from`、`category_root`；`slterm_app/src/encrypted_backup/maintenance.rs`；`docs/featureSpec/backup-and-recovery.md` 的「备份类别」「整包恢复」；`docs/devPlan/design/settings-recovery.md` 的「BackupPayload 与加密格式」「恢复点与维护期恢复」。
- 现象：实现只有 `Settings`、`Session`、`Themes` 这些类别。`Settings` 是 `settings.json`，`Themes` 是主题目录，`Session` 是 `%LOCALAPPDATA%/slterm/session.json` 的原始字节。没有设计要求的 `workspace` 类别：项目、布局、ViewKey 状态、侧栏、终端元数据与按机器窗口几何都不进包；也没有 `BackupPayload`、逻辑对象投影、来源机器几何标识或 `RelocationRequired` 的后链路。
- 根因：备份实现保留了旧的“文件清单”模型，而设计已切换为按逻辑对象生成 `settings` 与 `workspace` 类别；`session.json` 在 `session.rs` 中标注为生产不读取的旧 schema，不能代表工作区，也不能承载 ViewKey 与项目布局。
- 影响：验收场景「仅选工作区类别时排除设置及永久排除内容」不可能成立，因为工作区类别不存在；跨机恢复工作区、未重定位前不读正文／不启动 PTY 的验收也没有输入面。`backup_workspace_category_exclusions` 测试只断言物理库和若干文件名不在包内，不能替代该场景；`backup_cross_machine_requires_relocation` 另在 `app_shell/app_root/tests/terminal_restore.rs` 用人工构造的 `RelocationRequired` 快照验证通用项目状态，与备份包无关。
- 证据：`BackupCategory` 枚举与 `collect_from` 的分支只覆盖现有类别；全仓检索 `BackupPayload`、`export_workspace`、`portable` 无实现；`collect_from` 对 `Session` 直接 `identity` 拷贝 `session.json`，而 `session.rs` 明确“生产 AppRoot 不读取此格式”。`plan.md` 要求“完成类别选择、永久排除、逻辑工作区导出和跨机几何标记”，追溯矩阵却把对应验收锚登记为已自动化。
- 优化方向：以工作区数据库的已登记 DTO 生成有界、版本化的 `workspace` 投影，按逻辑对象导出并保留来源机器几何标识；不要用旧 `session.json` 代替工作区，也不要在备份层引入第二套布局序列化。

### 阻断｜加密备份没有生产入口、预览、原子导出、跨重启恢复点与撤回

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的模块注释、`collect`、`seal`、`restore`、`summary`；`slterm_app/src/encrypted_backup/recovery.rs` 的 `restore_selected`、`undo`；`slterm_app/src/encrypted_backup/maintenance.rs` 的 `stage_restore`；`slterm_app/src/app_shell` 中设置页接线；`docs/featureSpec/backup-and-recovery.md` 的「加密导出」「整包恢复」「恢复点与撤回」。
- 现象：`slterm_app/src` 的生产代码对 `encrypted_backup` 的引用只有 `app_shell/mod.rs` 里注册启动维护恢复器。没有类别选择、口令输入、导出目标选择、整包预览、排除项展示、恢复预览、关闭预检、原子写用户目标、撤回入口或恢复点列表；`collect`／`seal`／`restore`／`restore_selected`／`undo`／`stage_restore` 都没有 UI 或命令调用方。`encrypted_backup` 模块自身也带 `#[allow(dead_code)]`，其注释承认“恢复与撤回 UI 尚未接线”，同时声称“设置页安全分区的加密备份导出已接”，与代码检索结果不符。
- 根因：备份模块只完成了引擎函数与维护期恢复器，没有完成设置页安全分区的产品接线；`RestorePoint` 是进程内句柄，`restore_selected` 生成的恢复点文件没有写入 `workspace.db` 的索引，重启后没有可枚举的撤回入口。维护期路径在成功后立即清理恢复点与恢复点行，也不保留用户撤回材料。
- 影响：featureSpec 的导出、整包恢复、恢复点撤回、恢复预览、跨重启保留与“导出先原子写用户目标”均不成立；`integration_encrypted_backup`、`backup_undo_fingerprint_conflict`、`backup_restore_does_not_restore_credentials` 只是函数级测试，不能证明产品能力。用户无法发起任何加密备份或恢复操作。
- 证据：全仓检索 `crate::encrypted_backup` 只命中 `app_shell/mod.rs` 的维护恢复器；检索 `BackupSelection`／`seal`／`restore_selected`／`undo` 只命中模块内部与测试；设置页相关文件没有 backup 控件。`RestorePoint.targets` 只存在于内存；`maintenance_request` 的恢复点行在成功 `cleanup` 中删除。
- 优化方向：在唯一设置入口接入类别选择、预览、口令与关闭预检；恢复与撤回只经维护请求，并把恢复点索引持久化到工作区库，使跨重启撤回可由持久事实驱动。导出必须复用同一整包鉴权入口并原子写用户目标。

### 阻断｜`stage_restore` 把解密后的备份条目明文写盘

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/maintenance.rs` 的 `stage_restore`、`StagedRestore`；`slterm_app/src/encrypted_backup/recovery.rs` 的 `write_point`；`slterm_app/src/encrypted_backup/CLAUDE.md` 的「维护期应用与回滚」；`docs/devPlan/design/settings-recovery.md` 的「确认恢复前将已鉴权包复制为本地根暂存原子文件」。
- 现象：`stage_restore` 把筛选后的 `BackupEntry.bytes`（`settings.json`、`session.json`、主题正文的解密字节）序列化为 JSON，写入 `local_root/maintenance/<request_id>/staged.restore`。这是明文；`write_point` 使用 `atomic_file::write` 而非 `write_current_user`，暂存目录也没有当前用户 ACL 收敛。
- 根因：暂存层把“维护期需要可用的条目”直接物化为解密内容，而不是保存已鉴权密文包或受 DPAPI 保护的暂存件；`read_original` 与 `write_point` 的 ACL 规则也被绕过。
- 影响：违反本地纪律“口令与解密明文不落盘”。若设置过滤漏掉某个未知敏感键、或 `Session` 原始文件带有手工凭据，暂存件会成为本机明文凭据落点；同机其他进程／管理员可读。即使当前恢复 UI 未接线，未来接线或测试夹具生成请求后即成立。
- 证据：`stage_restore` 的 `let payload = serde_json::to_vec(&StagedRestore { entries })` 与 `crate::atomic_file::write(&path, &payload)`；`StagedRestore.entries` 直接持有 `BackupEntry.bytes`；`write_point` 调用 `crate::atomic_file::write`。本地 `encrypted_backup/CLAUDE.md` 明确要求“解密明文不落盘”，featureSpec 要求暂存“已鉴权包”。
- 优化方向：暂存已鉴权密文或经 DPAPI 保护的受控材料，维护期再解密使用；如必须落明文，需有当前用户 ACL、明确豁免与清理兜底，并确保不在任何失败路径留下无主明文。

### 阻断｜灾难副本从未生成与刷新，正常安装下主库损坏会走双损隔离

- 置信度：确认
- 位置：`slterm_app/src/persistence/workspace.rs` 的 `WorkspaceDatabase::backup_to_new`；`slterm_app/src/persistence/startup/recovery.rs` 的 `open_workspace_with_recovery`；`slterm_app/src/persistence/workspace_writer.rs` 的关闭路径；`slterm_app/src/app_shell/global_close.rs`；`docs/devPlan/design/persistence.md` 的「灾难副本」。
- 现象：`backup_to_new` 的生产调用点只有 `open_workspace_with_recovery` 在“已有一份有效副本”时生成临时恢复库。正常关闭、成功维护、显式破坏操作提交后都没有把工作区库 Online Backup 到 `workspace.backup.db` 的任务；`WorkspaceRecoveryNotice::RestoredFromBackup` 也不携带副本世代。
- 根因：设计要求的“副本任务”没有落地，副本只在测试或恢复分支被手动创建，正常运行不会生成 `workspace.backup.db`。
- 影响：正常安装上一旦主库物理损坏，实际路径会是“无有效副本 → 双损隔离 → 空工作区”，无法享受设计中“先校验有效副本再 Online Backup 恢复”的保护；`workspace_unrecoverable_isolated` 与 `workspace_missing_disaster_copy_is_not_a_valid_empty_backup` 通过是因为测试自己写了副本或根本没有副本，不能证明正常运行会产生可用副本。恢复分支即使被手工触发，也缺少“副本不是实时镜像”的世代提示。
- 证据：全仓检索 `backup_to_new` 只有 `WorkspaceDatabase` 定义、`open_workspace_with_recovery` 的恢复临时库和测试；`WorkspaceWriter` 的 Shutdown、全局关闭与维护完成路径均无副本任务；`WorkspaceRecoveryNotice` 的现有结果没有世代字段。
- 优化方向：把 Online Backup 作为正常关闭、成功维护和显式破坏操作的显式持久任务，并让恢复提示携带副本的世代或时间事实；每次恢复后按新事实刷新副本，避免副本永久停留在旧世代。

### 阻断｜备份恢复在 `store_epoch` 轮换与 `Applied` 之间崩溃会永久阻断启动

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/maintenance.rs` 的 `BackupRestoreRecoverer::apply`、`roll_back`；`slterm_app/src/persistence/startup/maintenance.rs` 的 `MaintenanceRunner::reconcile_maintenance_requests`、`refresh_epoch`。
- 现象：`apply` 在完成全部目标写入后先调用 `rotate_store_epoch`，再单独调用 `transition_maintenance_request(Applying → Applied)`。两者属于不同事务。若在轮换与终态提交之间崩溃或断电，重启后请求仍是 `Applying`，但 `request.store_epoch` 仍是旧世代，工作区当前世代已是新值。`MaintenanceRunner` 在调用恢复器之前先 `refresh_epoch` 并比较 `request.store_epoch != self.store_epoch`，于是直接返回 `NeedsAttention { MaintenanceRequests }`，根本不会调用 `roll_back`。
- 根因：恢复器的“轮换世代”与“请求终态”不在同一事务，而协调器把“请求世代 ≠ 当前世代”一律当作不可触碰的旧材料。设计明确要求“重启遇到 Applying 只按恢复点回滚、不重放未知副作用”，但这条路径被世代闸门提前截断。
- 影响：一次备份恢复在此窗口崩溃后，启动永久停在维护诊断，重试每次都重复阻断；恢复点行与 DPAPI 文件仍保留，但没有任何代码会消费它们。文件可能已经是新内容、也可能是新内容的一部分，用户既不能完成恢复也不能回滚。
- 证据：`apply` 中 `rotate_store_epoch` 调用先于 `transition_maintenance_request(... Applied)`；`reconcile_maintenance_requests` 先 `refresh_epoch`，随后 `if request.store_epoch != self.store_epoch { return NeedsAttention }`；`roll_back` 只在请求状态为 `Applying` 且通过了前述世代检查时才可能被调用。现有 `maintenance_restore_tests` 覆盖成功应用、中断回滚、篡改阻断与旧世代请求阻断，没有覆盖“轮换后、Applied 前”的崩溃窗口。
- 优化方向：让世代轮换与请求终态在同一持久事务完成，或把“请求记录的旧世代”与“当前世代”的差异纳入恢复器可判定的回滚条件；必须用真实子进程在提交边界崩溃重开验证。

### 阻断｜双损隔离部分失败或中断后，重试会以空工作区正常启动

- 置信度：确认
- 位置：`slterm_app/src/persistence/startup/recovery.rs` 的 `open_workspace_with_recovery`、`quarantine_files`、`quarantine_directory`；`docs/devPlan/design/persistence.md` 的「灾难打开与隔离」。
- 现象：双损分支先把主库、`-wal`、`-shm`、副本及其 sidecar 逐个 `std::fs::rename` 到隔离目录，再调用 `WorkspaceDatabase::open(paths)` 创建新空库。若任一 `rename` 失败（文件被占用、权限、磁盘错误）或进程在改名过程中崩溃，函数返回错误或进程退出；但此时主库可能已经不在原路径。
- 根因：重试路径没有“缺口标记”或“隔离未完成”事实。下一次启动调用 `WorkspaceDatabase::open(paths)` 时，主库缺失会被 `Database::open` 当成首次使用，直接初始化空库并返回 `Clean`，不再进入副本校验或隔离分支。
- 影响：用户重试后看到一个正常的空工作区，原数据虽在隔离目录但没有提示也没有自动恢复；若用户继续工作，空库会成为新的权威库。设计明确要求“隔离或替换失败必须上抛，不得继续以空工作区掩盖故障”，该要求只在首次尝试成立，重试不成立。
- 证据：`quarantine_files` 顺序 `rename` 主库、sidecar、副本；失败即 `?` 返回，已改名的文件不回滚。`open_workspace_with_recovery` 对主库缺失没有额外判定；`Database::open` 的 `is_new` 条件把 `application_id == 0 && user_version == 0 && object_count == 0` 的新建空库视为首次使用。现有测试只覆盖完全成功和完全失败，没有覆盖“主库已移走但副本迁移失败后重试”。
- 优化方向：把“隔离开始／隔离完成”做成持久事实，或在主库缺失但存在未完成隔离目录／副本时拒绝初始化空库并继续诊断；隔离应作为一个整体完成或可恢复，不能把中间态交给下一次首次使用路径。

### 重要｜备份明文没有 64 MiB 上限，条目大小与数量不受边界约束

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的 `seal`、`open`、`decrypt_bytes`、`validate_archive`、`collect_from`；`docs/devPlan/design/settings-recovery.md` 的「新格式使用……明文上限 64 MiB」；`docs/featureSpec/backup-and-recovery.md` 的「加密导出」。
- 现象：`seal` 只校验版本与条目合法性，随后把整个归档序列化进内存并加密；`open` 解密整包后反序列化；`collect_from` 读取完整 `settings.json`／`session.json`／主题文件；`validate_archive` 不限制单条目、总条目或总明文大小。没有任何 64 MiB 上限检查。
- 根因：大小边界只写在文档里，代码路径没有对应的长度判定，也没有流式或分块处理。
- 影响：恶意或损坏的备份包可以在解密前占用与包大小同量级的内存，解密后再放大成 JSON 结构；导出也可把超大设置或主题整体读入。对本机导出是资源消耗风险，对导入是拒绝服务面。验收场景中的“大小上限”没有对应实现与测试。
- 证据：`encrypt_bytes`、`decrypt_bytes` 无长度检查；`validate_archive` 的每个 `allowed` 分支只校验内容形态；`collect_from` 的 `add_file` 直接 `fs::read`；测试没有超大包或超大条目用例。
- 优化方向：在鉴权前限制包总长，在解密后、落盘前限制明文总长与单条目长度；对收集侧也应用同一上限，并把拒绝作为整包失败而非截断。

### 重要｜设置导出使用敏感键黑名单而不是登记字段白名单，Session 原文完全未过滤

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的 `filter_sensitive_document`、`strip_sensitive_keys`、`SENSITIVE_KEYS`、`add_file`、`add_themes`；`docs/devPlan/design/settings-recovery.md` 的「未注册或无法证明非敏感的字段不入包」；`docs/featureSpec/backup-and-recovery.md` 的「未注册或无法证明非敏感的字段不按应用设置导出」。
- 现象：`Settings` 类别只删除键名命中 `SENSITIVE_KEYS` 的字段，其余所有未知命名空间与未知字段原样保留；测试 `collection_uses_allowlist_and_strips_sensitive_keys` 还断言 `handmade` 未知段“原样保留”。`Session` 类别直接拷贝 `session.json` 的全部字节，只要求是 JSON 对象，没有任何敏感键剥除或登记字段校验；`session.json` 的 `sidebar` 段是 `RawValue`，可以携带任意嵌套字段。`settings.json`／`session.json` 的收集也没有像主题那样检查符号链接。
- 根因：设置导出把“敏感”当成黑名单问题，而设计与 featureSpec 要求的是可证明非敏感的登记白名单；`Session` 是未登记类别，复用了原始文件通道。
- 影响：任何不在表内的凭据键（例如 `bearer`、`cookie`、`signing_key`、供应商自定义字段）都会进入备份；`Session` 中的手工字段不受任何过滤。若用户把包外发，可能造成凭据泄漏。符号链接的 `settings.json`／`session.json` 还会把根外文件内容带入包内，违反“白名单即安全边界”。
- 证据：`strip_sensitive_keys` 递归删除匹配键后原样返回；`document_is_sanitized` 只在导入复验时复用同一黑名单；`BackupCategory::Session` 走 `identity` 转换；`add_file` 没有 `symlink_metadata` 检查；`SENSITIVE_KEYS` 是有限字面量表。
- 优化方向：设置类别改用注册表导出白名单（与 `settings_transfer::export_settings` 同源），未知字段导出时剥除或整单拒绝；删除未登记的 `Session` 原始文件类别，或把它替换为有登记 DTO 的工作区投影；所有收集目标先做身份与符号链接／junction 检查。

### 重要｜恢复点与暂存目录没有当前用户 ACL 限制

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/recovery.rs` 的 `write_point`、`restore_selected_at`；`slterm_app/src/encrypted_backup/maintenance.rs` 的 `stage_restore`；`slterm_app/src/atomic_file.rs` 的 `write_current_user`、`write`；`docs/devPlan/design/settings-recovery.md` 的「恢复点正文使用 Windows DPAPI CurrentUser 加密，写入本地根原子文件并限制当前用户 ACL」。
- 现象：只有 Document 快照调用 `write_current_user`。恢复点文件、DPAPI 包与 `staged.restore` 都走 `crate::atomic_file::write`，即“复制目标 ACL 或继承父目录 ACL”；没有把 DACL 收敛为当前用户，也没有切断继承。
- 根因：ACL 收紧只在 `write_current_user` 中实现，备份模块没有复用它。
- 影响：恢复点虽经 DPAPI 保护，但 ACL 仍允许管理员／SYSTEM 之外的继承 ACE 读取密文；暂存明文（见 `stage_restore` 问题）则直接暴露给同一文件的继承读者。featureSpec 的显式 ACL 要求未满足。
- 证据：`write_point`、`restore_selected_at`、`stage_restore` 均调用 `crate::atomic_file::write`；`write_current_user` 只被 `DocumentSnapshotStore::write` 使用。
- 优化方向：恢复点、暂存材料与任何含原值／正文的备份文件统一使用当前用户 ACL 写入；对目录也建立受控 ACL，而不是只控制单文件。

### 重要｜暂存明文先于 owner 落盘，清理先清行后删文件，失败后不再构成阻断事实

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/maintenance.rs` 的 `stage_restore`、`BackupRestoreRecoverer::cleanup`、`clear_recovery_points`、`remove_file`；`slterm_app/src/persistence/maintenance_request.rs` 的 `clear_recovery_points`。
- 现象：`stage_restore` 先把明文 `staged.restore` 写入本地根，再读取工作区元数据并登记维护请求；若元数据读取失败或运行元数据缺失，函数直接返回错误，暂存明文没有 owner 也没有清理。成功应用后的 `cleanup` 先 `clear_recovery_points`，再删除 DPAPI 恢复点文件与 staged 文件；若文件删除失败，函数返回错误，但行已清空、请求已是终态。下一次启动 `unfinished_maintenance_requests` 不再包含该请求，`recovery_points` 也为空，维护期直接放行，遗留的加密恢复点或无主明文不再阻断。
- 根因：暂存材料在持久 owner 建立之前落盘；清理顺序把“行事实”当成先决条件，却没有把“文件未删”保留为新的持久阻断事实。
- 影响：违反“staged 内容与恢复点行必须双向一致”“清理失败保留阻断事实，不得静默忽略”。无主明文可能长期留在本地根；加密恢复点文件也可能成为无索引材料。诊断时只能靠目录扫描发现。
- 证据：`stage_restore` 的 `atomic_file::write` 早于 `store.metadata()` 与 `register_maintenance_request`，只有登记失败分支删除暂存文件；`cleanup` 的调用顺序为 `clear_recovery_points` → `remove_file(point)` → `remove_file(staged)`；请求在 `Applied`／`RolledBack` 后即被未完成枚举过滤。
- 优化方向：暂存材料只有在 owner 可持久登记后才具备落盘资格，或让启动侧能发现并收编无主暂存；清理以“材料删除完成”作为终态前提，删除失败应留下可重试的持久阻断事实，而不是先清行。

### 重要｜即时恢复在指纹回写失败时已写盘却返回失败，恢复点随即不可撤回

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/recovery.rs` 的 `restore_selected_at`、`file_fingerprint`、`undo_at`。
- 现象：`restore_selected_at` 先写 DPAPI 恢复点，再 `restore_to` 写所有目标；成功后逐项读取恢复后指纹并重新加密恢复点。如果 `restore_to` 成功但后续 `file_fingerprint` 或 `encode_point`／`atomic_file::write` 失败，函数返回 `Err`，但目标文件已经是恢复后的内容，恢复点仍是 `after: None`。
- 根因：恢复提交与“恢复后指纹”提交不是同一事务；失败路径没有回滚已写目标，也没有把恢复点标记为可用。`undo_at` 判断“可撤回”依赖 `after` 指纹，`after: None` 时当前文件既非原值也非已记录恢复值，会被当作冲突。
- 影响：调用方看到“恢复失败”，用户可能重试；重试会重新采原现场（此时原现场已是新内容），进一步覆盖唯一可回退材料。恢复点无法再按指纹撤回。这违反“恢复失败自动回滚”和“恢复结果与回滚失败逐项可见”。
- 证据：`restore_selected_at` 的顺序是 `restore_to(paths, &selected)?` → 循环 `file_fingerprint` → `atomic_file::write(point, packet)?`；任一步 `?` 直接返回，没有 `write_originals` 回滚；`undo_at` 的 `current == file.after` 比较在 `after: None` 时只能匹配“文件不存在”。
- 优化方向：把“恢复成功”与“恢复后指纹提交”做成同一持久步骤，或在指纹提交失败时回滚已写目标；恢复点在指纹未落定前不得被视为可撤回。

### 重要｜撤回只返回单一错误，逐项成功／冲突／失败不可见，成功后恢复点不失效

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/recovery.rs` 的 `undo_at`、`write_originals`；`docs/featureSpec/backup-and-recovery.md` 的「恢复点与撤回」。
- 现象：`undo_at` 先把“当前指纹等于恢复后指纹”的项收集为 `matched`，再 `write_originals(matched)`；若存在 `conflicts`，在写完后返回一个包含文件名列表的 `String` 错误。成功项数量、失败项、写入中途失败都没有结构化结果；`write_originals` 逐项写，任一项失败即提前返回，后续项不再处理。成功撤回也不删除恢复点文件，重复撤回只能靠“已是原现场”分支空操作。
- 根因：撤回 API 用“总体 `Result<(), String>`”表达部分成功；恢复点没有显式的“已撤回／已消费”状态。
- 影响：UI 无法逐项报告成功、冲突与失败；用户可能把“有冲突”误读为“全部失败”，也可能不知道哪些项已经回滚。部分写失败后没有逐项失败清单，恢复点仍保留但状态不明。
- 证据：`undo_at` 的冲突路径返回 `Err(format!("撤回冲突：……"))`；`write_originals` 使用 `?` 在任一失败处返回；`RestorePoint` 没有状态字段，成功后无删除或标记。
- 优化方向：撤回返回逐项结果（目标、结果、指纹、错误类别），冲突不阻断其他项；恢复点增加显式状态并在完整成功后消费或保留为可审计材料。

### 重要｜备份恢复、应用、回滚与撤回没有审计／恢复日志，设置写绕过 settings_store 与 audit

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/maintenance.rs` 的 `stage_restore`、`apply`、`roll_back`、`cleanup`；`slterm_app/src/encrypted_backup/recovery.rs` 的 `restore_selected_at`、`undo_at`；`slterm_app/src/audit.rs`；`slterm_app/CLAUDE.md` 的「审计通道」「设置写盘单点」；`docs/featureSpec/backup-and-recovery.md` 的「恢复日志只记录操作、来源、结果与必要指纹」。
- 现象：整个备份模块只在主题收集失败时发出一条 `backup_invalid_theme` 审计。恢复请求登记、staged 写入、原现场采样、逐项应用、世代轮换、回滚、撤回与清理都没有 audit 事件。恢复目标中的 `settings.json` 由 `crate::atomic_file::write` 直接写，不经过 `settings_store` 的白名单校验、1 MiB 限制、`.bak` 与审计包装；`write_originals` 撤回设置时同样直写。
- 根因：备份恢复被当作独立文件通道，未接入应用层的审计与设置写纪律；恢复日志只存在于错误字符串中。
- 影响：安全写事件无审计事实，事后无法证明恢复／撤回发生、影响了哪些对象、结果如何；恢复写入的设置可绕过 1 MiB 上限与敏感键复验中的设置注册表语义。若“设置写盘单点”适用于恢复，这是第二写通道；若不适用，也缺少显式豁免与审计替代。
- 证据：`rg 'audit::record' encrypted_backup` 只命中 `add_themes` 的坏主题；`apply` 中目标写入是 `crate::atomic_file::write(&target, &entry.bytes)`；`stage_restore` 与 `undo` 无 audit 调用；`settings_store::save_settings_blocking_at` 未被备份模块使用。
- 优化方向：恢复／撤回的每个安全写事件只经 `audit` facade 记录 operation／path／source／result 与必要指纹；恢复设置要么经 settings_store 的统一约束，要么登记为受控替代并保持 1 MiB 与敏感键复验。

### 重要｜审计成功事件在 Warn 级日志过滤下被丢弃

- 置信度：确认
- 位置：`slterm_app/src/logging.rs` 的 `initialize`、`Logger::log`、`enabled`、`is_allowed_target`；`slterm_app/src/audit.rs` 的 `record`。
- 现象：`logging::initialize` 把全局最大级别设为 `Warn`；`Logger::log` 先执行 `self.enabled(record.metadata())`（`level <= max_level`），Info 级事件在进入 `is_allowed_target` 之前就被丢弃。`audit::record` 对 `AuditResult::Ok` 与 `Acknowledged` 使用 `tracing::info!`，因此设置写成功事件、恢复成功事件等常态审计不会出现在日志里；只有 `Error`／`Suspended`／`Mismatch` 的 `warn!` 事件会落盘。
- 根因：审计通道把“成功即 info”与日志全局档位绑定；审计模块与 logging 模块都声称 audit target 不因档位静默，但过滤发生在 target 判定之前。
- 影响：审计通道只在失败时留下记录，成功的安全写没有取证事实；`audit_tests` 使用 tracing 作用域 subscriber 直接捕获，绕过了真实 Logger，因此无法发现该问题。
- 证据：`Logger::log` 的早退条件；`AuditResult::Ok` 使用 `tracing::info!`；`Cargo.toml` 启用了 `tracing/log-always`，事件确实桥接进 `log`；`logging_tests` 只断言 `is_allowed_target`，未测试 Info 审计经过 Logger。
- 优化方向：对 `audit` target 在级别过滤前无条件放行，或把成功审计提升到日志档位可覆盖的级别；测试必须经过真实 Logger 而不是仅作用域 subscriber。

### 重要｜根外 Document 的会话重授权没有生产调用点，授权占位不可操作

- 置信度：确认
- 位置：`slterm_app/src/persistence/document_snapshot/recovery.rs` 的 `DocumentSnapshotRecoverer::reauthorize`、`requires_authorization`；`slterm_app/src/app_shell/app_root/commands.rs` 的 `restore_recovery_panel`；`slterm_app/src/app_shell/app_root/document_recovery.rs` 的 `DocumentRecoveryView`；`docs/devPlan/design/document.md` 的「启动对账与授权」；`docs/featureSpec/path-policy.md` 的「根外文件重启后只显示重新授权占位」。
- 现象：恢复器可以把根外／无授权的快照正文恢复成 `read_revoked` 会话，并把 DocumentId 放入 `requires_authorization`；UI 只安装一个显示 `EditorReauthorizationRequired` 的静态占位。`reauthorize` 只被测试调用，生产没有任何命令或按钮触发会话授权、重新读盘、基线复核与冲突判定。根外无快照记录同样只得到占位，没有重授权入口。
- 根因：恢复器 API 与 UI 占位之间缺少命令接线；`DocumentExternalService` 的 `reauthorize_external_change` 只处理外部变化回执，不是启动恢复的显式会话授权。
- 影响：根外恢复的正文保存在内存 Registry 中但用户看不到、也无法重新授权；重启后这些草稿对用户不可达。验收锚 `integration_session_grant_restore` 只证明恢复器函数可重授权，不证明产品入口存在。
- 证据：全仓检索 `reauthorize(` 仅命中恢复器定义、恢复器内部调用与测试；`DocumentRecoveryView` 的 `render` 只有一个 label，没有按钮或 action；`commands.rs` 对 `document_requires_authorization` 分支只调用 `restore_recovery_panel(... Authorization)`。
- 优化方向：把授权占位接到真实显式命令，复用恢复器的 `reauthorize` 以复核目标与基线、保持 DocumentId／TextStateId 不变；恢复器与 UI 共享同一授权事实，不能在占位层另读盘。

### 重要｜布局或快照领域恢复失败时静默以空工作区开窗，快照位置不一致直接导致整次恢复失败

- 置信度：确认
- 位置：`slterm_app/src/persistence/document_snapshot/recovery.rs` 的 `recover_documents`；`slterm_app/src/persistence/startup/recovery.rs` 的 `StartupRecovery::recover`；`slterm_app/src/app_shell/app_root/side_view.rs` 的 `AppRootPersistence::load`；`slterm_app/src/app_shell/windowing.rs` 的 `open_window` 调用链；`docs/devPlan/design/lifecycle.md` 的「维护期」。
- 现象：`recover_documents` 在快照位置与布局记录位置不一致时返回错误，导致 `SnapshotRecoveryError::Failed`、`StartupRecoveryError::Snapshot`；`AppRootPersistence::load` 随即设置 `project_snapshot_recovery_failed` 并只写 `log::warn`，随后窗口照常以默认空 `ProjectSnapshot` 打开，写屏障阻止普通项目写。布局恢复本身失败时同样如此。没有脱敏用户提示，也没有进入启动诊断分支。
- 根因：领域恢复错误被当作“可继续的降级”，而启动设计要求“维护或执行失败只开放只读诊断与重试”；同时快照位置与布局位置的一致性被当成硬错误，而不是冲突或可归一材料。根重定位、外部改名或布局提交先于快照重写都可能合法地造成位置不一致。
- 影响：用户会遇到一个没有项目、没有提示、且不能正常保存的空窗口；原记录虽在库中，但普通操作被屏障阻止，只能靠日志猜测。快照位置不一致还会让所有 Document 的恢复整体失败，而不是只影响该 Document。
- 证据：`recover_documents` 的 `snapshot.location().canonical_path() != location.canonical_path()` 直接 `return Err(io::Error::new(InvalidData, ...))`；`side_view.rs` 的 `Err(error) => { persistence.project_snapshot_recovery_failed = true; log::warn!(...) }`；`windowing.rs` 仅用该标志决定是否打开初始 cwd；`project_snapshot_recovery_failed` 在 UI 中没有提示路径。现有测试 `snapshot_recovery_error_is_not_reported_as_ready` 只验证恢复 API 返回错误，不验证组合根的用户体验。
- 优化方向：区分单个快照材料异常、布局损坏与数据库不可读；前者按冲突／隔离降级并继续其他 Document，后者进入统一诊断分支并给出脱敏提示，不应以空工作区静默继续。快照位置变化应按显式重定位／冲突处理，而不是硬失败。

### 重要｜未完成关闭意图、NeedsAttention 维护请求与遗留恢复点只有阻断，没有解决路径

- 置信度：确认
- 位置：`slterm_app/src/persistence/startup/maintenance.rs` 的 `MaintenanceRunner::run`；`slterm_app/src/app_shell/startup_diagnostics.rs` 的 `retry`；`slterm_app/src/app_shell/crash_recovery.rs` 的 `close_commit_crash_reopens_with_exact_persistent_stage`；`slterm_app/src/persistence/CLAUDE.md` 的「关闭意图持久合同」「维护请求与恢复点」；`docs/devPlan/design/lifecycle.md` 的「维护期」。
- 现象：任何未完成的关闭意图、`NeedsAttention` 维护请求、未知 Journal kind，或 `maintenance_restore` 失败后仍存在的 `recovery_points` 行，都会让 `MaintenanceRunner` 返回 `NeedsAttention`，组合根只提供带“重试”的只读诊断窗口。重试重新执行同一启动链，仍会得到同一阻断；没有“继续完成原持久状态”“显式放弃”“清理恢复点”或“重新登记”的生产入口。`close_commit_crash_reopens_with_exact_persistent_stage` 对 `state_committed`／`pty_stopped`／`cleanup_committed` 明确断言 `NeedsAttention`。
- 根因：设计只规定“未知材料只阻断”“关闭意图不推进状态”，却没有为阻断事实指定所有者或用户动作；`close_intents`、`maintenance_requests`、`recovery_points` 的解除只在原进程关闭链或备份恢复器内部完成，启动侧没有对应恢复器。
- 影响：一次在关闭提交后、PTY 停止前／清理前／最终完成前的崩溃，都会让应用之后永久无法正常启动；备份恢复在 Applying 后失败或清理失败也会造成同类永久阻断。用户只能手工改库或等待新版本。
- 证据：`MaintenanceOutcome::NeedsAttention` 在 `app_shell/mod.rs` 直接映射为 `MaintenanceNeedsAttention`，诊断视图只有 `startup-diagnostics-retry`；`unfinished_close_intents` 只被维护检查读取，未被任何恢复器推进；`recovery_points` 遗留检查在请求列表为空之后直接阻断，`clear_recovery_points` 没有启动侧调用点。
- 优化方向：为每类阻断事实明确“继续原持久状态”“回滚”“显式放弃并保留诊断材料”的所有者与用户动作，并让重试进入同一决策链而不是重复阻断；若设计坚持只阻断，至少要在文档中登记不可自动化的人工解除步骤并让诊断窗口展示该步骤。

### 重要｜更高版本数据库先以读写连接打开，`query_only` 在读取版本之后才设置

- 置信度：疑似
- 位置：`slterm_app/src/persistence/database.rs` 的 `Database::open`；`slterm_app/src/persistence/workspace.rs` 的 `workspace_higher_version_is_query_only_and_not_downgraded`。
- 现象：`Database::open` 用 `Connection::open(path)` 以默认读写方式打开文件，读取 `application_id`／`user_version`／`sqlite_master` 后，才在更高版本分支执行 `pragma_update(query_only, true)`。在 WAL 模式下，打开连接本身可能触发 WAL 恢复或创建 `-shm`／`-wal`，这些动作发生在 `query_only` 之前。
- 根因：只读意图靠连接级 pragma 事后表达，文件打开模式没有按“先判定、后只读打开”设计；`query_only` 也不能阻止 SQLite 在打开阶段恢复 WAL。
- 影响：设计要求高版本“只读，不降级、不隔离、不覆盖原数据”。若打开阶段对 WAL 做了恢复写，主库或 sidecar 的事实可能被修改；当前测试只比较无 WAL 情况下的主库字节，无法证明这一点。
- 证据：`Connection::open` 早于 `pragma_update(query_only)`；`workspace_higher_version_is_query_only_and_not_downgraded` 在打开前先让 raw 连接正常关闭（checkpoint），没有保留 `-wal`／`-shm`，也没有断言 sidecar 字节。缺的证据是：带真实 WAL 的更高版本库在打开前后主库与 sidecar 字节完全不变。
- 优化方向：只读版本探测与只读打开必须共用同一合同，不能先读写打开再补 `query_only`；用真实 WAL 文件的字节比对验证“打开即不写”。

### 重要｜设置 IO／权限失败回退默认值，未进入只读与重试；设置文件没有高版本判定

- 置信度：确认
- 位置：`slterm_app/src/settings_store.rs` 的 `load_settings_at`、`load_raw_settings_lenient`、`load_runtime_settings_at`；`slterm_app/src/app_shell/config.rs` 的 `Settings::load`；`slterm_app/src/settings_state.rs` 的 `settings_damage_classification`；`docs/featureSpec/settings-and-persistence.md` 的「损坏与不兼容分流」。
- 现象：`load_settings_at` 在 `std::fs::read` 的权限／锁／IO 错误上返回 `Err`，但 `Settings::load` 捕获后 `log::warn` 并使用默认值；`load_raw_settings_lenient` 与 `load_runtime_settings_at` 直接 `unwrap_or_default()`。没有任何“保留原数据、只读、可重试”的 UI 状态或写屏障。设置文件本身没有版本字段或高版本探测；`settings_damage_classification` 只覆盖缺失、单字段非法与结构损坏，没有覆盖权限／锁／IO 与更高版本。
- 根因：启动路径选择“不把读取故障升级为启动失败”，把 IO 错误折叠为默认值；设计中的高版本分流缺少格式载体。
- 影响：权限被拒或文件被锁时，用户看到默认设置而不是“当前无法读取、保持原数据”；如果用户在设置页操作，写入会失败，但此前的展示已经误导。高版本设置无法按“只读、不覆盖”分流，未来字段只能被当作未知命名空间原样搬运。
- 证据：`Settings::load` 的 `Err(error) => { log::warn!(...); Self::default() }`；`load_raw_settings_lenient` 的 `unwrap_or_default`；`settings_damage_classification` 的用例范围；设置文档没有版本键的读取或拒绝逻辑。
- 优化方向：把读取故障与缺失分开，IO／权限失败进入只读展示与重试，不生成默认运行值；为设置文档引入显式版本判定，未知高版本只读并拒绝写入。

### 重要｜追溯矩阵把函数级、未接线或不同场景的测试当成已自动化验收锚

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的「加密备份与恢复」「设置与持久化」表；`docs/devPlan/progress.md` 的「设置、主题与恢复」验收行；对应测试 `backup_workspace_category_exclusions`、`backup_cross_machine_requires_relocation`、`integration_encrypted_backup`、`backup_undo_fingerprint_conflict`、`settings_damage_classification`、`snapshot_quota_does_not_evict_dirty`、`snapshot_delete_commit_boundary_crash`、`orphan_snapshot_requires_explicit_target`、`integration_session_grant_restore`。
- 现象：`backup_workspace_category_exclusions` 不测试“仅选工作区类别”，因为不存在工作区类别；`backup_cross_machine_requires_relocation` 使用人工构造的 `RelocationRequired` 快照，不经过备份包；`integration_encrypted_backup` 只串接函数；`backup_undo_fingerprint_conflict` 只验证函数返回冲突；`settings_damage_classification` 不覆盖权限／锁／IO 与高版本；快照系列测试使用未接入生产的 `DocumentSnapshotStore`／`DocumentSnapshotWriter`；`integration_session_grant_restore` 只验证恢复器 API。`progress.md` 仍登记这些能力“已验收”。
- 根因：验收锚按“函数存在且测试通过”登记，没有按 featureSpec 的前置条件、动作、可观察结果三者一致核对；未接线的生产路径没有产品级证据。
- 影响：验收与全量核验的证据链失真；读者无法从矩阵判断哪些能力真实可用。后续修复会以为已覆盖，重复遗漏。
- 证据：`backup_workspace_category_exclusions` 的断言集合；`terminal_restore.rs` 的 `backup_cross_machine_requires_relocation`；`encrypted_backup/recovery.rs` 的 `integration_encrypted_backup`；`settings_state.rs` 的 `settings_damage_classification`；`document_snapshot/tests.rs` 对 `DocumentSnapshotStore` 的直接使用。
- 优化方向：每个验收锚必须经过真实产品入口或明确的维护期入口；删除“不同场景借名”的锚，补充产品级集成测试，并把未接线能力标回待实现或延期。

### 重要｜崩溃与边界测试缺少关键窗口

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup/maintenance.rs` 的 `maintenance_restore_tests`；`slterm_app/src/encrypted_backup.rs` 的 `encrypted_backup_tests`；`slterm_app/src/persistence/startup/recovery.rs` 的测试；`slterm_app/src/persistence/document_snapshot/tests.rs`；`slterm_app/src/app_shell/crash_recovery.rs`。
- 现象：没有覆盖以下场景的自动化或真实子进程证据：`store_epoch` 轮换与 `Applied` 之间的崩溃；主库损坏但带有效 `-wal`／`-shm` 时的副本恢复；双损隔离中途失败后的重试；`stage_restore` 明文落盘与无主暂存清理；64 MiB／超大条目边界；符号链接／junction 父链；恢复点指纹回写失败后的回滚；撤回逐项失败；更高版本库带 WAL 的只读；设置权限／锁／IO 与高版本分流；工作区类别的跨机备份恢复。
- 根因：测试集中于函数级 happy path 与少数注入失败，没有把每个持久提交边界、外部文件身份边界和用户可见结果组合起来；部分关键路径因生产未接线而无法有产品级测试。
- 影响：上述阻断／重要问题都不会被门禁发现；“全绿”与真实行为脱节。
- 证据：上述测试模块的用例列表与崩溃测试只覆盖关闭意图提交边界、快照删除提交边界与元数据 WAL；`settings_damage_classification` 与 `backup_workspace_category_exclusions` 的范围见前文。`close_commit_crash_reopens_with_exact_persistent_stage` 断言 `NeedsAttention` 而未验证解除路径。
- 优化方向：为每个持久边界增加真实子进程崩溃重开；为备份增加包大小、明文落盘、符号链接、指纹更新失败与撤回部分失败用例；把“阻断后如何解除”作为测试显式断言，而不是只断言阻断。

### 一般｜快照存储在运行期遇到任意损坏文件即阻断全部新写入；保存删除最长可延迟一小时

- 置信度：确认
- 位置：`slterm_app/src/persistence/document_snapshot/store.rs` 的 `write`、`scan`、`scan_entries`、`delete_validated`；`slterm_app/src/persistence/document_snapshot/writer.rs` 的 `PendingSnapshot::Delete`、worker 的 `recv_timeout` 计算。
- 现象：`write` 先 `self.scan()?`，`scan` 在发现任何非本目录命名或解码失败的 `.snap` 时返回 `Corrupt`，导致所有 Document 的新快照写入失败，直到下次启动隔离该文件。`PendingSnapshot::Delete` 没有到期时间，worker 的等待时长只取 `Write` 的 due；没有新命令时删除最多等待固定的一小时超时。
- 根因：配额扫描与合法性扫描共用同一 `scan`；删除队列没有 due 字段，也没有参与唤醒。
- 影响：运行期一次外部损坏或残留文件即可让全局崩溃保护失效（用户看不到明显错误，除非失败转发器提示）；已保存文档的冗余快照长期滞留，增加磁盘占用与启动扫描成本。
- 证据：`write` 的 `let mut stored = self.scan()?`；`scan_entries` 的 `corrupt` 分支在 `allow_corrupt=false` 时直接 `return Err`；`next_wait` 只遍历 `PendingSnapshot::Write` 的 `due`；`Delete` 只在收到下一条命令或 Flush／Shutdown 时执行。
- 优化方向：配额扫描与损坏扫描分离，损坏文件只隔离／跳过而不阻断其他 Document；删除项设置明确的到期或直接唤醒，避免依赖下一条命令。

### 一般｜快照文件名与头身份不一致时被当作损坏隔离，违背“文件头才是权威”

- 置信度：确认
- 位置：`slterm_app/src/persistence/document_snapshot/store.rs` 的 `scan_entries`；`slterm_app/src/persistence/document_snapshot/envelope.rs` 的 `DocumentSnapshot::decode`；`docs/devPlan/design/persistence.md` 的「Document 快照」。
- 现象：`scan_entries` 只接受 `self.path(snapshot.document_id()) == path` 的文件；一个头部完整、正文散列正确但文件名不匹配的快照会被归入 `corrupt`，启动时被隔离，而不是按头部的 DocumentId 恢复到对应 Document。
- 根因：扫描层把文件名当成第二身份，覆盖了“文件名只是定位提示，文件头才是权威”的设计。
- 影响：合法但被改名、复制或恢复工具重命名的快照无法恢复，用户的草稿只能从隔离目录手工找回；与“文件名只是提示”的合同不一致。
- 证据：`scan_entries` 的 `filter(|snapshot| self.path(snapshot.document_id()) == path)`；`read(id)` 只按期望路径读取，不会发现改名文件。
- 优化方向：扫描以头部 DocumentId 为准，文件名冲突或重复时按确定规则隔离多余副本；至少不要把可解码且散列正确的材料当作损坏。

### 一般｜`snapshot_index` 与历史隔离表没有生产写入，维护期的“快照扫描与索引重建”环节实际为空

- 置信度：确认
- 位置：`slterm_app/src/persistence/workspace.rs` 的 `snapshot_index`、`quarantine_batches`、`quarantine_items`；`slterm_app/src/persistence/startup/maintenance.rs` 的 `MaintenanceTable::SnapshotIndex`、`HistoryQuarantine`；`docs/devPlan/design/lifecycle.md` 的「维护期」。
- 现象：生产代码只在这些表存在记录时阻断，没有任何代码写入 `snapshot_index` 或 `quarantine_*`。快照恢复器直接扫描目录，不重建索引；历史隔离恢复器也未接这些表。
- 根因：索引表与恢复扫描没有共用写入点；表被保留为“可重建索引”，但重建从未发生。
- 影响：该维护环节名义上存在，实际永不放行额外检查；如果未来有代码按索引期望工作，事实源会缺失。当前用户可见影响较小，但设计声明的“扫描与索引重建”没有实现证据。
- 证据：全仓检索 `snapshot_index` 与 `quarantine_batches` 只命中 schema、维护检查与测试；`recover_documents` 不写索引。
- 优化方向：明确索引是运行时缓存还是持久事实；若保留表，应由同一快照恢复器在扫描后重建并保持与正文一致，或删除表与对应维护环节。

### 一般｜主日志写入系统临时目录而不是本地数据根

- 置信度：确认
- 位置：`slterm_app/src/logging.rs` 的 `OnDemandLogFile::new`、`SLTERM_LOG` 发布；`docs/devPlan/design/persistence.md` 的「配置根保存……本地根保存……日志和维护请求」。
- 现象：主日志文件是 `%TEMP%/slterm-<pid>.log`，不是 `local_root` 下的日志；`debug_log` 与 panic 日志反而写本地根。日志文件不进入任何清理或保留规则。
- 根因：日志路径沿用旧实现，未随双数据根设计重定。
- 影响：日志可能被系统临时目录清理策略删除，或在多用户／漫游场景落在非预期位置；与数据根边界文档不一致，诊断收集需要额外特判。
- 证据：`OnDemandLogFile::new` 的 `let mut path = env::temp_dir(); path.push(format!("slterm-{}.log", process::id()));`；`debug_log` 与 `panic::attach_handler` 使用 `platform::dirs::local_dir()`。
- 优化方向：日志固定到本地根并纳入保留／清理规则，或明确登记临时日志的理由与采集方式。

### 一般｜备份归档 DTO 接受未知字段，空类别集也能解封成功

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的 `BackupArchive`、`BackupManifest`、`BackupEntry`、`validate_archive`、`restore_to`；`slterm_app/src/encrypted_backup/maintenance.rs` 的 `StagedRestore`；`docs/devPlan/design/persistence.md` 的「规范 DTO 与版本」。
- 现象：`#[derive(Deserialize)]` 的备份结构没有 `deny_unknown_fields`，未知字段被静默忽略；`manifest.categories` 允许为空，`entries` 允许为空，`restore_to` 对空归档返回 `Ok(())`；即时恢复路径还会为空归档写一个没有目标的恢复点并报告成功。`StagedRestore` 同样没有版本或未知字段拒绝。
- 根因：备份格式的 DTO 按“宽松读取”实现，而设计对规范 DTO 要求未知字段与未知变体拒绝；空选择/空材料只在 `stage_restore` 与 `collect_from` 被拒绝，`open`/`restore` 没有同一边界。
- 影响：版本演进的未知附加字段不能被识别为不支持；空包或空类别恢复会给出虚假的成功信号；与“完整鉴权 → 全量校验 → 落盘，任一条不过整单拒写”的严格性不一致。
- 证据：相关结构均无 `deny_unknown_fields`；`validate_archive` 不要求 `categories` 非空；`restore_to` 的循环在零条目时直接 `Ok`；`restore_selected_at` 随后仍写恢复点并返回句柄。
- 优化方向：备份外部格式显式声明未知字段拒绝、非空类别与条目下限，并让即时恢复与维护期恢复共享同一套 preflight。

### 一般｜学习写线程遇到一次瞬时数据库错误即永久停止持久学习，直到显式清空

- 置信度：确认
- 位置：`slterm_app/src/persistence/learning_writer.rs` 的 `run_writer`、`LearningWriterClient::status`；`docs/devPlan/design/persistence.md` 的「learning.db」。
- 现象：`run_writer` 在任一次 `record_command`／`record_directory` 返回错误后设置 `failed = true` 与 `STATUS_FAILED`，后续所有命令直接跳过；只有 `Clear` 成功才把 `failed` 复位。设计只要求损坏或高版本暂停持久学习并给显式重置；没有要求 transient 错误永久熔断。
- 根因：把“数据库不可用”与“单次写失败”合并成同一永久状态。
- 影响：一次临时磁盘忙、锁冲突或短时权限问题后，学习历史在本次运行剩余时间内全部丢失，且用户没有明显重试动作；这与“写失败只暴露封闭状态与脱敏错误”的容错意图不符。
- 证据：`run_writer` 的错误分支 `failed = true; status.store(STATUS_FAILED)`；`Clear` 成功才 `failed = false`；没有退避重试或下次命令重新探测。
- 优化方向：区分可重试 I/O 与不可用／损坏状态；对可重试错误保留队列或退避，不让一次失败熔断整个运行期。

### 一般｜设置 `.bak` 复制失败只告警，显式提交仍继续，削弱损坏恢复保证

- 置信度：确认
- 位置：`slterm_app/src/settings_store.rs` 的 `save_merged_locked`；`docs/featureSpec/settings-and-persistence.md` 的「设置导入、导出与恢复默认」「损坏与不兼容分流」。
- 现象：显式保存前对旧文件执行 `std::fs::copy(path, backup_path(path))`，失败只 `log::warn!`，随后继续原子写新文件。若复制失败而新文件后续损坏，没有有效备份可回退。
- 根因：把 `.bak` 当成 best-effort 辅助，而不是显式提交的恢复保证。
- 影响：featureSpec 要求显式动作“保留上一代有效备份、原子提交”；在复制失败时该保证不成立，且用户只可能从日志发现。
- 证据：`save_merged_locked` 的 `if let Err(error) = std::fs::copy(...) { log::warn!(...) }`；没有返回错误，也没有 UI 提示。
- 优化方向：把备份失败纳入显式提交的失败条件或明确豁免并让用户可见；恢复保证不应依赖静默降级。

### 一般｜备份恢复与设置分类相关测试只验证函数、未验证真实窗口或真实用户入口

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs`、`encrypted_backup/recovery.rs`、`encrypted_backup/maintenance.rs` 的测试；`slterm_app/src/settings_state.rs` 的 `settings_damage_classification`；`slterm_app/src/app_shell/app_root/tests` 中的备份相关用例。
- 现象：备份测试全部是纯函数或临时根调用；没有虚拟窗口的设置页备份入口、预览、口令错误提示、类别选择、恢复点列表或撤回冲突 UI 用例。`settings_damage_classification` 没有权限／锁／IO 与高版本。`DocumentSnapshot` 测试以未接线的写线程为主。
- 根因：功能未接线，测试只能覆盖引擎；验收标准把引擎测试当成产品验收。
- 影响：性能、焦点、错误提示、关闭预检与用户取消等真实行为没有证据；这是前文多个阻断的共因。
- 证据：测试模块直接调用 `collect_from`／`seal`／`open`／`restore_selected_at`／`stage_restore`；没有 `TestAppContext` 的备份流程用例；`settings_damage_classification` 的现有断言只覆盖缺失、单字段与结构损坏。
- 优化方向：在产品入口接线完成后补虚拟窗口集成用例，并把权限、高版本、关闭取消与用户可见提示纳入同一验收。


### 一般｜模块注释与实现、featureSpec 存在不一致

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的模块注释；`slterm_app/src/encrypted_backup/CLAUDE.md`；`docs/featureSpec/backup-and-recovery.md` 的「备份类别」。
- 现象：模块注释称类别清单包含设置、会话、自定义主题、命令历史与目录历史，实现只有 `Settings`、`Session`、`Themes`，而 featureSpec 永久排除命令历史与目录历史；注释称“设置页安全分区的加密备份导出已接”，生产代码没有该调用；本地模块文档称“口令与解密明文不落盘”，`stage_restore` 却写明文暂存件。
- 根因：文档随旧实现漂移，没有随类别模型、UI 接线与暂存加密方案更新。
- 影响：读者会把未实现或已明确排除的能力当成现状；验收与后续维护会基于错误事实。文档规范要求只记经核验的当前事实，这类表述不满足。
- 证据：上述模块注释与 `BackupCategory` 枚举、`collect_from` 分支、`stage_restore` 实现、全仓 `crate::encrypted_backup` 调用点直接对照即可见。
- 优化方向：按当前实现与 featureSpec 收敛文档，删除未接线或未实现的声明；文档不复述代码可自证的内容，只保留决策、边界与操作事实。

### 合同冲突｜备份 AAD 的范围与即时恢复路径的口径不一致

- 严重度：按合同单列，不在两者间预设对错。
- 置信度：确认（文档原文与代码均已读取）
- 位置：`docs/devPlan/design/settings-recovery.md` 的「BackupPayload 与加密格式」；`slterm_app/src/encrypted_backup/CLAUDE.md` 的「备份格式」；`slterm_app/src/encrypted_backup.rs` 的 `encrypt_bytes`、`decrypt_bytes`；`slterm_app/src/encrypted_backup/CLAUDE.md` 的「选择性恢复顺序」与 `docs/featureSpec/backup-and-recovery.md` 的「整包恢复」。
- 现象：设计写“完整头部作为 AAD”，本地模块文档写“魔数同时作 AAD”，代码只把 `MAGIC` 作为 AAD，salt 与 nonce 不进入 AAD。另一个冲突是恢复路径：本地模块文档把 `restore_selected` 描述为“原子写恢复、失败自动回滚”，并把它列为生产接缝；featureSpec 与设计则要求实际替换只发生在启动维护期，运行期只写 staged 材料与维护请求。
- 根因：同一外部契约在本仓不同文档中有不同表述；实现同时保留了即时恢复与维护期恢复入口，模块注释把它们都当作可用接缝。
- 影响：安全审计与互操作会按不同 AAD 口径评估；若未来 UI 选择即时恢复入口，会绕过 featureSpec 的关闭预检、维护期应用与世代轮换要求。审查无法在不知道哪份文档为准的情况下判定哪一方的实现构成缺陷。
- 证据：`encrypt_bytes` 的 `aad: MAGIC`；`decrypt_bytes` 的 `aad: MAGIC`；`docs/devPlan/design/settings-recovery.md` 的“完整头部作为 AAD”；`encrypted_backup/CLAUDE.md` 的“魔数同时作 AAD”；`recovery.rs` 的 `restore_selected` 与 `maintenance.rs` 的 `stage_restore` 并存，模块注释把前者列为生产接缝。
- 优化方向：先由合同所有者确定 AAD 口径与恢复时序，再让实现、测试与文档只保留一个口径；当前不应新增依赖任一未定口径的生产调用。

## 跨域观察

- `DocumentRecoveryView` 对 `Paged` 也只渲染静态提示。重启后打开过的大文件会变成 `PagedRecoveryRequired` 占位，视图没有重新打开分页查看器的入口；这不在本审查的持久化边界内，但会让“恢复后仍可阅读大文件”的产品行为缺失。
- `WorkspaceWriterClient` 的终态 ViewState 队列容量固定；超过容量的终态在 `submit_final_states` 中只能标记 `reconcile_required` 依赖后续 Flush 重交，`Drop` 路径则只记录告警。大文档集合退出时应验证后续 Flush 重交后仍能覆盖全部终态。
- `settings_store::recover_corrupted` 的 `copy + remove` 隔离不是原子替换；崩溃可能同时留下原件与隔离副本，虽不丢材料，但会让下一次读取再次进入损坏恢复分支。

## 存疑未证

- 主库损坏后用 Online Backup 恢复时，`preserve_files` 只复制旧 `-wal`／`-shm`，随后只替换主库文件；旧 WAL 是否会在打开新主库时被 SQLite 视为可应用帧，需要用一个“主库损坏但 WAL 校验有效”的真实库做实验确认。若成立，会造成恢复后数据污染。
- 更高版本数据库在 `Connection::open` 阶段是否已经写入 WAL 恢复或 `-shm`，需要保留真实 WAL 的字节比对实验；当前测试无法证明“打开即不写”。
- `restore_path` 使用 `symlink_metadata` 与 `FileType::is_symlink`；Windows junction 是否被该判定识别、父链 junction 是否可把写入引到根外，需要真实 junction 实验确认。`atomic_file` 的 `remove_if_matches` 已有 junction 测试，但写路径没有。
- DPAPI 的跨账户失败只在测试账户内验证篡改；真实跨 Windows 账户／跨机器账户的拒绝行为未验证。
- 备份恢复 `store_epoch` 轮换与 `Applied` 之间的崩溃路径由代码顺序推断为永久阻断；完成真实子进程崩溃重开后才能从“确认代码路径”升级为完整运行时证据。
- `MaintenanceRunner` 的 `NeedsAttention` 阻断是否需要人工解除、以及诊断窗口是否应提供解除动作，文档只规定“只阻断／只重试”，未规定解除所有者；这是合同缺口而非单纯实现缺陷。
