# 重构清单（Step 3 产物）

> 来源：`docs/review-00-summary.md` 及 6 份分报告（86 条，跨维度去重合并后 **77 个独立修复项**）。
> 本文档是唯一修复范围真值源；Stage 划分见 `docs/refactoring-stages.md`。

## 范围决策（已与用户确认）

| 决策点 | 结论 |
|--------|------|
| 修复范围 | **全部 86 条**（去重后 79 项），高/中/低全修 |
| 硬约束 #10 矛盾 | **改文档承认现状**（Tauri 2 自定义命令默认放行），仅移除生产 capabilities 的 `wdio-webdriver:default` |
| 路径沙箱 | **启用真沙箱**：新增 `set_project_root` 命令 + 前端切换项目时调用 + 校验函数漏洞修复 |
| 测试补写 | **纳入**：PTY 命令单测、useXterm 轻 mock 集成测试、titleManager.onDeletePage |

## ID 编号约定

- `SEC-xx` 安全（源自 review-06 及 01 交叉项）
- `BE-xx` Rust 后端（源自 review-01）
- `FE-xx` 前端（源自 review-02、05）
- `TE-xx` 测试（源自 review-03）
- `DOC-xx` 文档（源自 review-04 及 03 交叉项）

## 合并说明（跨维度重复项已并轨）

| 合并后主项 | 被合并的重复报告 |
|-----------|----------------|
| SEC-01 | 01-高#2（validate 绕过）+ 06-高#1（project_root 未设置）+ 06-高#2（fs_read_dir 无校验） |
| SEC-04 | 01-中#2（fs_rename Windows 不覆盖）+ 06-中#3（目录递归删除） |
| BE-01 | 01-高#1 + 05-高#1（同一 spawn_blocking 缺失） |
| BE-15 | 01-低#8 + 05-中#1（notify_watch 同一问题，按中危定级） |
| FE-12 | 02-低#2 + 04-中#8（useFontSizeBridge 死代码） |
| FE-13 | 02-低#3 + 04-高#2（forwardGlobalShortcuts 死代码 + 文档过时 → 代码删除走 FE-13，文档走 DOC-02） |
| DOC-03 | 03-高#1 + 04-高#3（test-inventory L3 失实） |
| DOC-01 | 01-中#5 + 04-高#1 + 06-中#5 的文档部分（约束 #10） |
| DOC-11 | 03-低#1 + 04-低#1（用例数零散失实，含 panels/shortcuts/__tests__ CLAUDE.md） |

---

## P0 — 安全与数据正确性（13 项）

| ID | 问题 | 位置 | 修复要点 |
|----|------|------|---------|
| SEC-01 | 路径沙箱全面失效 | `state.rs:73/89/99-135`、`fs/mod.rs:113-166`、`lib.rs`、前端 `stores/projects.ts`、`ipc/` | 新增 `set_project_root` Tauri 命令（lib.rs 注册 + capabilities 放行）；前端切换项目时调用；`validate_path_within_root` 相对路径先解析为绝对再 `dunce::canonicalize`，目标不存在拒绝；`fs_read_dir` 补校验；移除"None 跳过"（保留 `#[cfg(test)]` 豁免） |
| SEC-02 | pty_spawn 接受任意 shell/cwd | `spawn.rs:599-604`、`shell.rs:50-65` | shell 限白名单（pwsh/powershell/cmd 或 PATH 解析结果）；cwd 过 `validate_path_within_root` |
| SEC-03 | postMessage 键盘桥未校验 origin | `HtmlPanel.tsx:45-60/88-112` | `handleMessage` 校验 `e.origin === "null"`（srcdoc opaque origin，实测 WebView2 行为）；注入脚本 `postMessage` 不用 `"*"`；重放事件加信任标记 |
| SEC-04 | fs_rename 行为缺陷 | `fs/mod.rs:244-250` | 目标为目录 → 返回错误（不递归删）；目标为已存在文件 → 先 `remove_file` 再 rename（与注释"覆盖"一致） |
| SEC-05 | git_status/git_diff 无沙箱校验 | `git/mod.rs:71-112/118/176` | 入口校验 `repo_path`/`file_path` 在 project_root 内；`Repository::discover` 结果 workdir 也校验 |
| SEC-06 | 生产 capabilities 含 wdio 权限 | `capabilities/default.json` | 移除 `wdio-webdriver:default`（debug/e2e 由 `#[cfg(debug_assertions)]` 门控已够） |
| FE-01 | useCodeMirror 文件切换过期写入竞态 | `useCodeMirror.ts:232-289` | 引入 generation 计数（同 useFileTree 模式），异步回调先比对 `genRef.current` 再写 `viewRef` |
| FE-02 | HtmlPanel 注入脚本未转义 `</script>` | `HtmlPanel.tsx:45-61`、`lib/injectScript.ts` | 注入前将宿主 HTML 的 `</script>` 转义为 `<\/script>`（仅脚本字符串内容层面），补测试 |
| BE-01 | PTY 四命令同步阻塞 I/O | `spawn.rs:600/764/786/847` | `pty_spawn`/`pty_write`/`pty_resize`/`pty_reattach` 改 `async fn` + `spawn_blocking`（对齐 `pty_kill` 模式） |
| BE-02 | Job Object 失败子进程成孤儿 | `spawn.rs:686-705` | `add_to_job_object` 失败路径显式 `child.kill()` |
| TE-01 | PTY 命令零单测 | `spawn.rs`（`#[cfg(test)]`）、`tests/pty_integration_tests.rs` | mock AppState + 假 Channel：env 注入（COLORTERM/TERM/TERM_PROGRAM）、cwd 反斜杠、ring buffer 回放、session 移除不级联；集成测试补 reattach 用例 |
| TE-02 | useXterm 编排层测 mock 不测实现 | 新增 `src/__tests__/use-xterm-integration.test.ts` | 轻 mock：真实 Terminal/FitAddon、仅 mock `ipc/pty`；覆盖 rAF 轮询失败回退 80×24、`term.onData`→`pty.write`、visible 切换 WebGL 释放/重建 |
| TE-03 | titleManager.onDeletePage 未测 | `src/__tests__/title-manager.test.ts` | 补删除后终端编号重置、SaveAs 同名冲突用例 |

## P1 — 性能、架构合规（12 项）

| ID | 问题 | 位置 | 修复要点 |
|----|------|------|---------|
| BE-03 | fs_write_file 全量读文件测行尾 | `fs/mod.rs:79-87` | `File::open` + 只读前 `CRLF_SAMPLE_MAX_BYTES` |
| BE-05 | save_settings create_dir_all 在 spawn_blocking 外 | `settings.rs:34-37` | 移入闭包 |
| BE-06 | git 缓存 starts_with 误命中子仓库 | `git/mod.rs:78-90` | 删除 `workdir.starts_with(&search)` 分支 |
| BE-15 | notify_watch 持池锁创建 watcher | `notify/mod.rs:233-250` | 锁外 `FileWatcher::start`，短暂持锁插入 |
| FE-03 | Workspace render 阶段改 ref | `Workspace.tsx:168-182` | pageCallbacksRef 维护移入 `useEffect` |
| FE-04 | PageDockviewHost 监听器未释放 | `PageDockviewHost.tsx:262-304` | disposable 收集到 ref 数组，卸载统一 dispose |
| FE-05 | main.tsx setInterval 无上限无清理 | `main.tsx:8-19` | 最大轮次（200 次/10s）+ 超时报错 + 清理 |
| FE-06 | refreshExpanded gitStatus 无 generation 防护 | `useFileTree.ts:166-184` | genRef 快照比对 |
| FE-07 | ExplorerPanel addPanel 未 try-catch | `ExplorerPanel.tsx:100-110` | try-catch，成功后才注册 titleManager |
| FE-23 | 违反约束 #8：sessionId 副本 | `useXterm.ts:140/279-284`、`usePtyOutput.ts:238`、`usePtyResize.ts:69/99` | 删除 `sessionIdRef`，统一 `TerminalRegistry.get(panelId).sessionId` 读取；exit 状态经 Registry 通知 |
| FE-24 | 违反约束 #6：硬编码颜色 | `App.css:5-8`、`FileTree.tsx:71`、`SidebarTree.tsx:82`、`theme/colors.ts` | colors.ts 新增 token（含 SHADOW），组件/CSS 引用 |
| TE-16 | usePtyOutput 直写路径不检查 visibleRef | `usePtyOutput.ts:199-201` | 直写路径加 visible 门控（对齐设计文档"非焦点仅累积"），同步修正 NF3 测试 |

## P2 — 测试可信度（11 项）

| ID | 问题 | 位置 | 修复要点 |
|----|------|------|---------|
| TE-04 | Rust 集成测试固定 sleep flaky | `pty_integration_tests.rs:28-63/77-91/96-155` | 改输出 marker 轮询（上限 10s） |
| TE-05 | layout-serde.test.ts 误导注释 | `layout-serde.test.ts:3` | 删除"覆盖率 0%"注释 |
| TE-06 | layoutSerde 嵌套/activeGroup 未覆盖 | `layout-serde.test.ts` | 补嵌套 branch 修补、activeGroup 保留断言 |
| TE-07 | ShortcutRegistry 监听器/边界未测 | `shortcuts.test.ts` | spy addEventListener/removeEventListener；`setOverrides(undefined)`；exportContextBindings 覆盖影响；resolve handler 返回 false |
| TE-08 | pool.rs stop/Drop 断言不足 | `pool.rs:289-331` 测试 | make_test_watcher 线程真听 stop_rx；LRU/替换断言 `is_running()===false` |
| TE-09 | 条件断言假阳性 + NF3 标题矛盾 | `use-xterm-output.test.ts:811-834/984-1007/1197-1228` | 先断言 `writeCalls.length > 0`；NF3 标题随 TE-16 行为修正 |
| TE-10 | waitFor 未设超时 | `close-handler/editor-confirm/e2e-gating/explorer-*` 等多文件 | 显式 `{ timeout: 2000~5000 }` |
| TE-11 | debounce 测试用真实 setTimeout | `use-xterm-*.test.ts`、`use-file-tree.test.ts`、`explorer-notify/html-panel.test.tsx` | 统一 `vi.useFakeTimers()` + `advanceTimersByTime` |
| TE-12 | setup.ts 全局 mock 泄漏风险 | `setup.ts:58/61-64` | 注释警告 notify mock 需显式覆盖；canvas spy 用 beforeAll/afterAll 包装 |
| TE-13 | git 测试副本验证 diff 逻辑 | `git/mod.rs:1327-1389` | hunk 合并逻辑提取纯函数，测试直调（删副本） |
| TE-14 | fs/settings 命令包装层未直接测 | `fs/mod.rs:32-258`、`settings.rs:34-100` | 补命令包装单测（参数透传、错误映射、sandbox 校验分支） |

## P3 — 文档同步（13 项）

| ID | 问题 | 位置 | 修复要点 |
|----|------|------|---------|
| DOC-01 | 约束 #10 与实际权限模型矛盾 | 根 `.claude/CLAUDE.md:46`、`src-tauri/src/pty/CLAUDE.md:175` | 改为"Tauri 2 自定义命令默认放行，capabilities 只管插件权限"；pty 注意事项同步 |
| DOC-02 | HTML 键转发机制文档过时 | `src/features/shortcuts/CLAUDE.md:179`、`e2e-tests/CLAUDE.md:80` | 改述 postMessage 注入脚本机制；注明 forwardGlobalShortcuts 已删除（随 FE-13） |
| DOC-03 | test-inventory.md 全面失实 | `.claude/test-inventory.md` | 按 grep 实测重写：L1 190/L2 ~1020/L3 116/L4 12；统一计数口径为"vitest 展开用例数"；标注 E2E 键盘局限（并吞 TE-15）；声明本文档为用例数唯一真值源 |
| DOC-04 | 根 CLAUDE.md 含不存在的 claude/ 模块 + 用例数失实 | 根 `.claude/CLAUDE.md:33/71-74` | 移除 `claude/`；L1/L2 改为引用 test-inventory.md |
| DOC-05 | JobHandle 跨平台描述与代码矛盾 | `src-tauri/src/pty/CLAUDE.md:32` vs `state.rs:31-33`、`spawn.rs:747-750` | 二选一：文档改述 `#[cfg(windows)]` 初始化现状；或代码改为非 Windows 零占位始终初始化（推荐后者，消除非 Windows 编译错误 E0063） |
| DOC-06 | Workspace.tsx "已废弃"失实 | `src/workspace/CLAUDE.md:28` | 改述为主组件（三栏布局+页面生命周期），单页渲染委托 PageDockviewHost |
| DOC-07 | stores 持久化规则失实 | `src/stores/CLAUDE.md:71-76` | 改为"持久化委托 ipc/settings（fontSize/keybindings）或 ipc/fs（projects）" |
| DOC-08 | ipc/CLAUDE.md 缺 ping() | `src/ipc/CLAUDE.md:24` | 模块映射表补 index.ts 的 ping() |
| DOC-09 | panels 文档多处失实 | `src/panels/CLAUDE.md:50-54/160-162/206` | useFontSizeBridge→useFontSizeWheel；注入脚本改述三段式（CSS 注入+键盘转发+片段拦截，无 location.hash） |
| DOC-10 | layoutSerde 注释与策略矛盾 | `src/workspace/layoutSerde.ts:70-74` | 删"失败回退创建默认终端"注释，改述 Watermark 策略 |
| DOC-11 | 各 CLAUDE.md 用例数零散失实 | `src/__tests__/CLAUDE.md`、`src/panels/CLAUDE.md`、`src/features/shortcuts/CLAUDE.md` | 全部改为引用 test-inventory.md，不再各自维护数字；useXterm 行数描述删除 |
| DOC-12 | HTML 预览威胁模型记录缺失 | `src/panels/CLAUDE.md`（sandbox 决策节） | 补录 SEC-03 修复后的 postMessage origin 校验机制与威胁模型 |
| DOC-13 | e2e-tests/CLAUDE.md 未同步 | `e2e-tests/CLAUDE.md` | 同步 DOC-02/DOC-03 结论 |

## P4 — 低危清理（30 项）

| ID | 问题 | 位置 | 修复要点 |
|----|------|------|---------|
| BE-07 | RawChild try_wait/wait 未查 WaitForSingleObject 错误 | `spawn.rs:278-304` | 非 WAIT_OBJECT_0 返回 io::Error |
| BE-08 | clone_killer 失败 panic | `spawn.rs:259-269` | 改返回 io::Result |
| BE-09 | 锁中毒 expect panic（多处） | `spawn.rs:199-312` | 锁中毒转 io::Error |
| BE-10 | build_env_block HashMap 顺序不定 | `spawn.rs:101-120` | 改 Vec 保持顺序，extra_envs 覆盖 |
| BE-11 | which_full_path 固定 `;` 分割 PATH | `shell.rs:129-139` | 改 `std::env::split_paths` |
| BE-12 | SPAWN_LOCK 粒度超注释范围 | `spawn.rs:606-704` | 收窄到 create_conpty_pair + spawn_conpty_child |
| BE-13 | 启动序列剥离跨 16KB 边界失效 | `reader.rs:85-94` | 本轮全为启动序列时继续保持剥离状态 |
| BE-14 | COORD 尺寸 as i16 溢出 | `spawn.rs:205-208` | SpawnRequest 校验 cols/rows ≤ i16::MAX |
| BE-16 | unsafe 块缺 SAFETY 注释 | `spawn.rs:130-433` | 逐块补 SAFETY 注释 |
| SEC-07 | resolve_shell 旧路径接受任意 shell | `shell.rs:25-42` | 删除旧路径或复用 resolve_shell_info 校验 |
| SEC-08 | pty_write 不校验 session 归属 | `spawn.rs:764-777`、`state.rs` | PtySession 记录 panelId，write/resize/kill 校验归属 |
| FE-08 | usePtyOutput retry disposable 未清理 | `usePtyOutput.ts:156-178`、`useXterm.ts` cleanup | cleanup 补 `retryDisposableRef.current?.dispose()` |
| FE-09 | TerminalPanel icon 非空断言 | `TerminalPanel.tsx:72` | `state.icon ?? null` |
| FE-10 | layoutSerde `as any` | `layoutSerde.ts:97-98` | 改 `Parameters<DockviewApi["fromJSON"]>[0]` |
| FE-11 | isAlwaysRenderPanel 冗余分支 | `panelRegistry.ts:59-61` | 简化 |
| FE-12 | useFontSizeBridge.ts 死代码 | `src/panels/terminal/useFontSizeBridge.ts` | 删除文件及引用注释（文档同步走 DOC-09） |
| FE-13 | forwardGlobalShortcuts.ts 死代码 | `src/features/shortcuts/forwardGlobalShortcuts.ts`、`index.ts`、测试文件 | 删除实现 + re-export + 其专属测试文件（postMessage 路径已有 html-panel 测试覆盖） |
| FE-14 | fonts.ready Promise 无取消 | `useTerminalInstance.ts:123-135` | cleanup 置取消标志，rAF 回调双重检查 |
| FE-15 | useTerminalInstance dispose/cleanup 重复 | `useTerminalInstance.ts:159-180/208-224` | 提取 performDispose() 共用 |
| FE-16 | FsEvent detail 类型不一致 | `ipc/notify.ts:6-10`、`types/notify.ts:2-6` | 统一 `detail?: string`，消费处空值处理 |
| FE-17 | TerminalRegistry.getAll 返回可写 Map | `TerminalRegistry.ts:39-41` | 返回 `new Map(registry)` 副本 |
| FE-18 | window 扩展属性 as any | `main.tsx:10/14`、`ErrorBoundary.tsx:42` | 新增 `src/global.d.ts` 声明 Window 接口 |
| FE-19 | handleSave 根路径 repoDir 计算有误 | `useCodeMirror.ts:166-171` | 用 dirname 语义处理，无父目录跳过 gitDiff |
| FE-20 | SidebarTree 内联子组件每次渲染重建 | `SidebarTree.tsx:115-294` | Toolbar/ProjectRow/PageRow 提取到模块顶层 |
| FE-21 | FileTree paddingLeft 魔法数字 | `FileTree.tsx:388-599` | 提取 ICON_WIDTH/ARROW_WIDTH/INDENT 常量 |
| FE-22 | PageDockviewHost 访问 panel.view 非公共 API | `PageDockviewHost.tsx:165` | 改从 `panel.api.params` 判断 |
| DOC-05c | （见 DOC-05，代码侧改动） | `state.rs:31-33` | 若选代码方案：JobHandle 非 Windows 零占位始终初始化 |
| TE-17 | E2E 键盘测试局限未在清单标注 | `.claude/test-inventory.md` | 并入 DOC-03 执行，此处仅登记 |
| — | （低危文档项已并入 DOC-11） | — | — |

> 说明：P4 表中 BE-15 已升 P1、BE-04 并入 SEC-04、低危文档项并入 DOC-11，独立修复项 28 个。

## 统计

| 优先级 | 项数 | 内容 |
|--------|------|------|
| P0 | 13 | 安全 + 数据正确性 + 核心零覆盖补测 |
| P1 | 12 | 性能（spawn_blocking 之外的阻塞/冗余）+ 架构约束 #6/#8 |
| P2 | 11 | 测试可信度整改 |
| P3 | 13 | 文档同步 |
| P4 | 28 | 低危清理 |
| **合计** | **77** | — |
