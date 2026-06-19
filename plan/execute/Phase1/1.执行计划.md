# Phase 1 执行计划 — 核心外壳

> 对应开发文档：`plan/5.1 phase-1-核心外壳.md`
> 前置：Phase 0（骨架 + 测试基建）已完成
> 目标：统一分屏页签区 — 开 PowerShell、开文件编辑、拖拽分屏调比例、跑 claude

---

## 1. 编排总览

### 1.1 依赖分析

```
PTY 后端 (spawn→write→resize→kill, 强串行) ──┐
FS 后端 (read/write, 独立)                   ──┤
PowerShell profile 注入脚本                   ──┤
                                               ├── barrier ──┐
IPC 类型定义 + 命令注册 (需两侧 DTO 定型)     ←──────────────┘
                                                                │
                    ┌───────────────────────────────────────────┘
                    │
       Terminal 面板            Editor 面板
       (xterm.js + DOM)  ← parallel →  (CM6 + Ctrl+S)
       (keyboard handler)               (readFile/writeFile)
                    │                        │
                    └────── barrier ─────────┘
                              │
                    Workspace
                    (Dockview + 分屏/页签/右键菜单/布局持久化)
                              │
                    键盘透传 + Ctrl+C 智能分流
                              │
                    验证 Phase
                    (parallel: L1/L2/L3/L4)
```

### 1.2 编排决策

按 `plan-generation.md` 决策树，Phase 1 是**混合型**（部分串行 + 部分并行）：

| 层 | 方式 | 内容 |
|----|------|------|
| W0 前置 workflow | parallel 3 agent | portable-pty 0.9.x API 确认、Dockview API 确认、xterm.js/CM6 集成确认 |
| `/goal` 主干 | 30 turns 循环 | PTY 后端（最难核，需反复调试 ConPTY）→ barrier → 其余 → 验证 |
| W1 实现 workflow | barrier → parallel → barrier | 'PTY + FS 后端' → 'IPC 类型' → 'Terminal + Editor 面板'(parallel) → 'Workspace'(barrier) |
| W2 验证 workflow | parallel 4 agent | cargo test / npm test / L3 headless / L4 E2E |
| 主会话 | 人工 | IME 调优、真机验证、代码审查 |

### 1.3 三层结构

```
/goal Phase1-Core-Shell 完成条件（30 turns）
  │
  ├─ W0: 前置核验 (parallel 3 agent, 只读检索)
  │
  ├─ W1: 实现 (ultracode dynamic workflow)
  │   ├─ Phase 'PTY+FS后端' [barrier]
  │   ├─ Phase 'IPC类型+命令' [barrier: 依赖后端 DTO]
  │   ├─ Phase '面板' [parallel: Terminal∥Editor]
  │   └─ Phase 'Workspace+键盘' [barrier: 依赖面板]
  │
  └─ W2: 验证 (parallel 4 agent)
```

---

## 2. 如何使用 /goal

### 2.1 完成条件

```
/goal 实现 Phase 1 核心外壳，下列每项在本会话中均有真实命令输出自证：
(1) L1 后端单测：cargo test 退出 0，含 pty_roundtrip/pty_resize_applies/pty_kill_no_orphan/fs_read_write_roundtrip/osc_cwd_venv_parsed 全部 PASS
(2) L2 前端单测：npm test 退出 0，含 terminal_panel_spawns_on_mount/terminal_input_writes/editor_saves_edited_content/closing_last_panel_shows_empty_state 全部 PASS
(3) L3 终端渲染：npm run test:l3 退出 0，含 renders_pwsh_prompt/claude_tui_snapshot/key_encoding_shift_tab/key_encoding_ctrl_c 全部 PASS
(4) L4 E2E：npm run wdio 退出 0，含"打开终端→输入 echo e2e→DOM 出现 e2e"流程 PASS
(5) tauri build --debug 退出 0，exe 可启动，暗色空窗无报错
(6) 硬约束：不弱化 lint（eslint 退出 0、clippy 退出 0）；不放宽测试断言；不引入 Phase 1 范围外的业务功能
以上未全部满足则继续；或 30 turns 后停止并逐项汇报缺口。
```

### 2.2 运行方式

```
/claude
/goal ...（粘贴上述条件）
```

每轮结束后 Claude 自动评估条件，满足任一项即标记。全部满足 = 目标达成。

### 2.3 条件质量自检

- [x] 每项有明确的命令 + 期望输出
- [x] 每个期望输出可在对话中找到对应文本
- [x] 硬约束包含反作弊条款（不弱化 lint/断言/不引入业务功能）
- [x] 有止盈上限（30 turns）
- [x] CI 不放 /goal 条件（独立验证，贴 `gh run view` 回主会话）

---

## 3. 如何用 dynamic workflow

### 3.1 W0：前置核验（只读检索）

```
Phase '前置核验' [parallel]
  agent V0 — portable-pty 0.8.x API 确认（src-tauri/src/pty/，sonnet）
  agent V1 — Dockview 拖拽/右键菜单/布局序列化 API 确认（sonnet）
  agent V2 — xterm.js WebGL addon + CM6 最小接入 API 确认（sonnet）
```

**触发**：
```
ultracode: 按以下编排执行 slTerminal Phase 1 W0 前置核验，只读检索、不修改代码，输出 API 报告到主会话
```

### 3.2 W1：实现

```
Phase 'PTY+FS后端' [barrier]  ← pty/spawn.rs 必须串行
  agent A1 — pty/spawn.rs: ConPTY spawn + SPAWN_LOCK + CPR 响应 + stdin drop 保护（opus）
  agent A2 — pty/reader.rs: reader 线程 + Channel 推送输出（opus，依赖 A1 的 DTO 定型）
  agent A3 — pty/shell.rs: PowerShell profile 注入脚本 + OSC 9;9 + OSC 133（sonnet，独立文件）
  agent A4 — fs/mod.rs: read_file/write_file 命令（sonnet，独立）

Phase 'IPC类型+命令' [barrier: 依赖后端 DTO]
  agent A5 — src/types/ ↔ src-tauri/src/ DTO 双边对齐 + lib.rs 命令注册（sonnet）

Phase '面板' [parallel: Terminal∥Editor 互不冲突]
  agent A6 — Terminal 面板：TerminalPanel.tsx + useXterm.ts + ipc/pty.ts（sonnet）
  agent A7 — Editor 面板：EditorPanel.tsx + useCodeMirror.ts + ipc/fs.ts（sonnet）

Phase 'Workspace+键盘' [barrier: 依赖两个面板注册]
  agent A8 — Workspace.tsx + panelRegistry.ts + layoutSerde.ts + stores/sessions.ts（sonnet）
  agent A9 — keyboard handler: Ctrl+C 智能分流 + Ctrl+Shift+C/V 复制粘贴（sonnet）
```

**触发**：
```
ultracode: 按以下编排执行 slTerminal Phase 1 W1 实现，只做计划内修改、不做无关改动，每阶段完成后贴 cargo test/npm test 输出回主会话
```

> **同文件冲突处理**：A6 和 A9 都涉及 ipc/pty.ts 调用 → A9 在 A6 barrier 之后执行。A4 和 A7 都涉及 ipc/fs.ts 调用 → A7 在 A4 barrier 之后执行。

### 3.3 W2：验证

```
Phase '验证' [parallel]
  agent T1 — cargo test（L1 全量，含 Phase 0 回归 + Phase 1 新测试，sonnet）
  agent T2 — npm test（L2 全量，含 Phase 0 回归 + Phase 1 新测试，sonnet）
  agent T3 — npm run test:l3（L3 headless，含 Phase 0 回归，sonnet）
  agent T4 — npm run wdio（L4 E2E，含 Phase 0 回归，sonnet）
```

---

## 4. 各 agent 任务卡

##### Agent A1 — pty/spawn.rs（Phase 'PTY+FS后端'，opus）

| 项目 | 内容 |
|------|------|
| **任务** | 实现 ConPTY spawn：openpty → spawn_command → CPR 响应 → SPAWN_LOCK 串行化 |
| **操作** | ① 用 `portable-pty = "0.9"`（与 Phase 0/terax-ai 一致）② 实现 `pty_spawn` 命令：`native_pty_system().openpty(PtySize{...})` → `pair.slave.spawn_command(cmd)` ③ 握住 `SPAWN_LOCK: std::sync::Mutex<()>` 串行化所有 spawn ④ `openpty()` 后立即向 stdin 写 `\x1b[1;1R`（CPR 响应，`#[cfg(windows)]` 条件编译——补偿 ConPTY `VtIo::StartIfNeeded()` DSR 握手，xterm.js 自动响应 CPR 的回路此时未闭合）⑤ cwd 传入前 `replace("/", "\\")` 规范化 ⑥ stdin drop 加 `#[cfg(not(windows))]` 保护 ⑦ 通过 `CommandBuilder.env` 注入 `SLTERM_PANE_ID`（UUID）⑧ 每个子进程放入 Job Object，`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 防止孤儿进程 |
| **产出** | `src-tauri/src/pty/spawn.rs`（新建）、`mod.rs`（更新） |
| **验收** | `cargo check` 通过；`cargo test pty_spawn` PASS；`cargo clippy -- -D warnings` 退出 0 |

##### Agent A2 — pty/reader.rs（Phase 'PTY+FS后端'，opus）

| 项目 | 内容 |
|------|------|
| **任务** | 实现 PTY reader 线程：阻塞 read → Event emit 推送输出 |
| **操作** | ① spawn 后 `master.try_clone_reader()` 获取 reader ② `std::thread::spawn` 独立线程，不阻塞 tokio runtime ③ 循环 `reader.read(&mut buf)`，Ok(0)=EOF 退出 ④ 通过 `Channel<PtyEvent>`（spawn 模式长期持有）推送输出——命令函数立即返回 `Ok(())`，Channel 在后台任务中持续 `send()` ⑤ 实现 `pty_write` 命令：`writer.write_all(data)` ⑥ 实现 `pty_resize` 命令：`master.resize(PtySize{...})`；前端 fire-and-forget 调用，不阻塞 IPC ⑦ 实现 `pty_kill` 命令：先排空输出管道 → `clone_killer().kill()` → 从 PtyState 移除 → `emit("pty:exit", { session_id, code })` ⑧ writer 用 `Arc<Mutex<Box<dyn Write + Send>>>` 共享 ⑨ ClosePseudoConsole 前持续排空输出管道，防止死锁 |
| **产出** | `src-tauri/src/pty/reader.rs`（新建）、`state.rs`（更新，挂 PtyState + Channel 管理） |
| **验收** | `cargo test pty_roundtrip` PASS：spawn pwsh → write "echo hello\r\n" → Channel 接收含 "hello"；`cargo clippy -- -D warnings` 退出 0 |

##### Agent A3 — pty/shell.rs（Phase 'PTY+FS后端'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 实现 PowerShell profile 注入：OSC 9;9（CWD）+ OSC 133（命令边界） |
| **操作** | ① shell 选择逻辑：`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退 ② debug 模式读文件释放模式 `include_str!` 嵌入集成脚本 ③ spawn 时写脚本到 `%APPDATA%\slTerminal\shell-integration.ps1`（首次写入后后续 spawn 复用路径，`%APPDATA%` 比 `%TEMP%` 杀软友好）④ `pwsh -NoProfile -NoLogo -File <script.ps1>` 启动 ⑤ 注入 `SLTERM_PANE_ID` 环境变量 ⑥ 脚本开头加入 UTF-8 编码修复（`[Console]::OutputEncoding = UTF8` + `$OutputEncoding = UTF8` + `chcp 65001`——中文 Windows GBK/936 环境必须）⑦ 脚本内容：装饰器模式保存原始 prompt → 追加 OSC 7（file:// URI 事实标准）+ OSC 9;9（ConEmu 起源 Windows 原生路径）+ OSC 133 A/B/D → C 序列用 PreCommandLookupAction 钩子 ⑧ 嵌套检测：`$env:SLTERM_PANE_ID` 已存在则跳过注入 ⑨ 脚本 UTF-8 BOM 编码（PS 5.1 硬性要求），Escape 字符用 `$([char]0x1b)` 而非 `` `e ``（PS 5.1 兼容） |
| **产出** | `src-tauri/src/pty/shell.rs`（新建）、`assets/shell-integration.ps1`（新建） |
| **验收** | `cargo test osc_cwd_venv_parsed` PASS：注入后从输出解析出 OSC 7/OSC 9;9 CWD 与 OSC 133 命令边界序列；`cargo clippy -- -D warnings` 退出 0 |

##### Agent A4 — fs/mod.rs（Phase 'PTY+FS后端'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 实现文件读/写命令 |
| **操作** | ① `fs_read_file(path) -> Result<String>` ② `fs_write_file(path, content) -> Result<()>` ③ 阻塞 I/O 用 `spawn_blocking` 包装 |
| **产出** | `src-tauri/src/fs/mod.rs`（更新） |
| **验收** | `cargo test fs_read_write_roundtrip` PASS；`cargo clippy -- -D warnings` 退出 0 |

##### Agent A5 — IPC 类型 + 命令注册（Phase 'IPC类型+命令'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 前端 types/ ↔ 后端 DTO 双边对齐，lib.rs 统一注册所有命令，capabilities 显式放行 |
| **操作** | ① `src/types/pty.ts`：PtyChunk, SpawnRequest, SpawnResponse, ResizeRequest, WriteRequest ② `src/types/fs.ts`：ReadResult, WriteRequest ③ Rust 侧对应 DTO：snake_case ④ `lib.rs` 的 `generate_handler!` 注册：pty_spawn, pty_write, pty_resize, pty_kill, fs_read_file, fs_write_file ⑤ `capabilities/default.json` 显式放行所有新增命令 ⑥ `src/ipc/pty.ts`：封装 invoke 调用 + Event 监听 |
| **产出** | `src/types/pty.ts`, `src/types/fs.ts`, `src/ipc/pty.ts`, `src/ipc/fs.ts`, `lib.rs`, `capabilities/default.json` |
| **验收** | `cargo check` 通过；TypeScript 编译通过 |

##### Agent A6 — Terminal 面板（Phase '面板'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 实现 TerminalPanel + useXterm hook（WebGL 聚焦独占 + DOM 兜底） |
| **操作** | ① `useXterm.ts`：StrictMode guard（`if (terminalRef.current) return`）+ 创建 `new Terminal({ windowsPty: { backend: 'conpty', buildNumber: 21376 }, ... })` → `terminal.open(container)` → `FitAddon` ② `detectWebgl()` 预检（`failIfMajorPerformanceCaveat: true`）→ 通过则 `loadAddon(new WebglAddon())` + `onContextLoss` 回退 DOM；不可用则 DOM 兜底 ③ 仅焦点终端持有 WebGL context（失焦 `dispose()`，聚焦重新 `loadAddon`）④ 挂载时调 `ipc.pty.spawn({ panelId, cols, rows })` → 后端返回 sessionId ⑤ Channel `onmessage` 监听输出 → DEC 2026 包裹（`\x1b[?2026h`...data...`\x1b[?2026l`）原子渲染 + rAF 批处理回退（小数据 <256 bytes 立即写，大数据 rAF 合帧）⑥ `term.onData` → `ipc.pty.write({ sessionId, data })` ⑦ `ResizeObserver` → `fitAddon.fit()` → `ipc.pty.resize({ sessionId, cols, rows })`（fire-and-forget，不阻塞 UI）⑧ 显示加载遮罩"正在连接..."，首帧数据到达时淡出 ⑨ 组件卸载时 `ipc.pty.kill({ sessionId })` + 清理 Channel ⑩ 面板聚焦/失焦时切换 WebGL context（聚焦 `loadAddon`，失焦 `dispose`，500ms 降级迟滞防抖）⑪ 注册 `terminal` 面板到 panelRegistry ⑫ 字体预加载：`document.fonts.ready` 后 fit + refresh |
| **产出** | `src/panels/terminal/TerminalPanel.tsx`, `src/panels/terminal/useXterm.ts`, `src/panels/terminal/theme.ts`, `src/ipc/pty.ts` |
| **验收** | `npm test terminal_panel_spawns_on_mount` PASS（`src/__tests__/terminal.test.tsx`）；`npm test terminal_input_writes` PASS |

##### Agent A7 — Editor 面板（Phase '面板'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 实现 EditorPanel + useCodeMirror hook |
| **操作** | ① `useCodeMirror.ts`：`new EditorView({ state: EditorState.create({ doc, extensions: [basicSetup, oneDark] }), parent })` ② 打开文件时调 `ipc.fs.readFile(path)` → 填充 CM6 ③ Ctrl+S → `ipc.fs.writeFile(path, content)` ④ `Prec.highest(keymap.of([{ key: 'Mod-s', ... }]))` 绑定保存 ⑤ `view.destroy()` 在 cleanup 中箭头函数调用（防止 this 丢失）⑥ 注册 `editor` 面板到 panelRegistry ⑦ CM6 extensions 数组预留扩展点（方便 Phase 3 加载 `@codemirror/merge`） |
| **产出** | `src/panels/editor/EditorPanel.tsx`, `src/panels/editor/useCodeMirror.ts`, `src/ipc/fs.ts` |
| **验收** | `npm test editor_saves_edited_content` PASS（`src/__tests__/editor.test.tsx`） |

##### Agent A8 — Workspace（Phase 'Workspace+键盘'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 实现 Workspace + panelRegistry + layoutSerde + 标签页/分屏/右键菜单 |
| **操作** | ① `Workspace.tsx`：`<DockviewReact components={panelRegistry} onReady={...} />` ② `panelRegistry.ts`：注册 terminal/editor 两种面板类型 ③ 终端面板 `renderer: 'always'`（保持 PTY 存活）④ `layoutSerde.ts`：`api.toJSON()` → `stores/layout.ts` 持久化；`api.fromJSON(saved, { reuseExistingPanels: true })` 恢复 ⑤ `stores/sessions.ts`：维护 `Map<panelId, { sessionId, cwd, isActive }>` ⑥ watermarkComponent：显示"打开终端/编辑器"欢迎页 ⑦ 选项卡右键菜单：新建终端面板、新建编辑器面板、关闭面板 ⑧ 关闭最后面板 → 显示 watermark 空白态 |
| **产出** | `src/workspace/Workspace.tsx`, `src/workspace/panelRegistry.ts`, `src/workspace/layoutSerde.ts`, `src/stores/sessions.ts`, `src/stores/layout.ts` |
| **验收** | `npm test closing_last_panel_shows_empty_state` PASS |

##### Agent A9 — 键盘处理（Phase 'Workspace+键盘'，sonnet）

| 项目 | 内容 |
|------|------|
| **任务** | 实现 Ctrl+C 智能分流 + Ctrl+Shift+C/V 复制粘贴 |
| **操作** | ① Rust 侧：`tauri-plugin-prevent-default` v5.0.0（crates.io）——`Builder::default().with_flags(Flags::all().difference(Flags::FIND | Flags::DEV_TOOLS)).build()`，需在 `capabilities/default.json` 加 `"prevent-default:default"` 权限 ② `window.addEventListener('keydown', handler, { capture: true })` ③ Ctrl+Shift+C → `navigator.clipboard.writeText(term.getSelection())` → preventDefault；回退方案：Tauri Rust 菜单加速键（`MenuBuilder::accelerator("CmdOrCtrl+Shift+C")`）④ Ctrl+Shift+V → `navigator.clipboard.readText()` → `term.paste(text)` → preventDefault ⑤ Ctrl+C（终端聚焦 + `term.hasSelection()`）→ 复制选中，preventDefault ⑥ Ctrl+C（终端聚焦 + 无选区）→ 放行给 xterm.js，自然发 `\x03` 到 PTY ⑦ Ctrl+C（编辑器聚焦）→ 放行给 CM6 处理 ⑧ IME 防御：`e.isComposing || e.keyCode === 229` → 全透传；`compositionstart` 5s 超时 + `window.blur` 清理悬挂状态；WebView2 IME 组合期失焦崩溃防御（`mousedown + preventDefault()` 阻止焦点转移）⑨ `attachCustomKeyEventHandler` 返回 `true`/`false` 区分处理 |
| **产出** | `src/panels/terminal/keyboard.ts`（新建） |
| **验收** | L3 测试：`key_encoding_ctrl_c` 无选区发 `\x03`（`test/terminal/keyboard.test.ts`）；`key_encoding_shift_tab` 发 `\x1b[Z]`；L4 E2E 验证复制粘贴 |

---

## 5. 验收闭环

### 5.1 自动化验收

| 层 | 命令 | 通过标准 |
|----|------|---------|
| L1 | `cargo test` | pty_roundtrip, pty_resize_applies, pty_kill_no_orphan, fs_read_write_roundtrip, osc_cwd_venv_parsed 全部 PASS |
| L2 | `npm test` | terminal_panel_spawns_on_mount, terminal_input_writes, editor_saves_edited_content, closing_last_panel_shows_empty_state 全部 PASS |
| L3 | `npm run test:l3` | renders_pwsh_prompt, claude_tui_snapshot, key_encoding_shift_tab, key_encoding_ctrl_c 全部 PASS |
| L4 | `npm run wdio` | 打开终端 → 输入 `echo e2e` → DOM 出现 `e2e` PASS |
| Build | `tauri build --debug` | 退出 0，exe 可启动 |
| Lint | `cargo clippy -- -D warnings` | 退出 0 |
| Lint | `npx eslint src/` | 退出 0 |

### 5.2 CI 门

- `.github/workflows/ci.yml` 已在 Phase 0 配置，L1+L2+L3+build 在 CI 上运行
- 不新增 CI 步骤，不修改现有 CI 结构

### 5.3 人工验收

在所有自动化通过后：

- [ ] 真机敲命令输出正确（pwsh 启动、dir、echo）
- [ ] Ctrl+C 中断运行中命令（`ping -t` → Ctrl+C 停止）
- [ ] Ctrl+Shift+C/V 复制粘贴正常工作
- [ ] 打开 → 编辑 → Ctrl+S 保存 → 外部确认内容正确
- [ ] 拖到四边各正确分屏；嵌套；分隔条调比例
- [ ] 右键菜单新建终端/编辑器面板
- [ ] 真跑 `claude`：TUI 外观正确；Ctrl+C 取消；Shift+Tab 切 plan mode（可能回退到 Alt+M）；中文 IME 正常
- [ ] 窗口缩放后 FitAddon 自动适应

### 5.4 DoD（完成定义）

- 1.1–1.5 完成
- L1–L3 全绿、关键 L4 通过
- 人工清单全部勾选
- 不破坏 Phase 0 回归集

---

## 6. 执行顺序速查

```
1.  W0 前置核验（parallel 3 agent 只读）
2.  A1 pty/spawn.rs（ConPTY spawn + SPAWN_LOCK + CPR）
3.  A2 pty/reader.rs（reader 线程 + Event emit）
4.  A3 pty/shell.rs（PowerShell profile 注入脚本）
5.  A4 fs/mod.rs（文件读写命令）
6.  A5 IPC 类型 + 命令注册（barrier: 等 A1–A4 DTO 定型）
7.  A6 Terminal 面板 + A7 Editor 面板（parallel: 等 A5）
8.  A8 Workspace + A9 键盘处理（barrier: 等 A6+A7 面板注册）
9.  W2 验证（parallel 4 agent）
10. 人工验收
```

---

## 7. 注意事项（踩坑前置）

### 7.1 Windows 关键坑

- **spawn 串行化**：并发 spawn 卡死 ConPTY 输出管道 → `SPAWN_LOCK: Mutex<()>` 包裹
- **cwd 反斜杠**：传给 ConPTY 前规范化成 `\`
- **CPR 响应**：`openpty()` 后立即向 stdin 写 `\x1b[1;1R`（`#[cfg(windows)]` 条件编译——补偿 portable-pty 0.9.0 ConPTY `VtIo::StartIfNeeded()` DSR 握手，xterm.js 自动 CPR 回路此时未闭合）
- **stdin drop**：Windows 上绝对不能 drop stdin（立即杀子进程），加 `#[cfg(not(windows))]` 保护
- **孤儿进程**：每个子进程放入 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
- **ClosePseudoConsole 死锁**：关闭前先排空输出管道

### 7.2 渲染相关

- **WebGL + DOM 兜底**：`detectWebgl()` 预检（`failIfMajorPerformanceCaveat: true`），可用→WebGL，不可用→DOM
- **仅焦点终端持有 WebGL**：聚焦 `loadAddon(new WebglAddon())`，失焦 `dispose()`，500ms 降级迟滞。多面板 WebGL context 数始终 = 1
- **React StrictMode 防御**：`useRef` guard clause（`if (terminalRef.current) return`）防止双重挂载；cleanup 中 `dispose()` + 置 null
- **scrollback ≤ 2000**：控制 DOM 节点数量
- **字体预加载**：`document.fonts.ready` 后再 fit + refresh
- **ResizeObserver**：监听容器变化 → fit → 通知后端 resize（fire-and-forget，不阻塞 UI）
- **`windowsPty` 选项**：`{ backend: 'conpty', buildNumber: 21376 }` 启用 ConPTY reflow/scrollback 兼容

### 7.3 快捷键与 IME

- **`tauri-plugin-prevent-default` v5.0.0**：必须——Rust `Builder::default().with_flags(Flags::all().difference(Flags::FIND | Flags::DEV_TOOLS)).build()`，禁用 WebView2 浏览器加速键（Ctrl+W/T/F/R/P 等）。需 `capabilities/default.json` 添加 `"prevent-default:default"` 权限
- **`Ctrl+C` 不会被插件影响**：`Ctrl+C` 是"文本编辑键"（非浏览器加速键），不在插件管辖范围。前端仍需 handler 智能分流
- **Ctrl+C 智能分流**：`e.isComposing` 先检查，IME 组合中全透传
- **编辑器 Ctrl+S**：CM6 `Prec.highest(keymap)` 绑定，与终端互不干扰
- IME 防御：`isComposing` 标志位 + `compositionstart` 5s 超时 + `window.blur` 清理 + WebView2 IME 失焦崩溃防御（`mousedown + preventDefault()`）
- xterm.js IME 已知限制：#5734（IME 候选窗定位，修复仅在未发布 v7.0.0 中，npm 6.0.0 不包含）

### 7.4 portable-pty 版本

- **保持 0.9.0**（与 Phase 0/terax-ai 一致）：`PSEUDOCONSOLE_INHERIT_CURSOR` 标志导致 ConPTY `VtIo::StartIfNeeded()` 阶段发送 DSR 查询
- **必须 `openpty()` 后立即写 CPR `\x1b[1;1R`** 到 stdin：DSR 在 spawn 后子进程 attach 时发出，此时 xterm.js→前端→PTY 的 CPR 响应回路尚未闭合，ConPTY 3 秒内收不到 stdin 响应即超时
- Turborepo PR #11816 证明此方案稳定（CPR 提前排队在 stdin 管道中，DSR 发出后立即被满足）
- stdin drop 加 `#[cfg(not(windows))]` 保护

### 7.5 Dockview 使用

- `onReady` 中同步 `addPanel` 可能触发"resource is already disposed"竞态 → `useEffect` 延迟
- 终端面板 `renderer: 'always'` 保持 PTY 存活
- `api.toJSON()` 仅存储 `params` 中少量静态数据，动态状态用 Zustand

### 7.6 Shell Integration

- Windows 优先 OSC 9;9（`ESC ] 9 ; 9 ; C:\path ST`），OSC 7 作为跨平台补充
- `-File` 而非 `-Command`：退出码可靠、转义简单
- 脚本 UTF-8 BOM：Windows PowerShell 5.x 硬性要求
- 嵌套检测：`SLTERM_PANE_ID` 环境变量已存在则跳过注入

---

## 8. 决策台账

| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| E1 | 编排策略 | /goal 主干 + W0/W1/W2 workflow；PTY 后端最难点放 /goal 循环 | §1 |
| E2 | 渲染器 | Phase 1 加载 WebGL addon（`failIfMajorPerformanceCaveat` 预检 + DOM 兜底回退），仅焦点终端持有 1 个 context | Agent A6 |
| E3 | PTY IPC 输出 | Channel\<PtyEvent\>（spawn 模式长期持有），与架构文档一致；emit 仅用于退出通知等低频事件 | Agent A2 |
| E4 | PTY IPC 输入 | `invoke`，单次按键 1-5ms 延迟可接受 | Agent A5 |
| E5 | portable-pty 版本 | 保持与 Phase 0 一致的 0.9.0（terax-ai 同版本），CPR 手动写 `\x1b[1;1R` 补偿 ConPTY DSR 握手启动时序 | Agent A1 |
| E6 | PowerShell profile 注入 | CLI 参数注入：`-NoProfile -NoLogo -File script.ps1`；装饰器模式保留用户 prompt | Agent A3 |
| E7 | 集成脚本存储 | debug 读文件 + release `include_str!`；运行时写入 `%APPDATA%\slTerminal\` | Agent A3 |
| E8 | CWD 跟踪 | OSC 9;9 优先（Windows Terminal 默认），OSC 7 补充 | Agent A3 |
| E9 | 命令边界 | OSC 133 A/B/C/D；C 用 PreCommandLookupAction 钩子 | Agent A3 |
| E10 | 面板创建时序 | 前端先生成 panelId 建面板 → invoke pty_spawn → 后端返回 sessionId | Agent A6 |
| E11 | Panel ID 策略 | 前端 panelId（crypto.randomUUID）+ 后端 sessionId（Uuid::new_v4）双层 | Agent A5/A6 |
| E12 | 会话状态存储 | terminal Session 不放 Zustand，用模块级 Map（绕过 React 渲染循环）；低频 UI 状态放 Zustand | Agent A8 |
| E13 | 键盘策略 | 3 个 JS handler：Ctrl+Shift+C/V 复制粘贴 + Ctrl+C 智能分流；其余全部透传 | Agent A9 |
| E14 | WebView2 快捷键 | `tauri-plugin-prevent-default` v5.0.0（`Builder::default().with_flags(...)`），禁用全部浏览器加速键 | Agent A9 |
| E15 | 面板管理 | 纯鼠标：Dockview 拖拽分屏 + 右键菜单 + 中键/×关闭 | Agent A8 |
| E16 | 复制粘贴快捷键 | Ctrl+Shift+C 复制 / Ctrl+Shift+V 粘贴；Ctrl+Shift+C 另配 Rust 菜单加速键兜底 | Agent A9 |
| E17 | 全局热键 | Phase 1 不做（系统托盘点击切换显隐） | — |
