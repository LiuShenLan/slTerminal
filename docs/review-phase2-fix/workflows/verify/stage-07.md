# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-04**：grep `src/panels/html/HtmlPanel.tsx` 无「拿不到本面板 nonce」残留；`buildInjectedScript` 文档注释含威胁模型段（grep 「威胁模型」命中，Read 确认含「global 命令集最小化」结论）
- **SEC-04**：`src/panels/CLAUDE.md` SEC-03/SEC-04 节含防线分层结论（grep 「命令集最小化」命中），无「无法读取 buildInjectedScript 产出」失实表述残留
- **SEC-04**：`src/__tests__/command-catalog.test.ts` 含 global 命令集守卫用例（grep `global.closeTab` 命中，Read 确认为精确集合断言）
- **SEC-04**：`src/panels/html/HtmlPanel.tsx` 运行时代码零改动（Read 确认仅注释变更——diff 对照，无逻辑行改动）
- **FE-08**：grep `src/panels/terminal/keyboard.ts` 无 `.catch(() => {})` 残留；粘贴分支 catch 含 console.error（grep 「读取剪贴板失败」命中）
- **FE-42**：grep `src/ipc/window.ts` 无 `.catch(() => {})` 残留；cleanup catch 含 console.warn（grep 「取消关窗监听失败」命中）
- **FE-45**：grep `src/stores/` 无 `} catch {` 空块残留；5 处 catch 均含 console.warn（grep 「loadFromDisk 失败」命中 ≥5 处：fontSize/keybindings/sideBar/projects/loadAllProjects）
- **FE-10**：grep `src/panels/diff/DiffPanel.tsx` 外部修改重载两处 catch 均含 `setDiffStale(true)`（命中 ≥2）
- **FE-43**：grep `src/panels/diff/DiffPanel.tsx` 保存 toast 含 `getErrorMessage`
- **FE-44**：grep `src/panels/editor/useCodeMirror.ts` 保存 toast 含 `getErrorMessage`

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run test:l3`
