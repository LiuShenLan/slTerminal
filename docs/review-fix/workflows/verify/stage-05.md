# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态说明：本 Stage 依赖 Stage 04 产物（configEditor/configLayers 已入 profile 类型）；完成后 mockcli 双夹具（L2/L4）能力声明齐备。

## 断言清单

- **KZ-7（桩声明）**：`src/__tests__/helpers/mockCliProfile.ts` 的 mockcli capabilities.hooks 含 configEditor 桩组件（渲染可识别标记 data-e2e="mockcli-config-editor"，props 签名对齐 HooksConfigEditorProps）与 configLayers 桩声明（与 claude 三层可区分）；Read 确认
- **KZ-7（AC-4④ 双向分派）**：`src/__tests__/mock-cli-profile.test.tsx` 的 AC-4④ 段为双向断言（语义式——Read 确认：选中 mockcli → 桩标记渲染且 mockJsonMode 零调用；选中 claude → mockJsonMode 被调用且桩标记不存在；单向断言或仅桩渲染不判 fixed），且 L2 全绿
- **CS-3（E2E 夹具补桩）**：`e2e-tests/helpers.ts` 的 installMockCliProfile 补 configEditor（React.createElement 桩，同 data-e2e="mockcli-config-editor" 标记；含保存动作入口调用 writeHooksConfig 携带 mockcli cliId）+ configLayers（Read 确认）；E2E_ENABLED 内联 import.meta.env 字面量形态未动（grep helpers.ts 确认内联形态，禁区 6）
- **CS-3（L4 用例 ①）**：`e2e-tests/mockcli.e2e.ts` 存在 agent-event 注入用例（cliId="mockcli" 信号文件 → 页签 emoji + 活跃区建行断言），且 L4 全绿
- **CS-3（L4 用例 ②）**：存在 hub 分派 + 保存透传用例（选择行 mockcli 按钮 → 点击渲染桩 → 保存动作 → 后端「未知 cliId: mockcli」错误透传断言），且 L4 全绿
- **CS-3（豁免登记）**：`.claude/test-inventory.md` 豁免清单含两条 L4 不可行登记（历史条目展示、双击恢复注入）——各含理由（生产二进制无 mockcli 后端 provider，历史条目 cliId 恒 "claude"；不留生产后门）与兜底层级（L2 AC-4③/⑤）；新 L4 用例与 AC-4④ 重写后 L2 用例数同步登记（l4-mockcli 单点负责口径）
- **文档同步**：`src/__tests__/CLAUDE.md`（mockCliProfile 桩能力 + AC-4④ 双向口径）、`e2e-tests/CLAUDE.md`（mockcli 用例清单 + 桩能力）与代码终态一致（Read 对照核实）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1，必须单线程）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——其余命令全部完成后单独串行执行）
