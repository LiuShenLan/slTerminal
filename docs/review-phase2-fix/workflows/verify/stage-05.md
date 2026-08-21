# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-10**：`src/workspace/Workspace.tsx` 的 SEC-01 effect 含 activePageId 置 null 的 stopWatch 分支（Read 确认：`!activePageId` 时对 prevRootRef.current 调 `stopWatch` 并将 ref 置 null——语义断言，不限变量名）
- **FE-38**：同 effect 中 `setProjectRoot` 的 then 回调内含过期守卫（Read 确认：then 回调比较 prevRootRef.current 与目标 root，不等则丢弃不启动 watcher）
- **FE-38**：`startWatch` 仅出现在 `setProjectRoot` 的 then 回调内（语义断言，须 Read 确认——不存在与 setProjectRoot 并排放火的 startWatch 调用）
- **BE-10+FE-38**：`npm test` 全绿且含新增用例（activePageId 置 null → stopWatch 旧 root；setProjectRoot resolve 前 startWatch 未调用 / reject 时 toast 出现）——grep 测试文件确认用例存在
- **BE-10**：`src/workspace/CLAUDE.md` 含「activePageId 置 null → stopWatch（BE-10）」表述（grep 命中）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx tauri build --debug --no-bundle`
