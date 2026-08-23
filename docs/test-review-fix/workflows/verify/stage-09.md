# Stage 09 逐项验证断言（唯一真值源）

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TQ-CI-03**：`.github/workflows/ci.yml` 含 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 步骤且位于 Clippy 之前（grep + Read 顺序确认）。
- **TQ-CI-06**：`.github/workflows/ci.yml` 中 `timeout-minutes` 出现 ≥ 8 次（grep -c ≥ 8：job 级 1 + 步骤级 ≥ 7），且 Backend tests (L1) 步骤 ≤ 30、E2E 相关步骤 ≤ 60（Read 确认）。
- **TQ-CI-07**：`.github/workflows/ci.yml` 的 setup-node 含 `cache: 'npm'`（grep 命中）。
- **TQ-E-09**：`e2e-tests/wdio.conf.ts` 的 retries 改为 `WDIO_RETRIES` 环境变量驱动且默认 1（grep 命中）；`.github/workflows/ci.yml` 含 flakiness 观察面 job（grep `WDIO_RETRIES` + `continue-on-error` 命中）。
- **yaml 合法性**：ci.yml 可被 YAML 解析器无错解析（以门禁命令产出判定）。

## 全量测试（全部通过为门禁）

1. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`（本地先过，CI 才不红）
2. `node -e "const fs=require('fs');const y=require('js-yaml');y.load(fs.readFileSync('.github/workflows/ci.yml','utf8'));console.log('yaml ok')"`（js-yaml 不可用时用 `npx js-yaml .github/workflows/ci.yml > /dev/null` 替代）
3. `npm run e2e`（WDIO_RETRIES 改动冒烟）
