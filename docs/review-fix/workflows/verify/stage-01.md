# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-01**：Read `package.json`——所有 `@wdio/*` 依赖版本不低于 9.30.1（逐条列出）；`@wdio/tauri-plugin` 与 `@wdio/tauri-service` 均为 1.3.0
- **TE-01**：`package-lock.json` 中 `serialize-javascript` 全部实例版本 ≥7.0.5（grep 计数 + 逐版本号列出，存在 <7.0.5 即判 not_fixed）
- **TE-01**：Read `src-tauri/Cargo.toml`——`tauri-plugin-wdio-webdriver` = 1.3.0；`Cargo.lock` 中该包同步 1.3.0（跨边界契约：三处 1.3.0 对齐，任一偏离判 not_fixed）
- **TE-02**：`package.json` 的 `dependencies` 显式含 `json-schema` 与 `@lezer/highlight`（版本与 lock 解析值一致）
- **TE-05**：`src-tauri/Cargo.toml` git2 = "0.21"（vendored-libgit2 feature 保持）；`Cargo.lock` git2 为 0.21.x
- **TE-06**：`src-tauri/Cargo.toml` tauri 不低于 2.11.5；`package.json` `@tauri-apps/cli` 不低于 2.11.4
- **TE-12**：`knip.json` 存在于项目根；entry 配置含 `e2e-tests/` 的 glob（Read 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npx vite build`
7. `npx tauri build --debug --no-bundle`
