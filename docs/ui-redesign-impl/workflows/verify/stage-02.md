# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **FT-01**：grep `@fontsource/jetbrains-mono` 命中 `package.json`（dependencies）；grep `@fontsource/jetbrains-mono/400.css` 与 `@fontsource/jetbrains-mono/500.css` 均命中 `src/main.tsx`；`node_modules/@fontsource/jetbrains-mono` 目录存在
- **FT-02**：grep `JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace` 命中 `src/App.css`；grep `Cascadia Code\|Fira Code` 在 `src/App.css` 零命中
- **FT-03**：grep `Microsoft YaHei UI` 命中 `src/panels/editor/useCodeMirror.ts`（2 处）
- **FT-04**：grep `Microsoft YaHei UI` 命中 `src/panels/terminal/theme.ts`
- **FT-08**：Read `src/main.tsx` 错误页段确认 font-family 为规格栈（含 Microsoft YaHei UI），不再是裸 monospace
- **FT-201-扫描**：`grep "font-family\|fontFamily" src/ index.html` 全部命中行均为规格栈或规格栈子集——不存在规格栈外的其它字体名声明（不限声明写法，须 Read 命中行逐条确认；test 文件与注释除外）
- **FT-202-产物**：`npx vite build` 后 Glob `dist/assets/*.woff2` 至少 2 个命中（400/500 两字体文件随产物打包）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
7. `npx vite build`
