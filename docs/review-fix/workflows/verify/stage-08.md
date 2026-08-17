# Stage 08 逐项验证断言（唯一真值源）

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-02**：`src/ipc/appError.ts` 存在且导出 `parseAppError`、`getErrorMessage`（grep 命中）；`src/lib/index.ts` re-export 两函数（grep 命中）
- **FE-02**：L2 测试覆盖全 11 变体（含 ConfigParse）+ 非 AppError 兜底（Read 测试文件逐变体列出）
- **BE-15**：`src-tauri/src/error.rs` 含 `ConfigParse` 变体（grep 命中）；用户可见消息为业务语义（Read 抽查，技术细节进 tracing）
- **BE-13**：fs/settings/projects 命令内 `map_err` 注入路径上下文（语义式：抽查 ≥3 调用点，错误消息含路径变量——From<io::Error> 本身未改动）
- **FE-03**：`src/main.tsx` 与 `src/App.tsx` 启动链 catch 含 `console.warn` 带模块名（grep console.warn 于两文件，逐处列出）
- **FE-05**：App.tsx 关闭序列 kill 失败收集后统一一条 console.error 汇总（语义式，须 Read 确认含失败计数）
- **FE-06**：App.tsx `requestUserAttention` catch 内含 console.warn（grep 命中）
- **FE-07**：`useFileTree.ts` 存在按路径 error 状态；`ExplorerPanel.tsx` 渲染错误占位（错误消息 + 重试按钮——语义式，Read 确认）
- **FE-08**：`useXterm.ts` 关键路径（spawn 失败、write 连续失败 ≥3 次）toast；非关键路径（resize/kill/openUrl）console.error（语义式，须 Read 区分确认——全 toast 或全静默均判 partial）
- **FE-09**：`stores/fontSize.ts`、`keybindings.ts`、`sideBar.ts` 保存失败均 `toast.show(`（grep 逐文件命中）
- **FE-10**：`DiffPanel.tsx` 失败提示条（「内容可能过时」语义）；`useCodeMirror.ts` 重载失败状态条提示（语义式，Read 确认）
- **综合**：`grep -rn "\.catch(() => {})" src/` 于本 Stage 涉及文件零残留（其他文件残留不属本 Stage，列出即可）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
