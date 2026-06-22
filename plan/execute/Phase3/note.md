# Phase 3 前置笔记

> 记录 Phase 2→Phase 3 的边界划分决策与前后文，供 Phase 3 开工时参考。

---

## H6 终端会话保留——Phase 2/3 边界

### 问题

终端会话在页面切换后不保留。Phase 2 审计（`plan/execute/Phase2/9.验收审计与根因分析.md` §1）识别出 5 层连锁根因：

| 层 | 根因 | 位置 |
|----|------|------|
| 1 | Panel ID 不稳定（`Date.now()`）→ `reuseExistingPanels` 永不匹配 | `Workspace.tsx:67,90,119,257` |
| 2 | `term.dispose()` 无 `isLayoutSwitching` 保护 | `useXterm.ts:307-309` |
| 3 | Tauri Channel 绑定单次 `invoke`，reader 线程退出后无法重连 | `pty.ts:16-19` + `reader.rs:40-43` |
| 4 | `TerminalRegistry` 模块不存在（worktree 删除时误删） | `useXterm.ts:299,304` |
| 5 | 每次挂载无条件 `pty.spawn()`——无已有 session 查询 | `useXterm.ts:198-209` |

### Phase 2 修复范围（前端 Term 实例存活）

Phase 2 解决第 1、2、4、5 层——让 xterm.js `Terminal` 实例在页面切换时不销毁：

1. **A1——创建 TerminalRegistry 模块**
   - 模块级 `Map<panelId, { term, sessionId, webglAddon, fitAddon }>`
   - 暴露 `register/get/remove/has` 方法
   - Terminal 实例生命周期独立于 React 组件生命周期

2. **A2——useXterm 查 Registry 再决定 spawn**
   - mount 时先查 Registry——若有已有 session 则复用 Term 实例（`term.open(el)`）
   - 若无才 `pty.spawn()`

3. **A3——cleanup 中跳过 `term.dispose()`（当 `isLayoutSwitching=true`）**
   - 区分两种 unmount：
     | 场景 | `isLayoutSwitching` | 行为 |
     |------|---------------------|------|
     | 页面切换 | `true` | 不 kill、不 dispose、存回 Registry |
     | 用户关闭面板 | `false` | `pty.kill()` + `term.dispose()` + `Registry.remove()` |
     | 删除操作页面 | `false` | 同上（`api.clear()` 在 `onDeletePage` 中主动调用） |
   - 时序验证：`fromJSON` 同步调用 `clear()` → React cleanup 在同步调用栈中执行 → 此时 `isLayoutSwitching` 仍为 `true`

4. **A4——稳定 Panel ID**
   - `Date.now()` → `terminal-${pageId}-${seq}`（确定性）
   - 全局 per-page 自增计数器

5. **A5——`renderer: 'always'` 全局启用**
   - Dockview 使用 `visibility: hidden` 而非 `display: none`
   - 保留 WebGL 上下文

### Phase 3 修复范围（Channel 重连）

Phase 2 **不解决**第 3 层（Channel 重连）。

**原因**：

- reader 线程（`reader.rs:18-52`）启动时 `move` 消费 `Channel<PtyEvent>`
- 前端 Panel unmount → JS Channel 被 GC → Rust `channel.send()` 返回 `Err` → reader 线程退出（`reader.rs:40-43`）
- 即使 `isLayoutSwitching` 保护了 PTY 进程（`pty.kill` 不执行），reader 线程已退出，输出无法送达前端
- Tauri v2 Channel 不提供 `Clone` 或重新绑定 API——无官方重连机制
- **连 Tabby（66k stars）也不支持 live PTY reattach**——关闭标签 = 杀进程

**Phase 2 的务实折衷**：

```
页面 A → 切到页面 B → 切回页面 A:
  1. TerminalRegistry.get(panelId) 返回旧 Term 实例 ✅
  2. term.open(el) 重新挂载 DOM ✅（xterm.js 允许 second open，仅更新 window 引用）
  3. pty.spawn() 创建新 PTY session ❌（旧 session 残留）
  4. 新 Channel 绑定新 session ✅
  5. 用户看到：布局恢复 + 新终端（空白）⚠️
```

与修复前对比：
- 修复前：布局空白 + 新终端
- Phase 2 后：布局恢复 + 新终端（用户体验改进，但终端内容不保留）

### Phase 3 需要的后端能力

| # | 功能 | 说明 |
|---|------|------|
| P3-1 | `pty_reattach` 命令 | 前端传入 `sessionId` + 新 `Channel<PtyEvent>`，Rust 将 Channel 绑定到已有 reader 线程。需要 `Arc<RwLock<Option<Channel<PtyEvent>>>>` 包装 reader 线程的 Channel 引用 |
| P3-2 | Ring buffer 回放 | 后端缓存最近 N 行输出（如 1000 行），重连时通过 Channel 回放第一帧。用户看到上次内容恢复 |
| P3-3 | 延迟 Canvas 释放 | Tabby `visibility.ts` 模式：不可见时先保留 WebGL，30s 超时后释放。结合 Ring buffer 可在超时后回放 |

### 完整修复链路（Phase 2 + Phase 3）

```
页面 A → 切到页面 B → 切回页面 A:

Phase 2:
  1. Panel ID 匹配（stable ID）→ reuseExistingPanels 保护 → 面板不销毁 ✅
  2. 或：面板销毁但 isLayoutSwitching=true → cleanup 跳过 dispose → 存回 Registry ✅
  3. 挂载时查 Registry → 取回旧 Term 实例 → term.open(el) ✅
  4. 无已有 session → pty.spawn() 新 session ⚠️

Phase 3:
  5. Registry 中有 sessionId → pty_reattach(sessionId, newChannel) ✅
  6. Ring buffer 回放第一帧 → 终端内容恢复 ✅
  7. 后续输出通过新 Channel 送达 ✅
```

---

## 其他 Phase 2 问题——无 Phase 3 依赖

S1（删除页面残留）、S2（重命名卡死）、S4（重启空白）的修复均在 Phase 2 内完整解决，不依赖 Phase 3 基础设施。详见 `plan/execute/Phase2/9.验收审计与根因分析.md` §2-§4。
