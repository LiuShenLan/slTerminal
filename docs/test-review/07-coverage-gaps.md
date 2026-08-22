# 覆盖率缺口分析报告

## 量化概览

### 前端（Vitest + @vitest/coverage-v8）

```
Statements   : 92.26% (5430/5885)
Branches     : 86.00% (2594/3016)
Functions    : 89.11% (1253/1406)
Lines        : 93.93% (4921/5239)
```

- 覆盖范围默认包含 `src/__tests__` 与 `e2e-tests/helpers.ts`。`e2e-tests/helpers.ts` 自身覆盖率 39.31%，属于 E2E 辅助代码，**不计入生产代码缺口**。
- `vitest.config.ts` 未显式配置 `coverage.exclude`；默认排除 `node_modules`/`*.test.*`/`*.d.ts` 等，未额外排除 `e2e-tests`。由于 `src/__tests__` 中部分测试工厂会 import `e2e-tests/helpers.ts`，导致其进入报告，不影响结论。

### Rust（cargo-llvm-cov）

```
Filename     Regions    Missed   Cover   Functions  Missed  Executed  Lines   Missed  Cover
TOTAL        20340      2231     89.03%  1462       268     81.67%    11438   1407    87.70%
```

- 运行命令：`cargo llvm-cov --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
- 工具安装成功：`cargo install cargo-llvm-cov --locked` 完成，`llvm-tools-preview` 已就位。
- 测试全部通过：L1 724 用例（621 lib + 105 tests）通过。

## 问题清单

### [高] `src-tauri/src/main.rs` 入口与 panic hook 零覆盖
- **维度**: 覆盖率
- **证据**: `src-tauri/src/main.rs:4-18`
- **证据类型**: 实证（llvm-cov：`main.rs` Lines 0.00%，Functions 0.00%）
- **问题**: 整个 `main.rs` 无任何自动化测试覆盖。其中 panic hook 的「创建 crash.log 失败 → 回退 eprintln」分支、Windows 子系统声明的生效路径均无法回归。启动入口异常（如 panic hook 本身失败、库初始化 panic）只能在真实二进制启动时暴露，L4 亦不覆盖 panic 分支。
- **建议**: 若 panic hook 逻辑持续保留，应至少拆出纯函数 `write_crash_log` 并补 L1；或将其登记为 `.claude/test-inventory.md` 豁免项并明确 L4 兜底范围。

### [高] `src-tauri/src/lib.rs` 启动胶水低覆盖
- **维度**: 覆盖率
- **证据**: `src-tauri/src/lib.rs` Lines 25.86%，Functions 42.86%
- **证据类型**: 实证
- **问题**: `run()` 与 `.setup()` 中 Tauri 运行时初始化（窗口创建、菜单、HookSignalWatcher 启动、statusline 桥接 reinject、关闭清理注册）均未在 L1 执行。虽然 `run()` 在豁免表中列为 L4 兜底，但 setup 内的多段副作用（watcher 启动失败、reinject 失败、窗口事件处理）无直接断言，异常只能在 L4 偶发暴露。
- **建议**: 对 setup 中可注入部分（如 watcher 启动器、reinject 调用点）抽象 trait 或回调，补 L1 失败路径；其余在豁免表中细化兜底用例。

### [高] PTY Win32/ConPTY 核心路径覆盖不足
- **维度**: 覆盖率
- **证据**: `src-tauri/src/pty/spawn.rs` Lines 67.95%，Functions 58.33%；`src-tauri/src/pty/conpty_api.rs` Lines 63.19%，Functions 73.91%；`src-tauri/src/pty/reader.rs` Lines 77.83%
- **证据类型**: 实证
- **问题**: `spawn_conpty_child`（`CreateProcessW` + `AttrList`）、`conpty_api.rs` 的 vendor 提取/加载/回退、`reader_loop` 的 I/O 编排残余分支覆盖率偏低。这些分支涉及并发锁、Channel、系统调用，已部分登记豁免，但残余包含：容量超限后的 kill 清理、reader 线程 join 超时/失败、自定义 ConPTY 加载回退等真实 OS 交互路径。后果：Win10 捆绑 ConPTY 失败回退、并发 spawn 边界、终端输出卡死等回归难以自动守卫。
- **建议**: 在豁免表外，对可纯函数化的决策逻辑（如容量检查后的清理、flags/路径构造、读错误降级）继续抽取补测；L4 增加 Win10 实机滚轮/IME 回归作为最终兜底。

### [高] agent-event 信号消费链路覆盖缺口
- **维度**: 覆盖率
- **证据**: `src-tauri/src/hooks/signal.rs` Lines 82.77%，Functions 78.12%；`src-tauri/src/hooks/watcher.rs` Lines 77.41%，Functions 81.03%
- **证据类型**: 实证
- **问题**: `process_signal_file` 的部分分支、`run_one_tick`/事件循环的目录删除重建恢复、emit 失败后的降级路径未完全覆盖。这些路径直接决定页签状态圆点、导航树活跃行、历史会话注入等用户可见行为。后果：信号文件残留、agent-event 丢失/重复、watcher 目录重建后无法自恢复。
- **建议**: 对 `watcher` 事件循环引入 mock `EventEmitter` 的测试模式已存在，继续补全「目录删除后重建」「emit 失败仍删除」「批量残留文件」等分支；`signal.rs` 中对非法 JSON/ oversized / symlink 等已部分覆盖，继续补全边界。

### [中] `git/mod.rs` 大量函数未执行
- **维度**: 覆盖率
- **证据**: `src-tauri/src/git/mod.rs` Lines 77.75%，Functions 37.14%
- **证据类型**: 实证
- **问题**: 行覆盖率尚可，但函数覆盖率仅 37.14%，说明许多 git2 命令函数或错误处理分支从未被调用。git 沙箱拒绝、非仓库错误、HEAD 读取失败、diff 边界等已有 L1 覆盖，但剩余未执行函数可能包含新增命令或辅助函数。后果：git 功能扩展时，未执行函数的错误契约缺乏回归。
- **建议**: 用 `cargo llvm-cov --html` 定位具体未执行函数清单；对已无调用方的死函数进行清理，对仍有调用方的函数补命令层测试。

### [中] 终端面板生命周期与错误分支覆盖不全
- **维度**: 覆盖率
- **证据**: `src/panels/terminal/useTerminalInstance.ts` Lines 89.33%，Functions 69.23%；`src/panels/terminal/useXterm.ts` Lines 92.34%，Functions 71.42%；`src/panels/terminal/usePtyResize.ts` Lines 92.68%，Functions 71.42%
- **证据类型**: 实证
- **问题**: 函数覆盖率低于行覆盖率，说明部分回调/错误处理分支未触发：WebGL 检测抛异常后的 DOM fallback、resize 连续失败、fontSize 未定义跳过、terminal 实例 dispose 异常等。这些路径与 GPU 回退、PTY 错误可感知化（FE-08）直接相关。
- **建议**: 补全 `useTerminalInstance` 中 WebGL fallback/tryLoadWebgl 幂等、`usePtyResize` 错误回调、`useXterm` 部分事件 handler 的 L2/L3 用例。

### [中] 工作区 Dockview 宿主多实例与就绪分支覆盖不足
- **维度**: 覆盖率
- **证据**: `src/workspace/DockviewHost.tsx` Lines 79.76%，Functions 64.4%，Branches 62.85%
- **证据类型**: 实证
- **问题**: 多 Dockview 实例懒初始化、api 未就绪分支、布局恢复错误隔离、跨页实例复用等分支未完全覆盖。H6（终端跨页面存活）虽有 L2/L4 兜底，但 DockviewHost 内部状态机（ready/loading/error）的低函数覆盖率意味着边界行为（如 api 突变、ready 前 addPanel 失败）回归风险高。
- **建议**: 补全 `DockviewHost` 的 error/loading/未就绪状态及 `api` 引用变更的 L2 用例；结合 `workspace-multi-instance.test.tsx` 扩展跨实例身份断言。

### [中] 导航树页面行与资源管理器面板分支覆盖不足
- **维度**: 覆盖率
- **证据**: `src/features/navTree/NavPageRow.tsx` Lines 52.63%，Functions 44.44%；`src/features/explorer/ExplorerPanel.tsx` Lines 77.35%，Functions 65.71%
- **证据类型**: 实证
- **问题**: `NavPageRow` 的点击/右键/状态同步分支近半未覆盖；`ExplorerPanel` 的加载错误占位、空态、右键菜单等分支未覆盖。这些组件承载项目→页面→会话的核心导航交互。
- **建议**: 补全 `NavPageRow` 交互与 `ExplorerPanel` 错误/空态分支的 L2 用例；`explorer-error-placeholder.test.tsx` 已覆盖 `useFileTree` 错误状态，但 `ExplorerPanel` 自身的容器级错误渲染未直接覆盖。

### [低] 窗口控制 wrapper 错误分支未覆盖
- **维度**: 覆盖率
- **证据**: `src/ipc/window.ts` Lines 72.72%，Functions 76.92%
- **证据类型**: 实证
- **问题**: `minimizeWindow`/`toggleMaximizeWindow`/`closeWindow` 等 wrapper 的 reject/异常捕获分支未触发。属于薄 wrapper，业务风险低，但标题栏关闭/最大化失败时无回归。
- **建议**: 补全 mock `invoke` reject 路径的 L2 用例，或因其为纯透传 wrapper 在豁免表中登记。

### [低] 侧栏视图定义无直接测试
- **维度**: 覆盖率
- **证据**: `src/features/sideViews/sideViewDefs.ts` Lines 60.00%，Functions 0.00%
- **证据类型**: 实证
- **问题**: 该文件导出默认视图配置（`DEFAULT_ZONES`/`DEFAULT_OPEN`/图标映射等），无针对其本身的测试；其正确性由 `sideBarState.test.ts` 等消费侧测试间接守护。风险低，但若修改默认视图 ID 或顺序，可能漏同步。
- **建议**: 补充常量形态与 ID 集合的 L2 快照/守卫用例。

## 缺口总表

| 模块/文件 | 覆盖层级 | 风险分级 | 备注 |
|-----------|----------|----------|------|
| `src-tauri/src/main.rs` | 无覆盖 | 高危 | panic hook + 入口；未在 inventory 登记豁免 |
| `src-tauri/src/lib.rs` | L4（部分）/ L1 低 | 高危 | `run()` 已豁免；setup 副作用无直接覆盖 |
| `src-tauri/src/pty/spawn.rs` | L1 + L4（部分） | 高危 | 大量 Win32/ConPTY 路径低覆盖 |
| `src-tauri/src/pty/conpty_api.rs` | L1 + L4（部分） | 高危 | vendor 提取/加载/回退未完全覆盖 |
| `src-tauri/src/pty/reader.rs` | L1 + 豁免 | 高危 | reader_loop 残余 I/O 分支已豁免，但剩余低覆盖 |
| `src-tauri/src/hooks/signal.rs` | L1 | 中危 | agent-event 处理分支不全 |
| `src-tauri/src/hooks/watcher.rs` | L1 | 中危 | 事件循环/目录重建分支不全 |
| `src-tauri/src/hooks/mod.rs` | L1 + L4（部分） | 中危 | 命令包装覆盖；setup/广播路径不全 |
| `src-tauri/src/git/mod.rs` | L1 | 中危 | 函数覆盖率 37.14%，需定位未执行函数 |
| `src/panels/terminal/useTerminalInstance.ts` | L2/L3 | 中危 | WebGL fallback/错误分支 |
| `src/panels/terminal/useXterm.ts` | L2/L3 | 中危 | 部分事件 handler 未触发 |
| `src/panels/terminal/usePtyResize.ts` | L2 | 中危 | resize 错误回调 |
| `src/workspace/DockviewHost.tsx` | L2 | 中危 | 多实例/就绪/错误分支 |
| `src/features/navTree/NavPageRow.tsx` | L2 | 中危 | 页面行交互近半未覆盖 |
| `src/features/explorer/ExplorerPanel.tsx` | L2 | 中危 | 容器级错误/空态分支 |
| `src/ipc/window.ts` | L2 | 低危 | wrapper reject 分支 |
| `src/features/sideViews/sideViewDefs.ts` | 无直接测试 | 低危 | 默认视图常量 |
| `src/panels/gitshow/GitShowPanel.tsx` | L2 | 低危 | 部分错误/加载分支 |
| `src/features/navTree/NavHistoryRow.tsx` | L2 | 低危 | 50% 行覆盖，标题/时间分支 |
| `src/features/navTree/NavProjectRow.tsx` | L2 | 低危 | 66.66% 行覆盖 |
| `src/features/navTree/NavSessionRow.tsx` | L2 | 低危 | 84.61% 行覆盖 |
| `src/features/navTree/NavTreeContextMenu.tsx` | L2 | 低危 | 部分菜单分支 |

> 其余生产文件覆盖率均高于 90% 或已被 L1/L2/L3/L4 多层覆盖，未列入本表。

## 豁免清单核查表

| 豁免条目 | 原因是否成立 | 兜底层级是否属实 | 结论 |
|----------|--------------|------------------|------|
| `reader_loop` 残余 I/O 编排分支 | 是（Mutex/RwLock/Channel/系统调用） | 是：`pty_integration_tests` 7 条 + `micro_batch_*` 6 条 + 前端直写阈值 256B | 维持 |
| `spawn_conpty_child` 纯 Win32 调用部分 | 是（Win32 API 组合） | 是：`pty_spawn_custom_conpty` 集成测试 + Windows CI runner | 维持 |
| `lib.rs` `run()` | 是（Tauri 运行时胶水） | 是：`terminal.e2e.ts` L4 启动标题等用例 | 维持；但 setup 副作用未单独兜底，建议细化 |
| ActivityBar 拖拽 mock 理想化 | 是（jsdom 无真实 hit-test） | 是：`activityBar.test.tsx` L2 + `sidebar.e2e.ts` L4 | 维持 |
| `E2E_ENABLED=false` 生产分支 | 是（编译期字面量 DCE） | 是：`e2e-build-config.test.ts` 8 条 + CI dist grep | 维持 |
| L3 生产 WebGL renderer / mouse tracking | 是（headless 无 GPU；PASSTHROUGH 假阴性） | 是：L4 全屏 TUI 视觉回归（M2 人工确认）+ `compute_conpty_flags` 7 条锁 0x7 | 维持 |
| L4 真实 OS 级按键 | 是（embedded WDIO 无法投递真实按键） | 是：合成事件 + 页面内 dispatch 全链路 | 维持；WDIO 支持真实输入后替换 |
| HTML postMessage 真实 WebView2 行为 | 是（jsdom 无法模拟 opaque origin/CSP） | 是：`html.e2e.ts` L4 + L2 四负面用例 | 维持 |
| mockcli 历史条目展示（L4） | 是（无 mockcli 后端 provider） | 是：L2 AC-4③ `mock-cli-profile.test.tsx` | 维持 |
| mockcli 双击恢复注入（L4） | 是（无 mockcli 后端 provider） | 是：L2 AC-4⑤ `mock-cli-profile.test.tsx` | 维持 |
| SEC-17 hooks user 层写入审计日志 | **部分成立** | 部分属实：L1 `config_write_sync_*` 锁死行为属实；「人工可观测」属实但非最佳 | **建议优化**：tracing target `"audit"` 可通过 `tracing-test` 或自定义 subscriber 在 L1 断言；当前豁免理由「L1 不可断言」不完全成立，建议补 L1 日志断言或明确登记为「可观测性而非行为正确性」 |

## 审查覆盖声明

- **审阅范围**: `src-tauri/src/` 全部 Rust 生产文件；`src/` 前端全部生产目录（`ipc`/`types`/`stores`/`workspace`/`panels`/`features`/`lib`/`theme`/`panelRegistry.ts`）；`.claude/test-inventory.md` 既定豁免清单 11 条。
- **执行命令与结果**:
  - `npm run test:coverage` → 154 files passed, 2635 tests passed，产出 v8 覆盖率报告。
  - `rustup component add llvm-tools-preview` → 已是最新。
  - `cargo install cargo-llvm-cov --locked` → 安装成功 `cargo-llvm-cov v0.9.0`。
  - `cargo llvm-cov --manifest-path src-tauri/Cargo.toml -- --test-threads=1` → 724 tests passed，TOTAL Lines 87.70%。
- **未改动代码**：本报告仅做分析，未新增/修改任何生产或测试代码；未提交 `coverage/` 目录（前端报告未生成该目录）。
