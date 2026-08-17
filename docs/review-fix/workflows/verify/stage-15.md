# Stage 15 逐项验证断言（唯一真值源）

> stage-15 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> **注意**：某步骤被 agent 回滚（报告注明无法修复）时，对应 ID 判 partial 并摘录回滚理由，不判 not_fixed。

## 断言清单

- **TE-10**：`package.json` 中 jsdom 为 30.x、`@testing-library/jest-dom` 为 7.x、`@types/node` 为 26.x、`cross-env` 为 10.x（Read 逐条列出）
- **TE-07**：`package.json` 中 typescript 为 7.x（Read 确认）；`npx tsc --noEmit` 全绿（门禁命令 1 佐证）
- **TE-09**：`package.json` 中 json-schema-library 为 11.x（Read 确认）；去重评估结论存在于 agent 报告（若决策统一为单库，`package.json` 不再同时含两库——Read 核对）
- **TE-08**：`package.json` 中 dockview-react 为 8.x（Read 确认）；`src/workspace/layoutSerde.ts` 的 toJSON/fromJSON 契约保留（Read 确认仍经 Dockview serde API，约束 #7 未破）；layout-serde 相关测试全绿（门禁命令 4 佐证）
- **禁区核对**：`git diff` 确认 `compute_conpty_flags` 与 WebGL 检测逻辑零改动

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
7. `npx vite build`
8. `npx tauri build --debug --no-bundle`
