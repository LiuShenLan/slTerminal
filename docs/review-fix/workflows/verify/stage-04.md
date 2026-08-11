# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态说明：本 Stage 完成后 mockCliProfile/helpers.ts 的 mockcli 声明尚未补 configEditor/configLayers（Stage 05 才补）——字段可选故中间态合法，勿判缺失。

## 断言清单

- **KZ-1（hub 分派）**：`src/panels/hooksConfig/HooksConfigPanel.tsx` 不存在对 ClaudeHooksConfigEditor 的 import 或无条件渲染（语义式——Read 确认编辑器槽渲染来源为 selectedProfile 的 capabilities.hooks.configEditor 字段；缺失时渲染空态占位而非 claude 编辑器；选择行过滤条件 hasConfigEditor===true 不变）；L2 存在 hub 分派用例 + 「hasConfigEditor=true 但 configEditor 缺失 → 空态占位」用例，且 L2 全绿
- **KZ-1（组件入 profile）**：`src/features/cliProfiles/types.ts` 含 HooksConfigEditorProps（profile/onDirtyChange/askGuardRef 三 props）与 HooksCapability.configEditor 字段（Read 确认）；`src/features/cliProfiles/profiles/claude/index.ts` 挂载 configEditor = ClaudeHooksConfigEditor（cli-profile-claude.test.ts 断言存在且 L2 全绿）
- **KZ-1（依赖方向合法化）**：`src/features/cliProfiles/CLAUDE.md` 含 features→panels 依赖方向合法化说明（claude 合法领地引用 claude 专属资产；types.ts 仅类型 import）；`npx tsc --noEmit` 与 `npx vite build` 均无循环依赖报错/警告（vite build 报告为据）
- **KZ-4（configLayers 入 profile）**：HooksCapability 含 configLayers 字段（{ id; label; hint }[] 形态）；claude profile 的 configLayers = user/project/local 三层现值（label/hint 与退役前 LAYERS 一致——Read 对照 git diff 或原文）；cli-profile-claude.test.ts 断言存在
- **KZ-4（层驱动渲染）**：`src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx` 内不存在模块级 LAYERS 常量硬编码三层（语义式——层切换器数据源来自 profile.capabilities.hooks.configLayers，不限常量去留形态；PRIORITY_HINT 与 project/local 禁用判定可保留在编辑器内部——claude 合法领地）；`src/types/hooksConfig.ts` 的 HooksLayer = string（注释注明值集由 profile 声明）；`useHooksConfig.ts` 初始层取 configLayers 首项（含缺省防御）
- **KZ-4（后端零改动）**：本 Stage 不触碰 `src-tauri/`（git diff 确认该目录零改动——trait layer 参数本为字符串，parse_layer 是 claude provider 内部知识）
- **KZ-5（消解闭环）**：`ClaudeHooksConfigEditor.tsx` 的 `~/.claude/settings.json` 文案保留但整文件仅经 profile.configEditor 引用（grep HooksConfigPanel.tsx 对 ClaudeHooksConfigEditor 零直接引用 + Read 确认引用链：hub → profile.configEditor → 编辑器）
- **文档同步**：`src/panels/CLAUDE.md`（hub 段 + 层数据源）、`src/types/CLAUDE.md`（HooksLayer 泛化）、`src/features/hooksConfig/CLAUDE.md`（层声明入 profile）与代码终态一致（Read 对照核实）；`.claude/test-inventory.md` 登记本 Stage 用例变化（layers 单点负责口径）；hub 面板 claude 编辑器分派渲染全链列入人工验证点（本 Stage 无 L4，Stage 06 收尾 L4 兜底）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1，必须单线程）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npx vite build`（循环依赖打包图验证——KZ-1 新增 features→panels 依赖方向）
