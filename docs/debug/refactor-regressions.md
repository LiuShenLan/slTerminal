# 重构回归问题排查分析（Stage1–11 合入后）

> 日期：2026-07-18 · 状态：**根因已定位（代码证据确定性）** · 本文档仅分析 + 修复建议，不含代码改动

## 1. 总览

### 症状 → 根因映射

| # | 用户症状 | 归属 | 根因 | 引入 Stage |
|---|---------|------|------|-----------|
| ① | 打开操作页面后文件浏览器恒显示"空目录" | 故障A | 路径沙箱 × React effect 顺序竞态，`fs_read_dir` 必在 `set_project_root` 前到达后端被拒 | Stage1 `ad66725` |
| ② | 无法打开文件验证页签 | 故障A 衍生 | 文件树为空，无文件可点 | — |
| ③ | 终端键盘输入完全无响应 | 故障B | 后端 `pty_write/resize/kill` 新增必填 `panel_id`，前端 `src/ipc/pty.ts` 未同步 | Stage2 `5cbbda5` |
| ④ | 无法打开 claude 验证 TUI | 故障B 衍生 | 按键写不进 PTY，claude 收不到任何输入 | — |

### 实测事实清单（用户双向实测）

| 事实 | 含义 |
|------|------|
| 运行形态：`npx tauri build --debug --no-bundle` 产物 exe | 生产前端构建，无 E2E helper |
| 侧栏项目/页面树显示正常，目录确有文件 | projects store 与持久化数据完好，排除数据层 |
| 终端有 PS 提示符 | `pty_spawn` + 输出链路（Channel→xterm）完好 |
| `Ctrl+Shift+C` 复制正常 | 快捷键注册表 + active 派发 + xterm 选中正常（不触 `pty_write`） |
| `Ctrl+Shift+V` 粘贴失败 | 粘贴走 `term.paste()` → `onData` → `pty_write` → 失败 |
| `Ctrl+滚轮` 改字号正常 | 本地 wheel 监听正常，与键盘/IPC 无关 |

### 调查方法

3 个并行 Explore agent（11 Stage 改动梳理 / 文件树链路 / 终端输入链路）+ 用户实测双向确认 + 关键代码行号一手核实（本文所有 `文件:行号` 均经核实）。

---

## 2. 故障A：文件浏览器恒"空目录"（问题①②）

### 2.1 加载链路图

```
SidebarTree 点击页面
  → Workspace.switchToPage (src/workspace/Workspace.tsx:84-94)
    → layout.setActivePage(pageId)
    → React 同一 commit 重渲染 Workspace（父）与 ExplorerPanel（子）
    │
    ├─【子 effect，先执行】ExplorerPanel.tsx:24-41 推导 rootPath = activePage.cwd || proj.rootPath
    │   → useFileTree effect (useFileTree.ts:208-233)
    │     → loadRoot(gen) → loadDirectory → readDir(rootPath)  (src/ipc/fs.ts:21-23)
    │       → invoke("fs_read_dir", { path })
    │         → 后端 fs_read_dir (src-tauri/src/fs/mod.rs:115-123)
    │           → validate_path_within_root(project_root, path)  ← project_root 仍是 None/旧值
    │           → Err("项目根路径未设置，拒绝文件访问")
    │     → loadDirectory catch { return [] }  (useFileTree.ts:54-56)  ← 静默吞掉
    │     → setRootNodes([]) → FileTree.tsx:647 `nodes.length===0 && rootPath` → 渲染"空目录"
    │
    └─【父 effect，后执行】Workspace.tsx:197-212
        → setProjectRoot(proj.rootPath)（fire-and-forget，未 await）
          → invoke("set_project_root") → 写入 AppState.project_root  ← 太迟，无人重试
```

**关键机制**：React 同一 commit 的 passive effect（`useEffect`）**子组件先于父组件**执行。因此每次 `activePageId` 变化，`fs_read_dir` 必然先于 `set_project_root` 到达后端——这不是概率性竞态，是**确定性时序**。

### 2.2 根因（确定性）

1. **Stage1 引入路径沙箱**：`validate_path_within_root`（`src-tauri/src/state.rs:103-142`）——`project_root=None` 时非 `cfg!(test)` 一律拒绝；目标在 root 外也拒绝。覆盖全部 10 个命令：`fs_read_file/write/read_dir/create_dir/delete/rename`（`fs/mod.rs:39,70,122,189,215,252`）、`notify_watch`（`notify/mod.rs:230`）、`git_status`（`git/mod.rs:137`）、`git_diff`（`git/mod.rs:206,208`）、`pty_spawn` 的 cwd（`pty/spawn.rs:780`）。
2. **时序确定性失败**：`setProjectRoot` 在父组件 Workspace 的 effect（`Workspace.tsx:197-212`），`readDir` 在子组件 useFileTree 的 effect（`useFileTree.ts:208-233`）——子先父后，`readDir` 必被拒。
3. **静默吞错 + 无重试**：`loadDirectory` 的 `catch { return [] }`（`useFileTree.ts:54-56`）把拒绝错误变成空数组，UI 显示"空目录"且无任何日志；`switchToPage` 对同页面 early-return（`Workspace.tsx:86`），同页永不重试。
4. **恢复路径也被同一根因堵死**：`startWatch`（`ExplorerPanel.tsx:63-69`）同样子 effect 先跑 → `notify_watch` 被拒 → watcher 未启动 → 无 fs-event → `refreshExpanded` 永不触发；`gitStatus` 同理失败（`useFileTree.ts:220` catch → 空 Map，无可见症状）。

**旁证**（与实测互洽）：终端 spawn 正常——用户在页面打开**之后**才点"+"创建终端，此时 `set_project_root` 早已完成，`pty_spawn` 的 cwd 校验通过。若 sandbox 本身有缺陷，终端也起不来。

### 2.3 候选原因排查表

| 候选 | 结论 | 依据 |
|------|------|------|
| 沙箱拒绝（主根因） | **确认** | 上述时序分析 + 代码证据链完整 |
| `setProjectRoot` 本身失败（目录不存在/参数名错） | 排除 | 前端发 `{ path }` 与后端 `set_project_root(path: String, ...)`（`state.rs:148`）匹配；终端 spawn 成功反证 project_root 最终被正确设置 |
| `activePage.cwd` 异常 | 排除（本次） | `rootPath = cwd \|\| rootPath`（`ExplorerPanel.tsx:36`）；即便 cwd 异常，主时序问题仍必然复现；修复后若仍空再查持久化 JSON 的 `pages[*].cwd` |
| generation 竞态丢结果 | 排除 | gen 机制只在 rootPath 快速连变时丢旧结果（`useFileTree.ts:69`），本次是首次加载即失败 |
| ExplorerPanel 未挂载 / rootPath 为 null | 排除 | 显示的是"空目录"（`nodes.length===0 && rootPath`，FileTree.tsx:647）而非"选择一个项目"——rootPath 已推导成功 |
| `projects.ts` 的 `switchToPage` 未走 | 次要问题 | 生产 UI 只走 `Workspace.switchToPage`（不调 `projects.switchToPage`），`setProjectRoot` 责任分散两处，属设计债非本次根因 |

### 2.4 为何四级测试都没拦住

| 层级 | 盲区 |
|------|------|
| L1 Rust | `validate_path_within_root` 对 `None` 有 `cfg!(test)` 豁免（`state.rs:108`）——单测里 sandbox 形同虚设；豁免本身的测试（`state.rs:267`）还锁死了"None 放行" |
| L2 前端 | `../ipc/fs` 全 mock（`setup.ts` 全局 mock），`readDir` 永远成功，沙箱拒绝路径不可达 |
| L3 | 只测 xterm 渲染，不涉及 |
| L4 E2E | 无用例断言文件树内容；E2E 建项目后终端 spawn 成功（时序靠操作间隔天然拉开），恰好像真实用户一样绕过了竞态 |

### 2.5 验证步骤（实证确认，可选）

1. debug exe 中右键 → Inspect 打开 DevTools，切页面后看 console 是否有 `[slTerminal] 设置项目根路径失败:` 或 `启动文件监听失败:`（后者大概率存在）。
2. 临时在 `useFileTree.ts:54` 的 catch 加 `console.error("readDir failed:", err)`，重新构建 → 应看到 `项目根路径未设置，拒绝文件访问`。
3. 临时把 `Workspace.tsx:205` 改为 `await setProjectRoot(...)` 并在其后手动 `dispatchEvent` 触发刷新——树立即出现即确证。

### 2.6 修复建议（详细，不实施）

**方案A1（推荐）：`setProjectRoot` 前置为"切换页面"动作的同步前置条件**

语义：`project_root` 是 `activePageId` 生效的前提，不是它的副作用。

| 改动点 | 文件:位置 | 改法 |
|--------|----------|------|
| ① 切换前置 | `src/workspace/Workspace.tsx:84-94` `switchToPage` | 改为 async：先从 `useProjects.getState()` 查 pageId 所属项目的 `rootPath`，`await setProjectRoot(rootPath)`（失败 console.error 并中止切换或继续降级），再 `setActivePage(pageId)` |
| ② 启动恢复同样处理 | `src/App.tsx:60-63` localStorage 恢复 lastPage 处 | `setActivePage(lastPage)` 前同样先 `await setProjectRoot(对应项目 rootPath)` |
| ③ 兜底保留 | `Workspace.tsx:197-212` SEC-01 effect | 保留（服务 `pty_spawn` 等其他消费者 + E2E helper 直设 activePageId 的路径）；幂等（`prevRootRef` 防重复），与①不冲突 |
| ④ E2E helper 对齐 | `e2e-tests/helpers.ts` 中 `__slterm_e2e_createProject` / `__slterm_e2e_switchToPage` | 直设 `activePageId` 前同样 `await setProjectRoot`（否则 L4 对新代码路径失真） |

时序修正后：用户点页面 → `await setProjectRoot` 完成 → `setActivePage` → ExplorerPanel 加载 → `readDir` 校验必过。

**方案A2（备选）：useFileTree 加载前自行 await**

- `ExplorerPanel.tsx:43` 把 `projectRootPath` 一并传入 `useFileTree`；`useFileTree.ts:208-233` effect 内先 `await setProjectRoot(projectRootPath)` 再 `loadRoot(gen)`，`startWatch` effect 同样前置。
- 缺点：只覆盖文件树一条链路，`pty_spawn`/`git_status` 在"页面刚切换立即开终端"场景仍可能竞态；`setProjectRoot` 调用点更分散。**不如 A1 彻底**。

**方案A3（必配，无论选哪个）：可观测性补齐**

| 改动点 | 文件:位置 | 改法 |
|--------|----------|------|
| 静默吞错加日志 | `useFileTree.ts:54-56` | `catch (err) { console.error("[slTerminal] readDir 失败:", dirPath, err); return []; }` |
| gitStatus 吞错加日志 | `useFileTree.ts:229-232` | 同上，catch 内加 `console.error` |

**涉及测试改动**：

| 测试文件 | 改法 |
|----------|------|
| `src/__tests__/workspace*.test.tsx` | `switchToPage` 变 async 后，相关用例需 `await` 并 mock `setProjectRoot`；新增用例：`setProjectRoot` 先于 `setActivePage` 调用（调用顺序断言） |
| `src/__tests__/explorer-*.test.tsx` | mock 的 `../ipc/fs` 需包含 `setProjectRoot`；无需改断言（行为对齐后 readDir 照常 resolve） |
| 新增 L2 回归用例（建议） | `explorer-sandbox-race.test.tsx`：模拟 `setProjectRoot` 未完成时 `readDir` reject → 断言不出现"空目录"误态（或断言重试/恢复行为，取决于 A1/A2） |

**风险与取舍**：A1 让页面切换多等一个 IPC 往返（~ms 级，本地无感）；`setProjectRoot` 失败时的降级策略需定夺（建议：console.error + 仍继续切换，Explorer 显示"空目录"但至少终端可用——与现状相比无退化）。

---

## 3. 故障B：终端键盘输入完全无响应（问题③④）

### 3.1 输入链路图

```
键盘 keydown
  → xterm attachCustomKeyEventHandler (useXterm.ts:226-234)
    → ShortcutRegistry.resolve(event, "terminal") — 普通字符未命中命令 → 透传 ✓
  → xterm 处理按键 → term.onData (useXterm.ts:334-342)
    → TerminalRegistry.get(panelId)?.sessionId ✓（有值，spawn 成功已注册）
    → pty.write(sid, bytes)  (src/ipc/pty.ts:27-35)
      → invoke("pty_write", { sessionId, data })   ← 缺 panelId！
        → 后端 pty_write(state, session_id, panel_id, data)  (spawn.rs:974-979)
        → Tauri 反序列化失败：missing field `panel_id` → invoke reject
    → .catch(err => console.error("PTY write 失败:", err))  ← 静默吞掉
  → shell 永远收不到字节 → 无回显 → "键盘无响应"
```

### 3.2 根因（确定性）

Stage2（`5cbbda5`）给后端三条命令加了必填 `panel_id` 并做 SEC-08 归属校验，**前端 IPC 层未同步**：

| 命令 | 后端签名（spawn.rs） | 前端 payload（ipc/pty.ts） | 结果 |
|------|---------------------|---------------------------|------|
| `pty_write` | `(state, session_id, panel_id, data)` :974-979 | `{ sessionId, data }` :31-34 | 每次按键必败 |
| `pty_resize` | `(state, session_id, panel_id, cols, rows)` :1019-1025 | `{ sessionId, cols, rows }` :43 | 每次 resize 必败 |
| `pty_kill` | `(state, session_id, panel_id)` :1069-1073 | `{ sessionId }` :48 | 每次 kill 必败 → 子进程泄漏（Job Object 兜底） |

`pty_spawn`/`pty_reattach` 不受影响（spawn 经 `SpawnRequest.panel_id` 已带归属，`spawn.rs:738`；reattach 无 `panel_id` 参数）——所以终端能启动、能显示，只是写不进、缩不了、杀不掉。

### 3.3 实测事实互洽表

| 实测事实 | 解释 |
|----------|------|
| 有 PS 提示符 | `pty_spawn` 不含本次断裂参数，输出 Channel 正常 |
| 打字无回显 | `pty_write` 缺 `panelId` 必败，字节到不了 shell |
| `Ctrl+Shift+V` 粘贴失败 | `keyboard.ts:32-39` → `t.paste(text)`（`useXterm.ts:162`）→ xterm 内部触发 `onData` → 同一条断裂的 `pty_write` |
| `Ctrl+Shift+C` 复制正常 | 只读 xterm 选区 + 写系统剪贴板，不触 `pty_write` |
| `Ctrl+滚轮` 正常 | `useFontSizeWheel` 本地改字号，无 IPC |

### 3.4 波及面清单（所有断裂调用点）

| 调用点 | 文件:行号 | 场景 |
|--------|----------|------|
| `writeToPty` | `useXterm.ts:152-157` | 快捷键写入（`terminal.newline` 的 Ctrl+Enter） |
| E2E 写入 helper | `useXterm.ts:247-253` | `__e2e_writeToPty` |
| `onData` | `useXterm.ts:334-342` | **键盘输入主路径** |
| cleanup kill | `useXterm.ts:355-358` | 面板卸载杀 session |
| 字号变化 resize | `useXterm.ts:403-407` | Ctrl+滚轮改字号后 |
| 行变化 resize | `usePtyResize.ts:89` | 拖拽改高度 |
| 列变化 resize | `usePtyResize.ts:102` | 拖拽改宽度 |
| 关窗 kill 全部 | `App.tsx:83-91` | P1-19 窗口关闭清理（当前必败，靠 Job Object 兜底） |

### 3.5 测试盲区

- L2 `src/__tests__/ipc-contract.test.ts:72-113` 三条用例的期望 payload 本身就缺 `panelId`——测试与前端一起错，互为"印证"；`:145-169` 异常传播用例同样用旧签名。
- L1 Rust 测试直接调命令函数，不经过 Tauri 反序列化层——参数缺失在编译期就被类型系统满足（测试内手动传全参），暴露不了前端少传。
- 没有任何一层做"前端 payload ↔ 后端命令签名"的机械对账。

### 3.6 验证步骤（实证确认，可选）

1. debug exe 打开 DevTools，按任意键 → console 应见 ``PTY write 失败`` 且错误信息含 missing field `panel_id`（Tauri 参数反序列化错误）。
2. 修复后同一动作 console 无错误，终端出现回显即确证。

### 3.7 修复建议（详细，不实施）

**Step 1：`src/ipc/pty.ts` 三个 wrapper 补 `panelId`**（硬约束 #4：JS `panelId` ↔ Rust `panel_id`，Tauri 自动转 snake_case）

```ts
export async function write(sessionId: string, panelId: string, data: Uint8Array): Promise<void> {
  await invoke("pty_write", { sessionId, panelId, data: Array.from(data) });
}
export async function resize(sessionId: string, panelId: string, cols: number, rows: number): Promise<void> {
  await invoke("pty_resize", { sessionId, panelId, cols, rows });
}
export async function kill(sessionId: string, panelId: string): Promise<void> {
  await invoke("pty_kill", { sessionId, panelId });
}
```

**Step 2：调用点逐一同步**（panelId 均为现成在作用域内的值，零新增依赖）

| 文件:行号 | 改法 |
|-----------|------|
| `useXterm.ts:155` | `pty.write(sid, panelId, data)`（panelId 已是 `writeToPty` 的 useCallback dep） |
| `useXterm.ts:250` | E2E helper 内 `pty.write(sid, panelId, ...)` |
| `useXterm.ts:337` | onData 内 `pty.write(sid, panelId, ...)` |
| `useXterm.ts:357` | `pty.kill(entry.sessionId, panelId)` |
| `useXterm.ts:405` | `pty.resize(sid, panelId, dims.cols, dims.rows)` |
| `usePtyResize.ts:89` | `pty.resize(sid, panelId, dims.cols, dims.rows)`（panelId 是 hook 入参） |
| `usePtyResize.ts:102` | `pty.resize(currentSid, panelId, dims.cols, dims.rows)` |
| `App.tsx:83-86` | `allSessions.forEach((entry, panelId) => … pty.kill(entry.sessionId, panelId))`（Map key 即 panelId，无需改 `RegisteredTerminal` 结构） |

**Step 3：测试同步**

| 文件:行号 | 改法 |
|-----------|------|
| `ipc-contract.test.ts:72-113` | 三条用例改为带 panelId 调用并断言 payload 含 `panelId`（如 `pty.write('session-1','panel-1',data)` → 期望 `{ sessionId, panelId: 'panel-1', data }`） |
| `ipc-contract.test.ts:145-169` | 三条异常传播用例调用签名同步补 panelId |
| 其他 mock `../ipc` 或 `../../ipc` 的测试 | 全仓 grep `pty.write(/pty.resize(/pty.kill(`，mock 场景若断言调用参数则同步；仅 stub 不关心签名的无需动 |

**Step 4：验证**

1. `npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿
2. `npx tauri build --debug --no-bundle` 后实测：打字回显、Ctrl+Shift+V 粘贴执行、拖拽分屏 resize 后 TUI 无错位、关窗后无 pwsh/cmd 残留进程
3. （建议顺手补）L2 新增一条契约守卫：对 `pty_*` 命令断言 payload keys 与后端参数表一致（防再次单边改契约）

**风险**：零后端改动；`panelId` 在所有调用点都是确定值（闭包/hook 入参/Map key），无 null 分支。唯一注意点是参数顺序——建议统一 `(sessionId, panelId, ...)` 与后端阅读顺序一致。

---

## 4. 修复优先级与后续

1. **先修故障B**（Step 1–4）：单点契约补齐，改动 ~15 行 + 测试，确定性恢复输入/粘贴/resize/kill → 问题③④解决，可立刻验证 claude TUI。
2. **再修故障A**（方案A1 + A3）：涉及切换动作语义调整，改动面稍大，需同步 E2E helper 与新增回归用例 → 问题①②解决。
3. **防复发沉淀**：本次两故障同属"跨边界契约/时序靠人肉对齐"失效——建议把「后端命令参数表 ↔ 前端 payload」对账断言与「`set_project_root` 先行」时序写入 `code-refactor` skill 的契约写死清单，并在 `src-tauri/src/fs/CLAUDE.md` 记录 sandbox 对前端加载时序的要求。
