# Stage 8 逐项验证断言（唯一真值源）

> stage-08-frontend-cleanup.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-11**：`src/panelRegistry.ts` 的 `isAlwaysRenderPanel` 无冗余分支（对照 checklist 修复要点：简化后逻辑等价）
- **FE-13**：`src/features/shortcuts/forwardGlobalShortcuts.ts` 与 `src/__tests__/forward-global-shortcuts.test.ts` 均不存在；`src/features/shortcuts/index.ts` 无对应 re-export；`grep -rn "forwardGlobalShortcuts\|attachGlobalShortcutForwarder" src/` 无命中
- **FE-20**：`SidebarTree.tsx` 组件函数体内无内联组件定义（Toolbar/ProjectRow/PageRow 已提取到模块顶层）
- **FE-21**：`FileTree.tsx` 无裸 `8 + (depth + 1) * 16` 魔法数字表达式（已提取 ICON_WIDTH/ARROW_WIDTH/INDENT 常量）
- **FE-24**：`grep -rn "#1e1e2e\|#2b2b3c\|rgba(0,0,0,0.5)" src/ --include="*.tsx" --include="*.css"` 无命中（`theme/colors.ts` 除外）；`App.css`、`SidebarTree.tsx`、`FileTree.tsx` 均引用 colors.ts token

## 全量测试（5 条全部通过）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
