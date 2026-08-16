# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-07**：`src/theme/schemes/types.ts` UiTokens 含 `titlebarCloseHover` 槽位（含消费注释）；`schemes/linear.ts` ui 段含 `titlebarCloseHover: "#c04747"`；`colors.ts` 导出 `TITLEBAR_CLOSE_HOVER_BG`；`TitleBar.tsx` 无 `#c04747` 字面量（grep 零命中），关闭 hover 处引用该 token（须 Read 确认）；scheme-registry.test.ts 标量计数断言 = 27、colors.test.ts token 集合含新导出且通过
- **FE-08**：`colors.ts` ROOT_CSS_VARS 恰 6 键（原 2 + 新 4：`--sl-focus-border` / `--sl-scrollbar-slider` / `--sl-scrollbar-slider-hover` / `--sl-scrollbar-slider-active`，须 Read 确认取值来源 = ui.focusBorder 与 terminal.scrollbarSlider\* 三键）；`src/App.css` 无 hex/rgba 色值字面量（grep `#[0-9a-fA-F]{3,8}\|rgba?\(` 仅余注释中的说明性引用——须 Read 逐处确认非样式声明）；`:59,63,66,75` 四处为 var() 引用；colors.test.ts 键集合断言 6 键且通过；main.tsx 注入循环零改动（git diff 确认）
- **FE-20**：`FileIcon.tsx` isDir 分支 color 由 gitStatus 映射取值（须 Read 确认 `statusColorMap[gitStatus] ?? EXPLORER_COLORS.fg` 形态）；file-icon.test.tsx 增文件夹+gitStatus 用例（含回退分支）且通过
- **FE-21**：`TitleBar.tsx` 不再以 `(s) => s.projects` 整对象订阅（grep 确认）；标题行为不变（title-bar 测试全绿 + 新增无关变更不重渲染用例）
- **Stage 02 总纲**：`theme/index.ts:3` 注释计数与 colors.ts 实际导出名数一致（35）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
