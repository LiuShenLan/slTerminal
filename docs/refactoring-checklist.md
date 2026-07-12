# slTerminal 重构优化清单（全量）

> 基于 5 维度审查的完整重构列表，按模块和优先级组织

---

## 一、架构合规修复（6 项）

| # | 问题 | 文件 | 操作 |
|---|------|------|------|
| A1 | CSP=null → 设置最小 CSP | `tauri.conf.json` | `"default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: https://asset.localhost"` |
| A2 | notify→fs 穿透调用 | `notify/mod.rs` + `state.rs` | `validate_path_within_root` 移至 `state.rs` 共享 |
| A3 | HtmlPanel 硬编码 #FFFFFF/#6C6C6C | `HtmlPanel.tsx` | 改为引用 `theme/colors.ts` token |
| A4 | useXterm.ts 动态 import opener 绕过 ipc/ | `useXterm.ts` + 新建 `ipc/shell.ts` | 新增 `src/ipc/shell.ts` re-export `openUrl` |
| A5 | panelRegistry.ts 在 workspace/ 导致反向依赖 | `workspace/panelRegistry.ts` → `src/panelRegistry.ts` | 提取为共享配置层 |
| A6 | JobHandle #[cfg(windows)] 在 state.rs | `state.rs` + `pty/spawn.rs` | 非 Windows 零大小类型包装 |

---

## 二、构建配置优化（3 项）

| # | 问题 | 文件 | 操作 |
|---|------|------|------|
| B1 | 缺 [profile.release] | `Cargo.toml` | 添加 lto="thin" + codegen-units=1 + strip="symbols" + panic="abort" |
| B2 | 未使用 npm 依赖 5 个 | `package.json` | 删除 @headless-tree/core / @headless-tree/react / @codemirror/autocomplete / @codemirror/lint / @lezer/highlight |
| B3 | @tauri-apps/api 在 devDependencies | `package.json` | 移至 dependencies |

---

## 三、useXterm.ts 功能拆分（7 项）

| # | 新 Hook/文件 | 抽取内容 | 行数 |
|---|-------------|---------|------|
| C1 | `terminal/useTerminalInstance.ts` | Terminal 创建/销毁、WebGL 检测重试、fitAddon、StrictMode 守卫 | ~150 |
| C2 | `terminal/usePtyOutput.ts` | PTY 输出合帧缓冲（Idle+Max 双定时器）、DEC 2026、非焦点降频、直写阈值 | ~150 |
| C3 | `terminal/usePtyResize.ts` | ResizeObserver、X/Y 分离 debounce、NaN 守卫、cancelPendingFlush | ~80 |
| C4 | `terminal/useClipboardHandler.ts` | OSC 52 handler 注册/解析/CJK/焦点门控/payload 上限 | ~60 |
| C5 | `terminal/useCommandDetection.ts` | OSC 133 C/D handler、页签标题切换回调 | ~50 |
| C6 | `terminal/useFontSizeBridge.ts` | 字体大小 store 订阅 + 终端字号应用 + Ctrl+Wheel（终端特化） | ~60 |
| C7 | `terminal/webgl.ts` | `setupWebglWithRetry` 纯函数 + `detectWebgl` 缓存 | ~50 |

**useXterm.ts 保留**：编排上述 hooks，暴露统一接口 → ~100 行

**WebGL timer 泄漏修复**：重试 setTimeout 存入 ref，cleanup 中 clearTimeout

---

## 四、E2E 代码解耦（3 项）

| # | 操作 | 涉及文件 |
|---|------|---------|
| D1 | 抽取所有 `__slterm_e2e_*` / `__e2e_*` 到 `e2e-tests/helpers.ts` | `App.tsx` (~75行) + `Workspace.tsx` (~40行) + `useXterm.ts` (~20行) |
| D2 | 在 `main.tsx` 通过 `if (import.meta.env.DEV)` 条件挂载 | `main.tsx` |
| D3 | E2E DOM 选择器：`[style*="height: 100%"]` → `data-e2e` 属性 | `useXterm.ts` + `test.e2e.ts` |

---

## 五、面板间重复消除（4 项）

| # | 问题 | 操作 |
|---|------|------|
| E1 | activeTerminal/activeEditor 模板重复 | 提取 `createActivePointer<T>()` 泛型工厂 → `lib/activePointer.ts` |
| E2 | useXterm/useCodeMirror 的 Ctrl+Wheel ~50 行重复 | 提取 `useFontSizeWheel(container, getSize, setSize)` hook |
| E3 | keyboard.ts 工厂模式骨架重复 | 提取 `createPanelShortcuts<T>(meta[], getActive)` 泛型工厂 |
| E4 | `justSavedRef` 单值 → `Set<string>` 路径级去重 | `useCodeMirror.ts` |

---

## 六、前端渲染性能（4 项）

| # | 问题 | 文件 | 操作 |
|---|------|------|------|
| F1 | PageDockview 缺 React.memo | `Workspace.tsx` | 包裹 `React.memo` + props 引用稳定化 |
| F2 | onReady/onLayoutChange 内联箭头函数 | `Workspace.tsx` | `useCallback` 稳定化，savedLayout 通过 ref 读取 |
| F3 | useCodeMirror effect deps 含 handleSave | `useCodeMirror.ts` | 从 deps 移除（已通过 handleSaveRef 解耦） |
| F4 | 直写阈值用 text.length 而非 rawBytes.length | `useXterm.ts` | CJK 字符字节语义修正 |

---

## 七、后端关键修复（7 项）

| # | 问题 | 文件 | 操作 |
|---|------|------|------|
| G1 | `RawChild::clone_killer()` 用 `unimplemented!()` | `pty/spawn.rs` | 返回存根实现或 `compile_error!` 文档化 |
| G2 | Mutex::lock().unwrap() 9 处 → expect | `pty/spawn.rs` | 改为 `.expect("上下文")` |
| G3 | `get_windows_build_number().unwrap_or(0)` 静默降级 | `pty/spawn.rs` | 加 `tracing::warn!` |
| G4 | reader_loop `c.send()` 失败被吞 | `pty/reader.rs` | 加 `tracing::debug!` 区分断开 vs 错误 |
| G5 | DA1 响应注入 write_all 失败被吞 | `pty/reader.rs` | 加 `tracing::warn!` |
| G6 | `.bak` 备份 copy 失败被吞 | `settings.rs` | 加 `tracing::warn!` |
| G7 | PtySession 无 Drop impl — reader 线程隐式 detach | `state.rs` | 添加 `Drop` impl，`reader_handle.take().join()` |

---

## 八、死代码与依赖清理（4 项）

| # | 问题 | 涉及文件 | 操作 |
|---|------|---------|------|
| H1 | `set_project_root` / `clear_git_cache` 死命令 | `lib.rs` + `fs/mod.rs` | 删除 Rust 注册 + generate_handler! |
| H2 | `fs_watch` 命令前缀不一致 → `notify_watch` | `notify/mod.rs` + `lib.rs` + `ipc/notify.ts` | 重命名，双边同步 |
| H3 | `mod claude` 声明无对应实现 | `lib.rs` | 删除或加 TODO |
| H4 | sessions.ts store 评估去留 | `sessions.ts` | 若无消费方则删除 |

---

## 九、魔法数字与常量提取（4 项）

| # | 内容 | 涉及文件 |
|---|------|---------|
| I1 | 前端 fallback 尺寸 `80/24/14` 5 处 → `DEFAULT_COLS/ROWS/FONT_SIZE` | `useXterm.ts` |
| I2 | Rust CRLF 样本 `65536` 5 处 → `CRLF_SAMPLE_MAX_BYTES` | `fs/mod.rs` |
| I3 | 3 个 store 持久化 `2000`ms → `PERSIST_DEBOUNCE_MS` | `projects.ts` + `fontSize.ts` + `keybindings.ts` |
| I4 | 其他魔法数字提取 (30+ 处) → 命名常量 | 多文件（详见 03/05 报告） |

---

## 十、内存与资源管理（5 项）

| # | 问题 | 文件 | 操作 |
|---|------|------|------|
| J1 | titleManager 删除页面后条目泄漏 | `titleManager.ts` | `onDeletePage` 时清理 pageId 级条目 |
| J2 | fontSize/keybindings 无 cancelPendingSave | `fontSize.ts` + `keybindings.ts` | 添加并导出，关闭钩子中统一调用 |
| J3 | TerminalPanel 异步 setState 无取消 | `TerminalPanel.tsx` | 加 `cancelled` 标志 |
| J4 | Workspace.tsx 拆分 (~450行) | `Workspace.tsx` | `PageDockviewHost.tsx` + `WorkspaceLayout.tsx` |
| J5 | Dec2026 编码常量提取为模块级 | `useXterm.ts` | `DEC2026_PREFIX`/`SUFFIX` 常量 |

---

## 十一、测试体系改进（8 项）

| # | 问题 | 操作 |
|---|------|------|
| K1 | CI 增加 `cargo clippy -- -D warnings` 步骤 | `.github/workflows/ci.yml` |
| K2 | CI 增加独立 `npx tsc --noEmit` 步骤 | `.github/workflows/ci.yml` |
| K3 | test-inventory.md 自动生成 + 手动确认 + CI 统计检查 | 新增脚本 + `.claude/test-inventory.md` |
| K4 | CI 从 L2 include 中移除 L3 文件防重复执行 | `vitest.config.ts` |
| K5 | PTY 集成测试增加 pty_reattach + pty_kill 路径 | `tests/pty_integration_tests.rs` |
| K6 | 拆分 useXterm.test.ts (2,669行) → 4 个文件 | `src/__tests__/` |
| K7 | useXterm 合帧测试 fake timers 替换真实 setTimeout | `useXterm.test.ts` |
| K8 | `TerminalRegistry._clear()` → `_reset()` 命名统一 | `TerminalRegistry.ts` |

---

## 十二、文档与命名规范（6 项）

| # | 问题 | 操作 |
|---|------|------|
| L1 | `src/ipc/CLAUDE.md` 补充 window.ts | 更新模块映射表 |
| L2 | `src-tauri/src/pty/CLAUDE.md` 补充文件表格 + build.rs | 更新文档 |
| L3 | 6 处 "Phase 0/1" 过期注释更新 | 多文件 |
| L4 | 测试文件命名统一 kebab-case | 25+ 个测试文件 |
| L5 | `pty/build.rs` → `pty/win_build.rs` 避免 cargo 歧义 | 重命名 + 引用更新 |
| L6 | useXterm.ts 返回类型显式标注 `UseXtermReturn` | `useXterm.ts` |

---

## 十三、L3 终端测试扩充（1 项）

| # | 内容 | 目标 |
|---|------|------|
| M1 | 9 条 → 50+ 条 | 覆盖 scrollback、reflow、SGR 组合、CJK/Emoji、交替屏幕缓冲、OSC 序列 |

---

## 统计摘要

| 类别 | 项数 | 预计工时 |
|------|------|---------|
| 架构合规 | 6 | 2h |
| 构建配置 | 3 | 0.5h |
| useXterm 拆分 | 7 | 4-6h |
| E2E 解耦 | 3 | 1.5h |
| 面板重复消除 | 4 | 2h |
| 渲染性能 | 4 | 1.5h |
| 后端修复 | 7 | 2h |
| 死代码清理 | 4 | 0.5h |
| 魔法数字 | 4+ | 2h |
| 内存管理 | 5 | 1.5h |
| 测试改进 | 8 | 3h |
| 文档规范 | 6 | 1h |
| L3 扩充 | 1 | 3h |
| **合计** | **~62** | **~30h (约 4 工作日)** |
