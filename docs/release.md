# 发布打包（手动步骤）

> 一键打包优先用 `.\.claude\package.ps1 -Version "0.1.0"`（release 模式，单文件 exe → zip）；加 `-Debug` 用 debug 模式（exe + dll 两个文件）。手动步骤仅用于定制流程。

## 手动步骤

1. `npx tauri build --no-bundle` → `src-tauri/target/release/slterminal.exe`（单文件自包含）
2. `Compress-Archive src-tauri/target/release/slterminal.exe slterminal-v0.1.0-x64.zip`
3. GitHub Releases → 创建 Tag `v0.1.0` → 上传 zip

## 给他人分享

zip 解压到任意目录，双击 `slterminal.exe` 即可运行。不写注册表、不写 C 盘。

首次运行 Windows SmartScreen 会提示"Windows protected your PC"→ 点"更多信息"→"仍要运行"。
