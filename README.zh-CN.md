# slTerminal

面向 Windows 10/11、面向 AI CLI 调优的通用终端模拟器。

- Windows 原生运行 AI CLI(不走 WSL);单窗口单实例;仅暗色模式;GPU 加速渲染
- 默认 shell:PowerShell 7(`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退)
- 复制 = `Ctrl+Shift+C`(`Ctrl+C` 保留为中断,供 AI CLI 取消)
- 单进程 GPUI 应用:Rust workspace,无 WebView/Node 运行时

## 构建

```powershell
cargo build -p slterm --bin slterm -p slterm_hook --bin slterm-hook
```

发布构建统一走 `scripts/release.ps1`(共享构建 → Inno 安装器 → 便携
zip → 校验),产物落 `dist/`。

## 测试

```powershell
cargo test -p slterm --test lib_tests -- --test-threads=1
cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

静态检查与架构门禁:

```powershell
python scripts/check_architecture.py
python scripts/check_brand_regress.py staged
```

## 文档

- `CLAUDE.md` — 项目根指令(模块细节经各目录 CLAUDE.md 渐进披露)
- `CONTEXT.md` — 领域术语表
- `.claude/adr.md` — 架构决策记录
- `assets/runtime-control-api.md` — 随包协议事实文档
- `release-notes/` — 按版本的发布说明
