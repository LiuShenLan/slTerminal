# conda 激活失败修复清单（B17）

> 任务性质：debug 修复（根因已定位并实证，2026-08-29）。起点 = Step 3（跳过探索/汇总）。
> 优先级组织：P0 = 修复本体 + 防复发测试；P1 = 文档/清单同步。Stage 依赖顺序即执行顺序。

## 根因结论（已实证）

**机制链**：`src-tauri/src/pty/shell.rs` 的 `build_pwsh_command` / `build_pwsh_info` 给 PowerShell 固定注入 `-NoProfile` → 用户 profile（`$PROFILE.CurrentUserAllHosts` 等四文件）不加载 → `conda init` 写入的 shell 钩子（`conda` 函数 + prompt 包装）从未定义 → `conda` 落到 PATH 解析：

| 系统 | PATH 解析结果 | 表现 |
|------|--------------|------|
| win11（Anaconda3） | `Scripts\conda.exe`（PATHEXT 中 .EXE 先于 .BAT） | `CondaError: Run 'conda init' before 'conda activate'` |
| win10（miniforge） | `Library\bin\conda.bat`（root 不在 PATH → python 亦不可用） | bat 在 cmd 子进程内激活、随子进程退出蒸发 → **零输出、exit 0、父会话零变化** |

**本机实证（win11，Anaconda3 conda 26.5.0）**：
- 复现 1：`pwsh -NoProfile -Command "conda activate claude"` → CondaError，与 win11 现象逐字一致
- 复现 2：PATH 剔除 anaconda 目录、仅追加 `Library\bin` 后 `conda activate claude` → 无任何输出 + exit 0，与 win10 现象一致（机制见 `condabin\conda.bat`：`IF [%1]==[activate] "%~dp0_conda_activate" %*` —— 激活只作用于 cmd 子进程，退出即蒸发）
- 修复验证：去掉 `-NoProfile` 后，`pwsh/powershell -NoLogo -EncodedCommand <base64("conda activate claude; python --version")>` → 输出 `Python 3.14.3`（pwsh 7 与 powershell 5.1 双路径实证）。同时证明 PowerShell 启动顺序为 profile 先于 -EncodedCommand 执行 → 集成脚本 `$function:prompt` 捕获的是 conda 包装后的 prompt，OSC 包装链顺序正确

**修复决策（用户已确认）**：删除两处 `-NoProfile`，profile 原生加载。不加设置开关（YAGNI，无已知需关闭 profile 的场景）。

**波及面全量（`grep -rn NoProfile` 全仓 4 处命中，逐项决策）**：

| # | 位置 | 决策 |
|---|------|------|
| 1 | `src-tauri/src/pty/shell.rs:223`（`build_pwsh_command`） | **删**（B17-FIX） |
| 2 | `src-tauri/src/pty/shell.rs:237`（`build_pwsh_info`） | **删**（B17-FIX） |
| 3 | `src-tauri/src/pty/shell.rs:152/219`（文档注释） | 同步口径（含于 B17-FIX） |
| 4 | `e2e-tests/terminal.e2e.ts:412` | **不动**——wdio 辅助的一次性 PowerShell 命令（非交互终端 spawn），`-NoProfile` 是正确选择（确定性 + 速度）；不属于本故障链路 |

**调用点排查**：`resolve_shell_info` 生产调用点仅 `spawn.rs:1105`（ConPTY 主路径）；`resolve_shell` 仅 `spawn.rs:1187`（非 Windows fallback）。`pty_integration_tests.rs` 4 处调用均用 `Some("cmd.exe")`（cmd 无 args，不受影响）。无任何既有测试断言 `-NoProfile` 存在（grep 实证）→ 既有用例零适配，只新增防复发用例。

**静默失败面自查（skill 必备项）**：本修复无 IPC 契约变更（`SpawnRequest` DTO 不变）、无 catch 吞错点、无前端波及面、无共享集合/常量改动（启动参数为函数内字面量，无外部消费方）。e2e-tests/helpers 无同模式复制。

---

## B17-FIX（P0 · 后端 · 核心修复）

1. **位置**：`src-tauri/src/pty/shell.rs` —— 模块头注释 L4-5；`resolve_shell` 文档注释 L152；`build_pwsh_command` L217-229；`build_pwsh_info` L231-244
2. **现状**（原文摘录）：
   ```rust
   // L4-5（模块头）
   /// PowerShell 通过 -EncodedCommand（UTF-16LE Base64）内联集成脚本，
   /// 消除 %APPDATA% 文件写入——避免 AMSI/ASR 误杀。
   // L152（resolve_shell 文档注释）
   /// PowerShell 自动加入 -NoProfile -NoLogo -EncodedCommand <base64> 参数。
   // L217-229
   /// 为 PowerShell 构建带 profile 注入的 CommandBuilder
   ///
   /// 使用 -NoProfile -NoLogo -EncodedCommand <base64(UTF-16LE script)> 启动。
   /// 脚本通过 include_str! 嵌入，不写磁盘。
   fn build_pwsh_command(pwsh: &str) -> CommandBuilder {
       let mut cmd = CommandBuilder::new(pwsh);
       cmd.arg("-NoProfile");
       cmd.arg("-NoLogo");
       cmd.arg("-NoExit");
       cmd.arg("-EncodedCommand");
       cmd.arg(encode_utf16le_base64(get_shell_integration_script()));
       cmd
   }
   // L231-244
   /// 为 PowerShell 构建 ShellInfo（不依赖 portable-pty）
   /// pwsh 参数为完整路径（由 which_full_path 解析）
   fn build_pwsh_info(pwsh_path: &str) -> ShellInfo {
       ShellInfo {
           program: pwsh_path.to_string(),
           args: vec![
               "-NoProfile".to_string(),
               "-NoLogo".to_string(),
               "-NoExit".to_string(),
               "-EncodedCommand".to_string(),
               encode_utf16le_base64(get_shell_integration_script()),
           ],
       }
   }
   ```
3. **修复步骤**（照抄）：
   1. 模块头注释在 L5 之后追加两行（L1-5 原样保留）：
      ```rust
      /// 禁止 -NoProfile（B17）：用户 profile 必须先于集成脚本原生加载，
      /// 否则 conda init 等 profile 钩子失效（conda activate 报错或静默空转）。
      ```
   2. L152 整行替换为：
      ```rust
      /// PowerShell 自动加入 -NoLogo -NoExit -EncodedCommand <base64> 参数（禁止 -NoProfile，B17）。
      ```
   3. `build_pwsh_command`（L217-229）整体替换为：
      ```rust
      /// 为 PowerShell 构建带集成脚本的 CommandBuilder
      ///
      /// 使用 -NoLogo -NoExit -EncodedCommand <base64(UTF-16LE script)> 启动。
      /// 脚本通过 include_str! 嵌入，不写磁盘。
      /// 禁止 -NoProfile（B17）：用户 profile 必须先于集成脚本原生加载。
      fn build_pwsh_command(pwsh: &str) -> CommandBuilder {
          let mut cmd = CommandBuilder::new(pwsh);
          cmd.arg("-NoLogo");
          cmd.arg("-NoExit");
          cmd.arg("-EncodedCommand");
          cmd.arg(encode_utf16le_base64(get_shell_integration_script()));
          cmd
      }
      ```
   4. `build_pwsh_info`（L231-244）：文档注释追加一行 `/// 禁止 -NoProfile（B17）：用户 profile 必须先于集成脚本原生加载。`；args 中删除 `"-NoProfile".to_string(),` 一行。结果为：
      ```rust
      /// 为 PowerShell 构建 ShellInfo（不依赖 portable-pty）
      /// pwsh 参数为完整路径（由 which_full_path 解析）
      /// 禁止 -NoProfile（B17）：用户 profile 必须先于集成脚本原生加载。
      fn build_pwsh_info(pwsh_path: &str) -> ShellInfo {
          ShellInfo {
              program: pwsh_path.to_string(),
              args: vec![
                  "-NoLogo".to_string(),
                  "-NoExit".to_string(),
                  "-EncodedCommand".to_string(),
                  encode_utf16le_base64(get_shell_integration_script()),
              ],
          }
      }
      ```
4. **测试同步**：见 TE-B17（同 Stage 同 agent 完成）。既有用例零适配（grep 实证无 `-NoProfile` 断言）。
5. **文档同步**：Stage 02（DOC-B17a/b/c）。
6. **验证**：
   - `grep -n '"-NoProfile"' src-tauri/src/pty/shell.rs` → 零命中（`cmd.arg("-NoProfile")` 与 `"-NoProfile".to_string()` 均不存在；注释中的禁止说明不含带引号形态）
   - `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 通过
   - `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 通过

## TE-B17（P0 · 防复发测试）

1. **位置**：`src-tauri/src/pty/shell.rs` `#[cfg(test)] mod tests`，PTY-06 区块（L687-738）之后追加
2. **现状**：无任何 `-NoProfile` 断言（grep 实证）；`fake_exe`/`set_test_path` 辅助函数已存在（L661-685），直接复用
3. **修复步骤**：tests 模块内追加（照抄）：
   ```rust
       // ── B17：PowerShell 启动参数必须加载用户 profile（conda init 钩子依赖）──

       #[test]
       fn test_pwsh_args_no_noprofile_b17() {
           // B17 防复发：-NoProfile 致 conda activate 失效
           //（win11 CondaError / win10 conda.bat 静默空转）——
           // 自动检测命中 pwsh 时，两条构建路径的 args 均不得含 -NoProfile
           let dir = tempfile::tempdir().unwrap();
           fake_exe(dir.path(), "pwsh.exe");
           let _guard = set_test_path(&[dir.path()]);

           let info = resolve_shell_info(None).expect("应命中 pwsh");
           assert!(
               !info.args.iter().any(|a| a == "-NoProfile"),
               "resolve_shell_info 的 pwsh args 不得含 -NoProfile（B17），实际: {:?}",
               info.args
           );
           assert!(
               info.args.contains(&"-EncodedCommand".to_string()),
               "pwsh 仍应携带集成脚本参数"
           );

           let cmd = resolve_shell(None).expect("resolve_shell 应命中 pwsh");
           let argv = cmd.get_argv();
           assert!(
               !argv.iter().any(|a| a == "-NoProfile"),
               "resolve_shell 的 pwsh argv 不得含 -NoProfile（B17），实际: {:?}",
               argv
           );
       }
   ```
4. **测试同步**：本体即测试。test-inventory.md 计数同步归 DOC-B17c（Stage 02）。
5. **文档同步**：pty/CLAUDE.md 红线行（DOC-B17a）引用本用例名。
6. **验证**：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿，且输出含 `test_pwsh_args_no_noprofile_b17 ... ok`

## DOC-B17a（P1 · pty/CLAUDE.md）

1. **位置**：`src-tauri/src/pty/CLAUDE.md` ——「### Shell 白名单（SEC-01 / SEC-15）」段尾行 +「## 外部坑/红线」列表
2. **现状**：段尾行为 ``PowerShell 通过 `-EncodedCommand` 内联 `shell-integration.ps1`，避免 `%APPDATA%` 文件写入触发 AMSI/ASR。``
3. **修复步骤**：
   1. 段尾行整行替换为：
      > PowerShell 通过 `-EncodedCommand` 内联 `shell-integration.ps1`，避免 `%APPDATA%` 文件写入触发 AMSI/ASR。启动参数固定 `-NoLogo -NoExit -EncodedCommand`，**禁止 `-NoProfile`**——用户 profile 必须先于集成脚本原生加载（B17，守卫用例 `test_pwsh_args_no_noprofile_b17`）。
   2. 「## 外部坑/红线」列表追加一行（位置紧随现有「永不启用 0x8」类红线之后即可）：
      > - **PowerShell 交互 shell 禁止 `-NoProfile`**：用户 profile（conda init 钩子等）必须原生加载——缺钩子则 `conda activate` 失效（win11 CondaError / win10 conda.bat 静默空转，B17）。
4. **测试同步**：无。
5. **文档同步**：本体即文档。
6. **验证**：Read 确认两处存在，且描述与 shell.rs 修复后代码一致（启动参数枚举不多不少，不撒谎）。

## DOC-B17b（P1 · 根编号索引）

1. **位置**：`.claude/CLAUDE.md`「需求编号索引」表 B16 行之后
2. **现状**：B16 为最大 B 编号（grep 实证 B17/B18 零命中，编号无冲突）
3. **修复步骤**：B16 行后追加一行：
   `| B17 | 缺陷 | spawn PowerShell 携带 -NoProfile 致用户 profile 不加载——conda activate 失效（win11 CondaError / win10 conda.bat 静默空转）；修复 = 删 -NoProfile 恢复 profile 原生加载 |`
4. **测试同步**：无。
5. **文档同步**：本体即文档。
6. **验证**：`grep -n '| B17 |' .claude/CLAUDE.md` 命中 1 行。

## DOC-B17c（P1 · test-inventory.md）

1. **位置**：`.claude/test-inventory.md` —— L5 全量表头、L58 L1 段头、L74 shell.rs 行、豁免表（L20-36 表格内追加）
2. **现状**：L74 为 `| src-tauri/src/pty/shell.rs | 32 | shell 发现与回退/白名单/alias 兼容；SEC-01/SEC-15 |`；L5 全量 3731（Rust 794）；L58 L1 段头 39 文件 / 808 用例
3. **修复步骤**（登记纪律 TQ-CI-01：实跑双核对，禁凭计算）：
   1. 先取数：跑 `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`，汇总全部 `test result` 行的 passed 总数；跑 `grep -c '#\[test]' src-tauri/src/pty/shell.rs` 取该文件属性数
   2. L74 shell.rs 行：计数 32→33，描述改为 `shell 发现与回退/白名单/alias 兼容/B17 profile 加载守卫；SEC-01/SEC-15`
   3. L58 L1 段头与 L5 全量表头按实跑数同步（预期 grep 口径 808→809、cargo 口径 794→795、全量 3731→3732——以实跑数为准）
   4. 豁免表（L20-36）追加一行：
      `| win11/win10 真实终端 conda 激活实测（profile 加载链路 + conda 钩子 + prompt 包装链） | 依赖真实 conda/miniforge 环境与交互会话，CI 无此环境 | L1 B17 参数守卫（test_pwsh_args_no_noprofile_b17）+ 双系统 debug build 人工实测 | B17 |`
4. **测试同步**：无。
5. **文档同步**：本体即文档。
6. **验证**：表头/段头/段小计三处一致且与实跑数一致（TQ-CI-01）；豁免行 grep 命中 `B17`。

## MANUAL-B17（人工验证 · 不进 Stage，执行收尾项）

不可自动化（依赖真实 conda/miniforge 环境 + 交互会话），按硬约束 #11 登记豁免（DOC-B17c 第 4 步）：

1. **win11 本机**：`npx tauri build --debug --no-bundle` → 启动 slTerminal → 新终端页签 → `conda activate claude` → 提示符出现 `(claude)` 前缀；`python --version` → claude 环境 Python 版本
2. **win10 部署机**（miniforge）：同法验证前缀出现 + `python` 可启动
3. **冒烟**：切目录后 OSC cwd 跟踪正常（新开页签 cwd 跟随）；prompt 显示正常（conda 前缀包裹在 OSC 133 内、无转义序列泄漏字符）；`claude` 启动后页签标题/图标切换正常（OSC 133;C Enter 钩子未被 profile 破坏）
