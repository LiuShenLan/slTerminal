# 0.3.0 首发本机核验

## Status

已执行（2026-10-18，M11.4）。本机自动化项全绿；干净 Win10 / 另一台 PC、
安装向导 UI 外观与 GitHub 发布面仍待用户，故本 note 不构成无条件放行。

## Context

M11 收官步。0.3.0 是首发版本，安装器与便携 zip 已由发布链产出；发布清单
要求在本机完成安装 → 启动 → 退出 → 卸载与残留检查，并区分可自动化项与
只能人工观察的项。命令成功不等于发布核验完成，人工面不得虚构通过。

## Evidence

构建与资产：

- 先 `cargo clean --release -p slterm -p slterm_hook`，再跑
  `scripts/release.ps1 -Version 0.3.0 -Force`，退出 0；链内包含共享构建、
  安装器、便携 zip、`package-release.tests.ps1` 与 `verify-release.ps1`。
- B72 假红复现：M11.3 给根 `Cargo.toml` 与六个成员清单补 license 后 mtime
  晚于上一轮二进制，新鲜度检查在未重链接时会误判；本次按既定处置以
  `cargo clean` 强制重链接。
- `slTerminal-v0.3.0-windows-x64-setup.exe`：17,901,111 字节（17.07 MiB），
  SHA256 `e0411229b44f20083ac74d222276c5405faa1e536bc8faa2f2d3d7045329d567`。
- `slTerminal-v0.3.0-windows-x64.zip`：23,286,971 字节（22.21 MiB），
  SHA256 `ee97fee26cb6aadd882e5d6dd1b0d01b90186cff15eb1be177b49265af972b0d`。
- `scripts/verify-release.ps1 -Version 0.3.0` 独立复跑退出 0；资产名集合、
  1 MiB 下限、MZ 魔数与 SHA256SUMS 复算一致。构建产物、安装后 exe 与 zip
  解压 exe 的 `--version` 均为 `slterm 0.3.0`。

便携 zip 直跑：

- 解压到独立 tempdir 得 12 个文件，与 `PackageEntries` 清单全等。
- GUI 启动 6 秒后进程存活、主窗口标题含 slTerminal；正常关窗后 15 秒内
  退出；启动期子进程在退出后全部消失，无孤儿。

安装与卸载（生产 AppId，非夹具）：

- `setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG=...` 退出 0；
  非提权 shell 完成安装，落点为 `%LOCALAPPDATA%\Programs\slTerminal`。
- 安装目录含 12 个清单文件 + `unins000.exe` / `unins000.dat`，共 14 个。
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{D9D92995-C831-4610-87BC-4B82B43B3E71}_is1`
  的 DisplayName 为 `slTerminal 0.3.0`、DisplayVersion 为 `0.3.0`、
  InstallLocation 指向安装目录；`App Paths\slterm.exe` 指向安装后 exe。
- 默认任务下桌面与 Startup 均无快捷方式（两项默认不勾选）。
- 开始菜单快捷方式实测创建于 `Programs\(Default)\`（`slTerminal.lnk` 与
  `卸载 slTerminal.lnk`）；经该快捷方式启动成功、正常关窗、无孤儿。
- 第二轮以 `/TASKS=desktopicon,autostart` 安装：桌面与 Startup 快捷方式
  均创建；桌面快捷方式启动成功、正常退出；卸载后两项均被删除。
- `unins000.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART` 退出 0；HKCU
  卸载项、App Paths、开始菜单 / 桌面 / Startup 快捷方式均无残留。
- 安装目录残留 `.slterm-update.slterm-lock`（5 字节），导致目录本身保留；
  人工删除该文件后目录为空。该锁由 GUI 启动时在 exe 旁创建，Inno 卸载
  清单不含它。

hook 清理与用户数据：

- `slterm.exe setup-ai --remove` 实测退出 2，stderr 为
  `error: unrecognized subcommand 'setup-ai'`；`[UninstallRun]` 以 runhidden
  吞掉错误，故卸载 hook 清理为 no-op（B69 现状）。
- 用户既有 `~/.claude/settings.json` 为旧栈 node reporter hook，测试前后
  mtime 未变（09:30），本安装器未触碰。
- `%APPDATA%\slterm`（6 项，测试前已存在）与
  `%LOCALAPPDATA%\slterm\conpty` 在卸载后保留：前者为用户数据，后者为随包
  ConPTY 运行时提取缓存，均属「卸载不删用户数据」的预期。

## Decision

0.3.0 本机自动化核验通过；发布放行以 `scripts/release-checklist.md` 中
仍标「待用户」的项完成为条件。B69（hook 清理 no-op）、B78（安装目录锁
残留）与 B79（`(Default)` 开始菜单组）按已知项记录，不在本步擅自修改
安装器或更新链实现。

## Rejected alternatives

- 只用 `AcceptanceFixture` 验证：夹具隔离 AppId、AppName、路径与注册表键，
  无法证明生产安装标识与卸载行为。
- 复用 M11.2 的旧 `dist/`：根清单 mtime 已变，B72 下无法证明产物与源码
  同代；必须干净重建。
- 用强制杀进程代替正常关窗：会绕过「关窗即退出」与子进程回收验证。本轮
  正常关窗均在超时内完成，未触发强杀兜底。

## Consequences

- release notes 与 checklist 中的 SHA256 与本次资产字节绑定；任何重建都
  必须重新生成并回填，禁止手抄。
- 安装目录锁残留使「卸载后安装目录为空」不成立；若选择正常退出删锁或由
  卸载器清理 exe 目录，会改变 C29 的既有取舍，需重新实测本 note 的卸载段。
- 修复 `(Default)` 组命名会改变开始菜单中快捷方式的可见位置，属发布面
  变更，需与版本说明同步。

## Validation

- `scripts/release.ps1 -Version 0.3.0 -Force`：退出 0；`verify-release.ps1`
  独立复跑退出 0。
- `cargo test -p slterm --test lib_tests -- --test-threads=1`：1308 passed，
  0 failed，1 ignored。
- `cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`：
  1434 passed，0 failed，1 ignored。
- `cargo clippy --workspace --all-targets -- -D warnings` 与
  `cargo clippy -p slterm --all-targets --features gpui-test-support -- -D warnings`：
  退出 0；仅 6 条 cargo「lib.rs 出现在多个 target」结构性警告（TQ-COV-06
  既定形态），无 clippy lint 警告。
- `cargo fmt --all -- --check`：退出 0。
- `python scripts/check_architecture.py`：退出 0；
  `python scripts/check_brand_regress.py staged`：退出 0。
- 本机环境：Windows 11 Pro 10.0.26300，PowerShell 7.6.7。

## Supersedes

None。与 `2026-10-18-release-chain-and-volume-budget.md` 互补：该 note 记录
发布链与预算决策，本篇记录首发实测证据。

## Revisit when

- B69 落地 `setup-ai --remove`，需要重跑卸载 hook 清理验证时。
- B72 改为输入摘要判据后，重跑新鲜度链并更新 Evidence。
- B78/B79 裁决并修改更新锁或安装器后，重跑安装 / 卸载与快捷方式证据。
- 干净 Win10 / 另一台 PC 与 GitHub Release 更新链完成时，补记人工结论。
