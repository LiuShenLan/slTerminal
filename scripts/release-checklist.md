# 发布人工核验清单

**命令成功 ≠ 发布核验完成。** 每项须有人工观察/实测证据,逐项勾选后
才允许把结论落 `architecture/notes/packaging/`。

**运行环境**:全部 `.ps1` 首行声明 `#Requires -Version 7`,发布链必须在
PowerShell 7(`pwsh`)下执行;5.1 不受支持。

## 构建与资产

- [x] `CHANGELOG.md` 与 `release-notes/v0.3.0.md` 已按双语完成,表达同一事实
      (M11.4 逐条比对)。
- [x] `scripts/release.ps1 -Version 0.3.0 -Force` 全链通过(干净重建后未用
      `-SkipBuild` / `-AllowStale`;根清单 mtime 触发的新鲜度假红按 B72 以
      `cargo clean --release -p slterm -p slterm_hook` 规避)。
- [x] `dist/` 仅含 `slTerminal-v0.3.0-windows-x64-setup.exe`、
      `slTerminal-v0.3.0-windows-x64.zip`、`SHA256SUMS`。
- [x] 两个产物的文件名、大小与 `SHA256SUMS` 一致,哈希由 `verify-release.ps1`
      复算,未手抄(17,901,111 / 23,286,971 字节)。
- [x] `slterm.exe --version` 输出 `slterm 0.3.0`(构建产物、安装后 exe、zip
      解压 exe 三处实测)。
- [x] setup.exe 为 PE 文件(MZ 魔数)且静默安装退出码 0;安装向导 UI 外观
      待用户目视。

## 安装器

- [ ] 待用户:安装向导显示 GPL-3.0 许可页,中英双语可切换(向导 UI 非
      自动化观察面;LICENSE 已随包安装到 `licenses/`)。
- [x] 默认安装目录为 `%LOCALAPPDATA%\Programs\slTerminal`,无需管理员
      (非提权 shell 静默安装实测,注册表 InstallLocation 一致)。
- [x] 桌面图标/自动启动两项默认不勾选(默认静默安装后桌面与 Startup 均无
      快捷方式)。
- [x] 开始菜单与桌面快捷方式可启动应用(开始菜单快捷方式与
      `/TASKS=desktopicon` 桌面快捷方式均实测启动、正常关窗退出、无孤儿
      进程;开始菜单组落在 `(Default)` 的命名异常登记 B79)。
- [x] 快捷方式/说明明示:从 pwsh/cmd 直接启动会阻塞父 shell,日常用
      快捷方式或 `Start-Process slterm`(`README.md` / `README.zh-CN.md`
      均随包安装且载明)。

## 干净环境验收

- [ ] 待用户:在干净 Win10(或另一台 PC)完成安装 → 启动 → 退出 → 卸载
      全流程(本机 Win11 已完成同等全链静默实测)。
- [x] zip 解压后直接运行 `slterm.exe` 可启动(12 文件清单与打包清单全等;
      `--version` 为 0.3.0;GUI 启停正常、无孤儿进程)。
- [x] 卸载后 HKCU 卸载项、App Paths、开始菜单/桌面/Startup 快捷方式均无
      残留;安装目录残留 `.slterm-update.slterm-lock`(C29 已知项,后果
      登记 B78,人工清理后目录为空)。
- [x] 卸载后 AI CLI hook 清理:本版未实现 `setup-ai --remove`(B69 未裁),
      实测 `slterm.exe setup-ai --remove` 退出码 2、报错
      `unrecognized subcommand`,安装器 `[UninstallRun]` 为 no-op;用户既有
      旧栈 hook 未被本安装器触碰。按现状记录,不作发布失败。

## 更新链

- [ ] 待用户:GitHub Release 已挂同版本资产;更新链能发现并校验 setup.exe
      (本仓无发布流水线,未执行)。
- [ ] 待用户:从 0.2.0 等历史安装启动时,更新提示为「需手动重装」且不
      自动下载。
- [x] release notes 末尾 SHA256 块与 `dist/SHA256SUMS` 逐字节一致
      (M11.4 回填后复算)。

---

## 核验记录

- 2026-10-18 M11.4 本机自动化核验结论见
  `architecture/notes/packaging/2026-10-18-m11-4-first-release-verification.md`。
- 未勾选项 = 待用户执行项;不得以本机自动化结果替代干净环境与发布面核验。
