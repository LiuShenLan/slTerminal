# Review: 架构硬约束合规

## [高] 违反约束 #3：PTY spawn/write/resize 命令未使用 spawn_blocking

**位置**
- `D:\data\learn\code\slTerminal\src-tauri\src\pty\spawn.rs:600`：`pub fn pty_spawn(...)`
- `D:\data\learn\code\slTerminal\src-tauri\src\pty\spawn.rs:764`：`pub fn pty_write(...)`
- `D:\data\learn\code\slTerminal\src-tauri\src\pty\spawn.rs:785`：`pub fn pty_resize(...)`

**问题**
三个命令均为同步 `pub fn`，直接执行 ConPTY/PTY 阻塞 I/O：
- `pty_spawn` 调用 `CreatePseudoConsole`、`CreateProcessW`、`take_writer`、`write_all/flush`（:622-:683）
- `pty_write` 调用 `writer.write_all(&data)`、`writer.flush()`（:774-:775）
- `pty_resize` 调用 `master.resize(...)`（:797-:803）

项目其他模块（fs、git、settings）及 `pty_kill` 均遵循 `async + tokio::task::spawn_blocking` 模式，唯独这三个 PTY 命令将阻塞逻辑放在同步命令中执行，违反“阻塞 I/O 用 spawn_blocking”。

**建议**
将 `pty_spawn`、`pty_write`、`pty_resize` 改为 `pub async fn`，把阻塞部分移入 `tokio::task::spawn_blocking`。

## [中] 违反约束 #3：notify_watch 同步命令含阻塞 I/O

**位置**
- `D:\data\learn\code\slTerminal\src-tauri\src\notify\mod.rs:213-252`：`pub fn notify_watch(...)`

**问题**
`notify_watch` 为同步命令，内部执行 `watch_path.exists()`（:220）并调用 `FileWatcher::start`（:247），涉及文件系统与 `notify` 初始化等阻塞操作，未使用 `spawn_blocking`。

**建议**
改为 `pub async fn`，将路径存在检查与 watcher 启动的阻塞部分包裹进 `spawn_blocking`。

## [中] 违反约束 #6：多处硬编码颜色值

**位置**
- `D:\data\learn\code\slTerminal\src\App.css:5-8`
  ```css
  --sl-bg-primary: #1e1e2e;
  --sl-bg-secondary: #2b2b3c;
  --sl-fg-primary: #cdd6f4;
  --sl-fg-secondary: #a6adc8;
  ```
- `D:\data\learn\code\slTerminal\src\features\explorer\FileTree.tsx:71`
  ```tsx
  boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
  ```
- `D:\data\learn\code\slTerminal\src\features\sidebar\SidebarTree.tsx:82`
  ```tsx
  boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
  ```

**问题**
颜色值未集中在 `src/theme/colors.ts` 定义，而是直接硬编码于全局 CSS 与组件内联样式，违反“配色单点”。

**建议**
在 `theme/colors.ts` 增加对应 token，组件/CSS 统一引用 token；阴影可考虑新增 `SHADOW` token。

## [中] 违反约束 #8：面板 hook 自建 sessionId 副本书写 PTY

**位置**
- `D:\data\learn\code\slTerminal\src\panels\terminal\useXterm.ts:140`
  ```ts
  const sessionIdRef = useRef<string | null>(null);
  ```
- `D:\data\learn\code\slTerminal\src\panels\terminal\useXterm.ts:279-284`
  ```ts
  .then((sessionId) => {
    sessionIdRef.current = sessionId;
    // 注册到 TerminalRegistry（跨页面切换时可供 reattach 查询）
    TerminalRegistry.register(panelId, {
      term,
      sessionId,
      ...
    });
  })
  ```
- `D:\data\learn\code\slTerminal\src\panels\terminal\useXterm.ts:155-158`、`250-251`、`337-339`、`400-401`
- `D:\data\learn\code\slTerminal\src\panels\terminal\usePtyOutput.ts:238`
  ```ts
  ptySessionId.current = null;
  ```
- `D:\data\learn\code\slTerminal\src\panels\terminal\usePtyResize.ts:69`、`:99`
  ```ts
  const sid = ptySessionIdRef.current;
  const currentSid = ptySessionIdRef.current;
  ```

**问题**
`useXterm` 在 `TerminalRegistry` 之外维护了一份面板级 `sessionIdRef`，并把该 ref 透传给 `usePtyOutput`、`usePtyResize` 用于 `pty.write/resize`；`usePtyOutput` 还在 exit 时直接修改该 ref。这导致 PTY 会话 ID 同时存在于 `TerminalRegistry` 和多个 hook 的本地 ref 中，面板不仅“订阅”还“自存”会话元数据，违反“会话元数据单点”。

**建议**
`useXterm` 不应自建 `sessionIdRef`；需要 sessionId 时从 `TerminalRegistry.get(panelId).sessionId` 读取，或让子 hook 通过 panelId 自行查询。`usePtyOutput` 不应直接写 sessionId ref，退出状态应仅通知 `TerminalRegistry` 或触发重连回调。

## [低] 违反约束 #9：#[cfg(windows)] 扩散到非 pty 模块

**位置**
- `D:\data\learn\code\slTerminal\src-tauri\src\state.rs:343`：`#[cfg(windows)] fn validate_symlink_outside_root_rejected`
- `D:\data\learn\code\slTerminal\src-tauri\src\state.rs:367`：`#[cfg(windows)] fn validate_root_is_symlink`
- `D:\data\learn\code\slTerminal\src-tauri\src\fs\mod.rs:555`：`#[cfg(windows)] assert!(...)`
- `D:\data\learn\code\slTerminal\src-tauri\src\fs\mod.rs:557`：`#[cfg(not(windows))] assert!(...)`
- `D:\data\learn\code\slTerminal\src-tauri\src\lib.rs:109`：`#[cfg(windows)] { ... }`
- `D:\data\learn\code\slTerminal\src-tauri\src\lib.rs:119`：`#[cfg(not(windows))] { ... }`

**问题**
`#[cfg(windows)]` / `#[cfg(not(windows))]` 出现在 `state.rs`、`fs/mod.rs`、`lib.rs` 的测试代码中，超出“仅允许在 pty/ 模块明确处”的范围。

**建议**
将平台相关测试逻辑收敛到 `pty/` 模块，或在非平台关键路径改用运行时 `cfg!(windows)` 判断。

## 约束核查结论

| 约束 | 结论 |
|------|------|
| #1 IPC 单点 | 已核查无违规 |
| #2 后端模块隔离 | 已核查无违规 |
| #3 命令统一注册 | 有 4 处违规（PTY spawn/write/resize + notify_watch 未用 spawn_blocking） |
| #4 DTO 双边对应 | 已核查无违规 |
| #5 面板封闭 | 已核查无违规 |
| #6 配色单点 | 有 3 处违规 |
| #7 布局单点 | 已核查无违规 |
| #8 会话元数据单点 | 有 1 处违规（useXterm/usePtyOutput/usePtyResize 自建 sessionId ref） |
| #9 平台分支收敛 | 有 6 处违规 |
| #10 权限最小化 | 已核查无违规 |
