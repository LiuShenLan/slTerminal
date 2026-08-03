# 01 L1 pty+state 测试 Review

## 元信息

- **领域**：Rust 后端 PTY 管理 + 全局 State（`src-tauri/src/pty/*`、`src-tauri/src/state.rs`）
- **测试位置**：
  - `src-tauri/src/pty/reader.rs` `#[cfg(test)]`
  - `src-tauri/src/pty/spawn.rs` `#[cfg(test)]`
  - `src-tauri/src/pty/shell.rs` `#[cfg(test)]`
  - `src-tauri/src/state.rs` `#[cfg(test)]`
  - `src-tauri/tests/pty_integration_tests.rs`
- **用例数**：105（reader 30 + spawn 28 + shell 15 + state 24 + integration 8）
- **覆盖率概况**：
  - `reader.rs`：210/298 = 70.5%
  - `spawn.rs`：421/813 = 51.8%
  - `shell.rs`：163/189 = 86.2%
  - `state.rs`：303/348 = 87.1%
- **审查日期**：2026-08-03

## 覆盖率缺口

### 🔴 核心逻辑零覆盖

1. **`spawn.rs:1185-1263` Job Object 孤儿防护**：`add_to_job_object` / `create_and_assign_job` / `JobHandle::drop` 完全没有 L1 测试。P1-19 需求依赖此机制，但 L1 仅通过 `pty_kill_no_orphan` 验证主动 kill，未验证父进程退出时 OS 自动杀子进程。
2. **`spawn.rs:756-970` `pty_spawn` 命令核心校验路径**：尺寸超限校验（762-767）、shell 白名单校验（770-772）、cwd 路径沙箱校验（775-781）均嵌入在 async 命令中，无 Rust 单元测试直接覆盖。
3. **`spawn.rs:977-1183` `pty_write/resize/kill/reattach` 四个 Tauri 命令**：无 Rust 单元测试；SEC-08 的 `panel_id` 归属校验逻辑仅在命令内部实现，L1 未验证。

### 🟡 边界分支未覆盖

4. **`reader.rs:166-205` `strip_conpty_startup`**：非 Windows 平台原样返回分支（168-169）无测试；`find_osc_end` / `match_csi_startup` 中仅 OSC 0/2 及有限 CSI 被测试，未覆盖非 0/2 的 OSC 保留、CSI 3J 等分支。
5. **`state.rs:201-218` `ring_buffer_append`**：`map_or(drain_target, |pos| pos + 1)` 的 `or` 分支（1024 字节内无换行时按 1024 淘汰）未覆盖；现有淘汰测试均使用含换行数据。
6. **`shell.rs:94-127` `resolve_shell_info`**：自动检测 `pwsh.exe → powershell.exe → cmd.exe` 回退顺序无直接测试；当前用例只验证返回完整路径。
7. **`spawn.rs:81-99` `build_cmdline`**：程序路径含空格加引号、参数含空格/制表符加引号逻辑无单元测试。
8. **`spawn.rs:398-459` `spawn_conpty_child`**：组合了 cmdline/env/cwd/AttrList/CreateProcessW，但 L1 无针对该函数的单元测试，仅由集成测试 `pty_spawn_custom_conpty` 覆盖。
9. **`spawn.rs:201-217` `ConPtyMaster::resize`**：HPCON 已 invalid（`hpc.is_invalid()`）时仅更新 size 的分支未覆盖。

### 🟢 低风险未覆盖

10. **`state.rs:100-131` `canonicalize_or_ancestor`**：直接传入 relative path 的情况未测试（`validate_path_within_root` 会先 join 成绝对路径）。
11. **`shell.rs:173-183` `which_full_path`**：PATH 遍历顺序、多个目录匹配第一个、大小写边界未测试。
12. **`reader.rs:31-154` `reader_loop`**：剩余 I/O 编排（channel 替换、DA1 注入 write_all/flush、ring buffer 写入、EOF Exit 发送）无法纯函数化，文档 M11 已说明"已尽力"，依赖 L3/L4 覆盖。

## 问题列表

### P-1 [🔴 覆盖度] Job Object 孤儿防护核心逻辑零 L1 覆盖

- **位置**：`src-tauri/src/pty/spawn.rs:1185-1263`（`add_to_job_object`、`create_and_assign_job`）、`spawn.rs:706-714`（`JobHandle::drop`）
- **代码片段**：
  ```rust
  unsafe fn create_and_assign_job(pid: u32, job_name_wide: &[u16]) -> Result<JobHandle, AppError> {
      let job = CreateJobObjectW(None, PCWSTR::from_raw(job_name_wide.as_ptr()))
          .map_err(|e| AppError::Pty(format!("CreateJobObject failed: {e}")))?;
      let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
      limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
      SetInformationJobObject(job, JobObjectExtendedLimitInformation, ...)?;
      let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid)?;
      AssignProcessToJobObject(job, process).map_err(...)?;
      let _ = CloseHandle(process);
      Ok(JobHandle::new(job))
  }
  ```
- **问题**：P1-19（窗口关闭前杀子进程）的关键防线完全由 Job Object 实现，但 L1 没有任何用例验证 `CreateJobObjectW`/`SetInformationJobObject`/`AssignProcessToJobObject` 的调用参数与返回值处理，也没有验证 `JobHandle` 是否在 `PtySession` 存活期间持有句柄。集成测试 `pty_kill_no_orphan` 只验证主动 `kill()` 后子进程退出，无法验证父进程异常退出时的孤儿防护。
- **改法**：将能拆分的纯逻辑（如 job_name 格式、limit flags 构造）抽取并补单元测试；无法自动化的 OS 行为应在 L4 E2E 增加"杀掉 slterminal.exe 后检查子进程是否残留"用例。
- **变异推演**：若把 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 误写成 `0`，或漏调 `AssignProcessToJobObject`，当前全部 105 条 L1 用例不会变红——因为没有任何用例命中这段代码。

### P-2 [🔴 覆盖度] `pty_spawn` 核心校验路径零 Rust 单元覆盖

- **位置**：`src-tauri/src/pty/spawn.rs:756-970`，重点是 762-767、770-772、775-781
- **代码片段**：
  ```rust
  if request.cols > i16::MAX as u16 || request.rows > i16::MAX as u16 {
      return Err(AppError::Pty(format!("终端尺寸超限: ...")));
  }
  if let Some(ref shell) = request.shell {
      shell::validate_shell_allowlist(shell)?;
  }
  if let Some(ref cwd) = request.cwd {
      let root = state.project_root.read().map_err(...)?;
      app_state::validate_path_within_root(&root, Path::new(cwd))?;
  }
  ```
- **问题**：尺寸超限、shell 白名单、cwd 路径沙箱三项关键校验全部内嵌在 async Tauri 命令中，L1 无 Rust 用例直接调用。L2 IPC 合约测试只验证前端 payload 结构正确，不验证后端校验逻辑。
- **改法**：将三项校验抽取为一个纯函数 `validate_spawn_request(root, request) -> Result<(), AppError>`，并分别添加单元测试：cols/rows 边界值、非法 shell、cwd 越界、cwd 在根内。
- **变异推演**：把 `request.cols > i16::MAX as u16` 改为 `>=`、把 `shell::validate_shell_allowlist(shell)?` 删除、把 `validate_path_within_root` 调用删除——当前 Rust L1 不会变红（L2 契约测试不会触发后端真实校验）。

### P-3 [🔴 覆盖度] `pty_write/resize/kill/reattach` 四个命令零 Rust 单元覆盖

- **位置**：`src-tauri/src/pty/spawn.rs:977-1183`
- **代码片段**：
  ```rust
  if session.panel_id != panel_id {
      return Err(AppError::Pty(format!("会话归属不匹配: ...")));
  }
  ```
- **问题**：四个命令均包含 SEC-08 panel_id 归属校验，但 L1 没有 Rust 用例验证"归属正确放行"与"归属错误拒绝"。只有前端 L2 IPC 合约测试验证了命令名和参数结构。
- **改法**：将归属校验抽取为可独立测试的函数 `validate_session_ownership(session, panel_id)`；或创建最小 `PtySession` 并调用命令的集成测试。
- **变异推演**：将 `session.panel_id != panel_id` 反转为 `==`，当前 L1 全部用例不会变红，SEC-08 校验失效无法被 Rust 单元测试发现。

### P-4 [🟡 覆盖度] `strip_conpty_startup` 非 Windows 分支与未覆盖 OSC/CSI 分支

- **位置**：`src-tauri/src/pty/reader.rs:166-235`
- **代码片段**：
  ```rust
  fn strip_conpty_startup(data: &[u8]) -> Vec<u8> {
      if !cfg!(windows) { return data.to_vec(); }   // 168-169
      ...
      if i + 2 < data.len() && (data[i + 2] == b'0' || data[i + 2] == b'2') {  // 178
          if let Some(end) = find_osc_end(&data[i..]) { ... }
      }
      ...
      if let Some(len) = match_csi_startup(&data[i..]) { ... }  // 191
  }
  ```
- **问题**：
  - 非 Windows 平台原样返回分支（168-169）无测试，若该分支被误改为 `return vec![]`，非 Windows CI 会失败但 Windows CI 不触发。
  - `find_osc_end` 仅在 OSC 0/2 时被调用，未测试 OSC 1/3/4/9 等序列应被保留。
  - `match_csi_startup` 覆盖了 H/2J/6n/?25h/l，但 CSI 3J 分支（223 行 `b'2' | b'3'`）未单独测试；若把 `b'3'` 误删除，现有测试不会变红。
- **改法**：增加 `strip_preserves_osc_1`、`strip_preserves_csi_3j`、`strip_preserves_csi_unknown` 等回归用例；非 Windows 分支可通过在 Windows 上无法直接测试，但应至少在 CI 配置中确保非 Windows runner 覆盖，或在代码注释中明确标注风险。
- **变异推演**：把 178 行条件改为 `data[i + 2] == b'0'`，`test_strip_cursor_visibility`（OSC ?25）和 `test_preserve_osc7_cwd` 仍可通过，但 OSC 2 窗口标题剥离会失效——现有测试不会变红，因为没有任何用例发送 OSC 2。

### P-5 [🟡 断言有效性] `ring_buffer_append` 无换行长行淘汰边界未覆盖

- **位置**：`src-tauri/src/state.rs:201-218`
- **代码片段**：
  ```rust
  while buf.len() > RING_BUFFER_CAPACITY {
      let drain_target = 1024usize.min(buf.len());
      let prefix: Vec<u8> = buf.iter().take(drain_target).copied().collect();
      let drain_len = prefix.iter().rposition(|&b| b == b'\n')
          .map_or(drain_target, |pos| pos + 1);  // or 分支未覆盖
      for _ in 0..drain_len { buf.pop_front(); }
  }
  ```
- **问题**：淘汰策略的"按换行边界"路径被 `test_ring_buffer_eviction_at_newline_boundary` 覆盖，但"1024 字节内无换行则按 1024 原量淘汰"的 `map_or` 右侧分支未测试。超长单行（如 base64 图片输出）可能触发此分支，若该分支计算错误会导致数据截断或死循环。
- **改法**：新增 `test_ring_buffer_eviction_without_newline`，写入 >256KB 且无换行的数据，断言 buf 长度 ≤ capacity、剩余尾部正确、未 panic。
- **变异推演**：把 `map_or(drain_target, |pos| pos + 1)` 改为 `map_or(drain_target - 1, |pos| pos + 1)`，无换行分支会少淘汰 1 字节；在超长单行场景下经过多次循环后 `buf.len()` 可能永远 > capacity 导致死循环——现有测试不会触发，因为均含换行。

### P-6 [🟡 覆盖度] `resolve_shell_info` 自动检测回退逻辑无直接测试

- **位置**：`src-tauri/src/pty/shell.rs:94-127`
- **代码片段**：
  ```rust
  } else if let Some(path) = which_full_path("pwsh.exe") {
      build_pwsh_info(&path)
  } else if let Some(path) = which_full_path("powershell.exe") {
      build_pwsh_info(&path)
  } else {
      let cmd_path = which_full_path("cmd.exe")
          .unwrap_or_else(|| r"C:\Windows\System32\cmd.exe".to_string());
      ShellInfo { program: cmd_path, args: vec![] }
  }
  ```
- **问题**：当前用例 `test_resolve_shell_info_returns_full_path` 仅验证返回路径含分隔符，未验证回退顺序。若把 `pwsh.exe` 与 `powershell.exe` 顺序交换，或把 cmd 回退提到最前，测试不会变红。
- **改法**：通过临时修改 `PATH` 环境变量构造三种场景（只有 pwsh、只有 powershell、都没有），断言返回的 program 文件名。
- **变异推演**：交换 `pwsh.exe` 与 `powershell.exe` 检测顺序——`test_resolve_shell_info_returns_full_path` 仍绿，因为该用例只检查路径含分隔符。

### P-7 [🟡 覆盖度] `build_cmdline` 引号处理未测试

- **位置**：`src-tauri/src/pty/spawn.rs:81-99`
- **代码片段**：
  ```rust
  fn build_cmdline(program: &str, args: &[String]) -> Vec<u16> {
      let mut s = String::new();
      let quote = program.contains(' ');
      if quote { s.push('"'); }
      s.push_str(program);
      ...
      for arg in args {
          s.push(' ');
          if arg.contains(' ') || arg.contains('\t') { s.push('"'); s.push_str(arg); s.push('"'); }
          else { s.push_str(arg); }
      }
      to_wide_null(&s)
  }
  ```
- **问题**：PowerShell 路径含空格、集成脚本参数含空格时，`CreateProcessW` 的 `lpCommandLine` 必须正确加引号。该函数目前无单元测试。
- **改法**：添加 `test_build_cmdline_quotes`，覆盖：program 含空格、arg 含空格、arg 含制表符、arg 无空格、空 args。
- **变异推演**：删除 `if quote { s.push('"'); }` 分支——当前 L1 不会变红；真实场景下 `C:\Program Files\PowerShell\7\pwsh.exe` 路径会解析失败。

### P-8 [🟡 覆盖度] `spawn_conpty_child` 组合逻辑仅由集成测试覆盖

- **位置**：`src-tauri/src/pty/spawn.rs:398-459`
- **问题**：`spawn_conpty_child` 组合了 `build_cmdline`、`build_env_block`、cwd 反斜杠转换、`AttrList` 生命周期、`CreateProcessW` 调用，但 L1 没有针对该函数的单元测试。其子函数虽有测试，但组合时序与参数错误无法被子函数测试捕获。
- **改法**：保持现有集成测试 `pty_spawn_custom_conpty` 作为该函数主要回归；在 L1 层面明确标注该函数依赖 L3/L4 覆盖，或在 CI 中确保集成测试必跑。
- **变异推演**：把 cwd 处理从 `cwd.map(|c| to_wide_null(&c.replace('/', "\\")))` 改为不替换正斜杠——子函数 `test_cwd_forward_slash_to_backslash_encoding` 仍绿，但 `pty_spawn_custom_conpty` 会变红（如果 cwd 含 `/`）。

### P-9 [🟡 覆盖度] `ConPtyMaster::resize` HPCON invalid 分支未覆盖

- **位置**：`src-tauri/src/pty/spawn.rs:201-217`
- **代码片段**：
  ```rust
  fn resize(&self, size: PtySize) -> Result<(), Error> {
      let mut inner = self.inner.lock().map_err(...)?;
      if inner.hpc.is_invalid() {
          inner.size = size;
          return Ok(());  // 未覆盖
      }
      unsafe { ResizePseudoConsole(inner.hpc, coord)?; }
      inner.size = size;
      Ok(())
  }
  ```
- **问题**：`ConPtyInner` drop 后 `hpc` 会置为 `INVALID_HANDLE_VALUE`，此后 `resize` 应静默更新 size 而不调用 Win32 API。该分支无测试，若错误地调用 `ResizePseudoConsole` 会导致未定义行为。
- **改法**：在测试中创建 `create_conpty_pair` 后 drop master，再调用 `resize` 验证返回 Ok 且不 panic。
- **变异推演**：删除 `if inner.hpc.is_invalid()` 分支——当前 L1 不会变红，真实 drop 后 resize 可能崩溃。

### P-10 [🟡 用例设计质量] `resolve_shell` 回退顺序与白名单 PATH 解析测试不足

- **位置**：`src-tauri/src/pty/shell.rs:68-86`、`shell.rs:283-322`
- **问题**：
  - `resolve_shell` 的 `pwsh → powershell → cmd` 自动检测路径只有一个 `test_resolve_shell_accepts_env_vars`（验证 cmd 不 panic），没有测试自动检测顺序。
  - `validate_shell_allowlist` 中"短名不在白名单，通过 PATH 解析完整路径后再比较"分支只有 `test_allowlist_resolves_via_path` 覆盖 cmd，未覆盖解析后仍是非法 shell 的情况。
- **改法**：为 `resolve_shell` 增加回退顺序测试；为白名单增加"PATH 解析后仍是 evil.exe"的拒绝用例。
- **变异推演**：把 `resolve_shell` 中检测顺序改为 `powershell → pwsh → cmd`，当前 L1 不会变红。

### P-11 [🟡 覆盖度] `validate_path_within_root` 相对路径含 `..` 穿越未覆盖

- **位置**：`src-tauri/src/state.rs:138-177`
- **代码片段**：
  ```rust
  let target = if target.is_relative() {
      canonical_root.join(target)
  } else {
      target.to_path_buf()
  };
  ```
- **问题**：相对路径会先 join 到 root 下，但测试只覆盖了绝对路径和已 join 后的相对路径，未覆盖 `target = "../outside.txt"` 这种穿越场景。虽然 `canonicalize_or_ancestor` 会处理 `..`，但边界行为应在 `validate_path_within_root` 层面回归。
- **改法**：新增 `validate_relative_path_traversal_rejected` 用例。
- **变异推演**：删除 `canonical_root.join(target)` 步骤，直接对 relative target 调用 `canonicalize_or_ancestor`——现有测试仍绿，但 relative 路径会逃到根外。

### P-12 [🟢 结构与可维护性] `reader_loop` I/O 编排无法纯函数化，依赖上层覆盖

- **位置**：`src-tauri/src/pty/reader.rs:31-154`
- **问题**：主循环中的 channel 替换、DA1 注入 write_all/flush、ring buffer 写入、EOF Exit 发送均依赖 Mutex/RwLock/Channel/系统调用，无法抽取为纯函数。源码注释 M11 已说明"已尽力"。
- **改法**：无需代码改动，但应在测试文档中明确标注这些路径由 `pty_reattach_ring_buffer_replay`（自定义 reader 模拟）和 L4 E2E 覆盖，避免未来误以为 L1 已覆盖。
- **变异推演**：不适用（已确认无法单元化）。

### P-13 [🟢 结构与可维护性] `spawn.rs` 模块级测试清理代码重复

- **位置**：`src-tauri/src/pty/spawn.rs:1365-1371`、`1399-1403`、`1426-1430`
- **代码片段**：三个测试均包含：
  ```rust
  if let Some(s) = pty_state.sessions.write().unwrap().remove(sid) {
      if let Ok(mut c) = s.child.lock() { let _ = c.kill(); }
  }
  ```
- **问题**：复制粘贴式清理增加维护成本；若清理逻辑变更（如还需 join reader_handle），需修改三处。
- **改法**：抽取 `cleanup_session(pty_state, sid)` 测试辅助函数。
- **变异推演**：不适用（结构问题）。

### P-14 [🟢 稳定性风险] PTY 集成测试依赖真实 `cmd.exe` 与 ConPTY，非 Windows 部分跳过

- **位置**：`src-tauri/tests/pty_integration_tests.rs:7-438`
- **问题**：
  - `pty_roundtrip`、`pty_resize_applies`、`pty_kill_no_orphan` 使用 `portable_pty::native_pty_system()`，测试的是第三方库而非项目自定义 ConPTY 路径。
  - 所有用例依赖 `cmd.exe` 在 PATH 中，且需要真实 ConPTY API；在非 Windows runner 上多个用例被 `#[cfg(windows)]` 跳过。
- **改法**：无需修改，这是平台限制；但应在 CI 中确保 Windows runner 执行全部 PTY 集成测试，且不把跳过等同于通过。
- **变异推演**：不适用。

### P-15 [🟢 覆盖度] `canonicalize_or_ancestor` relative path 与 `which_full_path` 顺序未测试

- **位置**：`src-tauri/src/state.rs:100-131`、`src-tauri/src/pty/shell.rs:173-183`
- **问题**：
  - `canonicalize_or_ancestor` 直接调用时 relative path 行为未测试（`validate_path_within_root` 会先 join）。
  - `which_full_path` 只测试存在/不存在，未验证 PATH 多个目录时返回第一个匹配。
- **改法**：补充边界用例。
- **变异推演**：把 `which_full_path` 中 `for dir in std::env::split_paths(&path)` 改为 `rev()`（反向遍历 PATH）——当前测试不会变红。

## 已做变异推演的用例清单

| 用例 | 被测函数 | 篡改假设 | 是否会变红 | 原因 |
|------|---------|---------|-----------|------|
| `test_strip_startup_preserve_shell_output` | `strip_conpty_startup` | 删除 `match_csi_startup` 中 `b'H'` 分支 | 是 | 输入含 `ESC[H`，预期输出会多留 `ESC[H` |
| `test_preserve_osc7_cwd` | `strip_conpty_startup` | 把 OSC 剥离条件从 `b'0' \|\| b'2'` 改为仅 `b'0'` | 否 | 测试只覆盖 OSC 7（保留）和 OSC 0（剥离），改后 OSC 0 仍剥离 |
| `strip_startup_with_16k_boundary` | `strip_conpty_startup` | 把 `Vec::with_capacity(data.len())` 改为 `0` | 否 | 容量不影响输出内容，只影响性能 |
| `da1_standard_query_detected` | `mirror_da1_query` | 把 `rest.first() == Some(&b'c')` 改为 `== Some(&b'd')` | 是 | `ESC[c` 不再匹配 |
| `startup_strip_across_buffer_boundary` | `apply_startup_strip` | 把 `None` 分支也置 `startup_drained = true` | 是 | 第二轮仍用 false 测试，结果会预期外剥离 |
| `flags_win11_24h2_returns_0x7` | `compute_conpty_flags` | 把 `PSEUDOCONSOLE_INHERIT_CURSOR \| ...` 改为 `0xF` | 是 | 断言 `== 0x7` 失败 |
| `test_build_env_block_extra_overrides_existing` | `build_env_block` | 删除覆盖逻辑，改为直接 `env.push((k, v))` | 是 | `COMPUTERNAME=` 出现次数 >1，断言 `count == 1` 失败 |
| `test_cwd_forward_slash_to_backslash_encoding` | `to_wide_null` + 路径规范化 | 删除 `cwd.replace('/', "\\")` | 是 | 解码后路径仍含 `/` |
| `test_ring_buffer_eviction_at_newline_boundary` | `ring_buffer_append` | 把 `pos + 1` 改为 `pos`（不包含换行） | 是 | 淘汰后首字节不再是 `X` |
| `test_ring_buffer_eviction` | `ring_buffer_append` | 把 `RING_BUFFER_CAPACITY` 改为 `capacity - 1` | 是 | 断言 `buf.len() <= RING_BUFFER_CAPACITY` 失败 |
| `validate_path_outside_root_rejected` | `validate_path_within_root` | 把 `starts_with(&canonical_root)` 改为 `starts_with(canonical_root.parent().unwrap())` | 是 | 根外路径被误放行 |
| `canonicalize_or_ancestor_path_traversal_via_dotdot` | `canonicalize_or_ancestor` | 删除上溯循环，直接返回 `dunce::canonicalize(target)` | 是 | 不存在的路径会 Err，断言失败 |
| `pty_roundtrip` | `spawn_cmd` + PTY 读写 | 把 `writer.write_all` 改为不 flush | 是 | marker 不会到达 cmd，轮询超时 |
| `pty_spawn_custom_conpty` | `conpty_custom::create_conpty_pair` + `spawn_conpty_child` | 把 `compute_conpty_flags` 固定返回 `0xF` | 是 | 0xF 下 PTY 可能仍能启动，但后续鼠标/输入行为会变；该用例只验证 echo，可能**不会**变红——说明该用例无法守卫 PASSTHROUGH_MODE 回归 |
| `pty_reattach_ring_buffer_replay` | ring buffer drain 模式 | 把 `drain(..)` 改为 `split_off(0)` 后不回清 | 是 | 断言 `is_empty()` 失败 |

*说明：变异推演为思想实验，未实际修改源码运行。*
