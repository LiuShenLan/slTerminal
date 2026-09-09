# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-01**：`rg "export function getJson" scripts/check-ts7-trigger.mjs` 命中；Read 确认 getJson 内含非 2xx 守卫（`res.statusCode` 判定 + `res.resume()` + reject 含 HTTP 状态码）与 http/https 分派（`url.startsWith("https:")`）；`rg "getJson" scripts/check-ts7-trigger.d.mts` 命中（导出声明）；脚本头注退出码 2 语义含「含非 2xx」。
- **TE-01**：`src/__tests__/deps-ts7-trigger.test.ts` 含本地 http server 桩两用例（403 reject + 200 resolve）——Read 确认用例存在，且 npm test 绿（测试 agent 结果承载，本 Stage 中间态 = 199 文件/3238 例 = 基线 3236 + 新增 2 例）。
- **TE-02**：`scripts/check-ts7-trigger.d.mts` 的 evaluateTrigger 两参声明为 `string | null | undefined`（Read 确认）；`rg "as unknown as string" src/__tests__/deps-ts7-trigger.test.ts` 零命中。
- **TE-03**：`rg "Guard — src/types" .github/workflows/ci.yml` 命中；Read 确认该 step 的 run 块含 `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests export_bindings -- --test-threads=1` 与 `git diff --exit-code -- src/types` 两条（逐字）；`rg "CI 门禁 step 已落地" src/types/CLAUDE.md src-tauri/src/CLAUDE.md` 各命中一处；门禁命令 4/5（export_bindings + git diff --exit-code -- src/types）exit 0（测试 agent 结果承载）。
- **TE-07**：`rg "datalearncodeterax" vitest.config.ts` 零命中；Read 确认 exclude = `['node_modules', '.temp', 'e2e-tests']`；npm test 绿且用例数无漂移（测试 agent 结果承载）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests export_bindings -- --test-threads=1`
5. `git diff --exit-code -- src/types`
