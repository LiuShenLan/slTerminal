# Review: 安全审查

审查范围：路径沙箱、命令注入、HTML 预览、IPC 攻击面、敏感信息、构建配置、E2E helper 门控。

---

## [严重级别:高] project_root 从未被后端设置，路径沙箱完全失效

- **位置**
  - `src-tauri/src/state.rs:73/89/99-102`
  - 全代码库无 `project_root.write()` / `project_root = ...` 写入点
  - `e2e-tests/test.e2e.ts:450` 注释明确说明“后端 project_root 未设置 → 路径 sandbox 跳过”

- **威胁场景**
  任意前端上下文（包括通过 XSS 或恶意扩展注入的脚本）调用受沙箱保护的 `fs_*` / `notify_watch` 命令时，后端 `validate_path_within_root` 因 `project_root == None` 直接返回 `Ok(())`，不再校验目标路径。

- **问题**
  `AppState.project_root` 只被初始化为 `None`，没有任何命令或启动逻辑在打开项目时将其设置为真实项目根目录。这导致所有依赖该字段的校验（`fs_read_file`、`fs_write_file`、`fs_create_dir`、`fs_delete`、`fs_rename`、`notify_watch`）在形式上存在、实际上完全失效，可被用于读取/写入/删除/监听沙箱外任意路径。

- **建议**
  在项目打开/切换时通过 Tauri 命令写入 `project_root`（如新增 `set_project_root` 命令，并在前端 `useProjects` 切换项目时调用）；移除“`None` 跳过校验”的兼容逻辑，或仅在测试配置下允许跳过。

---

## [严重级别:高] `fs_read_dir` 完全没有路径沙箱校验

- **位置**
  - `src-tauri/src/fs/mod.rs:113-166`

- **威胁场景**
  任何能触发 IPC 的前端代码都可以直接调用 `fs_read_dir("C:/Windows/System32")` 等任意路径。

- **问题**
  `fs_read_dir` 命令签名没有接收 `AppState`，也未调用 `validate_path_within_root`。其余 `fs_*` 命令虽然调用了校验函数，但因上一条问题（`project_root` 未设置）同样失效。目录枚举是信息泄露和进一步攻击（发现敏感文件、SSH 密钥等）的第一步。

- **建议**
  给 `fs_read_dir` 增加 `state: State<'_, AppState>` 参数并调用 `validate_path_within_root`；同时修复 `project_root` 未设置问题，使沙箱真正生效。

---

## [严重级别:高] `pty_spawn` 允许前端指定任意 shell 路径与工作目录

- **位置**
  - `src-tauri/src/pty/spawn.rs:599-604/611/669`
  - `src-tauri/src/pty/spawn.rs:80-98` (`build_cmdline`)
  - `src-tauri/src/pty/shell.rs:50-65` (`resolve_shell_info`)

- **威胁场景**
  前端通过 `pty.spawn({ shell: "C:/Windows/System32/calc.exe", cwd: "C:/Users/Attacker" })` 即可让后端直接 `CreateProcessW` 启动任意可执行文件。

- **问题**
  `SpawnRequest.shell` 和 `cwd` 原样传到 `resolve_shell_info` / `spawn_conpty_child`，既未校验可执行文件是否属于允许列表（`pwsh/powershell/cmd`），也未校验 `cwd` 是否位于项目根目录内。`build_cmdline` 虽对空格参数做了引号处理，但无法阻止“指定任意程序”这一根本风险。结合 Tauri 2 默认放行所有自定义命令，任何前端代码获得执行能力后等于本地任意代码执行。

- **建议**
  1. 限制 `shell` 只能是内部允许列表中的程序（或通过 PATH 解析的 `pwsh/powershell/cmd`），拒绝用户传入任意路径。
  2. 对 `cwd` 调用 `validate_path_within_root` 并确保其在项目根内。
  3. 考虑在 `build_cmdline` 中拒绝包含 `&`, `|`, `>`, `<`, `\n`, `\r` 等 shell 元字符的参数（当前参数固定，但未来扩展时容易遗漏）。

---

## [严重级别:中] HTML 预览 iframe 的 postMessage 键盘桥未校验来源

- **位置**
  - `src/panels/html/HtmlPanel.tsx:45-60`（注入脚本 `postMessage(..., "*")`）
  - `src/panels/html/HtmlPanel.tsx:88-112`（`handleMessage` 未检查 `e.origin`）

- **威胁场景**
  用户预览的不可信 HTML 文件内嵌脚本可以向父窗口发送 `slterm_key` postMessage，只要指纹匹配任意 `global` 上下文绑定即可触发对应命令。

- **问题**
  虽然 iframe 使用 `sandbox="allow-scripts"` 且不含 `allow-same-origin`（跨源，无法直接访问父窗口 JS 对象），但跨源 iframe 仍然可以调用 `window.parent.postMessage`。父窗口的 `handleMessage` 未验证 `e.origin`，注入脚本使用 `targetOrigin: "*"`，因此 iframe 内容可以伪造键盘事件指纹，触发当前 `global` 快捷键（目前至少包括 `global.closeTab`，未来若增加更多 global 命令则风险扩大）。这属于不可信项目内容直接触发主应用行为的攻击面。

- **建议**
  - 在 `handleMessage` 中仅接受来自预期来源的消息；`srcDoc` iframe 为 opaque origin，可将 `e.origin` 与 `"null"` 进行严格比对（需同时确认 WebView2 行为）。
  - 注入脚本的 `postMessage` 不应使用 `"*"`，而是 `"null"` 或更严格的 origin。
  - 对重放的 `KeyboardEvent` 增加额外标记，确保只能来自受信任的 iframe 键盘桥。

---

## [严重级别:中] 全局 CSP 为 HTML 预览放宽 `script-src 'unsafe-inline'`

- **位置**
  - `src-tauri/tauri.conf.json:24-25`

- **威胁场景**
  主窗口将来若出现任何 XSS 注入点（恶意文件名渲染、Markdown/HTML 内容未正确转义、第三方依赖漏洞等），攻击者可直接执行内联脚本。

- **问题**
  为让 `srcDoc` iframe 中的内联脚本运行，全局 CSP 设为 `script-src 'self' 'unsafe-inline'` 并关闭 `script-src` 的 nonce 注入。这是有文档记录的设计权衡，但代价是主应用失去 CSP 对内联脚本的主要防护。当前主应用输入面（xterm ANSI、CodeMirror 纯文本、React 转义）虽无已知注入路径，但“全局放宽”增加了未来 XSS 的影响面。

- **建议**
  如果业务上必须保持该策略，应在 CLAUDE.md 中持续维护“已知放宽项”清单，并在每次引入新渲染面（如 Markdown 预览、通知弹窗）时重新评估是否需要为主窗口和 iframe 使用不同 CSP。长期看，可考虑为 iframe 使用独立 `blob:`/`data:` 文档或自定义 protocol，以将 CSP 放宽限制在预览面板。

---

## [严重级别:中] `fs_rename` 在目标为目录时无条件递归删除

- **位置**
  - `src-tauri/src/fs/mod.rs:246-248`

- **威胁场景**
  用户（或被诱导的前端操作）将文件重命名为一个已存在的目录名时，该目录及其全部内容会被静默删除。

- **问题**
  代码在 `dst_path.exists() && dst_path.is_dir()` 时调用 `std::fs::remove_dir_all(&dst)`，没有任何确认或回收站保护。虽然目标路径仍受项目根沙箱约束（但当前沙箱失效，见高严重问题 #1），即使沙箱生效，也可能导致项目内大量数据意外丢失。

- **建议**
  重命名目标为目录时应返回错误而不是直接删除；若业务需要覆盖，应要求显式参数并在前端弹窗确认。优先使用回收站 API（如 `trash` crate）或至少将原目录重命名为备份名。

---

## [严重级别:中] Git 命令未对 `repo_path` / `file_path` 做项目根沙箱校验

- **位置**
  - `src-tauri/src/git/mod.rs:71-112` (`get_or_open_repo`)
  - `src-tauri/src/git/mod.rs:118/176` (`git_status` / `git_diff`)

- **威胁场景**
  当用户选择的项目根目录位于某个更大 Git 仓库的子目录时，`Repository::discover` 会上溯到父仓库，`workdir()` 可能返回项目根之外的路径，从而暴露父仓库的文件状态与 diff。

- **问题**
  `git_status` 和 `git_diff` 未调用 `validate_path_within_root`。`get_or_open_repo` 使用 `Repository::discover` 上溯查找 `.git`，如果用户项目路径恰好位于一个更大的仓库内部，后端会读取并返回该父仓库的 git 信息，超出用户预期的项目范围。

- **建议**
  在 `git_status` / `git_diff` 入口处校验 `repo_path` 和 `file_path` 是否位于 `project_root` 内；或者限制 `Repository::discover` 的行为，确保只使用传入路径本身或其严格子目录内的仓库。

---

## [严重级别:中] 能力声明过宽且生产二进制仍包含 `wdio-webdriver` 权限

- **位置**
  - `src-tauri/capabilities/default.json:7-13`
  - `src-tauri/src/lib.rs:24-28/37-46`
  - `src-tauri/Cargo.toml:28`

- **威胁场景**
  生产发布二进制理论上包含 WebDriver 插件依赖，且 `capabilities/default.json` 声明了 `wdio-webdriver:default` 权限；若插件初始化逻辑被破坏或未来误删 `#[cfg(debug_assertions)]` 门控，攻击面会被打开。

- **问题**
  `lib.rs` 通过 `#[cfg(debug_assertions)]` 在 release 构建中不初始化 `tauri-plugin-wdio-webdriver`，但 Cargo 依赖始终编译，capabilities 文件也没有按 profile 分离。`core:default` 加上 Tauri 2 默认行为意味着所有自定义命令（包括 `pty_spawn`、`fs_write_file` 等）默认对主窗口全部放行，缺少按命令白名单的 defense-in-depth。

- **建议**
  1. 将 `wdio-webdriver:default` 权限从生产 `default.json` 中移除，或创建仅用于 debug/e2e 的独立 capability。
  2. 考虑在 `build.rs` 中配置 `AppManifest::commands` 白名单，使自定义命令必须经过 capability 显式放行，而不是依赖 Tauri 2 默认放行行为。

---

## [严重级别:低] `resolve_shell` 传统路径仍接受任意用户 shell

- **位置**
  - `src-tauri/src/pty/shell.rs:25-42`

- **威胁场景**
  当前 Windows 构建走 `resolve_shell_info` + `spawn_conpty_child`，`resolve_shell` 仅在非 Windows fallback 路径使用。若该函数未来被重新启用或跨平台复用，其 `CommandBuilder::new(shell)` 会无条件执行用户传入的程序。

- **问题**
  `resolve_shell` 未对用户传入的 `shell` 做 PATH 解析或白名单校验，与 `resolve_shell_info` 不一致。作为死代码/备用路径保留该行为增加了未来误用的风险。

- **建议**
  删除该备用路径，或在 `resolve_shell` 中复用 `resolve_shell_info` 的解析/校验逻辑，确保两套接口行为一致。

---

## [严重级别:低] `pty_write` 只要有 session_id 即可向任意 PTY 写入

- **位置**
  - `src-tauri/src/pty/spawn.rs:764-777`

- **威胁场景**
  若前端恶意代码获取到另一个面板的 session ID，可向该 PTY 注入输入。

- **问题**
  `pty_write` 只校验 session_id 是否存在，不校验调用者是否拥有该 session。session_id 是随机 UUID，难以猜测，因此直接利用风险较低；但若配合 XSS 或前端内存读取，理论上可跨面板写输入。

- **建议**
  在 `PtySession` 中增加创建者/面板标识，并在 `pty_write`/`pty_resize`/`pty_kill` 中校验调用者身份与 session 归属是否一致。

---

## 实际审查过的文件清单

后端 Rust：
- `src-tauri/src/state.rs`
- `src-tauri/src/fs/mod.rs`
- `src-tauri/src/notify/mod.rs`
- `src-tauri/src/pty/spawn.rs`
- `src-tauri/src/pty/shell.rs`
- `src-tauri/src/pty/reader.rs`
- `src-tauri/src/git/mod.rs`
- `src-tauri/src/settings.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/default.json`

前端 TypeScript：
- `src/panels/html/HtmlPanel.tsx`
- `src/lib/injectScript.ts`
- `src/panels/terminal/useClipboardHandler.ts`
- `src/panels/terminal/useTerminalInstance.ts`
- `src/panels/terminal/useXterm.ts`
- `src/lib/e2eEnabled.ts`
- `src/main.tsx`
- `src/workspace/Workspace.tsx`
- `e2e-tests/helpers.ts`
- `e2e-tests/test.e2e.ts`
- `package.json`
