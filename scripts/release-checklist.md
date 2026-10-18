# 发布人工核验清单

**命令成功 ≠ 发布核验完成。** 每项须有人工观察/实测证据,逐项勾选后
才允许把结论落 `architecture/notes/packaging/`。

## 构建与资产

- [ ] `CHANGELOG.md` 与 `release-notes/vX.Y.Z.md` 已按双语完成,表达同一事实。
- [ ] `scripts/release.ps1 -Version X.Y.Z` 全链通过(全新构建,未用
      `-SkipBuild` / `-AllowStale`)。
- [ ] `dist/` 仅含 `slTerminal-vX.Y.Z-windows-x64-setup.exe`、
      `slTerminal-vX.Y.Z-windows-x64.zip`、`SHA256SUMS`。
- [ ] 两个产物的文件名、大小与 `SHA256SUMS` 一致,哈希未被手抄改动。
- [ ] `slterm.exe --version` 输出 X.Y.Z(版本回读)。
- [ ] setup.exe 为 PE 文件且可正常启动安装向导。

## 安装器

- [ ] 安装向导显示 GPL-3.0 许可页,中英双语可切换。
- [ ] 默认安装目录为 `%LOCALAPPDATA%\Programs\slTerminal`,无需管理员。
- [ ] 桌面图标/自动启动两项默认不勾选。
- [ ] 安装完成后开始菜单与桌面快捷方式可启动应用。
- [ ] 快捷方式/说明明示:从 pwsh/cmd 直接启动会阻塞父 shell,日常用
      快捷方式或 `Start-Process slterm`。

## 干净环境验收

- [ ] 在干净 Win10(或另一台 PC)完成安装 → 启动 → 退出 → 卸载全流程。
- [ ] zip 解压后直接运行 `slterm.exe` 可启动。
- [ ] 卸载后安装目录、HKCU 卸载项、App Paths 注册表键无残留。
- [ ] 卸载后注入各 AI CLI 的 hook 已清理(若该版本已实现 `setup-ai --remove`)。

## 更新链

- [ ] GitHub Release 已挂同版本资产;更新链能发现并校验 setup.exe。
- [ ] 从 0.2.0 等历史安装启动时,更新提示为「需手动重装」且不自动下载。
- [ ] release notes 末尾 SHA256 块来自 `SHA256SUMS`,未手抄。
