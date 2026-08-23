# 测试质量修复清单（test-review-fix）

> 输入：`docs/test-review/` 7 份分报告（65 项）。去重 1 项、翻案 2 项后 **64 项（62 修复 + 2 翻案留痕）**。
> 每项六段式：位置 / 现状 / 修复步骤 / 测试同步 / 文档同步 / 验证。执行 agent 照做即可，禁止自行设计。
> 组织方式：不使用 P0-P4，**优先级由 Stage 依赖顺序表达**（见 `stages.md`）。
> 事实核验：全部修复点已经三轮 Explore agent 实读原文核验（2026-08-23），与报告漂移处逐项留痕。

## 去重与翻案留痕

| 处理 | 项 | 说明 |
|------|----|------|
| 去重 | 01 报告 M-1 → 并入 TQ-CI-01 | 同一问题（L1 inventory 724 vs 726），以 06 报告证据为准 |
| 翻案 | TQ-CI-04（tempfile 移 [dev-dependencies]） | **报告失实**：`tempfile::NamedTempFile` 被 4 个生产文件用于原子写（settings.rs:11,92-93 / projects.rs:12,30 / hooks/claude/config.rs:20,240 / hooks/claude/inject.rs:20,365,376,495,511,681）。不移段，仅留痕（见 TQ-CI-04 条目） |
| 翻案 | TQ-E-07（launcher 下载 msedgedriver） | **报告失实**：run-wdio.cjs 全文无 msedgedriver 逻辑，wdio.conf.ts 已 `driverProvider: 'embedded'` 直连。实跑日志下载噪音来自便携 Node 22（Node 26 undici 8 兼容，已有 >1MB 判活缓存），设计合理保留。仅留痕（见 TQ-E-07 条目） |

## ID → 报告对照

| ID | 来源报告条目 | 严重度 |
|----|--------------|--------|
| TQ-L1-01 | 01 L-1 settings 重试 helper | 低 |
| TQ-L1-02 | 01 L-2 条件 skip CI 空跑 | 低 |
| TQ-L1-03 | 01 L-3 独立 SPAWN_LOCK | 低 |
| TQ-L1-04 | 01 L-4 Mutex 中毒无回归 | 低 |
| TQ-L1-05 | 01 I-1 pty_integration_tests 无平台守卫 | 低 |
| TQ-L1-06 | 01 I-2 write_if_size_differs 大小判定 | 低 |
| TQ-A-01 | 02 H-1 diff-panel 偶发失败（实证） | 高 |
| TQ-A-02 | 02 M-1 全局 stub 不恢复 | 中 |
| TQ-A-03 | 02 M-2 CM6 几何测量噪音 | 中 |
| TQ-A-04 | 02 M-3 Watermark 手写组件 | 中 |
| TQ-A-05 | 02 M-4 barrel mock 部分导出 | 中 |
| TQ-A-06 | 02 L-1 阈值魔数双维护 | 低 |
| TQ-A-07 | 02 L-2 注释 40px vs 46px | 低 |
| TQ-A-08 | 02 L-3 弱断言 | 低 |
| TQ-B-01~19 | 03 报告 19 项（顺序同报告） | 高3/中8/低8 |
| TQ-C-01~04 | 04 报告 4 项 | 中2/低2 |
| TQ-E-01~10 | 05 报告 10 项 | 高5/中3/低2 |
| TQ-CI-01~07 | 06 报告 7 项 | 高2/中4/低1 |
| TQ-COV-01~10 | 07 报告 10 项 | 高4/中4/低2 |

---

# TQ-L1（Rust L1 测试，6 项）

## TQ-L1-01 settings 并发保存测试的重试 helper 容忍度高于生产

- **位置**：`src-tauri/src/settings.rs:419-460`
- **现状**：`run_save_with_retry`（:419-430）做 5 次 × 50ms 重试调用真实 `save_settings`；生产 `save_settings`（:70-101）在 `SETTINGS_SAVE_LOCK` 保护下单次 persist、失败即 `Err`，无重试。
- **修复步骤**（按用户决策取「注释登记」方案，不改生产行为）：
  1. 在 `run_save_with_retry` 函数文档注释中追加两句（可照抄）：
     ```rust
     /// 【容忍度声明】本重试仅容忍 Windows 杀软/索引扫描占用文件的瞬时窗口，
     /// 不代替生产锁语义——生产 save_settings 无重试（单次 persist 失败即 Err）。
     /// 若 SETTINGS_SAVE_LOCK 被移除/失效，本测试可能假绿，锁语义由
     /// pty/模块并发用例与 code review 兜底。
     ```
- **测试同步**：无（注释改动，用例数不变）。
- **文档同步**：无。
- **验证**：grep `容忍度声明` 命中 `src-tauri/src/settings.rs`；`cargo test --manifest-path src-tauri/Cargo.toml concurrent_saves_never_torn -- --test-threads=1` 通过。

## TQ-L1-02 条件 skip 用例在 CI 上可能空跑

- **位置**：`src-tauri/src/pty/shell.rs:630-639`；`src-tauri/src/hooks/signal.rs:439-466`（`try_create_symlink` 定义 :467-476）；`src-tauri/src/hooks/watcher.rs:291-305`；`src-tauri/src/notify/mod.rs:1189-1249`；`src-tauri/src/agent_history/claude/ops.rs:438-514`（三处）
- **现状**：5 处用例在 alias 不存在 / symlink 创建失败时 `eprintln!` + `return` 跳过；CI runner 无权限时计「通过」但未真实执行分支。
- **修复步骤**（登记方案，不改测试代码）：
  1. 在 `.claude/test-inventory.md` L1 段末尾追加「条件跳过用例」小节（可照抄）：
     ```markdown
     ### 条件跳过用例（有效覆盖依赖 runner 环境）

     以下用例依赖 Windows 应用执行别名或 symlink 创建权限（管理员/开发者模式），
     环境不满足时 eprintln + return 跳过但仍计「通过」——CI 上对应分支可能空跑：
     - `pty/shell.rs::test_allowlist_accepts_real_alias_when_present`（依赖 %LOCALAPPDATA%\Microsoft\WindowsApps alias）
     - `hooks/signal.rs::process_symlink_signal_deletes_without_read`、`hooks/watcher.rs::collect_excludes_symlink_files`、`notify/mod.rs` symlink 两用例、`agent_history/claude/ops.rs` symlink 三用例（依赖 symlink 特权，BE-17/D5 豁免先例）
     本地开发机（已开开发者模式）为真实覆盖来源；CI runner 未开权限时上述分支覆盖记为「不确定」。
     ```
- **测试同步**：无。
- **文档同步**：同上（inventory）。
- **验证**：grep `条件跳过用例` 命中 `.claude/test-inventory.md`。

## TQ-L1-03 PTY 集成测试使用独立 SPAWN_LOCK，不验证生产锁

- **位置**：`src-tauri/tests/pty_integration_tests.rs:7`（`static SPAWN_LOCK: Mutex<()>`）、:16（持锁）
- **现状**：集成测试直接用 portable-pty + 自有锁；生产锁为 `AppState.pty.spawn_lock`（state.rs:52，pub），测试不经 AppState 无法共用。
- **修复步骤**（注释方案）：
  1. 在 `:7` 的 `static SPAWN_LOCK` 上方追加注释（可照抄）：
     ```rust
     /// 【锁边界声明】本锁仅隔离测试进程自身的并发 spawn（ConPTY 并发 spawn 死锁红线）。
     /// 不验证生产 AppState.pty.spawn_lock 的串行化范围——生产锁语义由
     /// spawn.rs 内 pty_capacity_* / validate_spawn_request 等用例与 BE-01/BE-12 注释锁死。
     ```
- **测试同步**：无。
- **文档同步**：无。
- **验证**：grep `锁边界声明` 命中 `src-tauri/tests/pty_integration_tests.rs`；`cargo test --manifest-path src-tauri/Cargo.toml --test pty_integration_tests -- --test-threads=1` 通过。

## TQ-L1-04 std Mutex 中毒错误分支无回归用例

- **位置**：`src-tauri/src/settings.rs:70-72`；`src-tauri/src/pty/spawn.rs:1138-1140`（及多处 `.lock().map_err`）；`src-tauri/src/state.rs:138-140`
- **现状**：多处 `.lock().map_err(...)` 中毒分支无用例；项目决策为临界区无 panic、中毒不可达。
- **修复步骤**（登记方案）：
  1. 在 `src-tauri/src/pty/CLAUDE.md` 追加一行（位置：该文件「测试模式」或坑位小节，无则文末新增「豁免登记」小节）：
     ```markdown
     - **Mutex 中毒分支无回归用例**（TQ-L1-04）：spawn.rs/settings.rs/state.rs 的 `.lock().map_err` 中毒路径按「临界区无 panic、中毒不可达」决策不设回归用例；临界区未来引入可 panic 代码时须补中毒场景测试或改不中毒原语。
     ```
- **测试同步**：无。
- **文档同步**：`src-tauri/src/pty/CLAUDE.md` 如上。
- **验证**：grep `Mutex 中毒分支无回归用例` 命中 `src-tauri/src/pty/CLAUDE.md`。

## TQ-L1-05 pty_integration_tests.rs 无 Windows 平台守卫

- **位置**：`src-tauri/tests/pty_integration_tests.rs:1-2`
- **现状**：文件硬编码 `cmd.exe`（:31），无 cfg 守卫，非 Windows 编译运行即失败。
- **修复步骤**：
  1. 在 `:1`（`/// PTY 集成测试` 文档注释行）**之前**插入 crate 级守卫（可照抄）：
     ```rust
     //! PTY 集成测试（Windows-only：硬编码 cmd.exe / ConPTY 语义）
     #![cfg(windows)]
     ```
     同时将原 `:1` 的 `/// PTY 集成测试` 普通文档注释并入上一行 `//!` 注释（避免 `///` 悬空在 cfg 属性前导致语义变化）。
- **测试同步**：无（Windows 上用例数不变）。
- **文档同步**：无。
- **验证**：`src-tauri/tests/pty_integration_tests.rs` 前 3 行含 `#![cfg(windows)]`；`cargo test --manifest-path src-tauri/Cargo.toml --test pty_integration_tests -- --test-threads=1` 通过（Windows）。

## TQ-L1-06 write_if_size_differs 仅按文件大小判定 vendor 升级

- **位置**：`src-tauri/src/pty/conpty_api.rs:152-160`；测试 :265-276
- **现状**：函数注释已写「嵌入内容编译期固定，大小判定足够」（:152）；按用户决策**不改行为**（不改哈希判定）。
- **修复步骤**（登记假设）：
  1. 在 `src-tauri/src/pty/CLAUDE.md` 追加（与 TQ-L1-04 同一小节内）：
     ```markdown
     - **vendor 升级大小判定假设**（TQ-L1-06）：`write_if_size_differs` 仅以文件大小判定覆盖——假设「不同 vendor 版本大小必然不同」（嵌入内容编译期固定，同内容同大小必相同，故无漏更新风险）；若未来 vendor 来源改为外部可变文件，须改内容哈希判定并同步 `write_if_size_differs_overwrites_only_on_size_mismatch` 用例。
     ```
- **测试同步**：无。
- **文档同步**：`src-tauri/src/pty/CLAUDE.md` 如上。
- **验证**：grep `vendor 升级大小判定假设` 命中 `src-tauri/src/pty/CLAUDE.md`。

---

# TQ-A（L2 工作区/面板，8 项）

## TQ-A-01 diff-panel 脏态弹窗用例组合运行 1/8 偶发失败（实证）

- **位置**：`src/__tests__/diff-panel.test.tsx:620-650`（用例）；`getDiffView` helper :114-120
- **现状**（实证 1/8 失败 `TypeError: Cannot read properties of null (reading 'dispatch')`）：
  ```ts
  const rightView = getDiffView(container, "diff-right")!;
  rightView.dispatch({ changes: { from: 0, insert: "dirty" } });
  ```
  `getDiffView` 仅在 `.cm-editor` DOM 存在且 `EditorView.findFromDOM` 反查到实例时返回非空；测试只 `waitFor` 了外层 `[data-e2e="diff-panel"]`。
- **修复步骤**：
  1. 在 `rightView.dispatch` 之前插入等待（可照抄）：
     ```ts
     // 等 CM6 编辑器实例挂载完成（组合运行时可能滞后于面板 DOM——实证 1/8 偶发失败）
     await waitFor(() => {
       expect(getDiffView(container, "diff-right")).toBeTruthy();
     });
     const rightView = getDiffView(container, "diff-right")!;
     ```
  2. 删除原 `const rightView = ...` 行（合并进上方）。
  3. 同文件内若存在其他「`getDiffView(...)!` 后紧跟 dispatch」的用例（含 :653-672 确认重载用例），同样前置 waitFor。
- **测试同步**：本文件用例数不变；修复后组合重跑验证（见验证段）。
- **文档同步**：无。
- **验证**：①`npx vitest run src/__tests__/diff-panel.test.tsx` 通过；②组合重跑（17 文件清单见 `docs/test-review/02-l2-workspace-panels.md` 复跑段）连续 3 轮全绿。

## TQ-A-02 模块级全局 stub 不在 afterAll 恢复

- **位置**：`src/__tests__/workspace.test.tsx:33-38`；`workspace-multi-instance.test.tsx:40-44`；`workspace-switch-order.test.tsx:40-44`；`workspace-page-dockview.test.tsx:45-49`；`use-xterm-integration.test.ts:162-167`
- **现状**：四文件顶层 `global.ResizeObserver = class {...}` 无恢复；use-xterm-integration 顶层 `vi.spyOn(HTMLCanvasElement.prototype, "getContext")` 依赖 setup.ts afterAll 兜底。
- **修复步骤**（4 个 ResizeObserver 文件逐一，共 4 处）：
  1. 顶层 stub 前保存原值并注册恢复（可照抄，每文件同样式）：
     ```ts
     // 模块级 stub 须 afterAll 恢复——防同 worker 后续文件被污染（TQ-A-02）
     const originalResizeObserver = global.ResizeObserver;
     global.ResizeObserver = class ResizeObserver {
       observe() {}
       unobserve() {}
       disconnect() {}
     };
     afterAll(() => {
       global.ResizeObserver = originalResizeObserver;
     });
     ```
  2. 各文件 import 行补 `afterAll`（vitest 导入清单追加）。
  3. `use-xterm-integration.test.ts`：在顶层 spyOn 之后追加：
     ```ts
     afterAll(() => {
       (HTMLCanvasElement.prototype.getContext as unknown as { mockRestore?: () => void }).mockRestore?.();
     });
     ```
     并补 `afterAll` import。
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：grep `originalResizeObserver` 命中 4 个 workspace 测试文件；grep `mockRestore?.()` 命中 use-xterm-integration.test.ts；`npm test` 全绿。

## TQ-A-03 CM6 几何测量 stderr 噪音（jsdom 缺 Range.getClientRects）

- **位置**：`src/__tests__/setup.ts:62-73`（既有 beforeAll/afterAll 段之后）
- **现状**：跑 diff-panel*.test.tsx 时 stderr 反复 `TypeError: textRange(...).getClientRects is not a function`，测试仍通过——CM6 measurement 走异常回退。
- **修复步骤**：
  1. 在 setup.ts 的 `beforeAll` 块内（getContext stub 之后）追加几何 stub（可照抄）：
     ```ts
     // jsdom 缺 Range.getClientRects——CM6 几何测量走异常回退路径并刷 stderr（TQ-A-03）。
     // 返回空矩形列表：测量结果为零尺寸，断言不依赖具体几何值，仅消除异常路径。
     if (typeof Range !== "undefined" && !Range.prototype.getClientRects) {
       Range.prototype.getClientRects = () => ({
         length: 0,
         item: () => null,
         [Symbol.iterator]: [][Symbol.iterator],
       }) as unknown as DOMRectList & (() => DOMRectList);
     }
     ```
     注意：jsdom 的 Range.prototype 上 `getClientRects` 本就 **不存在**（错误来自 CM6 直接调用），故 stub 直接赋值即可；若赋值报类型错，用 `(Range.prototype as unknown as { getClientRects: unknown }).getClientRects = ...` 形态。
  2. `Element.prototype.getBoundingClientRect` 不动（jsdom 有默认返回 0 值实现）。
- **测试同步**：用例数不变；diff 相关断言若因测量路径变化而失败，逐一核实为「旧断言依赖异常回退」后按真实零尺寸语义修正（禁止为通过而改阈值）。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/diff-panel.test.tsx src/__tests__/diff-panel-stale-banner.test.tsx 2>&1` 输出不再含 `getClientRects is not a function`；`npm test` 全绿。

## TQ-A-04 Watermark 回归用手写组件，未覆盖生产 createWatermark

- **位置**：生产 `src/workspace/PageDockviewHost.tsx:79-117`（模块私有）；测试 `src/__tests__/workspace-header-actions.test.tsx:356-442`
- **现状**：测试内联手写 `Watermark` 组件模拟 `addPanel` 参数；生产 `createWatermark` 未导出。
- **修复步骤**：
  1. 生产微改：`PageDockviewHost.tsx:79` `function createWatermark(` 改为 `export function createWatermark(`（仅加 export，函数体不动）。
  2. 测试改写：`workspace-header-actions.test.tsx` 的 "Watermark 回归" describe 删除手写组件，`import { createWatermark } from "../workspace/PageDockviewHost";`，`renderWatermark` 改为渲染真实组件（可照抄骨架）：
     ```ts
     function renderRealWatermark(pageId: string, cwd: string) {
       const nextId = makeNextPanelId(pageId);
       const addPanelSpy = vi.fn();
       const Watermark = createWatermark(nextId, pageId, cwd);
       render(<Watermark containerApi={{ addPanel: addPanelSpy } as never} />);
       const btn = screen.getByRole("button", { name: "新建终端" });
       return { addPanelSpy, clickBtn: () => fireEvent.click(btn) };
     }
     ```
     W1/W2/W3 三用例断言主体（addPanel 参数：不传 position / component:"terminal" / renderer:"always" / params.panelId===id / params.cwd 透传）保持不变，仅将 `renderWatermark` 换为 `renderRealWatermark`。
  3. 若 createWatermark 内部引用的 `titleManager.getTerminalTitle` 在 jsdom 下依赖 store，沿用文件既有 seed 方式（参考该测试文件其他用例的 store 种子）。
- **测试同步**：用例数不变（3 例改写）。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/workspace-header-actions.test.tsx` 通过；grep `createWatermark` 命中该测试文件 import 行；grep -c `const Watermark: React.FC<any>` 该文件为 0（手写组件已删）。

## TQ-A-05 barrel `../lib` mock 只导出部分成员（3 处）

- **位置**：`src/__tests__/use-xterm-error-toast.test.ts:186-191`；`diff-panel.test.tsx:55-65`；`diff-panel-stale-banner.test.tsx:54-62`
- **现状**：`vi.mock("../lib", () => ({ toast: {...}, getErrorMessage: ... }))` 覆盖整个 barrel，未来新增引用即 `undefined`。
- **修复步骤**（3 处同一模式，范例来自 `editor-confirm.test.ts:107-114`）：
  1. 每处改为 importOriginal 形态（以 diff-panel.test.tsx 为例，可照抄）：
     ```ts
     vi.mock("../lib", async (importOriginal) => {
       const actual = await importOriginal<typeof import("../lib")>();
       return {
         ...actual,
         confirmDialog: mockConfirmDialog,
         toast: { ...actual.toast, show: mockToastShow },
         getErrorMessage: mockGetErrorMessage,
       };
     });
     ```
  2. `use-xterm-error-toast.test.ts` 同理（其 mock 无 confirmDialog，仅 toast/getErrorMessage 两成员覆盖）。
  3. 注意三文件均已单独 `vi.mock("../lib/useFontSizeWheel", ...)`（子路径 mock 不受 barrel 影响）——保留不动。
  4. 若 importOriginal 引入的真实成员在 jsdom 下初始化抛错（如 E2E_ENABLED 读取 import.meta.env），降级方案：显式列出实际被引用的全部成员而非部分（在 mock 工厂顶部注释「新增 ../lib 引用须同步本清单」）。
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/use-xterm-error-toast.test.ts src/__tests__/diff-panel.test.tsx src/__tests__/diff-panel-stale-banner.test.tsx` 全绿；grep `importOriginal` 命中 3 文件。

## TQ-A-06 阈值魔数双维护（实现私有常量 vs 测试硬编码）

- **位置**：生产 `src/panels/terminal/usePtyOutput.ts:20-29`；测试 `src/__tests__/use-xterm-output.test.ts:207-216,:355-359,:486-494,:1077-1080,:1166-1180`
- **现状**：`DIRECT_WRITE_THRESHOLD=256` / `IDLE_FLUSH_MS=2` / `MAX_FLUSH_MS=16` / `MAX_PENDING_BYTES=65536` / `E2E_BUFFER_MAX_LINES=1000` 均未导出；测试 5 处硬编码同值。
- **修复步骤**：
  1. 生产微改：5 个常量各加 `export`（可照抄）：
     ```ts
     /** 合帧分流阈值（字节）：小于等于此值直写终端，大于此值走合帧 + DEC 2026 路径（FE-18：64→256，后端已合并小写） */
     export const DIRECT_WRITE_THRESHOLD = 256;
     /** Idle 定时器间隔（毫秒）：2ms 无新数据则 flush，适应 Ink 高频 burst 输出 */
     export const IDLE_FLUSH_MS = 2;
     /** Max 定时器间隔（毫秒）：最多 16ms 强制 flush 一次，防止饥饿 */
     export const MAX_FLUSH_MS = 16;
     /** 非焦点终端待输出数据量上限（字节），防止内存无限增长 */
     export const MAX_PENDING_BYTES = 65536; // 64KB
     /** E2E 测试文本缓冲行数上限 */
     export const E2E_BUFFER_MAX_LINES = 1000;
     ```
  2. 测试改造：`use-xterm-output.test.ts` 顶部 import 追加 `import { DIRECT_WRITE_THRESHOLD, IDLE_FLUSH_MS, MAX_FLUSH_MS, MAX_PENDING_BYTES, E2E_BUFFER_MAX_LINES } from "../panels/terminal/usePtyOutput";`；5 处魔数替换：
     - `:207` 用例名与注释保留语义，断言内 `256` → `DIRECT_WRITE_THRESHOLD`；`:355-359` 的 255/256 → `DIRECT_WRITE_THRESHOLD - 1` / `DIRECT_WRITE_THRESHOLD`
     - `:486-494` 的 `vi.advanceTimersByTime(5)` 注释说明「5 > IDLE_FLUSH_MS(2) 且 < MAX_FLUSH_MS(16)」→ 改为 `vi.advanceTimersByTime(IDLE_FLUSH_MS + 3)`
     - `:1077-1080` 局部 `const MAX_PENDING_BYTES = 65536;` 删除，改用 import
     - `:1166-1180` 的 1000/1005 → `E2E_BUFFER_MAX_LINES` / `E2E_BUFFER_MAX_LINES + 5`
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/use-xterm-output.test.ts` 通过；grep -c `65536` 该文件为 0；grep `DIRECT_WRITE_THRESHOLD` 命中该文件 import 行。

## TQ-A-07 workspace-sideviews 注释 40px 与实际 46px 不符

- **位置**：`src/__tests__/workspace-sideviews.test.tsx:1-4` 顶部注释
- **现状**：注释写「活动栏 40px 固定」；实际 `ACTIVITY_BAR_SIZE = 46`（sideBarState.ts:45，NAV-05 已从 40 改 46）。
- **修复步骤**：注释中「40px」改为「46px（以 ACTIVITY_BAR_SIZE 为准）」。
- **测试同步**：无。
- **文档同步**：无。
- **验证**：grep `46px` 命中该文件头部注释；grep -c `40px` 该文件为 0。

## TQ-A-08 弱断言（use-code-mirror / editor-confirm）

- **位置**：`src/__tests__/use-code-mirror.test.ts:156-194`；`src/__tests__/editor-confirm.test.ts:258-316`
- **现状**：use-code-mirror 前两例仅 `expect(capturedStateExtensions).toBeDefined()`；第三例仅断言 `mockDispatch`/`mockReconfigure` 被调用；editor-confirm 脏态保存分支未断言磁盘写入结果。
- **修复步骤**：
  1. `use-code-mirror.test.ts`：
     - 用例 1（fontSize=18）：追加 `expect(capturedStateExtensions.length).toBeGreaterThan(0);` 并断言扩展数组内存在 fontSize 相关配置（查被测 `useCodeMirror` 实现中 fontSize 注入点——若为 `fontSizeTheme`/`textStyle`，断言 `JSON.stringify(capturedStateExtensions)` 含 `"18"` 不可行时，改为断言 `mockReconfigure` 在 rerender 后以 20 调用：已存在第三例，第一二例至少补 `toBeGreaterThan(0)` + EditorView 文档内容断言 `expect(view.state.doc.toString())` 非空（view 经既有 helper 获取）。
     - 用例 3：断言 `mockReconfigure` 调用参数含 fontSize=20 对应扩展（经 `mockReconfigure.mock.calls[0][0]` 检查非空）。
     - 原则：每个用例至少一条「用户可见行为」断言（文档内容/重配置参数），不只工厂存在。
  2. `editor-confirm.test.ts` 脏态保存分支（:258-316 区域）：在「确认重载」路径补磁盘断言——`expect(h.mockReadFile).toHaveBeenCalledTimes(1)`（重载触发重新读取）；「取消」路径已有 `mockReadFile not.toHaveBeenCalled()` 保留；补一条 `writeFile` 方向断言（若保存路径涉及 `fsWriteFile` mock 则断言其调用参数含编辑后内容）。
- **测试同步**：用例数不变（断言增强）。
- **文档同步**：无。
- **验证**：两文件 `npx vitest run` 通过；grep -c `toBeDefined()` use-code-mirror.test.ts ≤ 1（仅保留真正只验存在的场景）。

---

# TQ-B（L2 features 域，19 项）

## TQ-B-01 explorer-virtualization jsdom 视口模拟与计数不可靠

- **位置**：测试 `src/__tests__/explorer-virtualization.test.tsx:66-115`；生产 `src/features/explorer/FileTree.tsx:194`（TreeNodeRow 根 div）
- **现状**：`renderedRowCount()` 用 `screen.getAllByText(/^f\d+\.ts$/)` 计数（StrictMode 双渲染虚高）；`1000 节点 <100 行` 断言在虚拟化失效时也可能通过；TreeNodeRow 根 div 无 data-testid。
- **修复步骤**：
  1. 生产微改：`FileTree.tsx:194` TreeNodeRow 根 div 加属性 `data-testid="tree-node-row"`（仅加属性，结构不动）。
  2. 测试改造：`renderedRowCount()` 改为（可照抄）：
     ```ts
     function renderedRowCount(): number {
       // 按行容器 testid 计数——getAllByText 在 StrictMode 双渲染下计数虚高（TQ-B-01）
       return document.querySelectorAll('[data-testid="tree-node-row"]').length;
     }
     ```
  3. 「容器高度未测得 → 全量渲染兜底」用例（:111-115）：断言改为「行数 === 输入节点数 且 无重复 key」：
     ```ts
     const rows = document.querySelectorAll('[data-testid="tree-node-row"]');
     expect(rows.length).toBe(120);
     ```
     （120 例若 StrictMode 双渲染出 240，则同步给 `renderFileTree` 加显式非 StrictMode 渲染路径——查该文件 render helper 是否经 `<React.StrictMode>` 包裹，是则在虚拟化测试文件内改直渲染。）
  4. 1000 节点用例保留 `toBeLessThan(100)` 断言（此时按行容器计数，虚拟化失效 → 1000 行 → 必然红）。
- **测试同步**：用例数不变；依赖同一 testid 的 TQ-B-05 随之受益。
- **文档同步**：`src/features/explorer/CLAUDE.md` 文件表/测试模式处补一句「TreeNodeRow 根 div 带 `data-testid="tree-node-row"`（虚拟化/行断言锚点，TQ-B-01）」。
- **验证**：`npx vitest run src/__tests__/explorer-virtualization.test.tsx` 通过；grep `data-testid="tree-node-row"` 命中 FileTree.tsx。

## TQ-B-02 sideViewRegistry 单例隔离——阻断 sideViewDefs 混入

- **位置**：`src/__tests__/sideBar.test.ts:20-99`；`sideBarArea.test.tsx:52-153`；`activityBar.test.tsx:30-174`
- **现状**：三文件 `_reset()` 后注册 stub；当前均未 import `sideViewDefs`，但实现链任何一环未来 import 它（side-effect 注册 nav/explorer/commit 真实视图，sideViewDefs.ts:24-45）即污染。
- **修复步骤**（3 文件同模式）：
  1. 在 import 块后、`vi.mock` 区追加（可照抄）：
     ```ts
     // 阻断 side-effect 注册：真实 nav/explorer/commit 视图不得混入本文件的
     // sideViewRegistry 隔离（_reset 后仅注册 stub；TQ-B-02）
     vi.mock("../features/sideViews/sideViewDefs", () => ({}));
     ```
  2. beforeEach 内 `_reset()` 之后追加防御断言：`expect(sideViewRegistry.getAll().length).toBe(0);`（再注册 stub）。
- **测试同步**：用例数不变（beforeEach 内断言非新用例）。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/sideBar.test.ts src/__tests__/sideBarArea.test.tsx src/__tests__/activityBar.test.tsx` 全绿；grep `sideViewDefs", () => ({})` 命中 3 文件。

## TQ-B-03 explorer-sandbox-race 名为竞态实为顺序执行

- **位置**：`src/__tests__/explorer-sandbox-race.test.tsx:257-315`
- **现状**：两用例均为手动编排的顺序调用；DBG-10 故障（effect 子先于父 → readDir 在 setProjectRoot 前到达后端被拒）无法回归。mocks 已有 deferred 基建（`resetDeferred`/`mockSetProjectRoot`/`resolveSetProjectRoot`/`sprCallOrder`/`rdCallOrder`）。
- **修复步骤**：
  1. 新增用例（可照抄骨架，基于既有 deferred 基建）：
     ```ts
     it("DBG-10 真竞态：setProjectRoot pending 期间 mount ExplorerPanel → readDir 不得先于 setProjectRoot resolve 发出", async () => {
       mocks.resetDeferred();
       makeVfs(mocks.mockReadDir, {
         "/proj": [mockEntry("main.ts", false, "/proj/main.ts")],
       });
       // 不 await——setProjectRoot 保持 pending，模拟 effect 子先于父的窗口
       mocks.mockSetProjectRoot("/proj");
       render(<ExplorerPanel />);
       // pending 窗口内：readDir 不应被调用（沙箱拒绝或上层等待，二者取其一即防线）
       expect(mocks.mockReadDir).not.toHaveBeenCalled();
       mocks.resolveSetProjectRoot();
       await waitFor(() => {
         expect(mocks.mockReadDir).toHaveBeenCalledWith("/proj");
       }, { timeout: 3000 });
       expect(mocks.sprCallOrder).toBeLessThan(mocks.rdCallOrder);
     });
     ```
     若现状行为是「readDir 发出但被后端 mock 拒绝重试」，则断言改为「readDir 首次成功调用晚于 resolve」——执行时先跑通观察实际行为再定断言，禁止为绿而放宽到时序无约束。
  2. ExplorerPanel 渲染所需种子沿用本文件/邻近 explorer 测试的既有 seed（useProjects/useLayout setState 模式）。
- **测试同步**：+1 用例（inventory L2 +1，Stage 10 统一校准）。
- **文档同步**：无。
- **验证**：新用例通过；`npx vitest run src/__tests__/explorer-sandbox-race.test.tsx` 全绿。

## TQ-B-04 explorer-race-cleanup G3 初始加载时机假设过强

- **位置**：`src/__tests__/explorer-race-cleanup.test.tsx:131-161`
- **现状**：`await vi.advanceTimersByTimeAsync(0)` 后立即断言 `rootNodes.length === 1`，初始 readDir+gitStatus 微任务链未必完成。
- **修复步骤**：
  1. 将 `advanceTimersByTimeAsync(0)` + 同步断言改为 waitFor（可照抄）：
     ```ts
     const { result, unmount } = renderHook(() => useFileTree({ rootPath: "/proj" }));
     // 等初始加载真实完成（微任务链时长不定，0ms 推进假设过强——TQ-B-04）
     await waitFor(() => expect(result.current.rootNodes.length).toBe(1), { timeout: 3000 });
     mocks.mockReadDir.mockClear();
     ```
     注意 fake timers 下 `waitFor` 需确认本文件是否已 `vi.useFakeTimers()`（:132 有）——fake timers 下 waitFor 默认 setInterval 也被 mock，若 waitFor 不前进，改用 `await vi.waitFor(...)`（vitest 自带，自动推进 fake timers）。
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/explorer-race-cleanup.test.tsx` 连续 3 轮通过。

## TQ-B-05 rowBackground 的 closest("div") 可能取错行容器

- **位置**：`src/__tests__/explorer-crud-success.test.tsx:103-111`
- **现状**：找到文本 span 后 `closest("div")`——虚拟化包裹结构下最近 div 可能不是 TreeNodeRow。
- **修复步骤**（依赖 TQ-B-01 的 testid，同 Stage 同 agent 执行）：
  1. `rowBackground` 改为（可照抄）：
     ```ts
     function rowBackground(fileName: string): string {
       // 按行容器 testid 限定（虚拟化 DOM 下 closest("div") 可能取到包裹层——TQ-B-05）
       const rows = document.querySelectorAll<HTMLElement>('[data-testid="tree-node-row"]');
       const row = Array.from(rows).find((r) => r.textContent?.includes(fileName));
       if (!row) throw new Error(`找不到行: ${fileName}`);
       return row.style.background;
     }
     ```
  2. C1 用例（:132-165）的两次 `rowBackground` 调用不变。
- **测试同步**：用例数不变。
- **文档同步**：无（随 TQ-B-01 的 explorer CLAUDE.md 一句覆盖）。
- **验证**：`npx vitest run src/__tests__/explorer-crud-success.test.tsx` 通过。

## TQ-B-06 commit-view-status 旧请求丢弃测试未等状态稳定

- **位置**：`src/__tests__/commit-view-status.test.ts:233-294`
- **现状**：`resolveOld(...)` 后仅 `advanceTimersByTimeAsync(10)` 即断言 `textContent` 不含 old.ts。
- **修复步骤**：
  1. resolveOld 后的断言改为 vi.waitFor 轮询 + 稳定期复查（可照抄）：
     ```ts
     resolveOld([makeEntry("C:/repo1/old.ts", "modified")]);
     // 轮询确认渲染稳定后旧数据仍不落地（单次 10ms 推进抓不住闪屏——TQ-B-06）
     await vi.waitFor(() => {
       const view = document.querySelector('[data-e2e="commit-view"]');
       expect(view?.textContent).toContain("x.ts");
     }, { timeout: 3000 });
     const commitView = document.querySelector('[data-e2e="commit-view"]');
     expect(commitView?.textContent).not.toContain("old.ts");
     ```
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/commit-view-status.test.ts` 连续 3 轮通过。

## TQ-B-07 activityBar 全局 getBoundingClientRect mock 过宽

- **位置**：`src/__tests__/activityBar.test.tsx:133-157`（installRectSpy）；拖拽用例 :269-429
- **现状**：原型级 mock 使所有 HTMLElement 测量走同一实现（已按 data-e2e 分支，但仍为全局）；测试退化为 computeDropTarget 间接验证。
- **修复步骤**：
  1. 组件级拖拽用例收窄断言：只验「事件委托触发 + dataTransfer 设置 + computeDropTarget 入参」，删除对全局几何的依赖——`installRectSpy` 保留但限定只 mock `data-e2e="activity-bar"` 容器与其直接子钮（其他元素调用原实现）：在 mockImplementation 的 fallback 分支调用 `originalGetBoundingClientRect.call(this)`（先 `const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;` 保存）。
  2. 落点算法：检查 `src/features/sideViews/dropTarget.ts`（或 computeDropTarget 所在文件）是否已有纯函数测试（grep `dropTarget` src/__tests__/）；无则新建 `src/__tests__/drop-target.test.ts`，覆盖：上半/下半插入位、跨 zone 移动、边界 index（用例 3-5 条）。
- **测试同步**：可能 +3~5 用例（drop-target 纯函数）。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/activityBar.test.tsx` 通过；新 drop-target 测试通过；grep `originalGetBoundingClientRect` 命中 activityBar.test.tsx。

## TQ-B-08 explorer-delete E6 键盘 Del 绕过真实焦点链路

- **位置**：`src/__tests__/explorer-delete.test.tsx:682-779`
- **现状**：直接 `window.dispatchEvent(new KeyboardEvent("keydown"))` + 手动 `pushContext("explorer")`，不验证 `focusin → pushContext + setActiveExplorer` 链路。
- **修复步骤**：
  1. 新增集成用例（可照抄骨架）：
     ```ts
     it("E6-集成：点击文件行聚焦 ExplorerPanel 后按 Delete → deleteSelected 经真实焦点链路触发", async () => {
       // 真实链路：容器 focusin → usePanelFocus pushContext("explorer") + setActiveExplorer
       render(<ExplorerPanel />);
       await waitFor(() => expect(getAllByText("locked.ts").length).toBeGreaterThan(0), { timeout: 3000 });
       fireEvent.click(getAllByText("locked.ts")[0]); // 选中（onSelect）
       const panel = container.querySelector('[data-e2e="explorer-panel"]') ?? container.firstChild;
       fireEvent.focusIn(panel as Element); // 触发真实 focusin 链路
       fireEvent.keyDown(window, { key: "Delete", code: "Delete" });
       await waitFor(() => expect(mocks.mockConfirmDialog).toHaveBeenCalled(), { timeout: 3000 });
     });
     ```
     执行时先 Read ExplorerPanel 容器 tabIndex/focusin 接线（ExplorerPanel.tsx:106-125 handleDeleteSelected 与容器 props）确认选择器；断言按生产实际（confirmDialog 或 deleteEntry 直接调用）校准，禁止 mock 掉 focusin 链路本身。
- **测试同步**：+1 用例。
- **文档同步**：无。
- **验证**：新用例通过；`npx vitest run src/__tests__/explorer-delete.test.tsx` 全绿。

## TQ-B-09 nav-tree waitFor 后紧跟同步 style 断言

- **位置**：`src/__tests__/nav-tree.test.tsx:410-452`、:655-714`、:876-933`（另 nav-tree-history.test.tsx:195-250 同模式）
- **现状**：`waitFor` 后同步 `style.color/backgroundColor/textContent` 断言，React 重渲染可能未稳定。
- **修复步骤**：
  1. 三处（及 nav-tree-history 同模式段）将「fireEvent 后紧跟的同步样式断言」包入 waitFor，例如（可照抄模式）：
     ```ts
     fireEvent.click(pageRow);
     await waitFor(() => {
       expect(normColor(pageRow.style.backgroundColor)).toBe(normColor(ACTIVE_SELECTION_BG));
     }, { timeout: 3000 });
     ```
  2. mouseEnter 类断言同样处理。
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/nav-tree.test.tsx src/__tests__/nav-tree-history.test.tsx` 连续 3 轮通过。

## TQ-B-10 explorer/commit/nav-tree 未统一重置全部 Zustand stores

- **位置**：`src/__tests__/commit-view.test.tsx:68-104`；`nav-tree.test.tsx:228-262`；`explorer-crud-success.test.tsx:113-125`；共享 helper `src/__tests__/helpers/workspace-setup.ts:21-41`
- **现状**：各文件 resetStore 只重置 projects/layout/titleManager，漏 useSideBar/useKeybindings 等共享单例。
- **修复步骤**：
  1. 扩 `helpers/workspace-setup.ts`：在 `resetProjectStores()` 追加 useSideBar 与 useKeybindings 重置（可照抄）：
     ```ts
     import { useSideBar } from "../../features/sideViews/sideBarState"; // 按实际导出路径校准
     import { DEFAULT_ZONES, DEFAULT_OPEN, WIDTH_DEFAULT, SPLIT_DEFAULT } from "../../features/sideViews/sideBarState";
     import { useKeybindings } from "../../stores/keybindings";
     // resetProjectStores 内追加：
     useSideBar.setState({
       zones: { top: [...DEFAULT_ZONES.top], bottom: [...DEFAULT_ZONES.bottom] },
       open: { ...DEFAULT_OPEN },
       width: WIDTH_DEFAULT,
       splitRatio: SPLIT_DEFAULT,
       loaded: true,
     });
     useKeybindings.setState({ overrides: {}, loaded: true });
     ```
     （import 路径以实际文件为准；useSideBar 定义在 sideBarState.ts 还是 stores/sideBar.ts 先 Read 确认。）
  2. 三个测试文件的 beforeEach 改为调用 `resetProjectStores()`（删除各自私有 resetStore 或改为薄包装）。
- **测试同步**：用例数不变。
- **文档同步**：`src/__tests__/CLAUDE.md` 共享工厂说明处补一句「resetProjectStores 覆盖 projects/layout/titleManager/sideBar/keybindings 全量（TQ-B-10）」。
- **验证**：`npx vitest run src/__tests__/commit-view.test.tsx src/__tests__/nav-tree.test.tsx src/__tests__/explorer-crud-success.test.tsx` 全绿。

## TQ-B-11 file-viewer-registry 私有恢复函数与生产初始化解耦

- **位置**：生产 `src/features/fileViewers/FileViewerRegistry.ts:85-97`；测试 `src/__tests__/file-viewer-registry.test.ts:208-234`
- **现状**：测试私有 `restoreDefaultRegistry` 复制生产注册（html/htm→htmlviewer）；生产预注册变更测试不红。
- **修复步骤**：
  1. 生产微改：`FileViewerRegistry.ts:85-97` 抽导出函数（可照抄）：
     ```ts
     // ---- 初始化全局单例 ----

     /** 默认扩展名→面板映射（生产模块级调用 + 测试 _reset 后恢复共用，TQ-B-11） */
     export function registerDefaultViewers(strategy: ExtensionBasedViewerStrategy): void {
       strategy.register("html", "htmlviewer");
       strategy.register("htm", "htmlviewer");
       // 后续扩展示例:
       // strategy.register("md", "markdownviewer");
     }

     const extensionStrategy = new ExtensionBasedViewerStrategy();
     registerDefaultViewers(extensionStrategy);

     /** 全局文件查看器注册表单例 */
     export const fileViewerRegistry = new FileViewerRegistry();
     fileViewerRegistry.addStrategy(extensionStrategy);
     ```
  2. 测试改造：删除私有 `restoreDefaultRegistry` 复制体，改为：
     ```ts
     import { fileViewerRegistry, registerDefaultViewers, ExtensionBasedViewerStrategy } from "../features/fileViewers/FileViewerRegistry";
     function restoreDefaultRegistry() {
       const es = new ExtensionBasedViewerStrategy();
       registerDefaultViewers(es); // 与生产初始化同一真值源
       fileViewerRegistry.addStrategy(es);
     }
     ```
     （ExtensionBasedViewerStrategy 若未导出则一并 export。）
- **测试同步**：用例数不变。
- **文档同步**：`src/features/fileViewers/CLAUDE.md` 补一句「默认注册经 registerDefaultViewers 导出（测试共用真值源，TQ-B-11）」。
- **验证**：`npx vitest run src/__tests__/file-viewer-registry.test.ts` 通过；grep `registerDefaultViewers` 命中生产与测试两文件。

## TQ-B-12 键盘事件构造重复不统一

- **位置**：`src/__tests__/global-commands.test.ts:74-131`；`src/__tests__/shortcuts.test.ts:21-42`
- **现状**：两处各自 `new KeyboardEvent`，默认字段处理不一致。
- **修复步骤**：
  1. 新建 `src/__tests__/helpers/keyboard.ts`（可照抄，以 shortcuts.test.ts:22-42 为蓝本泛化）：
     ```ts
     /** 共享键盘事件 helper（TQ-B-12）：统一 keydown 构造默认值与派发 */

     export interface KeydownOptions {
       ctrlKey?: boolean;
       shiftKey?: boolean;
       altKey?: boolean;
       metaKey?: boolean;
       code?: string;
       key?: string;
       isComposing?: boolean;
     }

     /** 构造 keydown 事件（不派发）——调用方需自行 dispatch 时用 */
     export function makeKeydown(opts: KeydownOptions): KeyboardEvent {
       return new KeyboardEvent("keydown", {
         ctrlKey: opts.ctrlKey ?? false,
         shiftKey: opts.shiftKey ?? false,
         altKey: opts.altKey ?? false,
         metaKey: opts.metaKey ?? false,
         code: opts.code ?? "",
         key: opts.key ?? opts.code ?? "",
         isComposing: opts.isComposing ?? false,
         bubbles: true,
         cancelable: true,
       });
     }

     /** 构造并派发到 window，返回事件 */
     export function dispatchKeydown(opts: KeydownOptions): KeyboardEvent {
       const event = makeKeydown(opts);
       window.dispatchEvent(event);
       return event;
     }
     ```
  2. `shortcuts.test.ts` 删除本地 `dispatchKeydown`，import 自 `../helpers/keyboard`；`global-commands.test.ts:80-86` 的 `new KeyboardEvent(...)` 改为 `makeKeydown({ ctrlKey: true, code: "KeyW" })`（该处是直接调 handler 不派发，故用 makeKeydown）。
  3. 其余文件内类似 `new KeyboardEvent("keydown", ...)` 构造（explorer-delete.test.tsx:682 makeKeyboardEvent）一并替换为 makeKeydown。
- **测试同步**：用例数不变；`src/__tests__/CLAUDE.md` helpers 清单补 keyboard.ts。
- **文档同步**：同上。
- **验证**：`npx vitest run src/__tests__/global-commands.test.ts src/__tests__/shortcuts.test.ts src/__tests__/explorer-delete.test.tsx` 全绿。

## TQ-B-13 commit 右键菜单依赖 position:fixed 全局选择器

- **位置**：生产 `src/features/commit/CommitFileList.tsx:112-127`（ContextMenu 根 div）；测试 `src/__tests__/commit-context-menu-ui.test.tsx:130-169`
- **现状**：`getMenuEl` 用 `div[style*="position: fixed"]` 取最后一个元素；根 div 无 testid。
- **修复步骤**：
  1. 生产微改：ContextMenu 根 div 加 `data-testid="commit-context-menu"`。
  2. 测试改造：`getMenuEl` 改为 `document.querySelector('[data-testid="commit-context-menu"]') as HTMLDivElement`（找不到时抛错）；「无菜单项不弹」用例（:156-169）改为 `expect(document.querySelector('[data-testid="commit-context-menu"]')).toBeNull()`。
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/commit-context-menu-ui.test.tsx` 通过；grep `data-testid="commit-context-menu"` 命中 CommitFileList.tsx。

## TQ-B-14 nav-history-row import profiles 触发注册未清理

- **位置**：`src/__tests__/nav-history-row.test.tsx:28`
- **现状**：`import "../features/cliProfiles/profiles"` 触发 CliProfileRegistry 注册 claude profile，无 afterEach `_reset`。
- **修复步骤**：
  1. afterEach 追加（可照抄）：
     ```ts
     afterEach(() => {
       cleanup();
       CliProfileRegistry._reset(); // import profiles 的注册残留不得外泄（TQ-B-14）
     });
     ```
     （CliProfileRegistry 按 cliProfiles 模块实际导出形态 import——getCliProfileRegistry() 或模块级单例，先 Read `src/features/cliProfiles/index.ts` 确认；注册表家族契约 #13 必有 `_reset`。）
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/nav-history-row.test.tsx` 通过。

## TQ-B-15 window.__dockviewApi 未 afterEach 清理

- **位置**：`src/__tests__/commit-open-file.test.ts:55-69`；`src/__tests__/explorer-crud-success.test.tsx:113-125`（beforeEach 内设 `(window as any).__dockviewApi`）
- **现状**：两文件 beforeEach 设置 `__dockviewApi`，均无 afterEach 删除。
- **修复步骤**：
  1. 两文件各追加：
     ```ts
     afterEach(() => {
       cleanup();
       delete (window as unknown as Record<string, unknown>).__dockviewApi; // 防过期 mock 外泄（TQ-B-15）
     });
     ```
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：两文件 `npx vitest run` 通过；grep `delete.*__dockviewApi` 命中 2 文件。

## TQ-B-16 wire-keybindings 只测 fake store

- **位置**：`src/__tests__/wire-keybindings.test.ts:1-66`；生产 `src/stores/keybindings.ts:43-90`
- **现状**：三用例均用 fake store 工厂，不验证真实 `useKeybindings` 的 subscribe/getState 签名契合。
- **修复步骤**：
  1. 新增集成用例（可照抄骨架）：
     ```ts
     import { useKeybindings } from "../stores/keybindings";

     it("与真实 useKeybindings 集成：setBinding 触发 wireKeybindings 重应用", () => {
       useKeybindings.setState({ overrides: {}, loaded: true });
       const applied: Array<Record<string, string | null>> = [];
       const unwire = wireKeybindings(useKeybindings, (overrides) => {
         applied.push({ ...overrides });
       });
       useKeybindings.getState().setBinding("terminal.copy", "Ctrl+Shift+C");
       expect(applied.length).toBeGreaterThanOrEqual(2); // 初始一次 + 变更一次
       expect(applied.at(-1)).toMatchObject({ "terminal.copy": "Ctrl+Shift+C" });
       unwire();
     });
     ```
     （wireKeybindings 的实际签名先 Read `src/features/shortcuts/` 对应实现校准——回调形态以生产为准；用例末尾恢复 `useKeybindings.setState({ overrides: {}, loaded: false })`。）
  2. 注意 keybindings.ts:80-90 有持久化订阅（2s debounce saveSettings）——测试用 fake timers 或 afterEach `vi.clearAllTimers()` 防真写入（saveSettings 在 setup.ts 全局 mock 范围内则无忧）。
- **测试同步**：+1 用例。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/wire-keybindings.test.ts` 通过。

## TQ-B-17 explorer 内联输入框用 querySelectorAll('input') 取最后一个

- **位置**：生产 `src/features/explorer/FileTree.tsx:552`（根级新建）、:683`（行内新建/重命名）；测试 `explorer-delete.test.tsx:478-490`、`explorer-crud-success.test.tsx:183-193,:233-246,:278-291`
- **现状**：输入框无 testid；测试取 `inputs[inputs.length - 1]`。
- **修复步骤**：
  1. 生产微改：两处 `<input` 加 `data-testid="explorer-inline-input"`。
  2. 测试改造：三处 `inputs[inputs.length - 1]` 改为 `document.querySelector('[data-testid="explorer-inline-input"]') as HTMLInputElement`（找不到抛错）；`querySelectorAll("input").length` 断言改为 `querySelectorAll('[data-testid="explorer-inline-input"]').length`。
- **测试同步**：用例数不变。
- **文档同步**：随 TQ-B-01 的 explorer CLAUDE.md 一句追加「内联输入框带 `data-testid="explorer-inline-input"`」。
- **验证**：三文件 `npx vitest run` 通过；grep `data-testid="explorer-inline-input"` 命中 FileTree.tsx。

## TQ-B-18 explorer-refresh-preserve R17 未控制 gitStatus 时序

- **位置**：`src/__tests__/explorer-refresh-preserve.test.tsx:455-478`
- **现状**：连续两次 `refresh()` 并发触发 gitStatus，只断言 rootNodes 终态，未控制 gitStatus 返回顺序。
- **修复步骤**：
  1. 用例内将 gitStatus mock 改为可控 resolved（可照抄模式）：
     ```ts
     // 两次 refresh 的 gitStatus 时序可控——防异步抖动 flaky（TQ-B-18）
     mocks.mockGitStatus.mockResolvedValue(new Map());
     await act(async () => {
       const first = result.current.refresh();
       vfs.set("/proj/src", [mockEntry("a.ts", false, "/proj/src/a.ts"), mockEntry("c.ts", false, "/proj/src/c.ts")]);
       const second = result.current.refresh();
       await Promise.all([first, second]);
     });
     await waitFor(() => {
       const src = findNode(result.current.rootNodes, "/proj/src");
       expect(src?.children.map((c) => c.entry.name)).toEqual(["a.ts", "c.ts"]);
     }, { timeout: 3000 });
     ```
     （mockGitStatus 的实际 mock 名/返回类型以本文件 mocks 工厂为准；终态断言从同步改为 waitFor。）
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/explorer-refresh-preserve.test.tsx` 连续 3 轮通过。

## TQ-B-19 keyboard.test.ts readTextMock 用 mockClear 未 mockReset

- **位置**：`src/__tests__/keyboard.test.ts:116-147` 及外层 beforeEach
- **现状**：`readTextMock` 在 beforeEach 仅 `mockClear()`（清调用记录不清 once 队列）；若前序用例 `mockResolvedValueOnce` 留队列会串扰。
- **修复步骤**：
  1. beforeEach 中 `readTextMock.mockClear()` 改为 `readTextMock.mockReset()`；同文件的 `writeTextMock` 同步改 mockReset。
  2. mockReset 后默认值丢失——在 beforeEach 显式补默认：`readTextMock.mockResolvedValue(""); writeTextMock.mockResolvedValue(undefined);`（各用例内自行覆盖）。
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/keyboard.test.ts` 通过。

---

# TQ-C（L2 数据层/基建，4 项）

## TQ-C-01 editorSyntaxHighlight 未覆盖 syntax 映射与层叠顺序

- **位置**：`src/__tests__/overrides.test.ts:187-198`；生产 `src/theme/overrides.ts`（`editorSyntaxHighlight`）；口径 `src/theme/CLAUDE.md:31,48`（ACC-05）
- **现状**：仅断言「函数存在」+「EditorState.create 不抛」；9 组 syntax tag→color 映射与 ACC-05 层叠顺序（syntax 须后置胜出）未断言。
- **修复步骤**：
  1. 新增用例（可照抄骨架；`themeRules` 的获取方式先 Read overrides.ts/CM6 导出面校准——若 HighlightStyle 无公开 rules 访问器，则改为逐 tag 渲染断言或经 `@lezer/highlight` tags 对照表验证）：
     ```ts
     it("syntax 9 组 tag→color 映射全部来自 active 方案 overrides.syntax", () => {
       const scheme = schemeRegistry.getActive();
       const syntax = scheme.editor.overrides.syntax;
       // 9 组键：propertyName/string/number/keyword/function/variableName/typeName/operator/punctuation/comment
       const ext = editorSyntaxHighlight();
       const rules = themeRules(ext); // CM6 HighlightStyle 规则提取（按生产导出校准）
       for (const [tag, color] of Object.entries(syntax)) {
         expect(rules).toContainEqual(expect.objectContaining({ value: color }));
       }
       expect(Object.keys(syntax)).toHaveLength(9);
     });

     it("ACC-05：syntax 规则挂载在 editorTheme 之后（数组顺序决胜）", () => {
       // 生产组合点：extensions = [editorTheme(), editorSyntaxHighlight()] 的顺序断言
       // 在组合入口处（useCodeMirror 或 editor 面板扩展装配点）grep 出顺序并固化
       const order = readExtensionOrder(); // 按生产装配代码提取顺序的既有 helper 或新增
       expect(order.indexOf("editorTheme")).toBeLessThan(order.indexOf("editorSyntaxHighlight"));
     });
     ```
     执行时先 Read `src/theme/overrides.ts` 全文确定可断言面（HighlightStyle.define 的 module/spec 可通过 `EditorState.create({extensions}).facet(...)` 或导出常量读取），再定最终断言 API；禁止用 `expect(ext).toBeDefined()` 蒙混。
- **测试同步**：+2 用例。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/overrides.test.ts` 通过；新用例名含 `syntax 9 组` 与 `ACC-05`。

## TQ-C-02 ipc-agent-history-contract 未覆盖 SEC-05 非法 sessionId

- **位置**：`src/__tests__/ipc-agent-history-contract.test.ts:105-190`；后端校验 `src-tauri/src/agent_history/provider.rs:29-35`
- **现状**：只覆盖正常 UUID + 通用异常传播；空串/路径穿越/非 UUID 未覆盖。helpers/ipc-contract.ts 已有 `mockThrow`/`expectReject` 能力。
- **修复步骤**：
  1. `deleteHistorySession` describe 内追加 3 个 case（可照抄）：
     ```ts
     { name: "SEC-05：空 sessionId 后端拒绝 → 异常透传", cmd: "agent_history_delete",
       call: () => agentHistory.deleteHistorySession(CLI_ID, ""), mockThrow: "Validation: 非法 sessionId",
       expectReject: "Validation: 非法 sessionId" },
     { name: "SEC-05：路径穿越 sessionId 后端拒绝 → 异常透传", cmd: "agent_history_delete",
       call: () => agentHistory.deleteHistorySession(CLI_ID, "../etc/passwd"), mockThrow: "Validation: 非法 sessionId",
       expectReject: "Validation: 非法 sessionId" },
     { name: "SEC-05：非 UUID sessionId 后端拒绝 → 异常透传", cmd: "agent_history_delete",
       call: () => agentHistory.deleteHistorySession(CLI_ID, "not-a-uuid"), mockThrow: "Validation: 非法 sessionId",
       expectReject: "Validation: 非法 sessionId" },
     ```
  2. `readHistoryTitle` describe 内同模式追加 1 个 case（非 UUID）。
- **测试同步**：+4 用例。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/ipc-agent-history-contract.test.ts` 通过；grep `SEC-05` 命中该文件 ≥4 处。

## TQ-C-03 theme 文档与测试标量计数口径漂移

- **位置**：`src/theme/CLAUDE.md:77`（写「6 组 + 26 标量」）；`src/__tests__/scheme-registry.test.ts:142-167`（断言 27）；真值 `src/theme/schemes/linear.ts:70-96`（实际 27）
- **修复步骤**：
  1. `src/theme/CLAUDE.md:77` 的「ui 6 组 + 26 标量」改为「ui 6 组 + 27 标量」。
  2. `scheme-registry.test.ts:153-154` 注释补全新增 4 标量名（可照抄）：
     ```ts
     // 27 标量（23 既有 + accentFg/selectionHoverBg/titlebarBg/titlebarCloseHover 4 新增，
     // 以 linear.ts ui 段实际键数为准——文档口径冲突时改文档不改断言，TQ-C-03）
     ```
- **测试同步**：用例数不变。
- **文档同步**：`src/theme/CLAUDE.md` 如上。
- **验证**：grep `27 标量` 命中 `src/theme/CLAUDE.md`；`npx vitest run src/__tests__/scheme-registry.test.ts` 通过。

## TQ-C-04 no-claude-literals 扫描硬编码白名单，新增目录逃脱

- **位置**：`src/__tests__/no-claude-literals.test.ts:42-55`
- **现状**：`SCAN_DIRS` 硬编码 8 条路径；豁免目录 `src/features/cliProfiles/profiles/claude`。
- **修复步骤**：
  1. 扫描范围改为递归全 `src`（可照抄骨架）：
     ```ts
     /** AC-5 扫描范围：全 src 递归，仅排除豁免目录（TQ-C-04——白名单制会让新增通用目录逃脱） */
     const SCAN_ROOT = "src";
     /** 目录级豁免：claude 合法领地（MC-213/223 身份域），整目录不参与扫描 */
     const EXEMPT_DIRS = [
       "src/features/cliProfiles/profiles/claude",
     ];
     /** 文件级豁免：本守卫自身 */
     const EXEMPT_FILES = [
       "src/__tests__/no-claude-literals.test.ts",
     ];
     ```
     遍历改为 `readdirSync(SCAN_ROOT, { recursive: true, withFileTypes: true })`（Node 22 支持 recursive），过滤 `EXEMPT_DIRS` 前缀与 `EXEMPT_FILES`，仅扫 `.ts/.tsx`。
  2. 若全量扫描首次命中既有违例（新纳入目录下的 claude 字面量）：逐一判定——身份域漏标 → 补豁免目录并注释理由；真违例 → 报告并在 stages 标注，禁止默默加豁免。
  3. 删除原 SCAN_DIRS 常量。
- **测试同步**：用例数不变（扫描范围扩大）。
- **文档同步**：无。
- **验证**：`npx vitest run src/__tests__/no-claude-literals.test.ts` 通过；grep -c `SCAN_DIRS` 该文件为 0。

---

# TQ-E（L3+L4，10 项，含 1 翻案留痕）

## TQ-E-01 L3 production-osc 复制生产实现 → 抽生产纯函数复用

- **位置**：测试 `test/terminal/production-osc.test.ts:44-121`（OSC52/133 复制体）、:221-246（OSC 8 复制体）；生产 `src/panels/terminal/useClipboardHandler.ts`（OSC52 注册体）、`useCommandDetection.ts`（OSC133 注册体）、`useXterm.ts:294-299`（linkHandler）
- **现状**：测试按生产实现原样复刻（文件头 :1-10 声明为 D4 已知降级）；生产 OSC handler 耦合在 React hook 内，无纯函数出口；`src/panels/terminal/oscHandlers.ts` 不存在（05 报告误引文件名）。
- **修复步骤**：
  1. **新建生产文件** `src/panels/terminal/oscHandlers.ts`（最小可测性改动，行为不变）：
     ```ts
     // OSC 处理器纯注册层（TQ-E-01）——从 React hook 抽离，L3 headless 与生产 hook 共用同一真值源。
     // 依赖（剪贴板/开链接/profile 匹配/会话标记）全部参数注入，本文件不 import ipc/store。
     import type { Terminal } from "@xterm/xterm";
     import type { IDisposable } from "@xterm/xterm";

     /** OSC 52 单条 payload 上限（字节，base64 前）——与 useClipboardHandler 原常量一致 */
     export const MAX_OSC52_PAYLOAD = 1048576;

     export interface Osc52Deps {
       /** 焦点门控：返回 false 时忽略写入（对应生产 visibleRef） */
       isVisible: () => boolean;
       writeText: (text: string) => Promise<void>;
     }

     /** 注册 OSC 52 剪贴板写 handler（c;base64 → 解码 → writeText） */
     export function registerOsc52(term: Terminal, deps: Osc52Deps): IDisposable {
       return term.parser.registerOscHandler(52, (data: string) => {
         const semicolonIdx = data.indexOf(";");
         if (semicolonIdx === -1) return true;
         const selector = data.substring(0, semicolonIdx);
         const payload = data.substring(semicolonIdx + 1);
         if (selector && selector !== "c") return true;
         if (payload === "?" || payload.length === 0) return true;
         if (payload.length > MAX_OSC52_PAYLOAD) return true;
         if (deps.isVisible() === false) return true;
         try {
           const binary = atob(payload);
           const bytes = new Uint8Array(binary.length);
           for (let i = 0; i < binary.length; i++) {
             bytes[i] = binary.charCodeAt(i);
           }
           const text = new TextDecoder().decode(bytes);
           deps.writeText(text).catch((e) => console.error("[OSC52] 写剪贴板失败:", e));
         } catch {
           // base64 非法——忽略（保持生产原行为）
         }
         return true;
       });
     }

     export interface Osc133Deps {
       isCommandRunning: { current: boolean };
       matchByCommand: (command: string) => { id: string; tabTitle: string } | null;
       setAgentSession: (cliId: string | null) => void;
       onTabStateChange: (state: { active: boolean; title?: string; status?: string }) => void;
     }

     /** 注册 OSC 133 命令边界 handler（C=开始/D=结束） */
     export function registerOsc133(term: Terminal, deps: Osc133Deps): IDisposable {
       return term.parser.registerOscHandler(133, (data: string) => {
         const semicolonIndex = data.indexOf(";");
         const type = semicolonIndex >= 0 ? data.slice(0, semicolonIndex) : data;
         if (type === "C") {
           const command = semicolonIndex >= 0 ? data.slice(semicolonIndex + 1).trim() : "";
           const profile = deps.matchByCommand(command);
           if (profile) {
             deps.isCommandRunning.current = true;
             deps.setAgentSession(profile.id);
             deps.onTabStateChange({ active: true, title: profile.tabTitle, status: "attention" });
           }
         } else if (type === "D" && deps.isCommandRunning.current) {
           deps.isCommandRunning.current = false;
           deps.onTabStateChange({ active: false });
           deps.setAgentSession(null);
         }
         return false;
       });
     }

     /** OSC 8 超链接激活（openUrl 注入，失败 console.error——与生产原行为一致） */
     export function makeLinkHandler(openUrl: (url: string) => Promise<void>): { activate: (event: unknown, url: string) => void } {
       return {
         activate: (_event: unknown, url: string) => {
           openUrl(url).catch((err) => console.error("打开链接失败:", err));
         },
       };
     }
     ```
  2. **生产 hook 改调用**：
     - `useClipboardHandler.ts`：useEffect 内改为 `const disposable = registerOsc52(terminal, { isVisible: () => visibleRef.current, writeText });`（import 自 ./oscHandlers；删除内联注册体与本地 MAX_OSC52_PAYLOAD 常量——其他文件若引用该常量改从 oscHandlers 导入）。
     - `useCommandDetection.ts`：useEffect 内改为 `registerOsc133(terminal, { isCommandRunning: isCommandRunningRef, matchByCommand: (cmd) => cliProfileRegistry.matchByCommand(cmd), setAgentSession: (cliId) => TerminalRegistry.setAgentSession(panelId, cliId ? { cliId, matchedCommand: cliId } : null), onTabStateChange: (s) => onTabStateChangeRef.current?.(s as TabState) });`
     - `useXterm.ts:294-299`：linkHandler 改为 `makeLinkHandler(openUrl)`。
     - 注意：OSC133 的 `setAgentSession` 生产原调用为 `TerminalRegistry.setAgentSession(panelId, { cliId: profile.id, matchedCommand: profile.id })`——参数注入闭包内保持原值形状，不改语义。
  3. **L3 测试改写**：`production-osc.test.ts` 删除三段复制体，改为 `import { registerOsc52, registerOsc133, makeLinkHandler } from "../../src/panels/terminal/oscHandlers";`，以注入 mock 依赖（writeText/matchByCommand/setAgentSession/onTabStateChange/openUrl 全 vi.fn）注册到 headless term；原断言（写剪贴板文本/CJK 解码/tabState 参数/openUrl 调用）全部保留。visible 门控用 `isVisible: () => visible` 变量控制。文件头注释更新为「复用生产 oscHandlers.ts 注册层（TQ-E-01 后不再复刻）」。
  4. L3 环境注意：oscHandlers.ts 不 import ipc/store（依赖注入设计），atob/TextDecoder 在 Node 22 全局可用 ✓。
- **测试同步**：L3 用例数不变（断言保留）；L2 中 use-xterm 相关测试若 mock 了 useClipboardHandler/useCommandDetection 内部——grep 确认无直接 mock 内联注册体的用例；`npm test` 全量验证。
- **文档同步**：`src/panels/CLAUDE.md` 补一句「OSC 52/133/8 注册层单点 oscHandlers.ts（hook 薄包装 + L3 复用，TQ-E-01）」。
- **验证**：①`npm run test:l3` 通过；②grep -c `registerOscHandler(52` useClipboardHandler.ts 为 0；③`npm test` 全绿；④grep `oscHandlers` 命中 production-osc.test.ts import 行。

## TQ-E-02 L3 不覆盖生产快捷键分发 → 抽纯函数 + 新增 L3 用例

- **位置**：生产 `src/panels/terminal/useXterm.ts:282-291`（attachCustomKeyEventHandler 内联匿名体）；测试 `test/terminal/keyboard.test.ts:1-12`（D4 降级标注，定位保留）
- **现状**：按键分发 `event → getShortcutRegistry().resolve(event, "terminal")` 内联在 hook；L3 只测 xterm 透传（已有 D4/E2E-01 降级声明，该定位保留不动）。
- **修复步骤**：
  1. **新建生产文件** `src/panels/terminal/keyEventHandler.ts`（最小可测性改动）：
     ```ts
     // 终端按键分发纯函数（TQ-E-02）——从 useXterm 内联体抽离，L3 headless 可挂。
     import { getShortcutRegistry } from "../features/shortcuts";

     /** xterm attachCustomKeyEventHandler 回调：ShortcutRegistry 消费则拦截（返回 false），否则透传 */
     export function handleTerminalKeyEvent(event: KeyboardEvent): boolean {
       if (event.type !== "keydown") return true;
       const consumed = getShortcutRegistry().resolve(event, "terminal");
       if (consumed) {
         event.preventDefault();
         return false;
       }
       return true;
     }
     ```
  2. **生产改调用**：`useXterm.ts:283-291` 匿名体替换为 `term.attachCustomKeyEventHandler(handleTerminalKeyEvent);`（import 自 ./keyEventHandler）。
  3. **新增 L3 文件** `test/terminal/shortcut-dispatch.test.ts`：
     ```ts
     // L3 — 生产按键分发链路（TQ-E-02）：headless term 挂生产 handleTerminalKeyEvent，
     // 断言 Ctrl+Shift+C/V 等被 ShortcutRegistry 消费（return false + preventDefault），
     // 未注册键透传（return true）。ShortcutRegistry 用真实注册表 + _reset 隔离。
     ```
     用例 ≥3：①注册 terminal context 的 Ctrl+Shift+C → handler 返回 false；②未注册键（如 KeyA 无修饰）→ 返回 true；③非 keydown 类型 → true。mock `../features/shortcuts` 的 getShortcutRegistry 或用真实 ShortcutRegistry（取其依赖最小者，执行时 Read src/features/shortcuts/index.ts 定）。
  4. keyboard.test.ts 的 D4 降级标注不动（xterm 基础行为回归定位保留）。
- **测试同步**：L3 +3 用例（新文件）。
- **文档同步**：`src/panels/CLAUDE.md` 随 TQ-E-01 同句追加「按键分发 keyEventHandler.ts」。
- **验证**：`npm run test:l3` 通过（含新文件 3 用例）；grep `handleTerminalKeyEvent` 命中 useXterm.ts 与 shortcut-dispatch.test.ts。

## TQ-E-03 L4 三处固定 350ms sleep → 条件等待

- **位置**：`e2e-tests/agent.e2e.ts:88`；`e2e-tests/history.e2e.ts:101`；`e2e-tests/mockcli.e2e.ts:232-233`
- **现状**：
  ```ts
  if (!clicked) return;
  await new Promise((r) => setTimeout(r, 350));
  ```
  树节点点击展开后无条件 sleep。
- **修复步骤**：
  1. 三处统一改为条件等待（可照抄模式；目标：点击后子节点/目标行出现）：
     ```ts
     if (!clicked) return;
     // 条件等待展开结果出现（替代固定 350ms sleep——TQ-E-03）
     await browser.waitUntil(
       async () => /* 目标子节点/行已渲染 */ (await /* 既有行查询 execute 调用 */) > 0,
       { timeout: 5000, interval: 100, timeoutMsg: "树节点展开超时" },
     );
     ```
     执行时 Read 三处上下文（循环体点击的是什么节点、下一步断言查什么行），将「下一步要交互的行选择器」作为 waitUntil 条件；禁止保留任何 setTimeout 固定值。
- **测试同步**：L4 用例数不变。
- **文档同步**：无。
- **验证**：`npm run e2e` 全绿；grep -c `setTimeout(r, 350)` e2e-tests/ 为 0。

## TQ-E-04 history.e2e.ts 吞掉 createProject 失败

- **位置**：`e2e-tests/history.e2e.ts:112-115`
- **现状**：
  ```ts
  const proj = await browser.execute((dir: string) => {
    return (window as any).__slterm_e2e_createProject?.(dir);
  }, e2eProjectDir).catch(() => null);
  void proj;
  ```
- **修复步骤**：
  1. 改为断言式（可照抄）：
     ```ts
     const proj = await browser.execute((dir: string) => {
       return (window as any).__slterm_e2e_createProject?.(dir);
     }, e2eProjectDir);
     // 创建失败立即 fail——后续扫描/恢复/删除断言不得基于不存在的状态（TQ-E-04）
     if (!proj) {
       throw new Error(`__slterm_e2e_createProject 返回空（dir=${e2eProjectDir}）——helper 未就绪或创建失败`);
     }
     ```
     （删除 `.catch(() => null)` 与 `void proj;`——execute 自身 reject 即自然 fail。）
- **测试同步**：用例数不变。
- **文档同步**：无。
- **验证**：`npm run e2e` 全绿；grep -c `catch(() => null)` history.e2e.ts 为 0。

## TQ-E-05 terminal.e2e.ts 粘贴断言只查注入标记

- **位置**：`e2e-tests/terminal.e2e.ts:87-131`；豁免「L4 真实 OS 级按键」（test-inventory.md 豁免表）
- **现状**：`browser.keys(["Control","Shift","v"])` 的按键是否被消费无断言（embedded WDIO 无法投递真实按键，豁免已登记）；验证文本经 `__e2e_writeToTerminal` 注入后读回——实际验证的是 E2E helper 写读往返。
- **修复步骤**：
  1. 用例改名对齐实际职责：`it("终端面板可通过 E2E helper 写入文本并读取（按键链路豁免见 inventory）", ...)`。
  2. 补一条真实能力断言：验证 `__slterm_e2e_writeClipboard` 真实写入了系统剪贴板（经 tauri clipboard plugin 读回）：
     ```ts
     // 剪贴板真实写入断言（不依赖按键投递）：写入后经生产 clipboard 读取通道读回
     const clipText = await browser.execute(async () => {
       // 生产读取通道：@tauri-apps/plugin-clipboard-manager（页面内可调）
       const mod = await import("@tauri-apps/plugin-clipboard-manager");
       return mod.readText();
     });
     expect(clipText).toBe("e2e_paste_marker");
     ```
     （若页面内动态 import 插件受限，降级：经既有 ipc clipboard readText 封装读取——执行时 Read `src/ipc/clipboard.ts` 确认导出。）
  3. 豁免清单细化（Stage 10 统一改 inventory）：「L4 真实 OS 级按键」条目兜底描述追加「terminal.e2e.ts 粘贴用例 = helper 写读往返 + 剪贴板读回断言；Ctrl+Shift+V 按键消费链路由 L2 keyboard.test.ts + L3 shortcut-dispatch.test.ts（TQ-E-02 新增）覆盖」。
- **测试同步**：用例数不变（断言增强）。
- **文档同步**：inventory 豁免条目（Stage 10）。
- **验证**：`npm run e2e` 全绿；用例名含「按键链路豁免见 inventory」。

## TQ-E-06 run-wdio.cjs 恢复失败静默污染用户目录

- **位置**：`e2e-tests/run-wdio.cjs`（`process.on('exit')` 同步恢复段，约 :100-160）
- **现状**：5 处恢复/清理全在 exit 同步回调内，各 try/catch 吞错；恢复失败无任何报告。exit 钩子是三启动路径统一兜底（有意设计，文件头注释），保留。
- **修复步骤**：
  1. 将 exit 回调体抽为命名函数 `restoreAll(): string[]`（返回失败项描述数组），exit 钩子改为（可照抄）：
     ```js
     process.on('exit', () => {
       const failures = restoreAll();
       if (failures.length > 0) {
         // 恢复失败必须可观测——静默会污染 ~/.slterminal 与 ~/.claude 真实用户数据（TQ-E-06）
         console.error(`[wdio-launcher] 用户目录恢复失败 ${failures.length} 项:`);
         for (const f of failures) console.error(`  - ${f}`);
         process.exitCode = 1;
       } else {
         console.log('[wdio-launcher] 用户目录恢复完成（全部成功）');
       }
     });
     ```
  2. `restoreAll()` 内把原 exit 回调的每段 try/catch 改为「catch 时 push 失败描述到 failures 而非吞掉」（恢复动作本身不变，逐段照移：projects json / projects.bak / settings.json / claude settings / statusline 备份 / hooks-events 清理 / hooks 目录还原三分支）。
- **测试同步**：无 L4 用例变更；人工验证点（stages 标注）：本地故意占用 hooks 目录跑一次 `npm run wdio`，确认输出失败清单且 exit code 非 0。
- **文档同步**：`e2e-tests/CLAUDE.md` 补一句「恢复失败非零退出（TQ-E-06）」。
- **验证**：grep `restoreAll` 命中 run-wdio.cjs；grep `process.exitCode = 1` 命中；`npm run e2e` 全绿（正常路径输出「恢复完成」）。

## TQ-E-07 ~~launcher 下载 msedgedriver~~【翻案留痕】

- **位置**：`e2e-tests/run-wdio.cjs`、`e2e-tests/wdio.conf.ts:31-33`
- **翻案证据**：run-wdio.cjs 全文无 msedgedriver 下载/探测逻辑；wdio.conf.ts 已 `driverProvider: 'embedded'`。实跑日志的下载行为是便携 Node 22（`https://nodejs.org/dist/v22.21.1/win-x64/node.exe`，Node ≥26 时触发，已有 `.temp/node22` 缓存 + >1MB 判活）。05 报告该项失实。
- **修复步骤**：不修复。仅本清单留痕。
- **验证**：grep -c `msedgedriver` e2e-tests/run-wdio.cjs 为 0（维持）。

## TQ-E-08 L4 beforeSuite 只重置 projects，settings 等跨 spec 泄漏

- **位置**：`e2e-tests/wdio.conf.ts`（beforeSuite 段）；`e2e-tests/helpers.ts:200-210`（resetProjects 旁）
- **现状**：仅 `__slterm_e2e_resetProjects()`；settings/keybindings/sideBar/fontSize 无重置 helper。
- **修复步骤**：
  1. `helpers.ts` 在 resetProjects 旁新增（可照抄骨架）：
     ```ts
     // __slterm_e2e_resetSettings —— spec 间隔离前端配置类 store（TQ-E-08）。
     // 后端 settings.json 由 run-wdio.cjs 备份/还原做进程级隔离；此处管
     // 同一 run 内跨 spec 的 Zustand 内存态（keybindings/sideBar/fontSize）。
     window.__slterm_e2e_resetSettings = () => {
       useKeybindings.setState({ overrides: {}, loaded: true });
       useSideBar.setState({
         zones: { top: [...DEFAULT_ZONES.top], bottom: [...DEFAULT_ZONES.bottom] },
         open: { ...DEFAULT_OPEN },
         width: WIDTH_DEFAULT,
         splitRatio: SPLIT_DEFAULT,
         loaded: true,
       });
       useFontSize.setState({ fontSize: 14 }); // 默认字号以生产 store 默认值为准
     };
     ```
     （import 与默认值以生产 stores 实际定义为准——先 Read sideBarState.ts / keybindings.ts / fontSize store；`installAllE2eHelpers` 内调用挂载；`global.d.ts` 同步声明类型。）
  2. `wdio.conf.ts` beforeSuite 改为同时调两个 reset：
     ```ts
     beforeSuite: async function () {
       await browser.execute(() => {
         (window as any).__slterm_e2e_resetProjects?.();
         (window as any).__slterm_e2e_resetSettings?.(); // TQ-E-08：settings 类 store 同步隔离
       });
     },
     ```
  3. 注意：resetSettings 不得清 hooks 注入状态（hooks.e2e.ts 依赖 ensureHooksInjected 的幂等）——fontSize/sideBar/keybindings 三项之外不动。
- **测试同步**：L4 用例数不变。
- **文档同步**：`e2e-tests/CLAUDE.md` beforeSuite 说明更新。
- **验证**：`npm run e2e` 全绿；grep `__slterm_e2e_resetSettings` 命中 helpers.ts 与 wdio.conf.ts。

## TQ-E-09 L4 retries:1 掩盖 flakiness 观察面

- **位置**：`e2e-tests/wdio.conf.ts`（mochaOpts.retries）；`.github/workflows/ci.yml`
- **修复步骤**（保留 retries:1，新增 CI 观察面——用户已确认纳入）：
  1. `wdio.conf.ts` 的 retries 改为环境变量驱动（可照抄）：
     ```ts
     // E2E-15：用例级重试。默认 1（单条 flaky 不拖垮整轮）；
     // WDIO_RETRIES=0 时关闭——CI 观察面 job 用它暴露真实 flakiness（TQ-E-09）。
     retries: Number(process.env.WDIO_RETRIES ?? "1"),
     ```
  2. ci.yml 新增独立 job `e2e-flakiness-probe`（不阻塞合并：`continue-on-error: true`），步骤 = 复用 build-and-test 的构建段 + `npm run wdio`，env `WDIO_RETRIES: '0'`；job 头注释「flakiness 观察面——失败不阻断，人工巡检」。
- **测试同步**：无。
- **文档同步**：`e2e-tests/CLAUDE.md` 补观察面说明。
- **验证**：grep `WDIO_RETRIES` 命中 wdio.conf.ts 与 ci.yml。

## TQ-E-10 L3 不覆盖 Windows ConPTY/真实后端——职责边界登记

- **位置**：`.claude/test-inventory.md` L3 段；`test/terminal/`（7 文件）
- **修复步骤**（登记方案）：
  1. inventory L3 段表头下追加（可照抄）：
     ```markdown
     **职责边界（TQ-E-10）**：L3 全部用例运行于 node + @xterm/headless，无后端 PTY/ConPTY/shell 集成。
     以下生产关键路径不在 L3 覆盖范围，归属 L1/L4：reader 微批（L1 micro_batch_*）、ConPTY flags（L1 compute_conpty_flags 锁 0x7）、OSC 7/133 注入（L1 hooks 注入用例 + L4）、DA1 模拟（L1）、SPAWN_LOCK（L1）、路径沙箱（L1/L4）。
     L3 自身定位 = 网格状态正确性 + 生产 OSC/按键分发注册层（oscHandlers.ts / keyEventHandler.ts，TQ-E-01/02 后复用生产实现）。
     ```
- **测试同步**：无。
- **文档同步**：inventory 如上。
- **验证**：grep `职责边界（TQ-E-10）` 命中 `.claude/test-inventory.md`。

---

# TQ-CI（CI/基建/inventory，7 项，含 1 翻案留痕）

## TQ-CI-01 inventory L1 用例数/文件数失实（724 vs 726；34 vs 33）

- **位置**：`.claude/test-inventory.md:5`（表头）、:45（L1 段头）、:85（口径注释）
- **现状**：表头「Rust 724」「L1 — 34 文件 / 724 用例」；实证 `cargo test` 726 通过、`grep '#\[test\]'` 726 处 / 33 文件。
- **修复步骤**（Stage 10 执行——此时全部测试增删已稳定）：
  1. 以 Stage 10 当时的实跑数为准重新校准（不照抄 726——执行 Stage 01-08 会净增用例）：跑 `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 2>&1 | tail -20` 取总数；`grep -rc '#\[test\]' src-tauri/src src-tauri/tests | grep -v ':0' | wc -l` 取文件数。
  2. 更新 :5 表头、:45 L1 段头、:85 口径注释三处数字与「历史变更」追加一条校准记录。
  3. 在表头下方追加自检说明（可照抄）：
     ```markdown
     > **登记纪律（TQ-CI-01）**：改测试后同步本文件——L1 以 `cargo test` 实跑总数 + `grep -c '#[test]'` 双核对；L2 以 `npm test` 实跑（vitest 报告）为准；段小计 = 段内行级用例数之和（逐段核对 it.each/工厂展开）。三处（表头/段头/段小计）必须一致。
     ```
- **测试同步**：无。
- **文档同步**：inventory 如上。
- **验证**：表头总数 = L1+L2+L3+L4 四段头之和；L2 段小计之和 = L2 段头数（机械核对）。

## TQ-CI-02 inventory L2 用例数失实且段小计与行级差 43

- **位置**：`.claude/test-inventory.md:5`（前端 2633）、:89（L2 段头 2630）、20 段小计（之和 2592）
- **修复步骤**（Stage 10 执行）：
  1. 以当时 `npm test` 实跑数为 L2 总数基准。
  2. 逐段核对：对每段涉及文件清单 `npx vitest run <文件清单>` 取实跑数，修正段小计；行级漏登的逐文件补登（重点 `it.each` 展开与 `describeIpcContract` 工厂）。
  3. 三处一致化：表头 = 段头 = 段小计之和 = 实跑数。
- **测试同步**：无。
- **文档同步**：inventory 如上。
- **验证**：机械核对三处一致；抽查 3 段的段内文件实跑数 = 段小计。

## TQ-CI-03 CI 无 cargo fmt 门禁

- **位置**：`.github/workflows/ci.yml`（Clippy 步骤之前）；口径 `.claude/CLAUDE.md:105`
- **修复步骤**：
  1. 在 `Setup Rust toolchain` 之后、`Clippy check` 之前插入：
     ```yaml
           - name: Rustfmt check
             run: cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
     ```
- **测试同步**：无。
- **文档同步**：无（CLAUDE.md:105 已含 fmt，CI 补齐即对齐）。
- **验证**：ci.yml 含 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`；本地 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 当前通过（否则先 fmt 修齐再入 CI）。

## TQ-CI-04 ~~tempfile 移 [dev-dependencies]~~【翻案留痕】

- **位置**：`src-tauri/Cargo.toml:46`
- **翻案证据**：`tempfile::NamedTempFile` 生产在用作原子写——settings.rs:11,92-93 / projects.rs:12,30 / hooks/claude/config.rs:20,240 / hooks/claude/inject.rs:20,365,376,495,511,681。06 报告「仅测试使用」失实。
- **修复步骤**：不修复（tempfile 留在 [dependencies]）。仅本清单留痕。
- **验证**：`cargo build --manifest-path src-tauri/Cargo.toml` 通过（维持现状）。

## TQ-CI-05 setup.ts 全局 mock 隐性耦合全部 L2

- **位置**：`src/__tests__/setup.ts:88-120`（`../ipc/notify`、`../ipc/agentHooks`、`@tauri-apps/api/window` 三个全局 vi.mock）
- **决策**（用户已确认最小改动原则）：**维持全局 mock**——下沉到 154 个测试文件改动面大、收益低；改为文档化 + opt-out 明确化。
- **修复步骤**：
  1. setup.ts 三个全局 mock 段注释强化：每段顶部注明「全局默认桩——需要真实实现的测试用 `vi.mock(..., async (importOriginal) => ...)` 显式覆盖（先例：editor-confirm.test.ts:107-114）」。
  2. `src/__tests__/CLAUDE.md` 补「全局 mock 清单」小节（可照抄）：
     ```markdown
     ## 全局 mock 清单（setup.ts，TQ-CI-05）

     | mock 目标 | 默认桩行为 | opt-out 方式 |
     |-----------|-----------|--------------|
     | `../ipc/notify` | onFsEvent 返回 no-op 取消函数 | importOriginal 覆盖 |
     | `../ipc/agentHooks` | inject 恒 notInjected / onAgentEvent no-op | importOriginal 覆盖 |
     | `@tauri-apps/api/window` | getCurrentWindow 桩 | importOriginal 覆盖 |

     新增全局 mock 必须登记本表；测试依赖真实契约时禁用默认桩。
     ```
- **测试同步**：无。
- **文档同步**：`src/__tests__/CLAUDE.md` 如上。
- **验证**：grep `全局 mock 清单` 命中 `src/__tests__/CLAUDE.md`。

## TQ-CI-06 CI job/步骤无 timeout-minutes

- **位置**：`.github/workflows/ci.yml:11`（job）及各步骤
- **修复步骤**：
  1. job 级：`build-and-test:` 下加 `timeout-minutes: 60`。
  2. 步骤级（与既有步骤名对齐插入）：
     - Clippy / Rustfmt / Lint / TypeScript / knip：`timeout-minutes: 15`
     - Frontend unit tests (L2) / Terminal render tests (L3)：`timeout-minutes: 20`
     - Backend tests (L1)：`timeout-minutes: 30`（ConPTY 死锁红线保护）
     - Tauri debug build + E2E tests (L4)：`timeout-minutes: 60`
- **测试同步**：无。
- **文档同步**：无。
- **验证**：grep -c `timeout-minutes` ci.yml ≥ 8。

## TQ-CI-07 CI 无 npm 缓存

- **位置**：`.github/workflows/ci.yml:17-20`（setup-node）
- **修复步骤**：
  1. setup-node 的 `with:` 追加：
     ```yaml
               cache: 'npm'
     ```
     （lockfile 在仓根 package-lock.json，setup-node 默认识别，无需 cache-dependency-path。）
- **测试同步**：无。
- **文档同步**：无。
- **验证**：grep `cache: 'npm'` 命中 ci.yml。

---

# TQ-COV（覆盖缺口补写，10 项）

## TQ-COV-01 main.rs 零覆盖 → 抽 install_panic_hook 到 lib.rs + L1

- **位置**：`src-tauri/src/main.rs:1-19`（全文）；`src-tauri/src/lib.rs`
- **现状**：main.rs 19 行全未覆盖（llvm-cov 0.00%）；panic hook 内联。
- **修复步骤**：
  1. `lib.rs` 新增（可照抄）：
     ```rust
     /// 安装 panic hook：panic 信息写 exe 同级 crash.log（诊断用，TQ-COV-01）。
     /// 写文件失败回退 eprintln（不 panic——hook 内再 panic 会二次崩溃）。
     pub fn install_panic_hook() {
         std::panic::set_hook(Box::new(|info| {
             let dir = std::env::current_dir().unwrap_or_else(|_| ".".into());
             let message = format!("PANIC: {:?}", info);
             if write_crash_log(&dir, &message).is_err() {
                 eprintln!("{}", message);
             }
         }));
     }

     /// 写 crash.log（目录参数化以便 L1 测试）——返回 io 结果供 hook 决定回退
     fn write_crash_log(dir: &std::path::Path, message: &str) -> std::io::Result<()> {
         use std::io::Write;
         let mut f = std::fs::File::create(dir.join("crash.log"))?;
         writeln!(f, "{}", message)
     }
     ```
  2. `main.rs` 改为（可照抄，全文替换 hook 段）：
     ```rust
     // 所有构建模式均隐藏 Windows 控制台窗口
     #![windows_subsystem = "windows"]

     fn main() {
         slterminal_lib::install_panic_hook();
         slterminal_lib::run()
     }
     ```
  3. `lib.rs` 内 `#[cfg(test)]` 模块补两例：
     - `write_crash_log_writes_file`：tempdir 调 `write_crash_log(dir, "PANIC: test")` → 断言 crash.log 存在且含 "PANIC: test"。
     - `write_crash_log_err_on_unwritable_dir`：传入不存在且无权限创建的路径（如 `\\?\Z:\nonexistent\` 或不存在的嵌套路径）→ 断言返回 Err（不 panic）。
- **测试同步**：L1 +2 用例。
- **文档同步**：豁免清单「lib.rs run()」条目不变（main.rs 不再零覆盖，Stage 10 更新 07 缺口表口径）。
- **验证**：`cargo test --manifest-path src-tauri/Cargo.toml write_crash_log -- --test-threads=1` 通过；`cargo clippy -- -D warnings` 通过；llvm-cov 复测 main.rs 关联逻辑有覆盖（main.rs 自身仅两行调用仍结构性零覆盖——属正常，覆盖体现在 lib.rs）。

## TQ-COV-02 lib.rs setup 副作用覆盖 → 豁免登记细化

- **位置**：`src-tauri/src/lib.rs:74-80`；`.claude/test-inventory.md` 豁免表「lib.rs run()」行
- **现状**：setup 仅 2 副作用——`start_signal_watcher`（已有可注入 start_signal_watcher_impl + 4 用例，hooks/mod.rs:70-100）与 `reinject_statusline_on_startup`（B15 已有 L1 锁死）。无需再抽。
- **修复步骤**（登记方案）：
  1. inventory 豁免表「lib.rs `run()`」行的「当前兜底层级」列更新为：
     ```
     L4 terminal.e2e.ts 启动标题等用例 + setup 两副作用各自的 L1 锁死（start_signal_watcher_impl 4 例 / reinject_statusline B15 用例）——setup 本体保持豁免（TQ-COV-02）
     ```
- **测试同步**：无。
- **文档同步**：inventory 如上。
- **验证**：grep `TQ-COV-02` 命中 `.claude/test-inventory.md`。

## TQ-COV-03 PTY 核心路径补测

- **位置**：`src-tauri/src/pty/spawn.rs`（`join_with_timeout` :1464-1468 调用点、`ensure_pty_capacity` :1067-1075）；豁免登记 `src-tauri/src/pty/CLAUDE.md`
- **现状**：spawn.rs 行 67.95%/函数 58.33%；conpty_api.rs 63.19%；reader.rs 77.83%（llvm-cov 实证）。reader_loop 决策逻辑已纯函数化且有测；容量 kill 清理（:1307-1315）为 I/O 不可抽。
- **修复步骤**：
  1. **join_with_timeout 补测**（先 Read 其定义确认签名 `join_with_timeout(handle: JoinHandle<()>, timeout: Duration) -> bool`）：
     - `join_with_timeout_finished_handle_returns_true`：spawn 一个立即结束的 thread → 断言返回 true 且快速（<1s）。
     - `join_with_timeout_blocked_thread_returns_false`：spawn 一个 park 的 thread + timeout=50ms → 断言返回 false（用例耗时可忽略）。
  2. **ensure_pty_capacity 边界**（若既有 `pty_capacity_*` 用例已覆盖上限判定则跳过——grep 确认；未覆盖则补）：`MAX_PTY_SESSIONS - 1` → Ok；`MAX_PTY_SESSIONS` → Err(Validation)。
  3. **豁免登记**（pty/CLAUDE.md）：容量超限 kill 清理（spawn.rs:1307-1315）、conpty_api vendor 提取/加载回退的残余 Win32 分支——补入豁免表（格式同 TQ-L1-04 小节）。
- **测试同步**：L1 +2~4 用例。
- **文档同步**：pty/CLAUDE.md 豁免小节；inventory 豁免表同步（Stage 10）。
- **验证**：`cargo test --manifest-path src-tauri/Cargo.toml join_with_timeout -- --test-threads=1` 通过。

## TQ-COV-04 hooks signal/watcher 分支补全

- **位置**：`src-tauri/src/hooks/signal.rs`（`process_signal_file_with` :79-134）；`src-tauri/src/hooks/watcher.rs`（`run_one_tick` :206-214）
- **现状**：signal.rs 82.77%、watcher.rs 77.41%（llvm-cov 实证）。两核心函数均已参数注入（emit / process 回调）。
- **修复步骤**（全部 L1，tempfile 隔离）：
  1. `process_signal_file_with` 补 3 例：
     - 超限：写入 `MAX_SIGNAL_FILE_BYTES + 1` 字节文件 → emit 计数 0、文件已删除。
     - 读失败：传目录路径（read_to_string 失败）→ emit 计数 0、不 panic（路径仍在或已删均可，断言不 panic + 无 emit）。
     - emit 失败仍删除：emit 闭包返回 `Err(tauri::Error::...)`（构造方式以现有测试的 Err 构造为准——查既有用例如何造 tauri::Error）→ 文件已删除。
  2. `run_one_tick` 补 2 例：
     - 目录删除后重建：tempdir 下建信号目录 → 删除 → `run_one_tick(信号目录, stop_rx, process)` → 断言目录已重建（`create_dir_all` 分支）。
     - 停止信号：`stop_tx.send(())` 后调 run_one_tick → 返回 true。
- **测试同步**：L1 +5 用例。
- **文档同步**：无。
- **验证**：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 signal watcher` 相关新用例通过；llvm-cov 复测 signal.rs/watcher.rs 行覆盖 ≥ 90%（或剩余分支补登记豁免）。

## TQ-COV-05 SEC-17 审计日志 L1 断言（tracing-test）

- **位置**：`src-tauri/Cargo.toml`（无 [dev-dependencies] 段）；`src-tauri/src/hooks/claude/config.rs:294-297`
- **现状**：`tracing::warn!(target: "audit", ...)` 仅人工可观测；豁免理由「L1 不可断言」不成立（07 报告核查）。
- **修复步骤**：
  1. Cargo.toml 末尾新增段：
     ```toml
     [dev-dependencies]
     tracing-test = "0.2"
     ```
  2. config.rs `#[cfg(test)]` 模块新增（可照抄骨架）：
     ```rust
     #[test]
     #[tracing_test::traced_test]
     fn user_layer_write_emits_audit_log() {
         // SEC-17：user 层写入必须留审计日志（TQ-COV-05——豁免理由「L1 不可断言」翻案）
         let dir = tempfile::tempdir().unwrap();
         let path = dir.path().join("settings.json");
         super::write_config_to_layer_for_test(&path, Layer::User, serde_json::json!({}));
         assert!(logs_contain("hooks user 层配置写入"));
     }

     #[test]
     #[tracing_test::traced_test]
     fn project_layer_write_no_audit_log() {
         let dir = tempfile::tempdir().unwrap();
         let path = dir.path().join("settings.json");
         super::write_config_to_layer_for_test(&path, Layer::Project, serde_json::json!({}));
         assert!(!logs_contain("hooks user 层配置写入"));
     }
     ```
     执行时先 Read config.rs 写盘函数实际签名（`write_config_to_layer_for_test` 为占位名——用既有 `config_write_sync_*` 用例调用的真实函数），断言目标行 :294-297 的 warn 文案。
  3. inventory 豁免表 SEC-17 行删除或改为「已由 L1 tracing-test 断言锁死（TQ-COV-05）」（Stage 10 统一）。
- **测试同步**：L1 +2 用例。
- **文档同步**：inventory 豁免表（Stage 10）；`src-tauri/src/hooks/CLAUDE.md` SEC-17 描述更新。
- **验证**：`cargo test --manifest-path src-tauri/Cargo.toml audit_log -- --test-threads=1` 通过。

## TQ-COV-06 git/mod.rs 函数覆盖 37.14% → 定位死函数 + 补测

- **位置**：`src-tauri/src/git/mod.rs`（5 命令 + 6 pub 函数，无内联 test 模块）；测试 `src-tauri/tests/git_*_tests.rs`（6 文件 97 用例）
- **现状**：行 77.75% 但函数仅 37.14%（llvm-cov 实证）——大量函数未执行。
- **修复步骤**（执行期先定位再分流）：
  1. 跑 `cargo llvm-cov --manifest-path src-tauri/Cargo.toml --html -- --test-threads=1`，打开 `target/llvm-cov/html/index.html` 定位 git/mod.rs **未执行函数清单**（Functions 列 0% 的函数名）。
  2. 逐个分流：
     - **无调用方的死函数**（grep 全仓无引用）：删除（commit 说明）；knip 式检查 `cargo clippy` 不死代码告警即过。
     - **有调用方但未测**：在 `src-tauri/tests/` 对应文件补命令层用例（经 tempfile 建 git 仓库夹具——沿用 git_status_tests.rs 等既有夹具模式）。
  3. 目标：git/mod.rs 函数覆盖 ≥ 80%；残余不可达分支在 `src-tauri/src/git/CLAUDE.md` 登记豁免。
- **测试同步**：L1 增量视定位结果（预估 +5~15 用例）。
- **文档同步**：`src-tauri/src/git/CLAUDE.md` 测试模式处更新函数覆盖口径。
- **验证**：llvm-cov 复测 git/mod.rs Functions ≥ 80%（或残余逐条登记）；`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿。

## TQ-COV-07 终端面板错误分支补测

- **位置**：`src/panels/terminal/useTerminalInstance.ts`（fit catch :162-166、performDispose :88-112）；`useXterm.ts`（handleWriteError :194-203、spawn catch :350-358、openUrl :294-299、kill catch :543-546、fontSize resize catch :590-600）；`usePtyResize.ts`（:92-118）
- **现状**：函数覆盖 69-71%（v8 实证）；WebGL 重试耗尽回退、写入失败 toast 阈值、resize 连续失败未测。
- **修复步骤**（在既有测试文件追加，不新建文件）：
  1. `terminal-instance.test.ts` 或 `use-xterm-*.test.ts`（按被测函数归属选择）补：
     - WebGL 重试耗尽 → `onFail` 调用 + console.warn（mock webgl.ts 的 tryLoadWebgl 或直接驱动 `WEBGL_RETRY_MAX` 次失败——Read webgl.ts:106-125 取注入点）。
     - `handleWriteError` 连续失败达 `WRITE_FAIL_TOAST_THRESHOLD` → toast.show("error", ...) 一次；未达阈值 → 仅 console.error。
     - `usePtyResize` resize reject → console.error("PTY resize 失败") 且不 throw。
     - fontSize 变化时 fit 抛异常 → catch 吞掉不 throw（:590-600 分支）。
  2. 每例断言「错误可感知化（FE-08）」：console.error/toast 调用参数，不只「不 throw」。
- **测试同步**：L2 +4~6 用例。
- **文档同步**：无。
- **验证**：新增用例通过；v8 复测三文件函数覆盖均 ≥ 85%（或残余登记）。

## TQ-COV-08 PageDockviewHost 覆盖补测（修订项）

- **位置**：`src/workspace/PageDockviewHost.tsx`（07 报告误写为 DockviewHost.tsx——该文件不存在，留痕）
- **现状**：07 报告引用的 79.76% 行覆盖数据对应文件需执行期以 v8 报告实际文件名为准复核。
- **修复步骤**：
  1. 执行期跑 `npm run test:coverage`，在 v8 报告中找到 PageDockviewHost.tsx 的真实覆盖数据与未覆盖分支清单。
  2. 对未覆盖分支在 `workspace-page-dockview.test.tsx` 补测（布局恢复错误隔离、createWatermark 交互、面板注册边界）。
  3. 若 v8 报告确有其他 `*DockviewHost*` 文件（多实例懒初始化等），同法处理。
- **测试同步**：L2 增量视复核结果（预估 +3~6 用例）。
- **文档同步**：无。
- **验证**：v8 复测 PageDockviewHost.tsx 行覆盖 ≥ 90%（或残余登记）。

## TQ-COV-09 NavPageRow / ExplorerPanel 交互补测

- **位置**：`src/features/navTree/NavPageRow.tsx`（confirmRename :54-58、handleKeyDown :60-66、chevron onClick :82-90）；`src/features/explorer/ExplorerPanel.tsx`（错误横幅 :399-435、加载失败占位 :449-490）
- **现状**：NavPageRow 行 52.63%/函数 44.44%；ExplorerPanel 行 77.35%（v8 实证）。
- **修复步骤**：
  1. NavPageRow 补（挂到 nav-tree.test.tsx 或新建 nav-page-row.test.tsx——按既有 nav 测试组织习惯）：重命名 Enter 确认 / Escape 取消 / chevron 点击 stopPropagation+onToggle / renaming 中行点击禁用。
  2. ExplorerPanel 补：readDir 失败 → `[data-testid="explorer-load-error"]` 占位出现 + retry 钮（`explorer-load-retry`）点击重新加载；操作失败 → `explorer-error-banner` 文案断言。（ExplorerPanel 已带这两个 testid——直接查询即可。）
- **测试同步**：L2 +5~8 用例。
- **文档同步**：无。
- **验证**：v8 复测 NavPageRow 行覆盖 ≥ 85%、ExplorerPanel ≥ 88%（或残余登记）。

## TQ-COV-10 低危组件补测打包

- **位置**：`src/ipc/window.ts`（minimize/toggleMaximize/close :68-94）；`src/features/sideViews/sideViewDefs.ts`（函数 0%）；`src/features/navTree/NavHistoryRow.tsx`（50%）、`NavProjectRow.tsx`（66.66%）、`NavSessionRow.tsx`（84.61%）、`NavTreeContextMenu.tsx`、`src/panels/gitshow/GitShowPanel.tsx`
- **修复步骤**：
  1. **ipc/window**：在既有 ipc 测试（或新建 `src/__tests__/ipc-window.test.ts`）mock `@tauri-apps/api/window` 的 getCurrentWindow，补三 wrapper 的 reject 传播断言（minimize/toggleMaximize/close 各 1 例）。
  2. **sideViewDefs 常量守卫**：新建用例 import sideViewDefs（触发注册）→ 断言 `sideViewRegistry.getAll()` 的 id 集合精确为 `["nav", "explorer", "commit"]`（注册序）；afterEach `_reset()`。
  3. **Nav*Row / GitShow / NavTreeContextMenu**：执行期按 v8 报告未覆盖分支补（NavHistoryRow 标题/时间分支、NavProjectRow 交互、GitShow 错误/加载分支）——每文件 ≥ 2 用例。
- **测试同步**：L2 +8~12 用例。
- **文档同步**：无。
- **验证**：v8 复测 sideViewDefs 函数 100%、ipc/window 行 ≥ 90%；其余文件行覆盖均 ≥ 85%（或残余登记）。

---

## 附：执行收尾统一动作（Stage 10）

1. **全量四级基线复跑**：L1（--test-threads=1）/ L2 / L3 / L4 全绿。
2. **coverage 复测对照**：`npm run test:coverage` + `cargo llvm-cov --manifest-path src-tauri/Cargo.toml -- --test-threads=1`，与 07 报告基线（前端行 93.93%/分支 86.00%；Rust 行 87.70%）对比——目标：前端行 ≥ 94.5%、Rust 行 ≥ 90%，重点文件（main.rs 关联、git/mod.rs 函数、hooks signal/watcher、NavPageRow、PageDockviewHost、sideViewDefs、useTerminalInstance/useXterm/usePtyResize）达标或逐条登记豁免。
3. **inventory 三处一致**（TQ-CI-01/02）+ 豁免清单更新（SEC-17 翻案、TQ-COV-02/03 新增、TQ-E-05 细化、TQ-E-10 边界）+ 条件跳过登记（TQ-L1-02）+ 登记纪律（TQ-CI-01 第 3 步）。
4. **模块 CLAUDE.md 同步**：explorer / fileViewers / panels / __tests__ / e2e-tests / pty / git / hooks / theme（各条目「文档同步」段的汇总）。
5. **人工验证点汇总**（stages.md 逐项列出，收尾实测）：TQ-E-06 恢复失败路径、Stage 06 OSC 复用后真实终端剪贴板/状态圆点行为、Stage 05 无。


