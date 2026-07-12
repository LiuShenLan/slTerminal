# slTerminal 测试体系审查报告

> 审查日期：2026-07-12 | 审查范围：L1~L4 全部测试层级 + CI 配置 + 测试基础设施

---

## 1. 测试体系总体评价

### 1.1 规模概览

| 层级 | 技术栈 | 文件数 | 实际用例数 | CLAUDE.md 声称 | test-inventory.md 声称 |
|------|--------|--------|-----------|---------------|----------------------|
| L1 | Rust `cargo test` | 11 源文件 + 1 集成测试文件 | **~198** (193 嵌入 + 5 集成) | ~193 | 113 |
| L2 | Vitest + jsdom | ~65 测试文件 | **~997** | ~1022 | 542 |
| L3 | Vitest + `@xterm/headless` | 2 文件 | **9** | 9 | 5 |
| L4 | WDIO + embedded driver | 1 文件 | **11** | 11 | 3 |

**实际总计：~1,215 用例**（去重 L3 在 L2 中的重复执行后约 1,206）。

### 1.2 文档准确性

**`test-inventory.md` 严重过时**，与实际代码偏差如下：

- L1：声称 113，实际 198（少计 85 条，差 75%）
- L2：声称 542，实际 997（少计 455 条，差 84%）
- L3：声称 5，实际 9（terminal-serialize 新增 4 条未更新）
- L4：声称 3，实际 11（Phase 1 新增 8 条未更新）
- `integration_test.rs` 声称存在 3 条测试，但该文件不存在

**`CLAUDE.md` 的数字（~193/~1022/9/11）更接近实际**。建议以 CLAUDE.md 为基准，删除或大幅更新 test-inventory.md。

### 1.3 整体评分

| 维度 | 评分 | 说明 |
|------|------|------|
| 测试数量 | B+ | 1200+ 用例，覆盖主要模块 |
| L1 质量 | B | 纯函数测试优秀，集成测试薄弱 |
| L2 质量 | B+ | 覆盖面广，mock 策略合理，存在个别大文件 |
| L3 覆盖 | C | 仅 9 条，终端渲染核心路径大量缺失 |
| L4 覆盖 | C+ | 11 条覆盖关键流程，但受限于 embedded driver |
| CI 完整度 | C+ | 缺 Clippy/tsc 独立检查，仅 Windows |
| 测试隔离 | B+ | Rust 端隔离良好，前端 jsdom 合理 |
| 文档一致性 | D | test-inventory.md 严重过时 |

---

## 2. L1 -- Rust 测试分析

### 2.1 测试分布

| 文件 | `#[test]` 数 | 测试方式 | 质量评估 |
|------|-------------|---------|---------|
| `git/mod.rs` | 58 | `git2::Repository::open` on tempdir | 覆盖面广，含边界和错误路径 |
| `pty/reader.rs` | 28 | 纯函数，无外部依赖 | 优秀，startup strip + DA1 全面覆盖 |
| `fs/mod.rs` | 26 | `tempfile::tempdir()` 隔离 | 覆盖 CRUD + CRLF/LF 保留 |
| `notify/mod.rs` | 24 | classify 纯函数(18) + FileWatcher 手动构造(6) | classify 优秀，FileWatcher 涉及线程 sleep |
| `pty/spawn.rs` | 14 | ConPTY flags 纯函数 + 真实 HPCON 创建 | flags 测试完善，RawChild/JobHandle 未测 |
| `notify/pool.rs` | 12 | `make_test_watcher` mock 工厂 | 覆盖 LRU 淘汰/pause/resume |
| `settings.rs` | 8 | tempfile 读写往返 | 含 .bak 降级和 JSON 损坏分支 |
| `pty/shell.rs` | 7 | 混合：PATH 依赖 + 纯 Base64 | 环境依赖路径脆弱 |
| `state.rs` | 5 | ring buffer append/eviction | 覆盖换行边界 |
| `error.rs` | 4 | serde + Display + From 派生 | 基础覆盖 |
| `lib.rs` | 2 | ping + get_windows_build_number | 冒烟测试 |
| **集成测试** | | | |
| `tests/pty_integration_tests.rs` | 5 | 真实 ConPTY spawn `cmd.exe` | 关键但薄弱，仅 happy path |

### 2.2 优秀实践

1. **纯函数抽取**（reader.rs）：`strip_conpty_startup`、`mirror_da1_query`、`apply_startup_strip`、`should_inject_da1` 均为无状态纯函数，可直接构造输入断言输出。reader.rs 的 28 条测试中 26 条是纯函数测试。

2. **事件分类纯函数化**（notify/mod.rs）：`classify_by_kind` 拆分后 18 条测试覆盖全部 7 种 `EventKind` 及子类型，零外部依赖。

3. **SPAWN_LOCK 串行化**（pty_integration_tests.rs）：ConPTY 并发 spawn 会死锁，`static SPAWN_LOCK: Mutex<()>` 正确保护了所有集成测试。

4. **手动构造绕过 Tauri 依赖**（notify）：`FileWatcher::start()` 需要 `AppHandle` 无法在单元测试中创建，通过直接构造结构体字段 + mpsc channel 模拟线程生命周期，实现了对 stop/Drop/替换/幂等性的全覆盖。

### 2.3 问题列表

#### P1 -- 高风险未测代码（严重）

| 代码 | 风险 | 说明 |
|------|------|------|
| `create_and_assign_job` / `JobHandle` | **孤儿进程泄漏** | 若该 unsafety 代码出错，父进程崩溃后子进程残留。零测试覆盖 |
| `RawChild::try_wait` / `wait` / `kill` | **进程管理失效** | 仅 `kill` 经集成测试间接覆盖，`try_wait`/`wait` 无专项测试 |
| `pty_reattach` 命令 | **页签切换白屏** | ring buffer 回放 + Channel 替换链路完全未测 |
| `reader_loop` 主循环 | **PTY 输出丢失** | 7 个分支的 I/O 编排逻辑仅在集成测试中随 cmd.exe 间接覆盖 |

优先级：应在 CI 中增加上述路径的集成测试，至少覆盖 `pty_reattach` + Channel 断连重连场景。

#### P2 -- 测试环境依赖导致的脆弱性

| 文件 | 问题 | 严重度 |
|------|------|--------|
| `pty/shell.rs` | `test_which_full_path_finds_pwsh_or_powershell` 依赖系统 PATH 中存在 pwsh。最简 CI 容器将失败 | 中 |
| `pty/shell.rs` | `test_resolve_shell_info_returns_full_path` 分支行为取决于运行时环境（找到 pwsh 则走一条路径，否则走 cmd.exe） | 中 |
| `pty/reader.rs` | `strip_conpty_startup` 测试在非 Windows 平台会失败（函数在非 Windows 原样返回，但测试断言剥离结果）。目前仅 Windows CI 运行故未暴露 | 低 |
| `notify/mod.rs` | `file_watcher_drop_stops_thread` 使用 `thread::sleep(100ms)` 等待线程退出，CI 高负载时可能 flake | 低 |
| `pty_integration_tests.rs` | `pty_roundtrip` 使用 `thread::sleep(800ms)` 等待 cmd.exe 启动，CI 慢时不足 | 低 |

#### P3 -- 测试质量问题

| 问题 | 位置 | 说明 |
|------|------|------|
| P11/P12 重复 | `notify/pool.rs` | 两条测试均验证 `pause() -> is_paused() -> resume() -> is_paused()`，P12 无增量覆盖 |
| Drop 空池测试无意义 | `notify/pool.rs:p9` | Drop 一个空池并断言无事发生--未验证 watcher 线程被正确 join |
| 魔数断言 | `pty/reader.rs` | `+ 10`（END_MARKER 长度）和 `+ 4`（TAIL 长度）硬编码，改字符串则测试静默失败 |
| DA1 边界缺失 | `pty/reader.rs` | 无双前导零 `ESC[00c`、CSI introducer 后接非命令字节 |
| OSC ST 终止符未测 | `pty/reader.rs` | `find_osc_end` 同时处理 BEL 和 `ESC\` 终止符，但仅 BEL 有测试 |
| 构造器绕过导致测试漂移 | `notify/mod.rs` | 5 条测试直接构造 `FileWatcher` 结构体，若新增字段编译失败但 `start()` 的初始化 bug 不会被这些测试捕获 |

---

## 3. L2 -- 前端测试分析

### 3.1 最大/最复杂测试文件评估

#### `useXterm.test.ts`（2,669 行，~98 条 -- 实际 120 条 `it()`）

**结论：过大，应拆分。**

- 覆盖 22 个 `describe` 块的广泛领域：rAF 轮询、ResizeObserver、DEC 2026 合帧、Idle+Max 双定时器、非焦点降频、OSC 52 剪贴板、OSC 133 命令边界、cancelPendingFlush、PTY 生命周期、键盘委托
- Mock 策略：7 个模块深度 mock，利用捕获回调（OSC 52/133 handler、keyEventHandler）验证集成行为--这是正确模式
- **问题**：
  - UA4 测试名重复出现两次（行 1305 和 1307），第一个为空函数体（复制粘贴残留）
  - `setTimeout(r, 5)` 等真实定时器在合帧测试中大量使用（DEC/TH/IT 套件），CI 中脆弱
  - 与 `useCodeMirror.test.ts` 的 Ctrl+Wheel 字体测试（测试 42-47 vs 5-9）几乎完全相同
  - `testMocks/xterm.ts` 提供了可复用 mock 工厂但 useXterm.test.ts 未使用，仍内联 mock
- **建议**：拆分为 4 个文件：输出合帧、ResizeObserver+布局、OSC 协议、PTY 生命周期

#### `shortcuts.test.ts`（645 行，41 条）

**结论：质量标杆。**

- 零 mock：使用真实 `ShortcutRegistry` 单例 + 真实 `KeyboardEvent` dispatch
- `_reset()` 在 `afterEach` 中隔离
- 覆盖注册/注销/引用计数/上下文栈/匹配排序/IME/global/重绑/resolve/export
- 无时序依赖、无环境依赖

#### `explorer-refresh-preserve.test.tsx`（474 行，17 条）

**结论：设计优雅。**

- 虚拟文件系统（`helpers/vfs.ts`）模式：`Map<string, DirEntry[]>` 模拟磁盘，动态增删触发 refresh
- 递归展开状态保留验证（A-E 矩阵）：单层/多层/新增文件/删除文件/目录变文件
- 竞态测试（R17）：`Promise.all` 并发 refresh + 中间 VFS 变更
- 三条触发路径（file-saved/fs-event/CRUD）均覆盖

#### `useCodeMirror.test.ts`（694 行，28 条）

**结论：mock 深度合理，部分测试过薄。**

- Mock 策略：完整替换 `EditorView`/`EditorState`/`Compartment`，保留真实 `keymap` 导出
- toggleWordWrap 测试（12-19）仅验证 `mockReconfigure` 调用--未验证实际的 CM6 wrapping 行为（这需要真实 DOM，不可行，但薄到仅测 mock 调用仍值得注意）
- handleSave 测试（HS1-HS8）验证了真实 IPC 调用链：save -> writeFile -> gitDiff -> gutter -> CustomEvent，这是实质性测试
- 测试 19（卸载后 toggleWordWrap）注释暗示测试顺序依赖

#### `ipc-contract.test.ts`（656 行，47 条）

**结论：系统且完整。**

- 四维验证模式（命令名/参数结构/返回值/异常传播）覆盖所有 IPC 模块
- Channel 绑定、Uint8Array 序列化（`[72, 101]` -> `number[]`）、settings 空值降级等关键边界均覆盖
- 每条 wrapper 函数至少 2 条测试（happy path + error path）

### 3.2 测试辅助工具评估

| 工具 | 质量 | 说明 |
|------|------|------|
| `helpers/vfs.ts`（60 行） | 优秀 | `makeVfs` + `mockEntry` + `findNode`，价值密度最高的辅助文件 |
| `helpers/xterm-test-utils.ts`（146 行） | 良好 | `mockRaf`/`ptyOutputSpy`/`mockResizeObserver`/`makeKeyEvent` 消除大量重复 |
| `helpers/workspace-setup.ts`（92 行） | 良好 | `seedExplorerProject`/`seedMultiPageProject` 消除 ~7 文件的 store 种子重复 |
| `testMocks/explorerMocks.ts`（98 行） | 良好 | 主要是文档--`globalThis` workaround 的用法说明 |
| `testMocks/xterm.ts`（136 行） | 待整合 | 可复用 mock 工厂存在但 `useXterm.test.ts` 未使用，内联了重复 mock |
| `setup.ts`（111 行） | 良好 | polyfill（crypto/ResizeObserver/matchMedia/fonts）+ IPC notify 全局 mock |

### 3.3 通用问题

| 问题 | 严重度 | 涉及文件 |
|------|--------|---------|
| Ctrl+Wheel 字体测试重复 | 低 | `useXterm.test.ts` 42-47 vs `useCodeMirror.test.ts` 5-9 |
| 真实 setTimeout 脆弱 | 中 | `useXterm.test.ts` DEC/TH/IT 套件大量 `setTimeout(r, 5)` |
| UA4 重复测试名 | 低 | `useXterm.test.ts` 行 1305-1326 |
| xterm mock 工厂未用 | 低 | `testMocks/xterm.ts` vs `useXterm.test.ts` 内联 mock |
| useCodeMirror toggle 测试仅验证 mock 调用 | 低 | `useCodeMirror.test.ts` 12-19 |
| test-inventory L2 计数 542 vs 实际 997 | 中 | `.claude/test-inventory.md` |

---

## 4. L3 -- 终端 headless 测试分析

### 4.1 覆盖范围

| 测试 | 覆盖内容 |
|------|---------|
| 基本文本写入 + serialize | `should contain "hi"` |
| 多行输出（12 行） | `should preserve all lines` |
| ANSI 颜色保留 | 红色前景/绿色背景/粗体 + 重置码 |
| 大块数据（>1000 字符） | 哨兵验证无丢失 |
| CSI 光标定位（CUP） | `\x1b[5;10H` + `\x1b[2;3H` |
| pwsh 提示符渲染 | `PS C:\Users\test>` 文本快照 |
| Claude TUI ANSI 颜色 | `\x1b[34mClaude Code\x1b[0m` |
| Shift+Tab CBT 编码 | `\x1b[Z` consumption + 光标回退效果 |
| Ctrl+C ETX 编码 | `\x03` onData 投递验证 |

### 4.2 显著缺失

- **滚动/回滚**：terminal 创建为 24 行，从未写入超过 24 行测试 scrollback 行为
- **Reflow**：从未 resize 终端测试文本重排
- **SGR 属性叠加**：粗体+下划线+颜色同时使用
- **宽字符/CJK/Emoji**：`wcwidth` 相关行为完全未测
- **交替屏幕缓冲**：`\x1b[?1049h/l` 切换从未测试
- **DEC 私用模式**：无任何 DECSET/DECRST 测试
- **输入法/组合字符**：grapheme cluster 处理未测
- **OSC 序列**：OSC 7（cwd）/ OSC 0,2（标题）/ OSC 8（超链接）均未测

### 4.3 评估

对于以终端模拟为核心的项目，L3 是防御终端渲染回退的首要防线，但目前仅覆盖最基础的 happy path。9 条测试不足以建立对终端渲染正确性的信心。

**建议**：将 L3 扩展为 50+ 条，重点覆盖 scrollback、reflow、SGR 组合、宽字符、交替屏幕，并建立 ANSI 序列的"黄金文件"快照测试。

---

## 5. L4 -- E2E 测试分析

### 5.1 覆盖的关键用户流程

| 流程 | 测试名 |
|------|--------|
| 应用启动 + 标题验证 | `应正常启动并显示 slTerminal 标题` |
| 完整 PTY 通信链路 | `打开终端->写入文本->验证缓冲含 e2e_marker` |
| 键盘快捷键->剪贴板 | `终端面板可通过 E2E helper 写入文本并读取` |
| H6 终端跨页面存活 | `should preserve terminal content after switching to another page and back` |
| 页签标题 `terminal-N` | `终端页签标题为 terminal-N` |
| 编辑器页签标题 | `编辑器页签标题为文件名` |
| 同名文件冲突标题 | `同名文件冲突时显示相对路径` |
| 关闭冲突面板标题恢复 | `关闭同名面板后剩余面板切回 basename` |
| 编辑器 Ctrl+S 保存 | `聚焦编辑器后 Ctrl+S -> 经 capture 路径真实写盘` |
| HTML iframe Ctrl+W 转发 | `iframe 内 Ctrl+W -> 转发关闭该 HTML 页签` |
| 编辑器 dirty->clean 保存 | `should persist modified content after external change triggers reload then Ctrl+S save` |

### 5.2 核心问题

#### P1 -- Embedded driver 的键盘限制（不可解）

`@wdio/tauri-service` 1.1.0 的 embedded driver **无法将 OS 级按键投递进 WebView2**。所有键盘快捷键测试使用合成 `KeyboardEvent` dispatch 替代真实的 OS->WebView2 键盘路径。

**已测试的部分**：JS 层 ShortcutRegistry、命令 handler、IPC 写盘
**未测试的部分**：Tauri -> WebView2 键盘桥、`tauri-plugin-prevent-default` 加速键拦截、WebView2 硬编码键处理

这是驱动层面的根本限制，非测试代码可修复。Phase 0 设计文档已记录此限制。

#### P2 -- 脆弱的 DOM 选择器

四个 E2E helper（`__e2e_sessionReady`、`__e2e_writeToPty`、`__e2e_writeToTerminal`、`__e2e_getTerminalText`）均通过 `document.querySelectorAll('[style*="height: 100%"]')` 定位终端容器。任何 CSS 重构改变内联样式都会破坏 E2E 发现机制。

**建议**：为终端容器添加专用 `data-e2e` 属性，替代内联样式选择器。

#### P3 -- `browser.pause(500)` 硬等待

H6 跨页面存活测试使用两处 `browser.pause(500)` 替代事件驱动等待。页面切换完成后应有明确的 DOM 状态变化（如 `display:block`），可改用 `waitUntil` 轮询该状态。

#### P4 -- 临时目录泄漏风险

三个测试（编辑器保存、HTML Ctrl+W、H6）使用 `mkdtempSync` + `finally` 清理。若 WDIO session 在测试执行中崩溃，临时目录泄漏在 CI runner 上。

---

## 6. CI 配置分析

### 6.1 当前配置（`.github/workflows/ci.yml`）

| 步骤 | 命令 | 状态 |
|------|------|------|
| Lint check | `npx eslint src/` | 仅检查 `src/`，不包含 `test/`、`e2e-tests/`、配置文件 |
| Frontend build | `npm run build`（含 `tsc && vite build`） | tsc 通过 build 间接执行 |
| L2 测试 | `npm test` | 运行 |
| L3 测试 | `npm run test:l3` | 运行（**L3 被重复执行**：`npm test` 的 include 也包含 `test/**/*.test.ts`） |
| Backend build | `cargo build` | 运行 |
| L1 测试 | `cargo test` | 运行（未指定 `--test-threads=1`，依赖测试内部 SPAWN_LOCK） |
| Tauri debug build | `npx tauri build --debug --no-bundle` | 运行 |
| L4 测试 | `npm run wdio` | 运行 |

### 6.2 缺失的门禁

| 缺失项 | 影响 | CLAUDE.md 是否提到 |
|--------|------|-------------------|
| `cargo clippy -- -D warnings` | Rust 代码质量未强制检查 | 是（列为静态检查门禁） |
| `npx tsc --noEmit`（独立步骤） | 类型错误仅在 build 时发现，且可能被 vite build 错误掩盖 | 是 |
| `cargo fmt --check` | Rust 格式化未检查 | 否 |
| 覆盖率上报 | `npm run test:coverage` 存在但 CI 不运行 | 否 |
| Linux/macOS runner | 跨平台 bug 永不被 CI 捕获 | 否 |
| Build artifact 上传 | 调试产物不可回溯 | 否 |

### 6.3 L3 重复执行

`vitest.config.ts` 的 `include` 同时包含 `src/__tests__/**` 和 `test/**/*.test.ts`，而 `vitest.l3.config.ts` 单独再跑一次 `test/terminal/**`。L3 的 9 条测试在 CI 中被执行两次。无明显危害但浪费 CI 时间。

**建议**：从 `vitest.config.ts` 的 include 中移除 `test/**/*.test.ts`，L3 仅通过 `npm run test:l3` 运行。

---

## 7. 测试改进建议（按优先级排列）

### 高优先级（应在下一个迭代完成）

1. **更新 test-inventory.md**：当前严重过时（L1 差 75%，L2 差 84%）。建议以其为单一真值源或删除之--CLAUDE.md 的数字更准确。

2. **CI 增加 Clippy 步骤**：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`。这是已文档化的门禁但 CI 未执行。

3. **CI 增加独立 `tsc --noEmit` 步骤**：在 `npm run build` 之前独立运行，确保类型错误不被 vite 构建掩盖。

4. **替换 E2E 脆弱的 DOM 选择器**：`[style*="height: 100%"]` 改为专用 `data-e2e` 属性，消除 CSS 变更对 E2E 的联动破坏。

5. **增加 PTY 集成测试**：至少覆盖 `pty_reattach`（Channel 替换 + ring buffer 回放）和 `pty_kill` Tauri 命令（非裸 `child.kill()`）。

### 中优先级（下两个迭代）

6. **拆分 `useXterm.test.ts`**：按关注点拆为 4 个文件（输出合帧/ResizeObserver+布局/OSC 协议/PTY 生命周期），每个控制在 600 行以内。

7. **L3 扩充至 50+ 条**：重点覆盖 scrollback、reflow、SGR 组合、宽字符（CJK/Emoji）、交替屏幕缓冲。

8. **useXterm 合帧测试改用 fake timers**：替换 `setTimeout(r, 5)` 等真实定时器，消除 CI 中的 flake 风险。

9. **useXterm.test.ts 使用 `testMocks/xterm.ts` 工厂**：消除内联 mock 与可复用工厂的重复。

10. **消除 Ctrl+Wheel 字体测试重复**：`useXterm.test.ts` 和 `useCodeMirror.test.ts` 中的 Ctrl+Wheel 测试提取为共享参数化测试。

### 低优先级（后续迭代）

11. **修复 useXterm.test.ts UA4 重复测试名**（行 1305 空函数体残留）。

12. **`notify/pool.rs` 删除 P12 重复测试**（与 P11 完全重复）。

13. **`notify/pool.rs:p9` Drop 空池测试** 改为插入 watcher 后验证线程被 join。

14. **`reader.rs` 增加 OSC ST 终止符和 DA1 边界测试**。

15. **`pty/shell.rs` 环境依赖测试** 增加 `#[ignore]` 或条件跳过（非 Windows / 无 pwsh 环境）。

16. **CI 从 L2 include 中移除 L3 文件**：消除重复执行。

17. **E2E `browser.pause(500)` 替换为 DOM 状态轮询**。

---

## 附录：测试文件完整清单

### L1 -- Rust 源文件测试（`#[test]` 计数）

| 文件 | 测试数 |
|------|--------|
| `src-tauri/src/git/mod.rs` | 58 |
| `src-tauri/src/pty/reader.rs` | 28 |
| `src-tauri/src/fs/mod.rs` | 26 |
| `src-tauri/src/notify/mod.rs` | 24 |
| `src-tauri/src/pty/spawn.rs` | 14 |
| `src-tauri/src/notify/pool.rs` | 12 |
| `src-tauri/src/settings.rs` | 8 |
| `src-tauri/src/pty/shell.rs` | 7 |
| `src-tauri/src/state.rs` | 5 |
| `src-tauri/src/error.rs` | 4 |
| `src-tauri/src/lib.rs` | 2 |
| **嵌入式测试小计** | **188** |

### L1 -- Rust 集成测试

| 文件 | 测试数 |
|------|--------|
| `src-tauri/tests/pty_integration_tests.rs` | 5 |

### L2 -- 前端测试（`it()` 计数，前 20 文件）

| 文件 | 测试数 |
|------|--------|
| `src/__tests__/useXterm.test.ts` | 120 |
| `src/__tests__/ipc-contract.test.ts` | 47 |
| `src/__tests__/shortcuts.test.ts` | 44 |
| `src/__tests__/projects.test.ts` | 41 |
| `src/__tests__/sidebar-actions.test.ts` | 33 |
| `src/__tests__/explorer-git-status.test.tsx` | 32 |
| `src/__tests__/canFit.test.ts` | 31 |
| `src/__tests__/titleManager.test.ts` | 29 |
| `src/__tests__/useCodeMirror.test.ts` | 28 |
| `src/__tests__/path.test.ts` | 27 |
| ...其余 ~55 文件 | ~565 |
| **L2 小计** | **~997** |

### L3 -- 终端 headless 测试

| 文件 | 测试数 |
|------|--------|
| `test/terminal/terminal-serialize.test.ts` | 5 |
| `test/terminal/keyboard.test.ts` | 4 |

### L4 -- E2E 测试

| 文件 | 测试数 |
|------|--------|
| `e2e-tests/test.e2e.ts` | 11 |
