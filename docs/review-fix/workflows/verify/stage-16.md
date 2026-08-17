# Stage 16 逐项验证断言（唯一真值源）

> stage-16 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-11**：`package.json` 的 `dependencies` 全部精确版本（语义式：无 `^`/`~` 前缀——逐条列出核对）；`devDependencies` 全部 `^`（既定例外如 xterm beta 精确锁定，须与 adr.md 登记一致——交叉核对）
- **TE-03**：`.claude/adr.md` 存在 xterm 升级审批约定条目（语义式：含全量 L3+E2E+实测滚轮要求——grep 命中并 Read 确认）
- **TE-04**：`.claude/adr.md` 存在 notify RC 保留登记（语义式：含 rc.4 即最新、无稳定版可升、L1 守护说明）
- **TE-13**：`.github/workflows/ci.yml` 含三命令（grep 逐条命中）：`npm audit`（含 `--audit-level=high`）、`knip --production`、`cargo audit`
- **TE-13**：三条命令位于 CI job 的有效 steps 内（Read 确认 YAML 结构合法、步骤位置合理）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
