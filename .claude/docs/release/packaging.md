# 打包发布链与体积预算

> 本地可复现的发布链、包内清单合同、版本线与体积门禁；具体脚本实现以代码为准。

## 决策与红线

- **单一发布入口**：正式发布只走 `scripts/release.ps1 -Version <X.Y.Z>`，依次执行共享构建 → Inno 安装器 → 便携 zip → 钉测试 → `verify-release`，逐失败即停。原因：发布步骤必须同源、可复现、不可跳步。边界：正式路径禁止 `-SkipBuild`/`-AllowStale`；两者只允许出现在脚本自测与发布彩排，且必须在命令中显式出现。
- **共享构建与 feature 图**：`scripts/build-windows-product.ps1` 以隔离的 `CARGO_TARGET_DIR` 显式构建 `slterm` 与 `slterm-hook`；安装器与便携 zip 使用同一构建产物与同一 feature 图。原因：两个产物必须同代。边界：不得从已有 `target/` 或旧 `dist/` 复用产物。
- **清单即合同**：包内文件集唯一真值源 = `build-windows-product.ps1` 的 `PackageEntries`；`package-release.ps1` 的 staging/zip 全等校验与 `installer.iss` 的 `[Files]` 必须同步，多一件少一件都红。原因：包内文件集是外部可见合同。边界：新增/删除包内文件必须同时改三处。
- **新鲜度判据**：二进制 mtime 必须 ≥ 全部成员 crate 与根清单的最新 `.rs`/`.toml` mtime。原因：cargo 对未变更目标不重链接，用运行时刻作基准会漏判旧二进制。边界：仅修改根清单注释也可能触发假红；必要时 `cargo clean` 强制重链接。根治方向是改为「参与二进制的输入摘要」判据。
- **版本与产物命名**：版本缺省回读 `slterm_app/Cargo.toml`；版本线续 `0.3.0`，更新链对 `< 0.3.0` 一律视为需手动重装。产物固定为 `slTerminal-v<version>-windows-x64-setup.exe` / `slTerminal-v<version>-windows-x64.zip`，`scripts/verify-release.ps1` 生成 `SHA256SUMS`。原因：资产命名与版本线是更新链的外部契约。边界：`verify-release` 校验资产名集合、1 MiB 下限、PE 魔数与 semver；任何重建都必须重新生成 SHA256。
- **安装器形态**：Inno Setup，用户级安装（`PrivilegesRequired=lowest`），`MinVersion=10.0.17763`；生产 AppId `{D9D92995-C831-4610-87BC-4B82B43B3E71}` 与夹具 AppId `{45EED1CD-AE5E-4D5C-84F2-8D1949722E9E}` 一经钉死永不变更；夹具模式必须隔离 AppId、AppName、输出名与注册表键。原因：AppId 是续装/升级/卸载标识，变更会割裂用户安装基。边界：中文翻译文件 `ChineseSimplified.isl` 固定上游提交 + SHA256，缺失或哈希不符时下载后仍不符则拒绝构建。
- **体积门禁**：release profile 冷代码 `opt-level = "s"`、thin LTO、单 codegen unit、strip；热路径清单（resvg/rustybuzz/taffy/ttf-parser/smol/gpui/gpui-component/slterm_terminal/image + `slterm` O3/16 CU）钉回 O3。预算值钉在 `scripts/tests/package-release.tests.ps1`；超预算即红。原因：依赖增长会静默吃掉发布包体积。边界：收紧预算永远合法；放宽须在提交信息说明原因；改动 profile 体积相关数值或钉包清单必须重跑钉测试。
- **合规四落点**：仓库根 `LICENSE` 全文 / workspace `Cargo.toml` 的 `license = "GPL-3.0"` 与成员 `license.workspace = true` / 安装器 `LicenseFile` / 应用内关于页；`licenses/` 与 `THIRD-PARTY-NOTICES` 随包。原因：GPL-3.0 再分发义务。边界：真实凭据禁入任何打包输入。
- **发布脚本版本**：全部 `.ps1` 首行 `#Requires -Version 7`；PowerShell 5.1 下不保证解析语义。原因：脚本使用了 7 的语法与语义。边界：发布机必须安装 PowerShell 7。
- **人工核验**：`scripts/release-checklist.md` 是发布人工核验清单；命令成功不等于发布核验完成。原因：安装向导外观、干净环境、更新链发现等只能人工观察。边界：未完成人工项不得以本机自动化结果替代。

## 验收与测试

- 钉测试：`pwsh -File scripts/tests/package-release.tests.ps1`（profile 钉值、清单、新鲜度、体积预算）；`pwsh -File scripts/tests/windows-runtime.tests.ps1`（随包运行时校验）。
- 发布：`pwsh -File scripts/release.ps1 -Version <X.Y.Z>`。
- 发布核验：`scripts/verify-release.ps1` 独立复跑；资产名集合、1 MiB 下限、PE 魔数与 SHA256SUMS 复算一致。
- 2026-10-18 首发本机核验证据：
  - 环境：Windows 11 Pro build 26300，PowerShell 7.6.7。
  - 安装器 `slTerminal-v0.3.0-windows-x64-setup.exe`：17,901,111 字节（17.07 MiB），SHA256 `e0411229b44f20083ac74d222276c5405faa1e536bc8faa2f2d3d7045329d567`。
  - 便携 zip `slTerminal-v0.3.0-windows-x64.zip`：23,286,971 字节（22.21 MiB），SHA256 `ee97fee26cb6aadd882e5d6dd1b0d01b90186cff15eb1be177b49265af972b0d`。
  - 体积预算按实测 + 15% 钉为 setup 19.63 MiB / zip 25.54 MiB。
  - 便携 zip 解压后文件集与 `PackageEntries` 全等；GUI 启动存活、正常关窗退出、无孤儿进程。
  - 生产 AppId 静默安装成功，落点 `%LOCALAPPDATA%\Programs\slTerminal`；卸载后卸载项、App Paths 与快捷方式无残留。
  - 已知边界：安装目录可能残留 `.slterm-update.slterm-lock`，导致目录本身保留；`setup-ai --remove` 子命令不存在，卸载 hook 清理为 no-op；开始菜单快捷方式落在 `Programs\(Default)` 组。
  - 用户数据 `%APPDATA%\slterm` 与 ConPTY 提取缓存 `%LOCALAPPDATA%\slterm\conpty` 在卸载后保留，属预期。

## 操作

- 发布前确认 CHANGELOG 对应版本小节完成；安装器与便携 zip 内置 CHANGELOG。
- 正式发布使用 debug 构建产物之外的 release 链；不要手动拼装 zip 或安装器。
- 发布后按 `scripts/release-checklist.md` 逐项人工核验，结论落版本说明或本次发布记录。
- 完整命令矩阵见 `.claude/docs/operations/commands.md`。
