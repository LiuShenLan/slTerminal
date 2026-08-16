# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-17**：`grep "transition" src/features/sideViews/ActivityBar.tsx` 零命中；activityBar.test.tsx 含「不含 transition」负断言且通过
- **FE-19**：`SideBarArea.tsx` 的 splitRatio 重置 effect 不再无条件执行（语义式：须 Read 确认存在「首次/越界才回退」的条件守卫）；sideBarArea.test.tsx 增「切换保留比例」用例且通过
- **FE-22**：`sideBarState.ts` reconcileZones 内 push 前有数组复制（`[...top]` 或等价形态——须 Read 确认入参 saved 的数组不被 mutate）；sideBarState.test.ts 增入参不被 mutate 断言且通过
- **FE-23**：`ActivityBar.tsx` onDragLeave 含 relatedTarget 判断或统一 dragend/drop 清理（须 Read 确认容器→子元素转移不再清指示线）；activityBar.test.tsx 增对应用例且通过

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
