# 发布人工核验清单

> 命令成功 ≠ 发布核验完成；每项须有人工观察或实测证据。

## 决策与红线

- **运行环境**：全部 `.ps1` 首行声明 `#Requires -Version 7`，发布链必须在 PowerShell 7（`pwsh`）下执行；5.1 不受支持。
- **人工面不可替代**：安装向导外观、干净环境安装/卸载、更新链发现等只能人工观察；未完成项不得以本机自动化结果替代。
- **纯步骤**：本清单只记录核验步骤，不记录历史勾选状态、批次号或过程编号；本次核验结论写入版本说明或发布记录。

## 验收与测试

### 构建与资产

- 确认 `CHANGELOG.md` 与 `release-notes/vX.Y.Z.md` 已按双语完成，表达同一事实。
- 执行 `pwsh -File scripts/release.ps1 -Version <X.Y.Z>`，确认全链通过；正式路径不得使用 `-SkipBuild` / `-AllowStale`。
- 确认 `dist/` 仅含安装器、便携 zip 与 `SHA256SUMS`。
- 确认两个产物的文件名、大小与 `SHA256SUMS` 一致，哈希由 `verify-release.ps1` 复算，未手抄。
- 确认 `scripts/sync-release-notes.ps1 -Version <X.Y.Z>` 复跑后版本说明无差异；版本说明 SHA256 块与 `SHA256SUMS` 同源，不要求两种格式逐字节相等。
- 确认构建产物、安装后 exe、zip 解压 exe 三处 `slterm.exe --version` 输出一致。
- 确认 setup.exe 为 PE 文件（MZ 魔数）且静默安装退出码为 0。
- 确认 `licenses/` 含 Maple Mono OFL、ConPTY、SQLite、dav1d 与 Latin Modern Math 文本；`python scripts/check_licenses.py` 与 `python scripts/check_bundled_runtime.py` 在发布机复跑通过。
- 签名边界：本版不提供 Authenticode 签名；确认发布说明或发布记录已登记 SmartScreen 与“仅哈希校验、不证明发布者身份”的风险。

### 安装器

- 目视确认安装向导显示 GPL-3.0 许可页，中英双语可切换；`LICENSE` 已随包安装到 `licenses/`。
- 确认默认安装目录为 `%LOCALAPPDATA%\Programs\slTerminal`，无需管理员权限。
- 确认桌面图标/自动启动两项默认不勾选。
- 确认开始菜单与桌面快捷方式可启动应用、正常关窗退出、无孤儿进程。
- 确认快捷方式与说明明示：从 pwsh/cmd 直接启动会阻塞父 shell，日常使用快捷方式或 `Start-Process slterm`。

### 干净环境验收

- 在干净 Win10 或另一台 PC 完成安装 → 启动 → 退出 → 卸载全流程。
- 确认 zip 解压后直接运行 `slterm.exe` 可启动，文件集与打包清单全等。
- 确认卸载后 HKCU 卸载项、App Paths、开始菜单/桌面/Startup 快捷方式与 `.slterm-update*` 残留无遗留；安装目录本身仅在仍有用户文件时保留。
- 确认卸载运行 `setup-ai --remove`，AI CLI hook 清理失败时安装器不把失败误报为成功。

### 更新链

- 确认 GitHub Release 已挂同版本资产，更新链能发现并校验 setup.exe。
- 确认从历史版本启动时，更新提示为「需手动重装」且不自动下载。
- 确认 release notes 末尾 SHA256 块由 `sync-release-notes.ps1` 从 `dist/SHA256SUMS` 派生且无差异。

## 操作

- 核验完成后，把用户可感知结论写入对应版本说明；过程证据与已知边界写入 `.claude/docs/release/packaging.md`。
- 发布链、体积预算与合规细节见 `.claude/docs/release/packaging.md`。
