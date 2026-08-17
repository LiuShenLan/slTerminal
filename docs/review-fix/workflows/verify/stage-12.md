# Stage 12 逐项验证断言（唯一真值源）

> stage-12 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-01**：`src/stores/projects.ts` 存在 `MAX_PAGES = 20`（grep 命中）；超限 addPage 拒绝 + toast.show（Read 确认）；`src/workspace/Workspace.tsx` 多 Dockview 实例逻辑保留（语义式：allPages 渲染结构未改单实例——H6 未动，仅加上限）
- **FE-15**：`useFileTree.ts` 的 file-saved 事件存在 300ms debounce（grep 300/debounce 命中）；已知路径变更只刷新受影响子树（语义式，须 Read 确认：定位最近展开祖先刷新，非全量 refreshExpanded）
- **FE-16**：`useNavTree.ts` 存在历史归属 Map 索引（语义式：Map<projectId, ...> 结构替代 O(N×M) 前缀匹配——Read 确认）
- **FE-17**：`TerminalPanel.tsx` 订阅回调存在 panelId 相等过滤（grep panelId 比较命中）
- **FE-19**：`src/ipc/agentHistory.ts` wrapper 含 `force?: boolean` 参数（Read 确认）；`useNavTree.ts` 挂载一次扫描、展开历史节点不重复 scan（语义式，须 Read 确认）
- **FE-20**：`src/App.tsx` 字体/快捷键/侧栏三 loadFromDisk 在 `Promise.all` 内（grep 命中）；loadAllProjects 仍在其后（Read 确认时序）
- **FE-21**：`SideBarArea.tsx` 隐藏视图按需卸载（语义式：条件渲染/不渲染替代 display:none 保挂载——Read 确认；仍全量挂载判 not_fixed）
- **FE-29**：`grep "transition" src/panels/terminal/TerminalPanel.tsx` 于遮罩处零命中（或确认残留 transition 与遮罩无关）
- **FE-32**：TerminalPanel 的 useLayout/useFontSize 改 selector 精确订阅（语义式，须 Read 确认传参 selector 仅取所需字段）
- **FE-33**：Workspace.tsx pageCallbacksRef 回调按 pageId 惰性创建 + 缓存（语义式：getOrCreate 模式——Read 确认）
- **FE-34**：焦点切换不再主动释放 WebGL 上下文（语义式，须 Read 确认释放逻辑已移除或仅失败回退路径保留）；**禁区核对**：failIfMajorPerformanceCaveat 相关检测逻辑零改动（git diff 确认）
- **BE-19**：`src-tauri/src/agent_history/claude/scan.rs` 存在缓存键结构（语义式：目录 mtime + 文件数）；`agent_history/mod.rs` 的 `agent_history_scan` 含 `force` 参数（grep 命中）；新增 L1 用例存在（缓存命中/失效/force 绕过）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
