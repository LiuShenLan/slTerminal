# slTerminal 性能与可维护性审查报告

> 审查日期：2026-07-12 | 审查方法：多 Agent 并行静态分析 + 手动文件审计

---

## 1. 总体评价

**综合评分：7.5/10**（良好，有明确改进空间）

项目整体工程质量扎实。前端 IPC 隔离、后端模块拆分、PTY 资源管理、测试金字塔覆盖等架构决策执行力强。10 条硬约束中 8 条完全合规。

主要薄弱点在三个领域：
- **React 渲染优化**：多 Dockview 实例场景下缺少 `React.memo`、部分 `useCallback` 依赖过宽
- **构建配置**：Cargo.toml 缺少 `[profile.release]` 优化段，CSP 为 null
- **模块边界**：2 处硬编码穿透（notify→fs、HtmlPanel 颜色），1 处 ipc/ 绕过

按严重程度统计：
| 严重程度 | 数量 | 占比 |
|----------|------|------|
| 🔴 严重 | 5 | 14% |
| 🟡 中等 | 12 | 33% |
| 🔵 建议 | 19 | 53% |

---

## 2. 渲染性能分析

### 2.1 多 Dockview 实例 + CSS display:none/block 策略

**整体评价**：策略正确（解决 xterm.js 不支持二次 `open()` 的根因），但 React 层面的优化有遗漏。

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 2.1.1 | `PageDockview` 缺少 `React.memo` | `src/workspace/Workspace.tsx` L225 | 🔴 严重 | 纯展示容器无 memo 包裹，每次 Workspace 重渲染时所有已初始化的 PageDockview 都重新执行函数体。任何 store 变更（expandedNodes、version 递增等）均触发级联重渲染 | 包裹 `React.memo<PageDockviewProps>(…)`，配套 props 引用稳定化（见 2.1.2） |
| 2.1.2 | `onReady`/`onLayoutChange` 内联箭头函数 | `src/workspace/Workspace.tsx` L504-505 | 🔴 严重 | 每次渲染创建新函数引用，`PageDockview` 的 `handleReady` 的 `useCallback` deps 包含 `savedLayout`（来自 store），形成依赖链：store 变更 → 新函数引用 → 下游重渲染 | `onReady`/`onLayoutChange` 用 `useCallback` 稳定化；`savedLayout` 通过 ref 读取，不进入 deps |
| 2.1.3 | `allPages` 依赖整个 `projects` 对象 | `src/workspace/Workspace.tsx` L384-397 | 🔵 建议 | `useProjects((s) => s.projects)` 返回整个 record，expandedNodes 变更也会触发重算 | 接受现状——扁平化 O(n) 足够轻量；真正问题在于下游无 memo 保护 |
| 2.1.4 | `DefaultTab` 未使用 `React.memo` | `src/workspace/Workspace.tsx` L139 | 🔵 建议 | Dockview 可能在 Tab 拖拽等操作时重复调用 `defaultTabComponent` | 包裹 `React.memo<IDockviewPanelProps>(…)` |
| 2.1.5 | `initializePages` Set 重建开销 | `src/workspace/Workspace.tsx` L401-406 | 🟡 中等 | `setInitializedPages(prev => new Set([...prev, pageId]))` 每次展开为数组再建 Set，O(n) | 改为 `new Set(prev).add(pageId)`，当前页面数少可忽略 |

### 2.2 终端输出合帧策略

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 2.2.1 | 直写阈值用 `text.length` 而非字节数 | `src/panels/terminal/useXterm.ts` L260-262 | 🟡 中等 | 中文字符 `text.length=1` 但 UTF-8=3 字节，CJK 内容更易落入直写路径而非合帧路径，语义偏差 | 改为 `rawBytes.length < 64` |
| 2.2.2 | `flushBuffer` 每次创建 `TextEncoder` + 编码常量 | `src/panels/terminal/useXterm.ts` L241-248 | 🔵 建议 | 高频 flush（每 16ms）重复编码 `"\x1b[?2026h"` 和 `"\x1b[?2026l"` | 提取为模块级常量 `DEC2026_PREFIX`/`SUFFIX` |
| 2.2.3 | Idle 2ms + Max 16ms 参数合理 | `src/panels/terminal/useXterm.ts` L283-294 | 通过 | Idle 2ms 对 Ink 60fps burst 足够短；Max 16ms 对齐 60fps 帧间隔防饥饿 | 维持不变 |
| 2.2.4 | 非焦点终端降频 64KB 上限合理 | `src/panels/terminal/useXterm.ts` L270-281 | 通过 | 约 1-2 秒输出量；超限 FIFO 丢弃最旧数据保留最新——语义正确 | 维持不变 |

### 2.3 ResizeObserver 性能

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 2.3.1 | ResizeObserver 缺少 rAF 节流 | `src/panels/terminal/useXterm.ts` L369-418 | 🟡 中等 | 用户快速拖拽分隔条时可能 100ms 内触发 10+ 次回调，每次都调用 `proposeDimensions()`（WebGL GPU readback） | 回调入口包裹 `requestAnimationFrame()` 推迟到下一帧 |
| 2.3.2 | X/Y 分离 debounce 策略合理 | `src/panels/terminal/useXterm.ts` L379-417 | 通过 | 仅行变化立即 fit+resize；列变化 100ms debounce；无变化跳过 | 维持不变 |
| 2.3.3 | NaN 守卫正确 | `src/panels/terminal/useXterm.ts` L372 | 通过 | `Number.isFinite()` 防止 WebGL 未就绪时 `proposeDimensions()` 返回 NaN 传入 `pty.resize()` | 维持不变 |

### 2.4 React 重渲染控制

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 2.4.1 | `useCodeMirror` 主 effect 依赖 `handleSave` | `src/panels/editor/useCodeMirror.ts` L272 | 🔴 严重 | `handleSave` 在其中通过 `handleSaveRef` 访问——放入 deps 意味着任何 handleSave 依赖变化都会触发 EditorView 重建（销毁+创建），丢失文档状态和光标位置 | 从 deps 中移除 `handleSave` |
| 2.4.2 | `handleTabStateChange` 依赖整个 `params` 对象 | `src/panels/terminal/TerminalPanel.tsx` L61-69 | 🟡 中等 | Dockview 可能每次传新 `params` 引用 → `useCallback` 重建 → `useXterm` 的 `onTabStateChange` 变化 | 只依赖 `params.panelId` 等实际使用字段，或用 `paramsRef` |
| 2.4.3 | TerminalPanel loading 用硬编码 1.5s 超时 | `src/panels/terminal/TerminalPanel.tsx` L73-76 | 🔵 建议 | PTY spawn 可能在 100ms 完成（延迟消失）或超过 1.5s（提前消失显示空白） | 用 `useXterm` 返回的 ready 回调驱动 loading 状态 |
| 2.4.4 | `handlePtyOutput` 通过 ref 解耦——设计优秀 | `src/panels/terminal/useXterm.ts` L312 | 通过 | 闭包依赖仅 `[flushBuffer]`（deps=[]），通过 ref 访问最新状态，PTY 回调永不重建 | 维持不变 |

---

## 3. I/O 性能分析

### 3.1 PTY Reader

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 3.1.1 | `READER_BUF_SIZE=16KB` 合理 | `src-tauri/src/pty/reader.rs` L20 | 🔵 建议（通过） | 189KB/s 场景下 ~12 次/秒 read() vs 4KB 的 ~47 次/秒，减少 75% 系统调用。Claude Code Ink ~60fps 单次 64-200 字节，16KB 足够容纳多帧合并 | 维持不变 |
| 3.1.2 | `startup_drained` 后仍走函数调用 | `src-tauri/src/pty/reader.rs` L76-84 | 🔵 建议 | 首轮之后每轮 `apply_startup_strip` 都是 `Some(data.to_vec())` 恒等操作，但仍走函数调用+match 分支 | 在 reader_loop 中 `if startup_drained { buf[..n].to_vec() } else { apply_startup_strip(...) }` 内联 |
| 3.1.3 | Ring buffer 256KB + 1KB 行边界淘汰 | `src-tauri/src/state.rs` L96-114 | 通过 | 容量缓存 ~4000+ 行；换行边界淘汰防止 UTF-8 截断；无换行符时退化为硬淘汰 | 维持不变 |
| 3.1.4 | reader_loop 每轮读锁获取 | `src-tauri/src/pty/reader.rs` L87-91 | 🟡 中等 | 12 次/秒 RwLock 读锁原子操作开销，但 Channel 替换极低频（页面切换级），正确性优先 | 维持不变（正确性优先于微小性能） |

### 3.2 文件监听

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 3.2.1 | ReadDirectoryChangesW 大目录注册 2s | `src-tauri/src/notify/mod.rs` | 🟡 中等 | target/ 26K 文件首次注册需 2s；pause/resume 缓存命中 <1ms。但首次打开大项目无进度提示 | 增加 tracing 日志；如未来用户反馈强烈，加"建立文件监听…"状态提示 |
| 3.2.2 | LruWatcherPool 容量=5 合理 | `src-tauri/src/state.rs` L83 | 通过 | 2-3 个典型项目数留有 2 个槽位余量；每个 watcher ~2KB 非分页池内存 | 维持不变 |
| 3.2.3 | pause/resume 替代 stop/start | `src-tauri/src/notify/pool.rs` | 通过 | AtomicBool 控制，watcher 线程继续运行、OS 句柄保留 | 维持不变 |

### 3.3 spawn_blocking 使用

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 3.3.1 | 所有阻塞 I/O 均正确使用 spawn_blocking | `src-tauri/src/fs/`、`src-tauri/src/git/`、`src-tauri/src/pty/`、`src-tauri/src/settings.rs` | 通过 | 审计 12 个 Tauri 命令，全部阻塞操作（文件 I/O、git2、pty kill）均经 `spawn_blocking` 包装，无遗漏 | 维持不变 |
| 3.3.2 | pty_kill 锁释放策略正确 | `src-tauri/src/pty/spawn.rs` L784-810 | 通过 | 先 `sessions.remove()`（<1ms，释放写锁），再 `spawn_blocking` 中 kill→join→drop（ClosePseudoConsole 可能永久阻塞） | 维持不变（设计优秀） |

### 3.4 git2 性能

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 3.4.1 | `git_status` 无超时/取消机制 | `src-tauri/src/git/mod.rs` L125 | 🟡 中等 | 大仓库（数万文件）可能消耗显著 CPU；前端页面切换后旧请求继续运行；`include_ignored(true)` 已移除 | 增加文件数上限（如 10000）；超时包裹 `tokio::time::timeout` |
| 3.4.2 | `git_repo_cache` 无容量限制 | `src-tauri/src/git/mod.rs` L68-112 | 🔵 建议 | HashMap 无 LRU，E2E 测试或自动化场景可能无限增长 | 维持（手动 `clear_git_cache` 足够，后续可参照 LruWatcherPool） |
| 3.4.3 | `git_diff` 已通过 pathspec 限定文件范围 | `src-tauri/src/git/mod.rs` L185 | 通过 | 单文件 diff 操作，不会扫描整个仓库 | 维持不变 |

---

## 4. 内存管理分析

### 4.1 xterm.js 生命周期

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 4.1.1 | WebGL 重试定时器未跟踪 | `src/panels/terminal/useXterm.ts` L99, L113 | 🔴 严重 | `setupWebglWithRetry` 指数退避 setTimeout（最长 16s）不存 timer ID，组件卸载后回调仍触发，对已 dispose Terminal 创建 WebglAddon 造成 GPU 资源泄漏 | 存储 timer ID 到 ref，cleanup 中 clearTimeout |
| 4.1.2 | dispose 清理链完整 | `src/panels/terminal/useXterm.ts` L617-659 | 通过 | 13 步清理：isDisposedRef→cancelAnimationFrame→clearTimeout×3→ResizeObserver.disconnect→pty.kill→TerminalRegistry.remove→OSC handler dispose×2→retryDisposable dispose→webglAddon dispose→fitAddon dispose→terminal dispose | 维持不变 |
| 4.1.3 | StrictMode smGuardRef 正确 | `src/panels/terminal/useXterm.ts` L319-323 | 通过 | 同 panelId 二次挂载直接 return；Terminal 实例数=1 | 维持不变 |
| 4.1.4 | Ctrl+Wheel 事件正确配对 | `src/panels/terminal/useXterm.ts` L747-750 | 通过 | addEventListener/removeEventListener 引用一致、capture 参数一致 | 维持不变 |

### 4.2 PTY 后端资源

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 4.2.1 | `PtySession` 无显式 Drop——reader 线程隐式 detach | `src-tauri/src/state.rs` L13-36 | 🟡 中等 | `reader_handle: Option<JoinHandle>` 默认 Drop 会 detach。生产路径（pty_kill）已显式 join，但 panic unwind 或 HashMap::clear 等隐式 drop 路径缺少防御 | 添加 `Drop` impl，显式 `reader_handle.take().join()` |
| 4.2.2 | `ConPtyInner::drop` 正确 | `src-tauri/src/pty/spawn.rs` L305-313 | 通过 | 先 drop writer 再 ClosePseudoConsole，顺序正确 | 维持不变 |
| 4.2.3 | ring buffer 淘汰按换行边界对齐 | `src-tauri/src/state.rs` L93-107 | 通过 | 在 drain_target 内找 `\n` 位置淘汰，防止 UTF-8 截断 | 维持不变 |

### 4.3 文件树内存

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 4.3.1 | gen 取消机制正确 | `src/features/explorer/useFileTree.ts` L185-213 | 通过 | `rootPath` 变化递增 `genRef`，异步回调检查 gen 匹配性，过期结果丢弃 | 维持不变 |
| 4.3.2 | `rootNodesRef` 镜像避免闭包陷阱 | `src/features/explorer/useFileTree.ts` L36-37 | 通过 | 每次渲染同步 `rootNodesRef.current = rootNodes`，`reloadPreservingExpanded` 中读"触发时刻"旧树 | 维持不变 |
| 4.3.3 | rootPath 变化同步清空 rootNodes | `src/features/explorer/useFileTree.ts` L203-210 | 通过 | `setRootNodes([])` 同步清空 + 后续异步加载在同一批次，中间状态不可见 | 维持不变 |
| 4.3.4 | TreeNode 数量无虚拟化 | `src/features/explorer/useFileTree.ts` L20-26 | 🔵 建议 | 大目录数千文件时 state 膨胀；`target/` 等已知大目录已有后端过滤 | 考虑单级目录文件数上限（如 500），超限截断提示 |

### 4.4 定时器与事件监听器

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 4.4.1 | 合帧 Idle+Max 定时器正确配对清理 | `src/panels/terminal/useXterm.ts` L200-224, L630-638 | 通过 | `cancelPendingFlush` + `flushBuffer` 均清除两个 timer，cleanup 兜底；三重保障无泄漏 | 维持不变 |
| 4.4.2 | `fontSize` / `keybindings` store 无公开 cancelPendingSave | `src/stores/fontSize.ts` L69-71, `src/stores/keybindings.ts` L76-78 | 🟡 中等 | `projects.ts` 有 `cancelPendingSave()` 供关闭钩子调用，但 fontSize 和 keybindings 没有，关闭时 2s 窗口内数据可能丢失 | 添加并导出 `cancelPendingSave()`，在关闭钩子中统一调用 |
| 4.4.3 | 全局 addEventListener 全部正确配对 | `useXterm.ts`、`useCodeMirror.ts`、`useFileTree.ts`、`Workspace.tsx`、`usePanelFocus.ts`、`ShortcutRegistry.ts` | 通过 | 审计全部 7 处 addEventListener，均有对应 removeEventListener，capture 参数一致 | 维持不变 |
| 4.4.4 | Tauri listen() unlisten 全部正确配对 | `useFileTree.ts`、`useCodeMirror.ts` | 通过 | `onFsEvent` 返回闭包 `() => unlisten.then(fn => fn())`，消费者 cleanup 中正确调用 | 维持不变 |
| 4.4.5 | ShortcutRegistry 引用计数正确 | `src/features/shortcuts/ShortcutRegistry.ts` L60-72 | 通过 | refCount>0 装监听器，refCount<=0 卸。`_reset()` 同步卸 | 维持不变 |

### 4.5 Dockview 面板生命周期

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 4.5.1 | `titleManager` 注册表页级条目不清理 | `src/workspace/titleManager.ts` L28, L31 | 🟡 中等 | `registry: Map<pageId, Map<panelId, Entry>>` 和 `counters: Map<pageId, Counter>` 删除页面后该页空 Map 和 Counter 条目永不删除。100 个废弃页面约泄漏 10KB | 在 `onDeletePage` 时清理 `pageId` 级条目 |
| 4.5.2 | `api.clear()` + `api.dispose()` 触发完整清理链 | `src/workspace/Workspace.tsx` L426-436 | 通过 | clear → 所有面板 unmount → useEffect cleanup → Terminal dispose/PTY kill；dispose → 销毁 Dockview 实例 | 维持不变 |
| 4.5.3 | `DefaultTab` 事件 dispose 正确 | `src/workspace/Workspace.tsx` L144-157 | 通过 | `onDidTitleChange`/`onDidParametersChange` disposables 在 cleanup 中正确 dispose | 维持不变 |

---

## 5. 构建配置分析

### 5.1 Cargo.toml

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 5.1.1 | 缺少 `[profile.release]` 配置段 | `src-tauri/Cargo.toml` | 🔴 严重 | 使用 Rust 默认值：lto=off（体积+15-30%）、codegen-units=16（欠优化）、strip="none"（含完整调试符号）、debug=true。当前 15.9MB exe | 添加 `[profile.release]`：`lto = "thin"`、`codegen-units = 1`、`strip = "symbols"`、`panic = "abort"`——预计减少 30-40% 体积，编译时间 +2-3x |
| 5.1.2 | `wdio-webdriver` 不在 dev-dependencies | `src-tauri/Cargo.toml` L27-31 | 🟡 中等 | `#[cfg(debug_assertions)]` 条件编译已禁用 release 初始化；但仍参与编译（+5-10s）、包体积含插件代码（+200-500KB） | Cargo 无更好条件依赖机制——可接受 |
| 5.1.3 | notify 使用 RC 版本 | `src-tauri/Cargo.toml` L35-38, L53-55 | 🟡 中等 | notify 9.0.0-rc.4 / notify-debouncer-full 0.8.0-rc.2 为预发布版本 | CI 中增加定期检查，关注正式发布 |

### 5.2 Vite 配置

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 5.2.1 | 默认 minify/treeshaking 已启用 | `vite.config.ts` | 通过 | ESBuild minify + Rollup treeshaking 生产构建默认开启；`chunkSizeWarningLimit: 1024` 合理（本地加载无网络传输） | 维持不变 |
| 5.2.2 | `optimizeDeps.exclude` workaround 正确 | `vite.config.ts` L25-26 | 通过 | 排除 dockview-react/core 防止 ESBuild 损坏 Orientation 枚举 | 维持不变 |
| 5.2.3 | 无显式 vendor 分包 | `vite.config.ts` | 🔵 建议 | xterm.js、CodeMirror、dockview 三大依赖未拆分，一个大 chunk | Tauri 本地加载无意义，但可提升热更新速度。非必要 |

### 5.3 Tauri 配置

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 5.3.1 | CSP 设置为 `null` | `src-tauri/tauri.conf.json` L24 | 🔴 严重（安全） | 完全禁用 Content Security Policy，XSS 攻击无缓解 | 设置最小 CSP：`"default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: https://asset.localhost"` |
| 5.3.2 | 窗口配置合理 | `src-tauri/tauri.conf.json` L13-19 | 🔵 建议 | 1200x800 暗色主题；无 minWidth/minHeight 限制 | 可设 `minWidth: 600, minHeight: 400` 防过小错位 |

---

## 6. 模块耦合度分析

### 6.1 前端模块依赖

**依赖方向（自底向上）**：
```
types/  ←  最底层，无项目内依赖
                              ↓
ipc/  ←  依赖 types/，不依赖任何业务模块
                              ↓
  lib/  utilities/  ←  纯函数，无依赖
                              ↓
┌──────────────┐
│   stores/    │  ←  依赖 ipc/ (load/save)，含 1 处类型导入异常（见 R1）
└──────┬───────┘
       ↓
┌──────────────┐
│  features/   │  ←  依赖 ipc/ + stores/ + lib/，含 1 处反向依赖 workspace/（见 M2）
└──────┬───────┘
       ↓
┌──────────────┐
│  panels/     │  ←  依赖 ipc/ (useXterm→ipc/pty) + features/ (shortcuts) + stores/
└──────┬───────┘
       ↓
┌──────────────┐
│  workspace/  │  ←  顶层组合层，依赖所有下层
└──────────────┘
```

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 6.1.1 | features→workspace 反向依赖 | `src/features/explorer/ExplorerPanel.tsx` L15, L17 | 🟡 中等 | features 引用 workspace/titleManager 和 workspace/panelRegistry。panelRegistry 本质是共享配置/注册表，物理位置导致跨层引用 | 将 `panelRegistry.ts` 提取为 `src/panelRegistry.ts` 共享配置层 |
| 6.1.2 | stores→features 类型导入 | `src/stores/keybindings.ts` L13 | 🔵 建议 | `import type { KeybindingOverrides } from "../features/shortcuts"`，store 不应知 features 概念 | 将 `KeybindingOverrides` 移至 `src/types/shortcuts.ts` |
| 6.1.3 | 无循环依赖 | 全局 | 通过 | 依赖方向清晰：types→ipc→stores→features→panels→workspace，无环 | 维持不变 |

### 6.2 Rust 模块依赖

**依赖方向**：
```
state.rs  ←  数据结构定义，被所有模块引用
                              ↓
pty/  ←  依赖 state.rs，不依赖其他业务模块
                              ↓
fs/  ←  依赖 state.rs，不依赖其他业务模块  [被 notify 穿透调用，见约束 #2]
                              ↓
git/  ←  依赖 state.rs，不依赖其他业务模块
                              ↓
notify/  ←  依赖 state.rs，含 1 处违规穿透调用 fs 函数
                              ↓
lib.rs  ←  顶层，聚合所有模块
```

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 6.2.1 | notify→fs 穿透调用 | `src-tauri/src/notify/mod.rs` L230 | 🔴 严重 | 违反约束 #2——notify 直接调 `crate::fs::validate_path_within_root()` | 将 `validate_path_within_root` 移至 `state.rs` |
| 6.2.2 | `mod claude` 声明无对应实现 | `src-tauri/src/lib.rs` L7 | 🔵 建议 | Rust 允许空 mod 占位不编译错误，但声明了无内容的模块 | 若无后续计划删除声明；有则添加 TODO |

---

## 7. 10 条架构约束合规性检查

| 约束 | 原文摘要 | 合规状态 | 违规项 |
|------|---------|----------|--------|
| #1 | 前端绝不直接碰 OS/文件/进程，invoke 仅在 src/ipc/ | 🟡 1 项穿透 | useXterm.ts 动态 import `@tauri-apps/plugin-opener` 不经过 ipc/ 封装（M1） |
| #2 | 后端按功能分模块，模块间不互相穿透，共享只经 state.rs | 🔴 1 项穿透 | notify 调用 `crate::fs::validate_path_within_root()`（S1） |
| #3 | 命令统一注册于 lib.rs generate_handler! | ✅ 合规 | 全部 20 个命令均在 generate_handler! 中注册 |
| #4 | DTO 双边对应，snake_case↔camelCase | ✅ 合规 | PTY/FS/Git/Notify 四模块 DTO 一一对应，IPC 合约测试 47 条四维验证 |
| #5 | 面板封闭，panels/index.ts + panelRegistry.ts + PANEL_TYPES | ✅ 合规 | terminal/editor/htmlviewer 三个类型完全对应 |
| #6 | 配色单点，颜色从 theme/colors.ts 引用 | 🔴 1 项硬编码 | HtmlPanel.tsx 中 `#FFFFFF` 和 `#6C6C6C`（S2）；SidebarTree/FileTree 右键菜单阴影 rgba 重复（R2） |
| #7 | 布局单点，只经 layoutSerde.ts 存取 | ✅ 合规 | 全部 toJSON/fromJSON 调用仅在 layoutSerde.ts |
| #8 | 会话元数据单点，sessions store + 模块级 Map | ✅ 合规 | PTY 进程映射在 TerminalRegistry（模块级 Map），sessions store 只存前端元数据 |
| #9 | #[cfg(windows)] 只允许在 pty/spawn.rs 和 pty/shell.rs | 🟡 1 项偏差 | state.rs 中 JobHandle 使用了 `#[cfg(windows)]`，属类型级条件编译非业务逻辑（M3） |
| #10 | 权限最小化，capabilities 显式放行，不用通配 `*` | ✅ 合规 | 全部显式声明 |

**合规率：8/10 完全合规，2 项有轻微/严重违规。**

---

## 8. 可测试性分析

### 8.1 纯函数分离

| # | 函数 | 位置 | 纯函数化程度 | 评价 |
|---|------|------|-------------|------|
| 8.1.1 | `classify_by_kind` | `src-tauri/src/notify/mod.rs` | 完全纯函数 | 输入 `EventKind`+路径列表，输出 `FsEventPayload`。直接从 `classify_event` 编排层拆出，19 条测试覆盖全部 7 种 EventKind |
| 8.1.2 | `strip_conpty_startup` / `apply_startup_strip` | `src-tauri/src/pty/reader.rs` | 纯函数 | 输入字节数组，输出剥离后数组。6+4 条测试 |
| 8.1.3 | `should_inject_da1` / `mirror_da1_query` | `src-tauri/src/pty/reader.rs` | 纯函数 | 布尔判断 + 模式匹配函数。4+5 条测试 |
| 8.1.4 | `reader_loop` | `src-tauri/src/pty/reader.rs` | I/O 编排层（不可纯化） | 7 个关键分支已逐分支标注依赖类型（Mutex/RwLock/系统调用） |
| 8.1.5 | `normalizePath` / `basename` / `isChildOf` / `relativePath` | `src/lib/path.ts` | 完全纯函数 | 27 条测试，零外部依赖 |
| 8.1.6 | keystroke 互转 | `src/features/shortcuts/keystroke.ts` | 完全纯函数 | `formatKeystroke`/`parseKeystroke`/`isValidKeystrokeString` 全部纯函数 |

### 8.2 测试隔离度

| # | 发现 | 文件 | 严重程度 | 分析 | 建议 |
|---|------|------|----------|------|------|
| 8.2.1 | useXterm 需 mock 7 个模块 | `src/panels/terminal/useXterm.ts` | 🟡 中等 | 测试依赖 xterm×3 + pty IPC + clipboard IPC + shortcuts + opener，98 条用例但 mock 门槛高 | 考虑拆分出 `usePtyBridge`（PTY+合帧+OSC）和 `useTerminalRenderer`（WebGL+fit+fontSize）；评估组合复杂度增加 vs 测试简化收益 |
| 8.2.2 | 4 个单例均有测试隔离方法 | `ShortcutRegistry._reset()`、`TabTitleRegistry._reset()`、`FileViewerRegistry._reset()`、`TerminalRegistry._clear()` | 通过 | 全部支持测试隔离 | `TerminalRegistry._clear()` 命名不一致，建议统一为 `_reset()` |
| 8.2.3 | `detectWebgl` 模块级缓存 | `src/panels/terminal/useXterm.ts` L50 | 🔵 建议 | 性能优化合理，但对测试是隐藏模块状态。当前测试已 mock WebGL addon，无实际影响 | 如将来需单 run 内切换返回值，添加 `_resetWebglCache()` 辅助函数 |
| 8.2.4 | 无全局可变状态 | 全项目 | 通过 | 后端无模块级 static mut；前端模块级 Map/Set 为只读常量或 `_reset()` 支持 | 维持不变 |

### 8.3 可测试性总结

**优势**：
- PTY reader 和 notify 模块已将可测试逻辑从 I/O 编排中分离为纯函数，有专门的单元测试覆盖
- 前端 IPC 合约测试用 `mockIPC` 四维验证每条封装函数，47 条测试无遗漏
- 4 个单例均有 `_reset()` 支持测试隔离
- `useXterm` 的 `_test` 接口暴露内部函数供测试直调
- 共享测试工厂（`xterm-test-utils.ts`、`vfs.ts`、`explorerMocks.ts`）消除重复

**改进空间**：
- `useXterm` 职责过重（10+ 项），mock 门槛 7 个模块——拆分可降低耦合
- `TerminalRegistry._clear()` 命名不一致

---

## 9. 重构建议汇总（按优先级排列）

### P0（建议在下一个 PR 中修复，预计总工时 2-3h）

| 编号 | 类别 | 严重 | 描述 | 文件 | 预计工时 |
|------|------|------|------|------|----------|
| P0-1 | 构建 | 🔴 | 添加 `[profile.release]`：lto="thin"、codegen-units=1、strip="symbols"、panic="abort" | `src-tauri/Cargo.toml` | 15min |
| P0-2 | 安全 | 🔴 | 设置最小 CSP 替代 `csp: null` | `src-tauri/tauri.conf.json` L24 | 10min |
| P0-3 | 架构-#2 | 🔴 | 将 `validate_path_within_root` 从 fs/mod.rs 移至 state.rs | `src-tauri/src/state.rs`、`notify/mod.rs` | 30min |
| P0-4 | 架构-#6 | 🔴 | HtmlPanel 硬编码颜色改为 token 引用 | `src/panels/html/HtmlPanel.tsx`、`theme/colors.ts` | 15min |
| P0-5 | 渲染 | 🔴 | `PageDockview` 包裹 `React.memo` + `onReady`/`onLayoutChange` 稳定化 | `src/workspace/Workspace.tsx` | 30min |
| P0-6 | 渲染 | 🔴 | 从 useCodeMirror 主 effect deps 移除 `handleSave` | `src/panels/editor/useCodeMirror.ts` L272 | 10min |
| P0-7 | 内存 | 🔴 | WebGL 重试 setTimeout 存入 ref + cleanup 中 clearTimeout | `src/panels/terminal/useXterm.ts` L99, L113 | 15min |

### P1（建议在下个迭代修复，预计总工时 3-4h）

| 编号 | 类别 | 严重 | 描述 | 文件 | 预计工时 |
|------|------|------|------|------|----------|
| P1-1 | 架构-#1 | 🟡 | 新增 `src/ipc/opener.ts` 封装 `openUrl` | `src/ipc/opener.ts`、`useXterm.ts` | 10min |
| P1-2 | 依赖 | 🟡 | `panelRegistry.ts` 从 workspace/ 提取为 `src/panelRegistry.ts` | `workspace/panelRegistry.ts` | 30min |
| P1-3 | 架构-#9 | 🟡 | `JobHandle` 非 Windows 零大小类型包装，消除 state.rs 的 `#[cfg(windows)]` | `src-tauri/src/pty/spawn.rs`、`state.rs` | 20min |
| P1-4 | 渲染 | 🟡 | 直写阈值从 `text.length` 改为 `rawBytes.length` | `src/panels/terminal/useXterm.ts` L262 | 5min |
| P1-5 | 渲染 | 🟡 | ResizeObserver 回调入口包裹 `requestAnimationFrame` | `src/panels/terminal/useXterm.ts` L369 | 10min |
| P1-6 | 内存 | 🟡 | `PtySession` 添加 `Drop` impl（防御性 join reader_handle） | `src-tauri/src/state.rs` | 20min |
| P1-7 | 内存 | 🟡 | `titleManager` 在 `onDeletePage` 时清理 pageId 级条目 | `src/workspace/titleManager.ts` | 20min |
| P1-8 | 内存 | 🟡 | `fontSize`/`keybindings` store 添加 `cancelPendingSave()` 并在关闭钩子中调用 | `stores/fontSize.ts`、`stores/keybindings.ts`、`App.tsx` | 20min |

### P2（后续迭代择机修复）

| 编号 | 类别 | 严重 | 描述 | 文件 | 预计工时 |
|------|------|------|------|------|----------|
| P2-1 | 渲染 | 🔵 | 合帧 `DEC2026_PREFIX`/`SUFFIX` 提取为模块级常量 | `src/panels/terminal/useXterm.ts` L241-248 | 5min |
| P2-2 | 渲染 | 🔵 | `DefaultTab` 包裹 `React.memo` | `src/workspace/Workspace.tsx` L139 | 5min |
| P2-3 | 渲染 | 🔵 | TerminalPanel loading 用 ready 回调替代 1.5s 超时 | `src/panels/terminal/TerminalPanel.tsx` L73-76 | 15min |
| P2-4 | I/O | 🔵 | reader_loop 中 startup_drained 后内联直接 to_vec | `src-tauri/src/pty/reader.rs` L76-84 | 5min |
| P2-5 | I/O | 🔵 | `git_status` 增加文件数上限 | `src-tauri/src/git/mod.rs` | 15min |
| P2-6 | I/O | 🟡 | `fs_watch` 首次注册添加 tracing 耗时日志 | `src-tauri/src/notify/mod.rs` | 5min |
| P2-7 | 内存 | 🔵 | TreeNode 目录文件数上限 + 截断提示 | `src/features/explorer/useFileTree.ts` | 20min |
| P2-8 | 依赖 | 🔵 | `KeybindingOverrides` 类型移至 `src/types/shortcuts.ts` | `types/shortcuts.ts`、`stores/keybindings.ts` | 10min |
| P2-9 | 架构-#9 | 🟡 | notify 关注正式版发布，增加定期检查 | CI | 5min |
| P2-10 | 可测试 | 🟡 | useXterm 职责拆分评估（usePtyBridge + useTerminalRenderer） | `useXterm.ts` | 2-4h |
| P2-11 | 可测试 | 🟡 | `TerminalRegistry._clear()` 重命名为 `_reset()` | `TerminalRegistry.ts` | 5min |
| P2-12 | 样式 | 🔵 | 右键菜单阴影提取为 `theme/colors.ts` token | `SidebarTree.tsx`、`FileTree.tsx` | 10min |

---

## 附录：审计范围

| 类别 | 审计文件数 | 审计方法 |
|------|-----------|---------|
| 前端渲染 | 8 | 静态代码分析 + ResizeObserver/useCallback/useMemo 模式审查 |
| 后端 I/O | 6 | 代码阅读 + spawn_blocking 搜索 + 缓冲区参数分析 |
| 内存管理 | 12 | 全量 addEventListener/removeEventListener 配对审计 + setTimeout/clearTimeout 配对审计 |
| 构建配置 | 4 | Cargo.toml/vite.config.ts/tauri.conf.json/package.json 逐项分析 |
| 模块耦合 | 10+ | import 依赖图 + use 语句审计 |
| 架构约束 | 17 | 10 条硬约束逐条 Grep 搜索验证 |
| 可测试性 | 8 | 纯函数统计分析 + 单例模式 `_reset()` 覆盖率 |
