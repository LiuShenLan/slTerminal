# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read/命令实跑逐条核实，给出证据（文件+行号/命令退出码）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-16**：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 退出码 0（取全量测试结果）
- **TE-16**：`git diff HEAD~1 --stat`（或以本 Stage commit 为界）显示 `src-tauri/src/pty/shell.rs` 与 `src-tauri/src/pty/spawn.rs` 的变化仅为格式调整——Read diff 确认无逻辑行改动（无语义增删，仅空白/换行/缩进）
- **TE-12**：`npx knip --production` 退出码 0（取全量测试结果；断言退出码，非项数）
- **TE-12**：`knip.json` 中每个 `ignoreExports` 条目对应的导出在源文件内有「测试专用」注释，或在 S01 执行报告中被备注为真死代码已删——抽查 Read 核对（防「无脑 ignore」）
- **TE-13**：grep `.github/workflows/ci.yml` 含 `npx knip --production`

## 全量测试（全部通过为门禁）

1. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
2. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
3. `npx tsc --noEmit`
4. `npx eslint src/`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm test`
7. `npx knip --production`
