# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TAB-01**：Read `src/workspace/PageDockviewHost.tsx` DefaultTab 确认——激活页签底部 2px 指示条（FOCUS_BORDER token，不限实现形式：底条元素/borderBottom 均可）；hover 不变底（无 hover 背景样式）
- **TAB-02**：Read 确认关闭 × 默认不可见、hover 页签才显（CSS :hover 或条件渲染，不限形式）；尺寸 14px、圆角 4px；激活页签也不常驻
- **TAB-03**：Read 确认——终端/agent 页签渲染 StatusDot + tabLogo img + 名称；文件型页签渲染 FileIcon（import 自 features/explorer/FileIcon）；无 emoji 文本分支残留（grep DefaultTab 区域无 emoji span 分支）
- **TAB-04**：Read RightHeader 确认「+」钮 22px、圆角 4px、fg-3、hover 底 token 引用
- **TAB-05**：`npm test` 通过（依测试 agent 结果）；grep `tabIcon` 在 `src/__tests__/` 零残留（已统一 tabStatus）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
