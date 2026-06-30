# 自动化测试用例清单

全量 **646** 用例（Rust 113 + 前端 525 + L3 5 + E2E 3），2026-06-30 实测更新。

> L3 5 用例同时被 `npm test`（T2）和 `npm run test:l3`（T3）覆盖，此处保留独立列出。去重后唯一用例数 641。

## 前端测试（41 文件 / 525 用例）— Vitest + jsdom

> `npm test` 实际运行 41 文件 525 用例（含 `test/terminal/` 下 2 文件 5 用例）。L3 用例详见独立章节。

### IPC 层（2 文件 / 41 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/ipc-contract.test.ts` | 40 | pty/fs/settings/notify/git 全模块 IPC 命令合约验证 |
| `src/__tests__/ipc-ping.test.ts` | 1 | mockIPC ping/pong 拦截 |

### 终端（7 文件 / 74 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/terminal.test.tsx` | 4 | TerminalPanel 挂载/加载遮罩/Windows build 号/spawn |
| `src/__tests__/terminal-lifecycle.test.ts` | 4 | 挂载→创建→卸载→dispose 完整生命周期 |
| `src/__tests__/terminal-strictmode.test.ts` | 4 | React StrictMode 双挂载防御 |
| `src/__tests__/useXterm.test.ts` | 14 | 焦点驱动、rAF 轮询 spawn、ResizeObserver resize 链路 |
| `src/__tests__/canFit.test.ts` | 15 | 五条件守卫 + null/undefined 参数防护 |
| `src/__tests__/keyboard.test.ts` | 26 | IME/Ctrl+Shift+C/V/多终端焦点/剪贴板/uninstall |
| `src/__tests__/TerminalRegistry.test.ts` | 7 | register/get/remove/has/幂等/clear |

### 编辑器（4 文件 / 71 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/useCodeMirror.test.ts` | 42 | 语言映射/gitDiff 参数/fs-event 外部修改重载(P0-11)/保存后 diff 清空/slterm:file-saved event |
| `src/__tests__/editor.test.tsx` | 3 | EditorPanel 渲染/panelId/filePath 传递 |
| `src/__tests__/editor-confirm.test.ts` | 6 | dirty=true/false 外部修改确认/边界条件 |
| `src/__tests__/gitGutter.test.ts` | 20 | StateEffect 映射/GutterMarker DOM/行级 DiffHunk/SpacerMarker |

### 工作区/布局（6 文件 / 39 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/workspace.test.tsx` | 3 | 无项目/有项目无页面/有活跃页面 → Dockview 初始化 |
| `src/__tests__/workspace-multi-instance.test.tsx` | 4 | 多 Dockview 实例惰性初始化/删除活跃页面 |
| `src/__tests__/workspace-e2e-ready.test.tsx` | 4 | `__slterm_e2e_workspaceReady` 标记设置/同步性 |
| `src/__tests__/layout.test.ts` | 4 | useLayout store CRUD |
| `src/__tests__/layout-switch.test.ts` | 8 | 页面切换集成/自切换守卫 |
| `src/__tests__/layoutSerde.test.ts` | 16 | 旧格式修补(component→contentComponent)/白名单过滤/深拷贝 |

### Store（2 文件 / 47 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/projects.test.ts` | 37 | CRUD/持久化(loadFromDisk/saveToDisk)/version 递增/ID 生成/expandedNodes/边界守卫 |
| `src/__tests__/sessions.test.ts` | 10 | setSession/removeSession/setActive/多 session 共存/幂等 |

### 资源管理器（4 文件 / 74 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/explorer-git-status.test.tsx` | 32 | useFileTree/gitStatusMap 查表着色/配色 token/F5 untracked/slterm:file-saved |
| `src/__tests__/explorer-delete.test.tsx` | 13 | ask 弹窗分支/右键菜单/操作失败 UI 通知 |
| `src/__tests__/explorer-notify.test.tsx` | 12 | startWatch 调用时机/projectRootPath/loadRoot/toggleExpand |
| `src/__tests__/FileIcon.test.tsx` | 17 | 8 种扩展名图标 + 目录图标 + git 状态着色 |

### 侧栏（1 文件 / 26 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/sidebar-actions.test.ts` | 26 | 树结构/右键菜单/内联重命名/项目删除确认/宽度自适应/添加项目 |

### 主题/配色（2 文件 / 72 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/colors.test.ts` | 61 | GIT_FILE_COLORS(7)/GIT_GUTTER_COLORS(4)/EXPLORER_COLORS(6)/通用 UI(20) hex 校验 |
| `src/__tests__/theme.test.ts` | 11 | terminalOptions: ANSI 16 色/font/cursor/scrollback |

### 启动/关闭（3 文件 / 13 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/bootstrap.test.ts` | 3 | `__TAURI_INTERNALS__` 轮询/立即挂载/永不就绪 |
| `src/__tests__/startup-restore.test.ts` | 4 | localStorage 恢复/空/异常降级/Loading→ready |
| `src/__tests__/close-handler.test.ts` | 6 | flush layout→saveAllProjects→localStorage/超时/异常 |

### E2E 辅助测试（3 文件 / 11 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/App.test.tsx` | 5 | `__slterm_e2e_createProject` 行为/E2E pending 标记 |
| `src/__tests__/e2e-create-project.test.ts` | 3 | pending 标记→localStorage 恢复交互 |
| `src/__tests__/e2e-clipboard-helper.test.ts` | 3 | writeClipboard/createProject 函数可用性 |

### 其他（5 文件 / 53 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/panelRegistry.test.ts` | 16 | 注册表/PANEL_TYPES/isValidPanelType/type predicate |
| `src/__tests__/default-layout-format.test.ts` | 8 | grid/panels/activeGroup/orientation 格式验证 |
| `src/__tests__/language-mapping.test.ts` | 23 | 22 种扩展名→CodeMirror 语言映射 + 未知回退 |
| `src/__tests__/detectWebgl.test.ts` | 3 | WebGL2 可用/不可用/抛异常 |
| `src/__tests__/error-boundary.test.tsx` | 3 | 正常透传/抛错 UI/`__sltermError` 赋值 |

## Rust 后端测试（11 文件 / 113 用例）— `cargo test`

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src-tauri/src/git/mod.rs` | 61 | `status_to_str`(12)/git_status(12)/git_diff(14)/line_callback(9)/dunce(3)/序列化(2)/repository(2)/其他(7) |
| `src-tauri/src/fs/mod.rs` | 13 | read_dir: 列表/过滤 .git/node_modules/创建目录/删除/重命名(8); write_file: CRLF/LF 保留/新建默认 CRLF(5) |
| `src-tauri/src/pty/reader.rs` | 8 | ConPTY 启动序列剥离: OSC/清屏/光标归位/DSR/光标显隐/透传正常文本/保留 OSC 7 |
| `src-tauri/src/notify/mod.rs` | 6 | FileWatcher 生命周期: 创建/stop/Drop/stale 替换/幂等 stop/序列化 |
| `src-tauri/src/state.rs` | 5 | PtyState 空初始化/AppState 创建/ring buffer append+eviction+换行边界 |
| `src-tauri/src/settings.rs` | 4 | 读写往返/文件不存在/JSON 损坏回退 .bak/无 .bak 降级 |
| `src-tauri/src/error.rs` | 4 | 序列化/Display/From<io::Error>/SessionNotFound 序列化 |
| `src-tauri/src/pty/shell.rs` | 3 | `which` 发现 pwsh/shell-integration.ps1 嵌入/UTF-16LE Base64 往返 |
| `src-tauri/src/lib.rs` | 2 | ping 返回 pong/`get_windows_build_number` 返回数字 |

**集成测试**（`src-tauri/tests/`，2 文件 / 7 用例）— 清单外增量

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src-tauri/tests/integration_test.rs` | 3 | AppError Display 不 panic/AppState 创建/PtyState 创建 |
| `src-tauri/tests/pty_integration_tests.rs` | 4 | PTY 往返/OSC cwd 解析/resize 生效/kill 无孤儿进程 |

Rust 模块中无测试的文件（4 个）：`pty/build.rs`、`pty/mod.rs`、`pty/spawn.rs`、`main.rs`。

## L3 终端 headless 测试（2 文件 / 5 用例）— `npm run test:l3`

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `test/terminal/terminal-serialize.test.ts` | 1 | xterm feed "hi" → serialize 含 "hi" |
| `test/terminal/keyboard.test.ts` | 4 | pwsh 提示符渲染/claude TUI ANSI 色/Shift+Tab CBT 编码/Ctrl+C ETX 编码 |

## E2E 端到端测试（1 文件 / 3 用例）— `npm run wdio`

| 用例 | 覆盖范围 |
|------|---------|
| 应正常启动并显示 slTerminal 标题 | 应用启动 + 窗口标题验证 |
| 打开终端→写入文本→验证缓冲含 e2e_marker | 完整 PTY 通信链路（createProject→addPanel→spawn→write→read） |
| Ctrl+Shift+V 将剪贴板内容粘贴到终端 | 键盘快捷键→剪贴板→终端粘贴 |

E2E 架构：`@wdio/tauri-service` 1.1.0 embedded driver（`webview2-com` COM 直连 `ICoreWebView2`，零 msedgedriver 依赖）。

## 静态检查门禁

| 门 | 命令 | 说明 |
|----|------|------|
| TypeScript | `npx tsc --noEmit` | 全量类型检查 |
| ESLint | `npx eslint src/` | 前端代码规范 |
| Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | Rust 代码规范 |
