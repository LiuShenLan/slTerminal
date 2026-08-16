# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-01**：`grep "window\.alert\|window\.confirm" src/panels/editor/useCodeMirror.ts` 零命中；`:177` 原 alert 处改为 `toast.show("error", ...)`；`:265`/`:392` 原 confirm 处改为 `await confirmDialog(...)`（须 Read 确认确认=继续/取消=中止语义与改前一致）；`use-code-mirror.test.ts`/`editor-confirm.test.ts` 不再 mock window.alert/confirm（grep 零命中），confirmDialog/toast mock 断言存在；相关用例在全量测试中通过
- **FE-02**：`grep "window\.confirm" src/panels/diff/DiffPanel.tsx` 零命中；`:363`/`:457` 两处改 `await confirmDialog(...)`；`diff-panel.test.tsx` 脏弹窗分支 mock confirmDialog 且通过
- **FE-03**：`grep "window\.confirm" src/features/navTree/NavTree.tsx` 零命中；项目删除 action 改 `confirmDialog({ ..., danger: true })`（须 Read 确认 danger: true 存在）；nav-tree.test.tsx 删除项目用例改 mock confirmDialog 且通过
- **FE-09**：`grep "TerminalRegistry" src/features/navTree/NavTree.tsx` 零命中（含 import 与类型引用——navTree 零引用 panels/terminal）；`src/workspace/pageApis.ts` 导出 `findPanelForSession` 与 `findPageIdForPanelId`（grep export 命中）；函数行为逐字同原实现（复合键 keyOf 匹配、usageSourcePath 回退、前缀匹配优先 + parseTerminalPageId 兜底——须 Read 两函数体确认）；NavTree 调用点改调 pageApis；pageapis.test.ts 新增两函数分支用例且通过
- **FE-24**：`NavTree.tsx` 的 `handleNewPage` 无 `return` pageId 语句（须 Read 函数体确认无返回值）
- **FE-01~03 总纲**：`grep "window\.alert\|window\.confirm" src/`（排除 `src/__tests__/` 中历史注释）零命中——原生弹窗全仓清零

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
